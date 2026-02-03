use anyhow::Result;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

use crate::db::{Database, DashboardStats, TaskResult as DbTaskResult};

// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    RunStarted { run_id: String, total_tasks: i32 },
    TaskStarted { run_id: String, task_name: String },
    TaskCompleted { run_id: String, task: DbTaskResult },
    RunCompleted { run_id: String, passed: i32, failed: i32 },
    Stats(DashboardStats),
    ArtifactsUpdated,
    FileChanged { files: Vec<String> },
    // Debug events from Vue/Tauri apps
    DebugEvent(DebugEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugEvent {
    pub source: String,      // "pinia", "tauri", "vue", "custom"
    pub event_type: String,  // "mutation", "action", "command", "event", "error"
    pub name: String,        // event/mutation/action name
    pub payload: Option<serde_json::Value>,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub struct AppState {
    pub db_path: std::path::PathBuf,
    pub project_dir: std::path::PathBuf,
    pub tx: broadcast::Sender<WsMessage>,
    pub shutdown_tx: broadcast::Sender<()>,
}

impl AppState {
    pub fn get_db(&self) -> Result<Database> {
        Database::open(&self.db_path)
    }
}

const EXCLUDED_DIRS: &[&str] = &["target", "node_modules", "dist", "out", ".git"];

pub async fn start_server(
    port: u16,
    db_path: std::path::PathBuf,
    project_dir: std::path::PathBuf,
    watch: bool,
) -> Result<()> {
    let (tx, _) = broadcast::channel::<WsMessage>(100);
    let (shutdown_tx, mut shutdown_rx) = broadcast::channel::<()>(1);

    let state = Arc::new(AppState {
        db_path: db_path.clone(),
        project_dir: project_dir.clone(),
        tx: tx.clone(),
        shutdown_tx,
    });

    let app = Router::new()
        .route("/", get(serve_dashboard))
        .route("/api/stats", get(get_stats))
        .route("/api/runs", get(get_runs))
        .route("/api/runs/:id", get(get_run))
        .route("/api/artifacts", get(get_artifacts))
        .route("/api/artifacts/:test_name", get(get_artifact))
        .route("/api/clear-history", post(clear_history_handler))
        .route("/api/debug", post(receive_debug_event))
        .route("/api/shutdown", post(shutdown_handler))
        .route("/ws", get(ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("🚀 Dashboard running at http://localhost:{}", port);

    if watch {
        println!("👀 Watch mode enabled - monitoring file changes");

        // Start file watcher in background
        let watch_tx = tx.clone();
        let watch_db_path = db_path.clone();
        let watch_project_dir = project_dir.clone();

        std::thread::spawn(move || {
            if let Err(e) = run_file_watcher(watch_project_dir, watch_db_path, watch_tx) {
                eprintln!("Watcher error: {}", e);
            }
        });
    }

    println!("   Press Ctrl+C to stop\n");

    tokio::select! {
        result = axum::serve(listener, app) => {
            result?;
        }
        _ = shutdown_rx.recv() => {
            println!("\n✓ Server stopped via dashboard");
        }
        _ = tokio::signal::ctrl_c() => {
            println!("\n✓ Server stopped via Ctrl+C");
        }
    }

    Ok(())
}

fn run_file_watcher(
    project_dir: std::path::PathBuf,
    db_path: std::path::PathBuf,
    tx: broadcast::Sender<WsMessage>,
) -> Result<()> {
    use std::sync::mpsc;

    let (watcher_tx, watcher_rx) = mpsc::channel();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = watcher_tx.send(event);
            }
        },
        Config::default(),
    )?;

    watcher.watch(&project_dir, RecursiveMode::Recursive)?;

    let mut last_run = Instant::now() - Duration::from_secs(10);

    loop {
        if let Ok(event) = watcher_rx.recv() {
            // Debounce - wait 500ms between runs
            if last_run.elapsed().as_millis() < 500 {
                continue;
            }

            // Filter changed files
            let changed_files: Vec<String> = event
                .paths
                .iter()
                .filter_map(|p| {
                    let path_str = p.to_string_lossy().to_string();

                    // Skip excluded directories
                    if EXCLUDED_DIRS.iter().any(|exc| path_str.contains(exc)) {
                        return None;
                    }

                    // Only watch Rust files
                    if !path_str.ends_with(".rs") {
                        return None;
                    }

                    p.strip_prefix(&project_dir)
                        .ok()
                        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
                })
                .collect();

            if changed_files.is_empty() {
                continue;
            }

            last_run = Instant::now();

            // Notify clients that files changed
            let _ = tx.send(WsMessage::FileChanged {
                files: changed_files.clone(),
            });

            println!("\n📝 Files changed: {}", changed_files.join(", "));
            println!("🔄 Running tests...\n");

            // Run tests and save to DB
            run_tests_and_broadcast(&project_dir, &db_path, &tx);
        }
    }
}

fn run_tests_and_broadcast(
    project_dir: &std::path::Path,
    db_path: &std::path::Path,
    tx: &broadcast::Sender<WsMessage>,
) {
    use crate::test_runner::TestRunner;
    use chrono::Utc;
    use uuid::Uuid;

    let run_id = Uuid::new_v4().to_string();

    // Open DB
    let db = match Database::open(db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("DB error: {}", e);
            return;
        }
    };

    // Run tests
    let runner = TestRunner::new(project_dir);
    let result = match runner.run_all() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Test error: {}", e);
            return;
        }
    };

    // Notify run started
    let _ = tx.send(WsMessage::RunStarted {
        run_id: run_id.clone(),
        total_tasks: result.test_results.len() as i32,
    });

    // Create run in DB
    let _ = db.create_run(&run_id, result.test_results.len() as i32);

    // Save each test result
    for test_result in &result.test_results {
        let status = match test_result.status {
            crate::test_model::TestStatus::Passed => "passed",
            crate::test_model::TestStatus::Failed => "failed",
            _ => "unknown",
        };

        let db_result = crate::db::TaskResult {
            id: Uuid::new_v4().to_string(),
            run_id: run_id.clone(),
            task_name: test_result.name.clone(),
            category: None,
            status: status.to_string(),
            duration_ms: test_result.duration_ms.unwrap_or(0) as i64,
            started_at: Utc::now(),
            output: if test_result.output.is_empty() {
                None
            } else {
                Some(test_result.output.join("\n"))
            },
        };

        let _ = db.insert_task_result(&db_result);

        // Notify task completed
        let _ = tx.send(WsMessage::TaskCompleted {
            run_id: run_id.clone(),
            task: db_result,
        });
    }

    // Finish run
    let _ = db.finish_run(&run_id, result.passed as i32, result.failed as i32);

    // Notify run completed
    let _ = tx.send(WsMessage::RunCompleted {
        run_id: run_id.clone(),
        passed: result.passed as i32,
        failed: result.failed as i32,
    });

    // Notify artifacts updated
    let _ = tx.send(WsMessage::ArtifactsUpdated);

    // Send updated stats
    if let Ok(stats) = db.get_dashboard_stats() {
        let _ = tx.send(WsMessage::Stats(stats));
    }

    // Print summary
    if result.failed > 0 {
        println!("❌ {} passed, {} failed", result.passed, result.failed);
    } else {
        println!("✅ {} passed", result.passed);
    }
    println!("\n👀 Watching for changes...");
}

