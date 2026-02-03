//! Test artifacts module
//!
//! Handles test output artifacts for custom visualizations (charts, graphs, etc.)

#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Directory where tests write their artifacts
pub const ARTIFACTS_DIR: &str = "target/runx/artifacts";

/// Chart type for visualization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChartType {
    Line,
    Bar,
    Gauge,
    Area,
    Scatter,
    Pie,
}

/// A data point for charts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub x: f64,
    pub y: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// A data series for charts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSeries {
    pub name: String,
    pub data: Vec<DataPoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// Test artifact with visualization data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestArtifact {
    /// Test name this artifact belongs to
    pub test_name: String,
    /// Chart type
    pub chart_type: ChartType,
    /// Chart title
    pub title: String,
    /// X-axis label
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_label: Option<String>,
    /// Y-axis label
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_label: Option<String>,
    /// Data series
    pub series: Vec<DataSeries>,
    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl TestArtifact {
    /// Create a new line chart artifact
    pub fn line_chart(test_name: &str, title: &str) -> Self {
        Self {
            test_name: test_name.to_string(),
            chart_type: ChartType::Line,
            title: title.to_string(),
            x_label: None,
            y_label: None,
            series: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add a data series
    pub fn add_series(&mut self, name: &str, data: Vec<(f64, f64)>) -> &mut Self {
        self.series.push(DataSeries {
            name: name.to_string(),
            data: data.into_iter().map(|(x, y)| DataPoint { x, y, label: None }).collect(),
            color: None,
        });
        self
    }

    /// Set axis labels
    pub fn with_labels(mut self, x_label: &str, y_label: &str) -> Self {
        self.x_label = Some(x_label.to_string());
        self.y_label = Some(y_label.to_string());
        self
    }

    /// Save artifact to file
    pub fn save(&self, project_dir: &Path) -> Result<PathBuf> {
        let artifacts_dir = project_dir.join(ARTIFACTS_DIR);
        fs::create_dir_all(&artifacts_dir)?;

        let filename = format!("{}.json", sanitize_filename(&self.test_name));
        let path = artifacts_dir.join(&filename);

        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)?;

        Ok(path)
    }
}

/// Sanitize test name for use as filename
fn sanitize_filename(name: &str) -> String {
    name.replace("::", "_").replace(['/', '\\', ' '], "_")
}

/// Load all artifacts from the artifacts directory
pub fn load_artifacts(project_dir: &Path) -> Result<Vec<TestArtifact>> {
    let artifacts_dir = project_dir.join(ARTIFACTS_DIR);

    if !artifacts_dir.exists() {
        return Ok(Vec::new());
    }

    let mut artifacts = Vec::new();

    for entry in fs::read_dir(&artifacts_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map(|e| e == "json").unwrap_or(false) {
            match load_artifact(&path) {
                Ok(artifact) => artifacts.push(artifact),
                Err(e) => eprintln!("Warning: Failed to load artifact {:?}: {}", path, e),
            }
        }
    }

    Ok(artifacts)
}

/// Load a single artifact from file
pub fn load_artifact(path: &Path) -> Result<TestArtifact> {
    let content = fs::read_to_string(path)?;
    let artifact: TestArtifact = serde_json::from_str(&content)?;
    Ok(artifact)
}

/// Get artifact for a specific test
pub fn get_artifact_for_test(project_dir: &Path, test_name: &str) -> Result<Option<TestArtifact>> {
    let filename = format!("{}.json", sanitize_filename(test_name));
    let path = project_dir.join(ARTIFACTS_DIR).join(&filename);

    if path.exists() {
        Ok(Some(load_artifact(&path)?))
    } else {
        Ok(None)
    }
}

/// Clear all artifacts
pub fn clear_artifacts(project_dir: &Path) -> Result<()> {
    let artifacts_dir = project_dir.join(ARTIFACTS_DIR);
    if artifacts_dir.exists() {
        fs::remove_dir_all(&artifacts_dir)?;
    }
    Ok(())
}

/// Macro helper for tests to easily create artifacts
/// Usage in tests:
/// ```ignore
/// runx_artifact!("test_name", "Chart Title", ChartType::Line, {
///     series: [("Series 1", vec![(0.0, 1.0), (1.0, 2.0)])],
///     x_label: "Time",
///     y_label: "Value"
/// });
/// ```
#[macro_export]
macro_rules! runx_artifact {
    ($test_name:expr, $title:expr, $chart_type:expr, $data:expr) => {{
        use std::path::Path;
        let artifact = $crate::artifacts::TestArtifact {
            test_name: $test_name.to_string(),
            chart_type: $chart_type,
            title: $title.to_string(),
            x_label: None,
            y_label: None,
            series: vec![],
            metadata: std::collections::HashMap::new(),
        };
        artifact.save(Path::new("."))
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_and_save_artifact() {
        let temp_dir = TempDir::new().unwrap();

        let mut artifact = TestArtifact::line_chart("my_module::test_drawdown", "Portfolio Drawdown");
        artifact.add_series("Drawdown %", vec![
            (0.0, 0.0),
            (1.0, -5.2),
            (2.0, -3.1),
            (3.0, -12.4),
            (4.0, -8.7),
        ]);

        let path = artifact.save(temp_dir.path()).unwrap();
        assert!(path.exists());

        // Reload and verify
        let loaded = load_artifact(&path).unwrap();
        assert_eq!(loaded.test_name, "my_module::test_drawdown");
        assert_eq!(loaded.series.len(), 1);
        assert_eq!(loaded.series[0].data.len(), 5);
    }

    #[test]
    fn test_load_all_artifacts() {
        let temp_dir = TempDir::new().unwrap();

        // Create multiple artifacts
        let artifact1 = TestArtifact::line_chart("test_one", "Chart 1");
        let artifact2 = TestArtifact::line_chart("test_two", "Chart 2");

        artifact1.save(temp_dir.path()).unwrap();
        artifact2.save(temp_dir.path()).unwrap();

        let artifacts = load_artifacts(temp_dir.path()).unwrap();
        assert_eq!(artifacts.len(), 2);
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("module::tests::test_one"), "module_tests_test_one");
        assert_eq!(sanitize_filename("test with spaces"), "test_with_spaces");
    }
}
