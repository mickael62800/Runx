//! # Runx Charts
//!
//! Bibliothèque de templates de graphiques pour Runx.
//!
//! ## Templates disponibles
//!
//! - **Performance** : Latence, throughput, temps de réponse
//! - **Memory** : Utilisation mémoire, heap, allocations
//! - **API** : Temps de réponse par endpoint, status codes
//! - **Coverage** : Couverture de code par module
//!
//! ## Exemple d'utilisation
//!
//! ```rust
//! use runx_charts::prelude::*;
//!
//! #[test]
//! fn test_api_performance() {
//!     let latencies = vec![12.5, 15.2, 11.8, 14.1, 13.3];
//!
//!     Performance::latency("test_api_performance")
//!         .title("API Latency")
//!         .unit("ms")
//!         .data(&latencies)
//!         .threshold(20.0)
//!         .save();
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub mod prelude {
    pub use crate::{Api, Coverage, Memory, Performance};
}

// ============================================================================
// Core Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartType {
    Line,
    Bar,
    Area,
    Pie,
    Gauge,
    Heatmap,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub x: serde_json::Value,
    pub y: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSeries {
    pub name: String,
    pub data: Vec<DataPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestArtifact {
    pub test_name: String,
    pub chart_type: ChartType,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_label: Option<String>,
    pub series: Vec<DataSeries>,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl TestArtifact {
    pub fn save(&self) {
        let dir = Path::new("target/runx/artifacts");
        fs::create_dir_all(dir).ok();

        let filename = format!("{}.json", self.test_name.replace("::", "_"));
        let path = dir.join(filename);

        if let Ok(json) = serde_json::to_string_pretty(self) {
            fs::write(path, json).ok();
        }
    }
}

// ============================================================================
// TEMPLATE 1: Performance (Latence, Throughput)
// ============================================================================

/// Template pour les métriques de performance
///
/// # Exemples
///
/// ```rust
/// // Latence simple
/// Performance::latency("test_db_query")
///     .title("Database Query Latency")
///     .data(&[12.5, 15.2, 11.8, 14.1])
///     .threshold(20.0)
///     .save();
///
/// // Throughput
/// Performance::throughput("test_http_server")
///     .title("HTTP Server Throughput")
///     .data(&[1000.0, 1200.0, 1150.0, 1300.0])
///     .save();
///
/// // Latence avec percentiles
/// Performance::latency_percentiles("test_api")
///     .p50(12.0)
///     .p90(25.0)
///     .p99(45.0)
///     .save();
/// ```
pub struct Performance {
    test_name: String,
    title: String,
    chart_type: ChartType,
    unit: String,
    data: Vec<f64>,
    labels: Vec<String>,
    threshold: Option<f64>,
    percentiles: Option<Percentiles>,
    compare_with: Option<Vec<f64>>,
}

#[derive(Debug, Clone)]
struct Percentiles {
    p50: f64,
    p90: f64,
    p99: f64,
    p999: Option<f64>,
}

impl Performance {
    /// Crée un graphique de latence (ligne)
    pub fn latency(test_name: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            title: "Latency".to_string(),
            chart_type: ChartType::Line,
            unit: "ms".to_string(),
            data: vec![],
            labels: vec![],
            threshold: None,
            percentiles: None,
            compare_with: None,
        }
    }

    /// Crée un graphique de throughput (barres)
    pub fn throughput(test_name: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            title: "Throughput".to_string(),
            chart_type: ChartType::Bar,
            unit: "req/s".to_string(),
            data: vec![],
            labels: vec![],
            threshold: None,
            percentiles: None,
            compare_with: None,
        }
    }

    /// Crée un graphique de percentiles (barres)
    pub fn latency_percentiles(test_name: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            title: "Latency Percentiles".to_string(),
            chart_type: ChartType::Bar,
            unit: "ms".to_string(),
            data: vec![],
            labels: vec!["p50".into(), "p90".into(), "p99".into()],
            threshold: None,
            percentiles: Some(Percentiles {
                p50: 0.0,
                p90: 0.0,
                p99: 0.0,
                p999: None,
            }),
            compare_with: None,
        }
    }

    pub fn title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    pub fn unit(mut self, unit: &str) -> Self {
        self.unit = unit.to_string();
        self
    }

    pub fn data(mut self, values: &[f64]) -> Self {
        self.data = values.to_vec();
        self
    }

    pub fn labels(mut self, labels: &[&str]) -> Self {
        self.labels = labels.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn threshold(mut self, value: f64) -> Self {
        self.threshold = Some(value);
        self
    }

    pub fn compare(mut self, baseline: &[f64]) -> Self {
        self.compare_with = Some(baseline.to_vec());
        self
    }

    pub fn p50(mut self, value: f64) -> Self {
        if let Some(ref mut p) = self.percentiles {
            p.p50 = value;
        }
        self
    }

    pub fn p90(mut self, value: f64) -> Self {
        if let Some(ref mut p) = self.percentiles {
            p.p90 = value;
        }
        self
    }

    pub fn p99(mut self, value: f64) -> Self {
        if let Some(ref mut p) = self.percentiles {
            p.p99 = value;
        }
        self
    }

    pub fn p999(mut self, value: f64) -> Self {
        if let Some(ref mut p) = self.percentiles {
            p.p999 = Some(value);
        }
        self
    }

    pub fn save(self) {
        let mut series = vec![];
        let mut metadata = HashMap::new();

        metadata.insert("unit".to_string(), serde_json::json!(self.unit));
        metadata.insert("template".to_string(), serde_json::json!("performance"));

        if let Some(threshold) = self.threshold {
            metadata.insert("threshold".to_string(), serde_json::json!(threshold));
        }

        if let Some(percentiles) = self.percentiles {
            // Percentiles chart
            let mut data = vec![
                DataPoint { x: serde_json::json!("p50"), y: percentiles.p50, label: None },
                DataPoint { x: serde_json::json!("p90"), y: percentiles.p90, label: None },
                DataPoint { x: serde_json::json!("p99"), y: percentiles.p99, label: None },
            ];
            if let Some(p999) = percentiles.p999 {
                data.push(DataPoint { x: serde_json::json!("p99.9"), y: p999, label: None });
            }
            series.push(DataSeries {
                name: "Latency".to_string(),
                data,
                color: Some("#3b82f6".to_string()),
            });
        } else {
            // Regular data chart
            let data: Vec<DataPoint> = self.data.iter().enumerate().map(|(i, &v)| {
                let x = if i < self.labels.len() {
                    serde_json::json!(self.labels[i].clone())
                } else {
                    serde_json::json!(i)
                };
                DataPoint { x, y: v, label: None }
            }).collect();

            series.push(DataSeries {
                name: "Current".to_string(),
                data,
                color: Some("#3b82f6".to_string()),
            });

            // Add comparison baseline if provided
            if let Some(baseline) = self.compare_with {
                let baseline_data: Vec<DataPoint> = baseline.iter().enumerate().map(|(i, &v)| {
                    let x = if i < self.labels.len() {
                        serde_json::json!(self.labels[i].clone())
                    } else {
                        serde_json::json!(i)
                    };
                    DataPoint { x, y: v, label: None }
                }).collect();

                series.push(DataSeries {
                    name: "Baseline".to_string(),
                    data: baseline_data,
                    color: Some("#9ca3af".to_string()),
                });
            }
        }

        let artifact = TestArtifact {
            test_name: self.test_name,
            chart_type: self.chart_type,
            title: self.title,
            x_label: Some("Time".to_string()),
            y_label: Some(self.unit),
            series,
            metadata,
        };

        artifact.save();
    }
}

