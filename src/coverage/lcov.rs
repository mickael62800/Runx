//! LCOV format parser

use anyhow::Result;
use std::fs;
use std::path::Path;

use super::{CoverageData, FileCoverage};

/// Parse an LCOV file
pub fn parse_lcov(path: &Path) -> Result<CoverageData> {
    let content = fs::read_to_string(path)?;
    parse_lcov_string(&content)
}

/// Parse LCOV content from a string
pub fn parse_lcov_string(content: &str) -> Result<CoverageData> {
    let mut data = CoverageData::default();
    let mut files: Vec<FileCoverage> = Vec::new();

    let mut current_file: Option<String> = None;
    let mut file_lines_found = 0u32;
    let mut file_lines_hit = 0u32;
    let mut file_branches_found = 0u32;
    let mut file_branches_hit = 0u32;

    for line in content.lines() {
        let line = line.trim();

        if line.starts_with("SF:") {
            // Source file start
            current_file = Some(line[3..].to_string());
            file_lines_found = 0;
            file_lines_hit = 0;
            file_branches_found = 0;
            file_branches_hit = 0;
        } else if line.starts_with("LF:") {
            // Lines found
            if let Ok(count) = line[3..].parse::<u32>() {
                file_lines_found = count;
            }
        } else if line.starts_with("LH:") {
            // Lines hit
            if let Ok(count) = line[3..].parse::<u32>() {
                file_lines_hit = count;
            }
        } else if line.starts_with("BRF:") {
            // Branches found
            if let Ok(count) = line[4..].parse::<u32>() {
                file_branches_found = count;
            }
        } else if line.starts_with("BRH:") {
            // Branches hit
            if let Ok(count) = line[4..].parse::<u32>() {
                file_branches_hit = count;
            }
        } else if line == "end_of_record" {
            // End of file record
            if let Some(file_path) = current_file.take() {
                let line_cov = if file_lines_found > 0 {
                    (file_lines_hit as f64 / file_lines_found as f64) * 100.0
                } else {
                    0.0
                };

                let branch_cov = if file_branches_found > 0 {
                    (file_branches_hit as f64 / file_branches_found as f64) * 100.0
                } else {
                    0.0
                };

                files.push(FileCoverage {
                    path: file_path,
                    lines_covered: file_lines_hit,
                    lines_total: file_lines_found,
                    branches_covered: file_branches_hit,
                    branches_total: file_branches_found,
                    line_coverage: line_cov,
                    branch_coverage: branch_cov,
                });

                // Add to totals
                data.lines_covered += file_lines_hit;
                data.lines_total += file_lines_found;
                data.branches_covered += file_branches_hit;
                data.branches_total += file_branches_found;
            }
        }
    }

    data.files = files;
    data.line_coverage = Some(data.calculate_line_percentage());
    data.branch_coverage = Some(data.calculate_branch_percentage());

    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lcov() {
        let lcov = r#"
TN:
SF:src/main.rs
FN:1,main
FNDA:1,main
FNF:1
FNH:1
DA:1,1
DA:2,1
DA:3,0
LF:3
LH:2
BRF:2
BRH:1
end_of_record
SF:src/lib.rs
DA:1,1
DA:2,1
LF:2
LH:2
end_of_record
"#;

        let data = parse_lcov_string(lcov).unwrap();

        assert_eq!(data.files.len(), 2);
        assert_eq!(data.lines_total, 5);
        assert_eq!(data.lines_covered, 4);

        // 4/5 = 80%
        assert!((data.line_coverage.unwrap() - 80.0).abs() < 0.01);
    }

    #[test]
    fn test_empty_lcov() {
        let data = parse_lcov_string("").unwrap();
        assert_eq!(data.files.len(), 0);
        assert_eq!(data.lines_total, 0);
    }
}
