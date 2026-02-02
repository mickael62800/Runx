//! Affected tests detection
//!
//! Maps source files to their associated tests for automatic re-running
//! when files change.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::test_model::TestNode;

/// Maps source files to affected tests
pub struct AffectedTestsMap {
    /// Map from source file path to test names that depend on it
    file_to_tests: HashMap<String, Vec<String>>,
    /// Set of all test file paths
    test_files: HashSet<String>,
}

impl AffectedTestsMap {
    pub fn new() -> Self {
        Self {
            file_to_tests: HashMap::new(),
            test_files: HashSet::new(),
        }
    }

    /// Build the affected tests map from a test tree
    /// This is a heuristic based approach:
    /// - Tests in src/foo.rs are affected by changes to src/foo.rs
    /// - Tests in tests/foo_test.rs are affected by changes to src/foo.rs
    /// - Module tests (mod tests) are affected by changes to the parent module
    pub fn from_test_tree(tree: &TestNode, project_dir: &Path) -> Self {
        let mut map = Self::new();

        for test in tree.all_tests() {
            // Parse test module path to infer source file
            let source_paths = infer_source_paths(&test.module_path, project_dir);

            for path in source_paths {
                map.file_to_tests
                    .entry(path.clone())
                    .or_insert_with(Vec::new)
                    .push(test.full_name.clone());
            }

            // Also track test files themselves
            if let Some(ref test_file) = test.source_file {
                map.test_files.insert(test_file.clone());
            }
        }

        map
    }

    /// Find tests affected by a file change
    pub fn find_affected_tests(&self, changed_file: &str) -> Vec<String> {
        let normalized = normalize_path(changed_file);

        // Direct match
        if let Some(tests) = self.file_to_tests.get(&normalized) {
            return tests.clone();
        }

        // Try partial matches (e.g., the file is a parent module)
        let mut affected = Vec::new();
        for (path, tests) in &self.file_to_tests {
            if path.starts_with(&normalized) || normalized.starts_with(path) {
                affected.extend(tests.clone());
            }
        }

        // If it's a test file, rerun its tests
        if is_test_file(&normalized) {
            // Extract module name from test file path
            if let Some(module) = extract_module_from_test_file(&normalized) {
                for (path, tests) in &self.file_to_tests {
                    if path.contains(&module) {
                        affected.extend(tests.clone());
                    }
                }
            }
        }

        // Deduplicate
        affected.sort();
        affected.dedup();
        affected
    }

    /// Check if a file change should trigger test re-runs
    pub fn should_rerun(&self, changed_file: &str) -> bool {
        let normalized = normalize_path(changed_file);

        // Rust source files
        if normalized.ends_with(".rs") {
            return true;
        }

        // Cargo.toml changes
        if normalized.ends_with("Cargo.toml") {
            return true;
        }

        false
    }
}

impl Default for AffectedTestsMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Infer source file paths from test module path
fn infer_source_paths(module_path: &[String], _project_dir: &Path) -> Vec<String> {
    let mut paths = Vec::new();

    if module_path.is_empty() {
        return paths;
    }

    // Common patterns:
    // 1. module::tests -> src/module.rs or src/module/mod.rs
    // 2. module::submod::tests -> src/module/submod.rs or src/module/submod/mod.rs

    let module_parts: Vec<&String> = module_path.iter()
        .filter(|p| *p != "tests" && *p != "test")
        .collect();

    if module_parts.is_empty() {
        // Root level tests
        paths.push("src/lib.rs".to_string());
        paths.push("src/main.rs".to_string());
        return paths;
    }

    // Build possible paths
    let module_path_str = module_parts.iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("/");

    // src/module.rs
    paths.push(format!("src/{}.rs", module_path_str));

    // src/module/mod.rs
    paths.push(format!("src/{}/mod.rs", module_path_str));

    // Just the first module (for nested modules)
    if module_parts.len() > 1 {
        paths.push(format!("src/{}.rs", module_parts[0]));
        paths.push(format!("src/{}/mod.rs", module_parts[0]));
    }

    paths
}

/// Normalize path separators and remove leading ./
fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

/// Check if a file is a test file
fn is_test_file(path: &str) -> bool {
    path.contains("/tests/")
        || path.ends_with("_test.rs")
        || path.ends_with("_tests.rs")
        || path.contains("test_")
}

/// Extract module name from a test file path
fn extract_module_from_test_file(path: &str) -> Option<String> {
    // tests/foo_test.rs -> foo
    // tests/integration/foo.rs -> foo

    let filename = path.rsplit('/').next()?;
    let name = filename.strip_suffix(".rs")?;

    // Remove _test or _tests suffix
    let module = name
        .strip_suffix("_test")
        .or_else(|| name.strip_suffix("_tests"))
        .unwrap_or(name);

    // Remove test_ prefix
    let module = module.strip_prefix("test_").unwrap_or(module);

    Some(module.to_string())
}

/// Find all tests that might be affected by changes to a set of files
pub fn find_affected_from_files(
    changed_files: &[String],
    test_tree: &TestNode,
    project_dir: &Path,
) -> Vec<String> {
    let map = AffectedTestsMap::from_test_tree(test_tree, project_dir);

    let mut affected = Vec::new();
    for file in changed_files {
        if map.should_rerun(file) {
            let tests = map.find_affected_tests(file);
            affected.extend(tests);
        }
    }

    // If we couldn't determine specific tests, return all
    if affected.is_empty() && changed_files.iter().any(|f| f.ends_with(".rs")) {
        return test_tree.all_test_names();
    }

    // Deduplicate
    affected.sort();
    affected.dedup();
    affected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_source_paths() {
        let project_dir = Path::new("/project");

        let paths = infer_source_paths(&["config".to_string(), "tests".to_string()], project_dir);
        assert!(paths.contains(&"src/config.rs".to_string()));
        assert!(paths.contains(&"src/config/mod.rs".to_string()));
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("./src/foo.rs"), "src/foo.rs");
        assert_eq!(normalize_path("src\\foo.rs"), "src/foo.rs");
    }

    #[test]
    fn test_extract_module() {
        assert_eq!(extract_module_from_test_file("tests/foo_test.rs"), Some("foo".to_string()));
        assert_eq!(extract_module_from_test_file("tests/test_bar.rs"), Some("bar".to_string()));
    }
}