// ============================================================================
// TEMPLATE 2: Memory Profiling
// ============================================================================

/// Template pour le profiling mémoire
///
/// # Exemples
///
/// ```rust
/// // Usage mémoire dans le temps
/// Memory::usage("test_memory_leak")
///     .title("Memory Usage Over Time")
///     .samples(&[100.0, 105.0, 112.0, 108.0, 115.0])
///     .limit(200.0)
///     .save();
///
/// // Breakdown par catégorie
/// Memory::breakdown("test_allocations")
///     .heap(50.0)
///     .stack(10.0)
///     .static_mem(5.0)
///     .save();
/// ```
pub struct Memory {
    test_name: String,
    title: String,
    mode: MemoryMode,
    samples: Vec<f64>,
    timestamps: Vec<String>,
    limit: Option<f64>,
    breakdown: Option<MemoryBreakdown>,
}

#[derive(Debug, Clone)]
enum MemoryMode {
    Usage,
    Breakdown,
    Allocations,
}

#[derive(Debug, Clone, Default)]
struct MemoryBreakdown {
    heap: f64,
    stack: f64,
    static_mem: f64,
    other: f64,
}

impl Memory {
    /// Graphique d'utilisation mémoire (aire)
    pub fn usage(test_name: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            title: "Memory Usage".to_string(),
            mode: MemoryMode::Usage,
            samples: vec![],
            timestamps: vec![],
            limit: None,
            breakdown: None,
        }
    }

    /// Graphique de répartition mémoire (pie)
    pub fn breakdown(test_name: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            title: "Memory Breakdown".to_string(),
            mode: MemoryMode::Breakdown,
            samples: vec![],
            timestamps: vec![],
            limit: None,
            breakdown: Some(MemoryBreakdown::default()),
        }
    }

    /// Graphique d'allocations (barres)
    pub fn allocations(test_name: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            title: "Memory Allocations".to_string(),
            mode: MemoryMode::Allocations,
            samples: vec![],
            timestamps: vec![],
            limit: None,
            breakdown: None,
        }
    }

    pub fn title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    pub fn samples(mut self, values: &[f64]) -> Self {
        self.samples = values.to_vec();
        self
    }

    pub fn timestamps(mut self, times: &[&str]) -> Self {
        self.timestamps = times.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn limit(mut self, mb: f64) -> Self {
        self.limit = Some(mb);
        self
    }

    pub fn heap(mut self, mb: f64) -> Self {
        if let Some(ref mut b) = self.breakdown {
            b.heap = mb;
        }
        self
    }

    pub fn stack(mut self, mb: f64) -> Self {
        if let Some(ref mut b) = self.breakdown {
            b.stack = mb;
        }
        self
    }

    pub fn static_mem(mut self, mb: f64) -> Self {
        if let Some(ref mut b) = self.breakdown {
            b.static_mem = mb;
        }
        self
    }

    pub fn other(mut self, mb: f64) -> Self {
        if let Some(ref mut b) = self.breakdown {
            b.other = mb;
        }
        self
    }

    pub fn save(self) {
        let mut metadata = HashMap::new();
        metadata.insert("unit".to_string(), serde_json::json!("MB"));
        metadata.insert("template".to_string(), serde_json::json!("memory"));

        if let Some(limit) = self.limit {
            metadata.insert("limit".to_string(), serde_json::json!(limit));
        }

        let (chart_type, series) = match self.mode {
            MemoryMode::Usage => {
                let data: Vec<DataPoint> = self.samples.iter().enumerate().map(|(i, &v)| {
                    let x = if i < self.timestamps.len() {
                        serde_json::json!(self.timestamps[i].clone())
                    } else {
                        serde_json::json!(i)
                    };
                    DataPoint { x, y: v, label: None }
                }).collect();

                (ChartType::Area, vec![DataSeries {
                    name: "Memory".to_string(),
                    data,
                    color: Some("#8b5cf6".to_string()),
                }])
            }
            MemoryMode::Breakdown => {
                let b = self.breakdown.unwrap_or_default();
                let data = vec![
                    DataPoint { x: serde_json::json!("Heap"), y: b.heap, label: None },
                    DataPoint { x: serde_json::json!("Stack"), y: b.stack, label: None },
                    DataPoint { x: serde_json::json!("Static"), y: b.static_mem, label: None },
                    DataPoint { x: serde_json::json!("Other"), y: b.other, label: None },
                ];

                (ChartType::Pie, vec![DataSeries {
                    name: "Memory".to_string(),
                    data,
                    color: None,
                }])
            }
            MemoryMode::Allocations => {
                let data: Vec<DataPoint> = self.samples.iter().enumerate().map(|(i, &v)| {
                    let x = if i < self.timestamps.len() {
                        serde_json::json!(self.timestamps[i].clone())
                    } else {
                        serde_json::json!(format!("Alloc {}", i + 1))
                    };
                    DataPoint { x, y: v, label: None }
                }).collect();

                (ChartType::Bar, vec![DataSeries {
                    name: "Allocations".to_string(),
                    data,
                    color: Some("#f59e0b".to_string()),
                }])
            }
        };

        let artifact = TestArtifact {
            test_name: self.test_name,
            chart_type,
            title: self.title,
            x_label: Some("Time".to_string()),
            y_label: Some("MB".to_string()),
            series,
            metadata,
        };

        artifact.save();
    }
}

