use anyhow::Result;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;

use crate::db::{Database, DashboardStats, RunSummary, TaskResult};

// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    RunStarted { run_id: String, total_tasks: i32 },
    TaskStarted { run_id: String, task_name: String },
    TaskCompleted { run_id: String, task: TaskResult },
    RunCompleted { run_id: String, passed: i32, failed: i32 },
    Stats(DashboardStats),
}

pub struct AppState {
    pub db_path: std::path::PathBuf,
    pub tx: broadcast::Sender<WsMessage>,
}

impl AppState {
    pub fn get_db(&self) -> Result<Database> {
        Database::open(&self.db_path)
    }
}

pub async fn start_server(port: u16, db_path: std::path::PathBuf) -> Result<()> {
    let (tx, _) = broadcast::channel::<WsMessage>(100);

    let state = Arc::new(AppState { db_path, tx });

    let app = Router::new()
        .route("/", get(serve_dashboard))
        .route("/api/stats", get(get_stats))
        .route("/api/runs", get(get_runs))
        .route("/api/runs/:id", get(get_run))
        .route("/ws", get(ws_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("🚀 Dashboard running at http://localhost:{}", port);
    println!("   Press Ctrl+C to stop\n");

    axum::serve(listener, app).await?;
    Ok(())
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
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
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

// Helper to broadcast messages from runner
pub fn broadcast_message(tx: &broadcast::Sender<WsMessage>, msg: WsMessage) {
    let _ = tx.send(msg);
}
