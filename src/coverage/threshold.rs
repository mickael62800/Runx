//! Coverage threshold validation

use colored::Colorize;

use super::CoverageData;

/// Result of threshold validation
#[derive(Debug, Clone)]
pub struct ThresholdResult {
    pub passed: bool,
    pub line_coverage: Option<f64>,
    pub branch_coverage: Option<f64>,
    pub line_threshold: Option<f64>,
    pub branch_threshold: Option<f64>,
    pub line_delta: Option<f64>,
    pub branch_delta: Option<f64>,
}

impl ThresholdResult {
    pub fn print_summary(&self) {
        if let (Some(coverage), Some(threshold)) = (self.line_coverage, self.line_threshold) {
            let delta = coverage - threshold;
            let status = if delta >= 0.0 { "✓".green() } else { "✗".red() };
            let delta_str = if delta >= 0.0 {
                format!("+{:.1}%", delta).green()
            } else {
                format!("{:.1}%", delta).red()
            };

            println!(
                "  {} Line coverage: {:.1}% (threshold: {:.1}%, {})",
                status, coverage, threshold, delta_str
            );
        }

        if let (Some(coverage), Some(threshold)) = (self.branch_coverage, self.branch_threshold) {
            let delta = coverage - threshold;
            let status = if delta >= 0.0 { "✓".green() } else { "✗".red() };
            let delta_str = if delta >= 0.0 {
                format!("+{:.1}%", delta).green()
            } else {
                format!("{:.1}%", delta).red()
            };

            println!(
                "  {} Branch coverage: {:.1}% (threshold: {:.1}%, {})",
                status, coverage, threshold, delta_str
            );
        }
    }
}

/// Validate coverage against thresholds
pub fn validate_threshold(
    data: &CoverageData,
    line_threshold: Option<f64>,
    branch_threshold: Option<f64>,
) -> ThresholdResult {
    let line_coverage = data.line_coverage;
    let branch_coverage = data.branch_coverage;

    let line_passed = match (line_coverage, line_threshold) {
        (Some(cov), Some(thresh)) => cov >= thresh,
        _ => true,
    };

    let branch_passed = match (branch_coverage, branch_threshold) {
        (Some(cov), Some(thresh)) => cov >= thresh,
        _ => true,
    };

    ThresholdResult {
        passed: line_passed && branch_passed,
        line_coverage,
        branch_coverage,
        line_threshold,
        branch_threshold,
        line_delta: match (line_coverage, line_threshold) {
            (Some(cov), Some(thresh)) => Some(cov - thresh),
            _ => None,
        },
        branch_delta: match (branch_coverage, branch_threshold) {
            (Some(cov), Some(thresh)) => Some(cov - thresh),
            _ => None,
        },
    }
}

/// Compare coverage between two runs
pub fn compare_coverage(old: &CoverageData, new: &CoverageData) -> CoverageComparison {
    let line_delta = match (old.line_coverage, new.line_coverage) {
        (Some(o), Some(n)) => Some(n - o),
        _ => None,
    };

    let branch_delta = match (old.branch_coverage, new.branch_coverage) {
        (Some(o), Some(n)) => Some(n - o),
        _ => None,
    };

    CoverageComparison {
        old_line_coverage: old.line_coverage,
        new_line_coverage: new.line_coverage,
        old_branch_coverage: old.branch_coverage,
        new_branch_coverage: new.branch_coverage,
        line_delta,
        branch_delta,
        improved: line_delta.map(|d| d > 0.0).unwrap_or(false)
            || branch_delta.map(|d| d > 0.0).unwrap_or(false),
        degraded: line_delta.map(|d| d < -1.0).unwrap_or(false)
            || branch_delta.map(|d| d < -1.0).unwrap_or(false),
    }
}

#[derive(Debug, Clone)]
pub struct CoverageComparison {
    pub old_line_coverage: Option<f64>,
    pub new_line_coverage: Option<f64>,
    pub old_branch_coverage: Option<f64>,
    pub new_branch_coverage: Option<f64>,
    pub line_delta: Option<f64>,
    pub branch_delta: Option<f64>,
    pub improved: bool,
    pub degraded: bool,
}

impl CoverageComparison {
    pub fn print_summary(&self) {
        println!("Coverage comparison:");

        if let (Some(old), Some(new), Some(delta)) = (
            self.old_line_coverage,
            self.new_line_coverage,
            self.line_delta,
        ) {
            let indicator = if delta > 0.0 {
                "↑".green()
            } else if delta < 0.0 {
                "↓".red()
            } else {
                "→".dimmed()
            };

            let delta_str = if delta > 0.0 {
                format!("+{:.1}%", delta).green()
            } else if delta < 0.0 {
                format!("{:.1}%", delta).red()
            } else {
                "0%".dimmed().to_string().into()
            };

            println!(
                "  {} Line: {:.1}% → {:.1}% ({})",
                indicator, old, new, delta_str
            );
        }

        if let (Some(old), Some(new), Some(delta)) = (
            self.old_branch_coverage,
            self.new_branch_coverage,
            self.branch_delta,
        ) {
            let indicator = if delta > 0.0 {
                "↑".green()
            } else if delta < 0.0 {
                "↓".red()
            } else {
                "→".dimmed()
            };

            let delta_str = if delta > 0.0 {
                format!("+{:.1}%", delta).green()
            } else if delta < 0.0 {
                format!("{:.1}%", delta).red()
            } else {
                "0%".dimmed().to_string().into()
            };

            println!(
                "  {} Branch: {:.1}% → {:.1}% ({})",
                indicator, old, new, delta_str
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threshold_validation() {
        let data = CoverageData {
            line_coverage: Some(75.0),
            branch_coverage: Some(50.0),
            ..Default::default()
        };

        // Passing case
        let result = validate_threshold(&data, Some(70.0), Some(40.0));
        assert!(result.passed);

        // Failing case
        let result = validate_threshold(&data, Some(80.0), Some(40.0));
        assert!(!result.passed);
    }

    #[test]
    fn test_coverage_comparison() {
        let old = CoverageData {
            line_coverage: Some(70.0),
            branch_coverage: Some(50.0),
            ..Default::default()
        };

        let new = CoverageData {
            line_coverage: Some(75.0),
            branch_coverage: Some(55.0),
            ..Default::default()
        };

        let comparison = compare_coverage(&old, &new);
        assert!(comparison.improved);
        assert!(!comparison.degraded);
        assert_eq!(comparison.line_delta, Some(5.0));
    }
}