// ============================================================================
// TEMPLATE 3: API Response Times
// ============================================================================

/// Template pour les métriques API
///
/// # Exemples
///
/// ```rust
/// // Temps de réponse par endpoint
/// Api::response_times("test_api")
///     .endpoint("/users", 45.0)
///     .endpoint("/orders", 120.0)
///     .endpoint("/products", 35.0)
///     .sla(100.0)
///     .save();
///
/// // Status codes distribution
/// Api::status_codes("test_api_errors")
///     .ok(950)
///     .client_error(30)
///     .server_error(20)
///     .save();
/// ```
pub struct Api {
    test_name: String,
    title: String,
    mode: ApiMode,
    endpoints: Vec<(String, f64)>,
    status_codes: StatusCodes,
    sla: Option<f64>,
}

#[derive(Debug, Clone)]
enum ApiMode {
    ResponseTimes,
    StatusCodes,
    Throughput,
}

#[derive(Debug, Clone, Default)]
struct StatusCodes {
    ok_2xx: u32,
    redirect_3xx: u32,
    client_error_4xx: u32,
    server_error_5xx: u32,
}

impl Api {
    /// Graphique des temps de réponse par endpoint
    pub fn response_times(test_name: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            title: "API Response Times".to_string(),
            mode: ApiMode::ResponseTimes,
            endpoints: vec![],
            status_codes: StatusCodes::default(),
            sla: None,
        }
    }

    /// Graphique de distribution des status codes
    pub fn status_codes(test_name: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            title: "HTTP Status Codes".to_string(),
            mode: ApiMode::StatusCodes,
            endpoints: vec![],
            status_codes: StatusCodes::default(),
            sla: None,
        }
    }

    /// Graphique de throughput par endpoint
    pub fn throughput(test_name: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            title: "API Throughput".to_string(),
            mode: ApiMode::Throughput,
            endpoints: vec![],
            status_codes: StatusCodes::default(),
            sla: None,
        }
    }

    pub fn title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    pub fn endpoint(mut self, name: &str, value: f64) -> Self {
        self.endpoints.push((name.to_string(), value));
        self
    }

    pub fn sla(mut self, ms: f64) -> Self {
        self.sla = Some(ms);
        self
    }

    pub fn ok(mut self, count: u32) -> Self {
        self.status_codes.ok_2xx = count;
        self
    }

    pub fn redirect(mut self, count: u32) -> Self {
        self.status_codes.redirect_3xx = count;
        self
    }

    pub fn client_error(mut self, count: u32) -> Self {
        self.status_codes.client_error_4xx = count;
        self
    }

    pub fn server_error(mut self, count: u32) -> Self {
        self.status_codes.server_error_5xx = count;
        self
    }

    pub fn save(self) {
        let mut metadata = HashMap::new();
        metadata.insert("template".to_string(), serde_json::json!("api"));

        if let Some(sla) = self.sla {
            metadata.insert("sla".to_string(), serde_json::json!(sla));
        }

        let (chart_type, series, y_label) = match self.mode {
            ApiMode::ResponseTimes => {
                metadata.insert("unit".to_string(), serde_json::json!("ms"));
                let data: Vec<DataPoint> = self.endpoints.iter().map(|(name, value)| {
                    DataPoint {
                        x: serde_json::json!(name),
                        y: *value,
                        label: None,
                    }
                }).collect();

                (ChartType::Bar, vec![DataSeries {
                    name: "Response Time".to_string(),
                    data,
                    color: Some("#10b981".to_string()),
                }], "ms")
            }
            ApiMode::StatusCodes => {
                let data = vec![
                    DataPoint { x: serde_json::json!("2xx OK"), y: self.status_codes.ok_2xx as f64, label: None },
                    DataPoint { x: serde_json::json!("3xx Redirect"), y: self.status_codes.redirect_3xx as f64, label: None },
                    DataPoint { x: serde_json::json!("4xx Client Error"), y: self.status_codes.client_error_4xx as f64, label: None },
                    DataPoint { x: serde_json::json!("5xx Server Error"), y: self.status_codes.server_error_5xx as f64, label: None },
                ];

                (ChartType::Pie, vec![DataSeries {
                    name: "Status Codes".to_string(),
                    data,
                    color: None,
                }], "count")
            }
            ApiMode::Throughput => {
                metadata.insert("unit".to_string(), serde_json::json!("req/s"));
                let data: Vec<DataPoint> = self.endpoints.iter().map(|(name, value)| {
                    DataPoint {
                        x: serde_json::json!(name),
                        y: *value,
                        label: None,
                    }
                }).collect();

                (ChartType::Bar, vec![DataSeries {
                    name: "Throughput".to_string(),
                    data,
                    color: Some("#6366f1".to_string()),
                }], "req/s")
            }
        };

        let artifact = TestArtifact {
            test_name: self.test_name,
            chart_type,
            title: self.title,
            x_label: Some("Endpoint".to_string()),
            y_label: Some(y_label.to_string()),
            series,
            metadata,
        };

        artifact.save();
    }
}

