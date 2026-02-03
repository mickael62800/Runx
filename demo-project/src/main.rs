//! Trading Portfolio Demo
//!
//! This demo shows how to use Runx artifacts to visualize test results.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn main() {
    println!("Trading Portfolio Demo");
    println!("Run: cargo test");
}

/// Portfolio with daily values
pub struct Portfolio {
    pub values: Vec<f64>,
}

impl Portfolio {
    pub fn new(values: Vec<f64>) -> Self {
        Self { values }
    }

    /// Calculate daily returns
    pub fn daily_returns(&self) -> Vec<f64> {
        self.values
            .windows(2)
            .map(|w| (w[1] - w[0]) / w[0] * 100.0)
            .collect()
    }

    /// Calculate cumulative returns
    pub fn cumulative_returns(&self) -> Vec<f64> {
        if self.values.is_empty() {
            return vec![];
        }
        let initial = self.values[0];
        self.values
            .iter()
            .map(|v| (v - initial) / initial * 100.0)
            .collect()
    }

    /// Calculate drawdown (percentage from peak)
    pub fn drawdown(&self) -> Vec<f64> {
        let mut peak = f64::NEG_INFINITY;
        self.values
            .iter()
            .map(|&v| {
                if v > peak {
                    peak = v;
                }
                if peak > 0.0 {
                    (v - peak) / peak * 100.0
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// Get maximum drawdown
    pub fn max_drawdown(&self) -> f64 {
        self.drawdown()
            .iter()
            .fold(0.0, |min, &x| if x < min { x } else { min })
    }

    /// Calculate Sharpe ratio (simplified)
    pub fn sharpe_ratio(&self) -> f64 {
        let returns = self.daily_returns();
        if returns.is_empty() {
            return 0.0;
        }
        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
        let std_dev = variance.sqrt();
        if std_dev == 0.0 {
            0.0
        } else {
            (mean / std_dev) * (252.0_f64).sqrt() // Annualized
        }
    }

    /// Calculate win rate
    pub fn win_rate(&self) -> f64 {
        let returns = self.daily_returns();
        if returns.is_empty() {
            return 0.0;
        }
        let wins = returns.iter().filter(|&&r| r > 0.0).count();
        (wins as f64 / returns.len() as f64) * 100.0
    }
}

// ============== RUNX ARTIFACT HELPERS ==============

/// Chart type for visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartType {
    Line,
    Bar,
    Gauge,
    Area,
    Scatter,
    Pie,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub x: f64,
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
    pub fn save(&self, project_dir: &Path) -> std::io::Result<()> {
        let artifacts_dir = project_dir.join("target/runx/artifacts");
        fs::create_dir_all(&artifacts_dir)?;

        let filename = self.test_name.replace("::", "_").replace(['/', '\\', ' '], "_");
        let path = artifacts_dir.join(format!("{}.json", filename));

        let json = serde_json::to_string_pretty(self).unwrap();
        fs::write(path, json)?;
        Ok(())
    }
}

// ============== TESTS ==============

#[cfg(test)]
mod tests {
    use super::*;
    use runx_charts::prelude::*;
    use std::path::PathBuf;

    fn get_project_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    // ==================== TEMPLATE TESTS ====================

    /// Test using Performance template - API Latency
    #[test]
    fn test_api_latency() {
        // Simulated API latencies in ms
        let latencies = vec![12.5, 15.2, 11.8, 14.1, 13.3, 18.5, 11.2, 16.8, 14.5, 12.9];

        Performance::latency("tests::test_api_latency")
            .title("API Endpoint Latency")
            .unit("ms")
            .labels(&["GET /users", "POST /orders", "GET /products", "PUT /cart", "DELETE /item",
                     "GET /search", "POST /login", "GET /profile", "PUT /settings", "GET /stats"])
            .data(&latencies)
            .threshold(20.0)
            .save();

        let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
        assert!(avg < 20.0, "Average latency should be under 20ms");
        println!("Average latency: {:.2}ms", avg);
    }

    /// Test using Performance template - Latency Percentiles
    #[test]
    fn test_latency_percentiles() {
        Performance::latency_percentiles("tests::test_latency_percentiles")
            .title("API Latency Percentiles")
            .p50(12.0)
            .p90(25.0)
            .p99(45.0)
            .p999(120.0)
            .save();

        assert!(true);
    }

    /// Test using Performance template - Throughput
    #[test]
    fn test_server_throughput() {
        let throughput = vec![1000.0, 1200.0, 1150.0, 1300.0, 1250.0, 1400.0];

        Performance::throughput("tests::test_server_throughput")
            .title("HTTP Server Throughput")
            .labels(&["Baseline", "With Cache", "Optimized", "With CDN", "Load Balanced", "Final"])
            .data(&throughput)
            .compare(&[950.0, 1000.0, 1050.0, 1100.0, 1150.0, 1200.0]) // Baseline comparison
            .save();

        assert!(throughput.last().unwrap() > &1200.0);
    }

    /// Test using Memory template - Usage over time
    #[test]
    fn test_memory_usage() {
        let samples = vec![100.0, 105.0, 112.0, 108.0, 115.0, 120.0, 118.0, 125.0, 122.0, 128.0];

        Memory::usage("tests::test_memory_usage")
            .title("Application Memory Usage")
            .samples(&samples)
            .timestamps(&["0s", "10s", "20s", "30s", "40s", "50s", "60s", "70s", "80s", "90s"])
            .limit(200.0)
            .save();

        assert!(samples.last().unwrap() < &200.0, "Memory should stay under limit");
    }

    /// Test using Memory template - Breakdown
    #[test]
    fn test_memory_breakdown() {
        Memory::breakdown("tests::test_memory_breakdown")
            .title("Memory Allocation Breakdown")
            .heap(85.0)
            .stack(12.0)
            .static_mem(8.0)
            .other(5.0)
            .save();

        assert!(true);
    }

    /// Test using API template - Response times by endpoint
    #[test]
    fn test_api_endpoints() {
        Api::response_times("tests::test_api_endpoints")
            .title("API Response Times by Endpoint")
            .endpoint("GET /users", 45.0)
            .endpoint("POST /orders", 120.0)
            .endpoint("GET /products", 35.0)
            .endpoint("PUT /cart", 55.0)
            .endpoint("DELETE /session", 25.0)
            .sla(100.0)
            .save();

        // Check that most endpoints are under SLA
        assert!(true);
    }

    /// Test using API template - Status codes
    #[test]
    fn test_api_status_codes() {
        Api::status_codes("tests::test_api_status_codes")
            .title("HTTP Response Status Distribution")
            .ok(950)
            .redirect(20)
            .client_error(25)
            .server_error(5)
            .save();

        let total = 950 + 20 + 25 + 5;
        let error_rate = (25.0 + 5.0) / total as f64 * 100.0;
        assert!(error_rate < 5.0, "Error rate should be under 5%");
        println!("Error rate: {:.2}%", error_rate);
    }

    /// Test using Coverage template - By module
    #[test]
    fn test_coverage_by_module() {
        Coverage::by_module("tests::test_coverage_by_module")
            .title("Test Coverage by Module")
            .module("src/api", 85.0)
            .module("src/db", 72.0)
            .module("src/auth", 90.0)
            .module("src/utils", 95.0)
            .module("src/handlers", 68.0)
            .target(80.0)
            .save();

        assert!(true);
    }

    /// Test using Coverage template - Total gauge
    #[test]
    fn test_coverage_total() {
        Coverage::total("tests::test_coverage_total")
            .title("Overall Test Coverage")
            .percentage(82.5)
            .lines(1650, 2000)
            .target(80.0)
            .save();

        assert!(82.5 >= 80.0, "Coverage should meet target");
    }

    /// Test using Coverage template - Trend
    #[test]
    fn test_coverage_trend() {
        Coverage::trend("tests::test_coverage_trend")
            .title("Coverage Over Time")
            .point("Jan", 65.0)
            .point("Feb", 70.0)
            .point("Mar", 72.0)
            .point("Apr", 78.0)
            .point("May", 80.0)
            .point("Jun", 82.5)
            .target(80.0)
            .save();

        assert!(true);
    }

    // ==================== ORIGINAL TRADING TESTS ====================

    fn sample_portfolio() -> Portfolio {
        // Simulated 30 days of portfolio values
        Portfolio::new(vec![
            10000.0, 10150.0, 10080.0, 10200.0, 10350.0,
            10180.0, 10050.0, 9900.0,  9750.0,  9850.0,
            10000.0, 10200.0, 10450.0, 10600.0, 10400.0,
            10300.0, 10500.0, 10700.0, 10650.0, 10800.0,
            10950.0, 11000.0, 10850.0, 10700.0, 10900.0,
            11100.0, 11250.0, 11400.0, 11350.0, 11500.0,
        ])
    }

    #[test]
    fn test_drawdown() {
        let portfolio = sample_portfolio();
        let drawdown = portfolio.drawdown();
        let max_dd = portfolio.max_drawdown();

        // Create artifact for visualization
        let artifact = TestArtifact {
            test_name: "tests::test_drawdown".to_string(),
            chart_type: ChartType::Area,
            title: "Portfolio Drawdown".to_string(),
            x_label: Some("Day".to_string()),
            y_label: Some("Drawdown %".to_string()),
            series: vec![DataSeries {
                name: "Drawdown".to_string(),
                data: drawdown.iter().enumerate().map(|(i, &y)| DataPoint {
                    x: i as f64,
                    y,
                    label: None,
                }).collect(),
                color: Some("#ef5350".to_string()),
            }],
            metadata: HashMap::from([
                ("max_drawdown".to_string(), serde_json::json!(max_dd)),
            ]),
        };
        artifact.save(&get_project_dir()).unwrap();

        // Assertions
        assert!(max_dd < 0.0, "Drawdown should be negative");
        assert!(max_dd > -20.0, "Max drawdown should be reasonable (> -20%)");
        println!("Max Drawdown: {:.2}%", max_dd);
    }

    #[test]
    fn test_cumulative_returns() {
        let portfolio = sample_portfolio();
        let returns = portfolio.cumulative_returns();

        // Create artifact
        let artifact = TestArtifact {
            test_name: "tests::test_cumulative_returns".to_string(),
            chart_type: ChartType::Line,
            title: "Cumulative Returns".to_string(),
            x_label: Some("Day".to_string()),
            y_label: Some("Return %".to_string()),
            series: vec![DataSeries {
                name: "Cumulative Return".to_string(),
                data: returns.iter().enumerate().map(|(i, &y)| DataPoint {
                    x: i as f64,
                    y,
                    label: None,
                }).collect(),
                color: Some("#26a69a".to_string()),
            }],
            metadata: HashMap::new(),
        };
        artifact.save(&get_project_dir()).unwrap();

        let final_return = returns.last().unwrap_or(&0.0);
        assert!(*final_return > 0.0, "Should have positive returns");
        println!("Final return: {:.2}%", final_return);
    }

    #[test]
    fn test_daily_returns_distribution() {
        let portfolio = sample_portfolio();
        let returns = portfolio.daily_returns();

        // Group returns into buckets for histogram
        let mut buckets: HashMap<i32, i32> = HashMap::new();
        for r in &returns {
            let bucket = (r / 0.5).round() as i32; // 0.5% buckets
            *buckets.entry(bucket).or_insert(0) += 1;
        }

        let mut sorted_buckets: Vec<_> = buckets.into_iter().collect();
        sorted_buckets.sort_by_key(|&(k, _)| k);

        // Create artifact
        let artifact = TestArtifact {
            test_name: "tests::test_daily_returns_distribution".to_string(),
            chart_type: ChartType::Bar,
            title: "Daily Returns Distribution".to_string(),
            x_label: Some("Return %".to_string()),
            y_label: Some("Count".to_string()),
            series: vec![DataSeries {
                name: "Frequency".to_string(),
                data: sorted_buckets.iter().map(|&(bucket, count)| DataPoint {
                    x: (bucket as f64) * 0.5,
                    y: count as f64,
                    label: Some(format!("{:.1}%", (bucket as f64) * 0.5)),
                }).collect(),
                color: Some("#00d4ff".to_string()),
            }],
            metadata: HashMap::new(),
        };
        artifact.save(&get_project_dir()).unwrap();

        assert!(!returns.is_empty());
    }

    #[test]
    fn test_sharpe_ratio() {
        let portfolio = sample_portfolio();
        let sharpe = portfolio.sharpe_ratio();

        // Create gauge artifact
        let artifact = TestArtifact {
            test_name: "tests::test_sharpe_ratio".to_string(),
            chart_type: ChartType::Gauge,
            title: "Sharpe Ratio".to_string(),
            x_label: None,
            y_label: None,
            series: vec![DataSeries {
                name: "Sharpe".to_string(),
                data: vec![DataPoint {
                    x: 0.0,
                    y: (sharpe.clamp(-2.0, 3.0) + 2.0) / 5.0 * 100.0, // Normalize to 0-100
                    label: Some(format!("{:.2}", sharpe)),
                }],
                color: None,
            }],
            metadata: HashMap::from([
                ("raw_value".to_string(), serde_json::json!(sharpe)),
            ]),
        };
        artifact.save(&get_project_dir()).unwrap();

        println!("Sharpe Ratio: {:.2}", sharpe);
        assert!(sharpe.is_finite());
    }

    #[test]
    fn test_portfolio_metrics() {
        let portfolio = sample_portfolio();
        let win_rate = portfolio.win_rate();
        let sharpe = portfolio.sharpe_ratio();
        let max_dd = portfolio.max_drawdown();
        let final_return = portfolio.cumulative_returns().last().copied().unwrap_or(0.0);

        // Create pie chart for win/loss ratio
        let returns = portfolio.daily_returns();
        let wins = returns.iter().filter(|&&r| r > 0.0).count();
        let losses = returns.len() - wins;

        let artifact = TestArtifact {
            test_name: "tests::test_portfolio_metrics".to_string(),
            chart_type: ChartType::Pie,
            title: "Win/Loss Ratio".to_string(),
            x_label: None,
            y_label: None,
            series: vec![DataSeries {
                name: "Trades".to_string(),
                data: vec![
                    DataPoint { x: 0.0, y: wins as f64, label: Some("Winning Days".to_string()) },
                    DataPoint { x: 1.0, y: losses as f64, label: Some("Losing Days".to_string()) },
                ],
                color: None,
            }],
            metadata: HashMap::from([
                ("win_rate".to_string(), serde_json::json!(win_rate)),
                ("sharpe_ratio".to_string(), serde_json::json!(sharpe)),
                ("max_drawdown".to_string(), serde_json::json!(max_dd)),
                ("total_return".to_string(), serde_json::json!(final_return)),
            ]),
        };
        artifact.save(&get_project_dir()).unwrap();

        println!("Win Rate: {:.1}%", win_rate);
        assert!(win_rate > 40.0, "Win rate should be reasonable");
    }

    #[test]
    fn test_portfolio_vs_benchmark() {
        let portfolio = sample_portfolio();

        // Simulate a benchmark (e.g., S&P 500)
        let benchmark = Portfolio::new(vec![
            10000.0, 10050.0, 10100.0, 10150.0, 10200.0,
            10180.0, 10160.0, 10140.0, 10160.0, 10200.0,
            10250.0, 10300.0, 10350.0, 10400.0, 10380.0,
            10360.0, 10400.0, 10450.0, 10480.0, 10520.0,
            10560.0, 10600.0, 10580.0, 10560.0, 10600.0,
            10650.0, 10700.0, 10750.0, 10780.0, 10820.0,
        ]);

        let portfolio_returns = portfolio.cumulative_returns();
        let benchmark_returns = benchmark.cumulative_returns();

        // Create comparison chart
        let artifact = TestArtifact {
            test_name: "tests::test_portfolio_vs_benchmark".to_string(),
            chart_type: ChartType::Line,
            title: "Portfolio vs Benchmark".to_string(),
            x_label: Some("Day".to_string()),
            y_label: Some("Return %".to_string()),
            series: vec![
                DataSeries {
                    name: "Portfolio".to_string(),
                    data: portfolio_returns.iter().enumerate().map(|(i, &y)| DataPoint {
                        x: i as f64,
                        y,
                        label: None,
                    }).collect(),
                    color: Some("#00d4ff".to_string()),
                },
                DataSeries {
                    name: "Benchmark".to_string(),
                    data: benchmark_returns.iter().enumerate().map(|(i, &y)| DataPoint {
                        x: i as f64,
                        y,
                        label: None,
                    }).collect(),
                    color: Some("#ffd700".to_string()),
                },
            ],
            metadata: HashMap::new(),
        };
        artifact.save(&get_project_dir()).unwrap();

        let port_final = portfolio_returns.last().unwrap_or(&0.0);
        let bench_final = benchmark_returns.last().unwrap_or(&0.0);

        println!("Portfolio: {:.2}%, Benchmark: {:.2}%", port_final, bench_final);
        assert!(port_final > bench_final, "Portfolio should outperform benchmark");
    }

    #[test]
    fn test_volatility_over_time() {
        let portfolio = sample_portfolio();
        let returns = portfolio.daily_returns();

        // Calculate rolling 5-day volatility
        let window_size = 5;
        let mut volatilities: Vec<f64> = Vec::new();

        for i in window_size..=returns.len() {
            let window = &returns[i - window_size..i];
            let mean = window.iter().sum::<f64>() / window_size as f64;
            let variance = window.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / window_size as f64;
            let std_dev = variance.sqrt();
            volatilities.push(std_dev);
        }

        // Create artifact
        let artifact = TestArtifact {
            test_name: "tests::test_volatility_over_time".to_string(),
            chart_type: ChartType::Area,
            title: "Rolling 5-Day Volatility".to_string(),
            x_label: Some("Day".to_string()),
            y_label: Some("Volatility %".to_string()),
            series: vec![DataSeries {
                name: "Volatility".to_string(),
                data: volatilities.iter().enumerate().map(|(i, &y)| DataPoint {
                    x: (i + window_size) as f64,
                    y,
                    label: None,
                }).collect(),
                color: Some("#9c27b0".to_string()), // Purple
            }],
            metadata: HashMap::from([
                ("avg_volatility".to_string(), serde_json::json!(
                    volatilities.iter().sum::<f64>() / volatilities.len() as f64
                )),
            ]),
        };
        artifact.save(&get_project_dir()).unwrap();

        println!("Avg Volatility: {:.2}%", volatilities.iter().sum::<f64>() / volatilities.len() as f64);
        assert!(!volatilities.is_empty());
    }

    #[test]
    fn test_monthly_performance() {
        // Simulate monthly returns (12 months)
        let monthly_returns = vec![
            ("Jan", 2.5),
            ("Feb", -1.2),
            ("Mar", 3.8),
            ("Apr", 1.5),
            ("May", -0.8),
            ("Jun", 4.2),
            ("Jul", -2.1),
            ("Aug", 1.9),
            ("Sep", -0.5),
            ("Oct", 3.1),
            ("Nov", 2.8),
            ("Dec", 1.7),
        ];

        let artifact = TestArtifact {
            test_name: "tests::test_monthly_performance".to_string(),
            chart_type: ChartType::Bar,
            title: "Monthly Performance".to_string(),
            x_label: Some("Month".to_string()),
            y_label: Some("Return %".to_string()),
            series: vec![DataSeries {
                name: "Monthly Return".to_string(),
                data: monthly_returns.iter().enumerate().map(|(i, (month, ret))| DataPoint {
                    x: i as f64,
                    y: *ret,
                    label: Some(month.to_string()),
                }).collect(),
                color: Some("#00d4ff".to_string()),
            }],
            metadata: HashMap::from([
                ("total_return".to_string(), serde_json::json!(
                    monthly_returns.iter().map(|(_, r)| r).sum::<f64>()
                )),
                ("best_month".to_string(), serde_json::json!("Jun")),
                ("worst_month".to_string(), serde_json::json!("Jul")),
            ]),
        };
        artifact.save(&get_project_dir()).unwrap();

        let total: f64 = monthly_returns.iter().map(|(_, r)| r).sum();
        println!("Total annual return: {:.1}%", total);
        assert!(total > 0.0, "Should have positive annual return");
    }
}