async fn receive_debug_event(
    State(state): State<Arc<AppState>>,
    Json(event): Json<DebugEvent>,
) -> impl IntoResponse {
    // Broadcast debug event to all connected clients
    let _ = state.tx.send(WsMessage::DebugEvent(event));
    (StatusCode::OK, "Event received")
}

async fn clear_history_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.get_db() {
        Ok(db) => match db.clear_all_history() {
            Ok(count) => {
                // Send updated stats to clients
                if let Ok(stats) = db.get_dashboard_stats() {
                    let _ = state.tx.send(WsMessage::Stats(stats));
                }
                (StatusCode::OK, format!("Cleared {} records", count)).into_response()
            }
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn shutdown_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let _ = state.shutdown_tx.send(());
    "Server shutting down"
}

async fn serve_dashboard() -> Html<&'static str> {
    Html(include_str!("dashboard.html"))
}

async fn get_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.get_db() {
        Ok(db) => match db.get_dashboard_stats() {
            Ok(stats) => Json(stats).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_runs(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.get_db() {
        Ok(db) => match db.get_recent_runs(50) {
            Ok(runs) => Json(runs).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_run(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> impl IntoResponse {
    match state.get_db() {
        Ok(db) => match db.get_run_summary(&id) {
            Ok(Some(summary)) => Json(summary).into_response(),
            Ok(None) => (StatusCode::NOT_FOUND, "Run not found").into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn ws_handler(
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    let mut rx = state.tx.subscribe();

    // Send initial stats
    if let Ok(db) = state.get_db() {
        if let Ok(stats) = db.get_dashboard_stats() {
            let msg = WsMessage::Stats(stats);
            if let Ok(json) = serde_json::to_string(&msg) {
                let _ = socket.send(Message::Text(json)).await;
            }
        }
    }

    // Listen for updates and forward to client
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(msg) => {
                        if let Ok(json) = serde_json::to_string(&msg) {
                            if socket.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    _ => {}
                }
            }
        }
    }
}

async fn get_artifacts(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    use crate::artifacts::{load_artifacts, TestArtifact};
    match load_artifacts(&state.project_dir) {
        Ok(artifacts) => Json::<Vec<TestArtifact>>(artifacts).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn get_artifact(
    State(state): State<Arc<AppState>>,
    Path(test_name): Path<String>,
) -> impl IntoResponse {
    use crate::artifacts::{get_artifact_for_test, TestArtifact};
    // Decode URL-encoded test name (e.g., "module%3A%3Atest" -> "module::test")
    let test_name = urlencoding::decode(&test_name)
        .map(|s| s.into_owned())
        .unwrap_or(test_name);

    match get_artifact_for_test(&state.project_dir, &test_name) {
        Ok(Some(artifact)) => Json::<TestArtifact>(artifact).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "Artifact not found").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
