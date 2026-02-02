//! Cobertura XML format parser

use anyhow::Result;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs;
use std::path::Path;

use super::{CoverageData, FileCoverage};

/// Parse a Cobertura XML file
pub fn parse_cobertura(path: &Path) -> Result<CoverageData> {
    let content = fs::read_to_string(path)?;
    parse_cobertura_string(&content)
}

/// Parse Cobertura XML content from a string
pub fn parse_cobertura_string(content: &str) -> Result<CoverageData> {
    let mut reader = Reader::from_str(content);
    reader.trim_text(true);

    let mut data = CoverageData::default();
    let mut files: Vec<FileCoverage> = Vec::new();

    let mut current_file: Option<String> = None;
    let mut file_lines_covered = 0u32;
    let mut file_lines_total = 0u32;
    let mut file_branches_covered = 0u32;
    let mut file_branches_total = 0u32;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                match e.name().as_ref() {
                    b"coverage" => {
                        // Extract overall coverage from attributes
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            match attr.key.as_ref() {
                                b"line-rate" => {
                                    if let Ok(rate) = String::from_utf8_lossy(&attr.value).parse::<f64>() {
                                        data.line_coverage = Some(rate * 100.0);
                                    }
                                }
                                b"branch-rate" => {
                                    if let Ok(rate) = String::from_utf8_lossy(&attr.value).parse::<f64>() {
                                        data.branch_coverage = Some(rate * 100.0);
                                    }
                                }
                                b"lines-covered" => {
                                    if let Ok(count) = String::from_utf8_lossy(&attr.value).parse::<u32>() {
                                        data.lines_covered = count;
                                    }
                                }
                                b"lines-valid" => {
                                    if let Ok(count) = String::from_utf8_lossy(&attr.value).parse::<u32>() {
                                        data.lines_total = count;
                                    }
                                }
                                b"branches-covered" => {
                                    if let Ok(count) = String::from_utf8_lossy(&attr.value).parse::<u32>() {
                                        data.branches_covered = count;
                                    }
                                }
                                b"branches-valid" => {
                                    if let Ok(count) = String::from_utf8_lossy(&attr.value).parse::<u32>() {
                                        data.branches_total = count;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    b"class" => {
                        // Extract filename and coverage for this class
                        let mut filename = String::new();
                        let mut line_rate = 0.0f64;
                        let mut branch_rate = 0.0f64;

                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            match attr.key.as_ref() {
                                b"filename" => {
                                    filename = String::from_utf8_lossy(&attr.value).to_string();
                                }
                                b"line-rate" => {
                                    if let Ok(rate) = String::from_utf8_lossy(&attr.value).parse::<f64>() {
                                        line_rate = rate;
                                    }
                                }
                                b"branch-rate" => {
                                    if let Ok(rate) = String::from_utf8_lossy(&attr.value).parse::<f64>() {
                                        branch_rate = rate;
                                    }
                                }
                                _ => {}
                            }
                        }

                        if !filename.is_empty() {
                            current_file = Some(filename);
                            file_lines_covered = 0;
                            file_lines_total = 0;
                            file_branches_covered = 0;
                            file_branches_total = 0;
                        }
                    }
                    b"line" => {
                        // Count lines
                        if current_file.is_some() {
                            file_lines_total += 1;

                            for attr in e.attributes().filter_map(|a| a.ok()) {
                                match attr.key.as_ref() {
                                    b"hits" => {
                                        if let Ok(hits) = String::from_utf8_lossy(&attr.value).parse::<u32>() {
                                            if hits > 0 {
                                                file_lines_covered += 1;
                                            }
                                        }
                                    }
                                    b"branch" => {
                                        if String::from_utf8_lossy(&attr.value) == "true" {
                                            file_branches_total += 1;
                                        }
                                    }
                                    b"condition-coverage" => {
                                        // Parse "50% (1/2)" format
                                        let value = String::from_utf8_lossy(&attr.value);
                                        if let Some(paren_start) = value.find('(') {
                                            if let Some(slash) = value.find('/') {
                                                if let Ok(covered) = value[paren_start + 1..slash].parse::<u32>() {
                                                    file_branches_covered += covered;
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                if e.name().as_ref() == b"class" {
                    // End of class, save file coverage
                    if let Some(file_path) = current_file.take() {
                        let line_cov = if file_lines_total > 0 {
                            (file_lines_covered as f64 / file_lines_total as f64) * 100.0
                        } else {
                            0.0
                        };

                        let branch_cov = if file_branches_total > 0 {
                            (file_branches_covered as f64 / file_branches_total as f64) * 100.0
                        } else {
                            0.0
                        };

                        files.push(FileCoverage {
                            path: file_path,
                            lines_covered: file_lines_covered,
                            lines_total: file_lines_total,
                            branches_covered: file_branches_covered,
                            branches_total: file_branches_total,
                            line_coverage: line_cov,
                            branch_coverage: branch_cov,
                        });
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("Error parsing Cobertura XML: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    data.files = files;

    // Calculate totals from files if not set
    if data.lines_total == 0 {
        data.lines_total = data.files.iter().map(|f| f.lines_total).sum();
        data.lines_covered = data.files.iter().map(|f| f.lines_covered).sum();
    }
    if data.branches_total == 0 {
        data.branches_total = data.files.iter().map(|f| f.branches_total).sum();
        data.branches_covered = data.files.iter().map(|f| f.branches_covered).sum();
    }

    // Calculate percentages if not set
    if data.line_coverage.is_none() {
        data.line_coverage = Some(data.calculate_line_percentage());
    }
    if data.branch_coverage.is_none() {
        data.branch_coverage = Some(data.calculate_branch_percentage());
    }

    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cobertura() {
        let xml = r#"<?xml version="1.0"?>
<coverage line-rate="0.8" branch-rate="0.5" lines-covered="80" lines-valid="100">
    <packages>
        <package name="src">
            <classes>
                <class name="main" filename="src/main.rs" line-rate="0.75" branch-rate="0.5">
                    <lines>
                        <line number="1" hits="1"/>
                        <line number="2" hits="1"/>
                        <line number="3" hits="0"/>
                        <line number="4" hits="1"/>
                    </lines>
                </class>
            </classes>
        </package>
    </packages>
</coverage>"#;

        let data = parse_cobertura_string(xml).unwrap();

        assert!((data.line_coverage.unwrap() - 80.0).abs() < 0.01);
        assert_eq!(data.lines_covered, 80);
        assert_eq!(data.lines_total, 100);
    }
}
