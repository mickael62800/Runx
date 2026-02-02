//! Test code annotator

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::providers::{AiClient, AiConfig, TestAnnotation};
use crate::db::Database;

/// Test annotator for generating AI-powered test descriptions
pub struct TestAnnotator {
    client: AiClient,
}

impl TestAnnotator {
    /// Create a new annotator
    pub fn new(config: &AiConfig) -> Result<Self> {
        let client = AiClient::new(config)?;
        Ok(Self { client })
    }

    /// Annotate all tests in a file
    pub async fn annotate_file(&self, file_path: &Path) -> Result<Vec<TestAnnotation>> {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        let tests = extract_tests(&content, file_path);

        if tests.is_empty() {
            return Ok(vec![]);
        }

        self.client.annotate_tests(&tests).await
    }

    /// Annotate a single test by name
    pub async fn annotate_single(&self, test_name: &str, test_code: &str) -> Result<TestAnnotation> {
        self.client.annotate_test(test_name, test_code).await
    }

    /// Annotate tests and store in database
    pub async fn annotate_and_store(
        &self,
        file_path: &Path,
        db: &Database,
    ) -> Result<Vec<TestAnnotation>> {
        let annotations = self.annotate_file(file_path).await?;

        for annotation in &annotations {
            db.store_test_annotation(annotation)?;
        }

        Ok(annotations)
    }
}

/// Extract test functions from source code
fn extract_tests(content: &str, file_path: &Path) -> Vec<(String, String)> {
    let extension = file_path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    match extension {
        "rs" => extract_rust_tests(content),
        "ts" | "tsx" | "js" | "jsx" => extract_js_tests(content),
        "py" => extract_python_tests(content),
        "go" => extract_go_tests(content),
        _ => vec![],
    }
}

/// Extract Rust tests (#[test] functions)
fn extract_rust_tests(content: &str) -> Vec<(String, String)> {
    let mut tests = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Look for #[test] or #[tokio::test]
        if line == "#[test]" || line.contains("#[tokio::test]") || line.contains("#[async_std::test]") {
            // Find the function definition
            let mut j = i + 1;
            while j < lines.len() {
                let fn_line = lines[j].trim();
                if fn_line.starts_with("fn ") || fn_line.starts_with("async fn ") || fn_line.starts_with("pub fn ") {
                    // Extract function name
                    let name = extract_fn_name(fn_line);

                    // Extract function body
                    let body = extract_function_body(&lines, j);

                    if let Some(name) = name {
                        tests.push((name, body));
                    }
                    break;
                }
                j += 1;
            }
            i = j;
        }
        i += 1;
    }

    tests
}

/// Extract JavaScript/TypeScript tests (it, test, describe blocks)
fn extract_js_tests(content: &str) -> Vec<(String, String)> {
    let mut tests = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Match it('...'), test('...'), it("..."), test("...")
        if (trimmed.starts_with("it(") || trimmed.starts_with("test("))
            && (trimmed.contains("'") || trimmed.contains("\""))
        {
            if let Some(name) = extract_js_test_name(trimmed) {
                let body = extract_js_block(&lines, i);
                tests.push((name, body));
            }
        }
    }

    tests
}

/// Extract Python tests (def test_* functions)
fn extract_python_tests(content: &str) -> Vec<(String, String)> {
    let mut tests = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Match def test_* or async def test_*
        if (trimmed.starts_with("def test_") || trimmed.starts_with("async def test_"))
            && trimmed.contains("(")
        {
            if let Some(name) = extract_python_fn_name(trimmed) {
                let body = extract_python_block(&lines, i);
                tests.push((name, body));
            }
        }
    }

    tests
}

/// Extract Go tests (func Test* functions)
fn extract_go_tests(content: &str) -> Vec<(String, String)> {
    let mut tests = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Match func Test*
        if trimmed.starts_with("func Test") && trimmed.contains("(") {
            if let Some(name) = extract_go_fn_name(trimmed) {
                let body = extract_function_body(&lines, i);
                tests.push((name, body));
            }
        }
    }

    tests
}

fn extract_fn_name(line: &str) -> Option<String> {
    // fn test_something() or async fn test_something()
    let line = line.trim_start_matches("pub ");
    let line = line.trim_start_matches("async ");
    let line = line.trim_start_matches("fn ");

    if let Some(paren_idx) = line.find('(') {
        Some(line[..paren_idx].trim().to_string())
    } else {
        None
    }
}

