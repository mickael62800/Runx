//! Coverage module
//!
//! Provides:
//! - LCOV parsing
//! - Cobertura XML parsing
//! - Threshold validation

mod cobertura;
mod lcov;
mod threshold;

pub use cobertura::*;
pub use lcov::*;
pub use threshold::*;

/// Coverage data from any source
#[derive(Debug, Clone, Default)]
pub struct CoverageData {
    pub line_coverage: Option<f64>,
    pub branch_coverage: Option<f64>,
    pub lines_covered: u32,
    pub lines_total: u32,
    pub branches_covered: u32,
    pub branches_total: u32,
    pub files: Vec<FileCoverage>,
}

impl CoverageData {
    pub fn calculate_line_percentage(&self) -> f64 {
        if self.lines_total == 0 {
            return 0.0;
        }
        (self.lines_covered as f64 / self.lines_total as f64) * 100.0
    }

    pub fn calculate_branch_percentage(&self) -> f64 {
        if self.branches_total == 0 {
            return 0.0;
        }
        (self.branches_covered as f64 / self.branches_total as f64) * 100.0
    }
}

/// Coverage data for a single file
#[derive(Debug, Clone)]
pub struct FileCoverage {
    pub path: String,
    pub lines_covered: u32,
    pub lines_total: u32,
    pub branches_covered: u32,
    pub branches_total: u32,
    pub line_coverage: f64,
    pub branch_coverage: f64,
}

/// Parse coverage from a file based on format
pub fn parse_coverage(path: &std::path::Path, format: &str) -> anyhow::Result<CoverageData> {
    match format.to_lowercase().as_str() {
        "lcov" => parse_lcov(path),
        "cobertura" => parse_cobertura(path),
        _ => anyhow::bail!("Unknown coverage format: {}. Supported: lcov, cobertura", format),
    }
}