// ============================================================================
// TEMPLATE 4: Test Coverage
// ============================================================================

/// Template pour la couverture de code
///
/// # Exemples
///
/// ```rust
/// // Coverage par module
/// Coverage::by_module("test_coverage")
///     .module("src/api", 85.0)
///     .module("src/db", 72.0)
///     .module("src/utils", 95.0)
///     .target(80.0)
///     .save();
///
/// // Coverage global (gauge)
/// Coverage::total("test_coverage_total")
///     .percentage(82.5)
///     .target(80.0)
///     .save();
/// ```
pub struct Coverage {
    test_name: String,
    title: String,
    mode: CoverageMode,
    modules: Vec<(String, f64)>,
    total: f64,
    target: Option<f64>,
    lines_covered: Option<u32>,
    lines_total: Option<u32>,
}

#[derive(Debug, Clone)]
enum CoverageMode {
    ByModule,
    Total,
    Trend,
}

impl Coverage {
    /// Graphique de couverture par module (barres)
    pub fn by_module(test_name: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            title: "Coverage by Module".to_string(),
            mode: CoverageMode::ByModule,
            modules: vec![],
            total: 0.0,
            target: None,
            lines_covered: None,
            lines_total: None,
        }
    }

    /// Jauge de couverture totale
    pub fn total(test_name: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            title: "Total Coverage".to_string(),
            mode: CoverageMode::Total,
            modules: vec![],
            total: 0.0,
            target: None,
            lines_covered: None,
            lines_total: None,
        }
    }

    /// Graphique d'évolution de la couverture (ligne)
    pub fn trend(test_name: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            title: "Coverage Trend".to_string(),
            mode: CoverageMode::Trend,
            modules: vec![],
            total: 0.0,
            target: None,
            lines_covered: None,
            lines_total: None,
        }
    }

    pub fn title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    pub fn module(mut self, name: &str, percentage: f64) -> Self {
        self.modules.push((name.to_string(), percentage));
        self
    }

    pub fn percentage(mut self, value: f64) -> Self {
        self.total = value;
        self
    }

    pub fn target(mut self, percentage: f64) -> Self {
        self.target = Some(percentage);
        self
    }

    pub fn lines(mut self, covered: u32, total: u32) -> Self {
        self.lines_covered = Some(covered);
        self.lines_total = Some(total);
        self
    }

    /// Ajoute un point de données pour le trend
    pub fn point(mut self, label: &str, percentage: f64) -> Self {
        self.modules.push((label.to_string(), percentage));
        self
    }

    pub fn save(self) {
        let mut metadata = HashMap::new();
        metadata.insert("unit".to_string(), serde_json::json!("%"));
        metadata.insert("template".to_string(), serde_json::json!("coverage"));

        if let Some(target) = self.target {
            metadata.insert("target".to_string(), serde_json::json!(target));
        }

        if let (Some(covered), Some(total)) = (self.lines_covered, self.lines_total) {
            metadata.insert("lines_covered".to_string(), serde_json::json!(covered));
            metadata.insert("lines_total".to_string(), serde_json::json!(total));
        }

        let (chart_type, series) = match self.mode {
            CoverageMode::ByModule => {
                let data: Vec<DataPoint> = self.modules.iter().map(|(name, value)| {
                    DataPoint {
                        x: serde_json::json!(name),
                        y: *value,
                        label: None,
                    }
                }).collect();

                (ChartType::Bar, vec![DataSeries {
                    name: "Coverage".to_string(),
                    data,
                    color: Some("#22c55e".to_string()),
                }])
            }
            CoverageMode::Total => {
                let data = vec![DataPoint {
                    x: serde_json::json!("Coverage"),
                    y: self.total,
                    label: None,
                }];

                (ChartType::Gauge, vec![DataSeries {
                    name: "Coverage".to_string(),
                    data,
                    color: Some(if self.total >= self.target.unwrap_or(80.0) {
                        "#22c55e".to_string()
                    } else {
                        "#ef4444".to_string()
                    }),
                }])
            }
            CoverageMode::Trend => {
                let data: Vec<DataPoint> = self.modules.iter().map(|(label, value)| {
                    DataPoint {
                        x: serde_json::json!(label),
                        y: *value,
                        label: None,
                    }
                }).collect();

                (ChartType::Line, vec![DataSeries {
                    name: "Coverage".to_string(),
                    data,
                    color: Some("#3b82f6".to_string()),
                }])
            }
        };

        let artifact = TestArtifact {
            test_name: self.test_name,
            chart_type,
            title: self.title,
            x_label: match self.mode {
                CoverageMode::ByModule => Some("Module".to_string()),
                CoverageMode::Total => None,
                CoverageMode::Trend => Some("Date".to_string()),
            },
            y_label: Some("%".to_string()),
            series,
            metadata,
        };

        artifact.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_latency() {
        Performance::latency("test_perf")
            .title("Test Latency")
            .data(&[10.0, 20.0, 15.0])
            .threshold(25.0)
            .save();
    }

    #[test]
    fn test_memory_usage() {
        Memory::usage("test_mem")
            .title("Test Memory")
            .samples(&[100.0, 110.0, 105.0])
            .limit(200.0)
            .save();
    }

    #[test]
    fn test_api_response() {
        Api::response_times("test_api")
            .endpoint("/users", 45.0)
            .endpoint("/orders", 80.0)
            .sla(100.0)
            .save();
    }

    #[test]
    fn test_coverage() {
        Coverage::by_module("test_cov")
            .module("api", 85.0)
            .module("db", 70.0)
            .target(80.0)
            .save();
    }
}