fn extract_js_test_name(line: &str) -> Option<String> {
    // it('test name', ...) or test("test name", ...)
    let start = line.find(['\'', '"'])?;
    let quote = line.chars().nth(start)?;
    let rest = &line[start + 1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn extract_python_fn_name(line: &str) -> Option<String> {
    let line = line.trim_start_matches("async ");
    let line = line.trim_start_matches("def ");

    if let Some(paren_idx) = line.find('(') {
        Some(line[..paren_idx].trim().to_string())
    } else {
        None
    }
}

fn extract_go_fn_name(line: &str) -> Option<String> {
    let line = line.trim_start_matches("func ");

    if let Some(paren_idx) = line.find('(') {
        Some(line[..paren_idx].trim().to_string())
    } else {
        None
    }
}

fn extract_function_body(lines: &[&str], start_idx: usize) -> String {
    let mut body = Vec::new();
    let mut brace_count = 0;
    let mut started = false;

    for line in lines.iter().skip(start_idx) {
        body.push(*line);

        for c in line.chars() {
            if c == '{' {
                brace_count += 1;
                started = true;
            } else if c == '}' {
                brace_count -= 1;
            }
        }

        if started && brace_count == 0 {
            break;
        }
    }

    body.join("\n")
}

fn extract_js_block(lines: &[&str], start_idx: usize) -> String {
    extract_function_body(lines, start_idx)
}

fn extract_python_block(lines: &[&str], start_idx: usize) -> String {
    let mut body = Vec::new();
    let base_indent = lines[start_idx].len() - lines[start_idx].trim_start().len();

    body.push(lines[start_idx]);

    for line in lines.iter().skip(start_idx + 1) {
        if line.trim().is_empty() {
            body.push(*line);
            continue;
        }

        let current_indent = line.len() - line.trim_start().len();
        if current_indent <= base_indent && !line.trim().is_empty() {
            break;
        }
        body.push(*line);
    }

    body.join("\n")
}

/// Database extension for test annotations
impl Database {
    /// Store a test annotation
    pub fn store_test_annotation(&self, annotation: &TestAnnotation) -> Result<()> {
        self.connection().execute(
            r#"INSERT OR REPLACE INTO test_annotations
               (test_name, description, purpose, tested_function, test_type, tags, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))"#,
            rusqlite::params![
                annotation.test_name,
                annotation.description,
                annotation.purpose,
                annotation.tested_function,
                annotation.test_type,
                serde_json::to_string(&annotation.tags).unwrap_or_default(),
            ],
        )?;
        Ok(())
    }

    /// Get annotation for a test
    pub fn get_test_annotation(&self, test_name: &str) -> Result<Option<TestAnnotation>> {
        let mut stmt = self.connection().prepare(
            r#"SELECT test_name, description, purpose, tested_function, test_type, tags
               FROM test_annotations WHERE test_name = ?1"#
        )?;

        let mut rows = stmt.query(rusqlite::params![test_name])?;

        if let Some(row) = rows.next()? {
            let tags_json: String = row.get::<_, String>(5)?;
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

            Ok(Some(TestAnnotation {
                test_name: row.get(0)?,
                description: row.get(1)?,
                purpose: row.get(2)?,
                tested_function: row.get(3)?,
                test_type: row.get(4)?,
                tags,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get all annotations
    pub fn get_all_annotations(&self) -> Result<Vec<TestAnnotation>> {
        let mut stmt = self.connection().prepare(
            r#"SELECT test_name, description, purpose, tested_function, test_type, tags
               FROM test_annotations ORDER BY test_name"#
        )?;

        let mut annotations = Vec::new();
        let mut rows = stmt.query([])?;

        while let Some(row) = rows.next()? {
            let tags_json: String = row.get::<_, String>(5)?;
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

            annotations.push(TestAnnotation {
                test_name: row.get(0)?,
                description: row.get(1)?,
                purpose: row.get(2)?,
                tested_function: row.get(3)?,
                test_type: row.get(4)?,
                tags,
            });
        }

        Ok(annotations)
    }

    /// Get annotations by tag
    pub fn get_annotations_by_tag(&self, tag: &str) -> Result<Vec<TestAnnotation>> {
        let all = self.get_all_annotations()?;
        Ok(all.into_iter().filter(|a| a.tags.contains(&tag.to_string())).collect())
    }

    /// Get annotations by test type
    pub fn get_annotations_by_type(&self, test_type: &str) -> Result<Vec<TestAnnotation>> {
        let mut stmt = self.connection().prepare(
            r#"SELECT test_name, description, purpose, tested_function, test_type, tags
               FROM test_annotations WHERE test_type = ?1 ORDER BY test_name"#
        )?;

        let mut annotations = Vec::new();
        let mut rows = stmt.query(rusqlite::params![test_type])?;

        while let Some(row) = rows.next()? {
            let tags_json: String = row.get::<_, String>(5)?;
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

            annotations.push(TestAnnotation {
                test_name: row.get(0)?,
                description: row.get(1)?,
                purpose: row.get(2)?,
                tested_function: row.get(3)?,
                test_type: row.get(4)?,
                tags,
            });
        }

        Ok(annotations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_rust_tests() {
        let code = r#"
#[test]
fn test_addition() {
    assert_eq!(2 + 2, 4);
}

#[tokio::test]
async fn test_async_thing() {
    let result = async_fn().await;
    assert!(result.is_ok());
}
"#;

        let tests = extract_rust_tests(code);
        assert_eq!(tests.len(), 2);
        assert_eq!(tests[0].0, "test_addition");
        assert_eq!(tests[1].0, "test_async_thing");
    }

    #[test]
    fn test_extract_js_tests() {
        let code = r#"
describe('Calculator', () => {
    it('should add two numbers', () => {
        expect(add(1, 2)).toBe(3);
    });

    test("should subtract numbers", () => {
        expect(subtract(5, 3)).toBe(2);
    });
});
"#;

        let tests = extract_js_tests(code);
        assert_eq!(tests.len(), 2);
        assert_eq!(tests[0].0, "should add two numbers");
        assert_eq!(tests[1].0, "should subtract numbers");
    }

    #[test]
    fn test_extract_python_tests() {
        let code = r#"
def test_addition():
    assert 2 + 2 == 4

async def test_async_function():
    result = await async_fn()
    assert result is not None
"#;

        let tests = extract_python_tests(code);
        assert_eq!(tests.len(), 2);
        assert_eq!(tests[0].0, "test_addition");
        assert_eq!(tests[1].0, "test_async_function");
    }
}
