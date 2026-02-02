//! Git diff parsing for affected task detection

use anyhow::{Context, Result};
use git2::{DiffOptions, Repository, StatusOptions};
use std::collections::HashSet;
use std::path::Path;

/// Git diff operations
pub struct GitDiff {
    repo: Repository,
}

impl GitDiff {
    /// Open a repository at the given path
    pub fn new(path: &Path) -> Result<Self> {
        let repo = Repository::discover(path)
            .with_context(|| format!("Failed to find git repository at {}", path.display()))?;

        Ok(Self { repo })
    }

    /// Get files changed since a given reference (commit, branch, or tag)
    pub fn get_changed_files(
        &self,
        since: Option<&str>,
        base: Option<&str>,
    ) -> Result<Vec<String>> {
        let mut changed_files: HashSet<String> = HashSet::new();

        // Get uncommitted changes (staged and unstaged)
        let uncommitted = self.get_uncommitted_changes()?;
        changed_files.extend(uncommitted);

        // Get committed changes since reference
        if let Some(ref_name) = since.or(base) {
            let committed = self.get_committed_changes_since(ref_name)?;
            changed_files.extend(committed);
        }

        let mut result: Vec<String> = changed_files.into_iter().collect();
        result.sort();

        Ok(result)
    }

    /// Get uncommitted changes (both staged and unstaged)
    pub fn get_uncommitted_changes(&self) -> Result<Vec<String>> {
        let mut files = Vec::new();
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        opts.recurse_untracked_dirs(true);

        let statuses = self.repo.statuses(Some(&mut opts))?;

        for entry in statuses.iter() {
            if let Some(path) = entry.path() {
                files.push(path.to_string());
            }
        }

        Ok(files)
    }

    /// Get files changed in commits since a reference
    fn get_committed_changes_since(&self, ref_name: &str) -> Result<Vec<String>> {
        let mut files: HashSet<String> = HashSet::new();

        // Resolve the reference
        let obj = self.repo.revparse_single(ref_name)
            .with_context(|| format!("Failed to resolve reference: {}", ref_name))?;

        let old_tree = obj.peel_to_commit()?.tree()?;

        // Get HEAD commit
        let head = self.repo.head()?.peel_to_commit()?;
        let new_tree = head.tree()?;

        // Get diff
        let mut diff_opts = DiffOptions::new();
        let diff = self.repo.diff_tree_to_tree(
            Some(&old_tree),
            Some(&new_tree),
            Some(&mut diff_opts),
        )?;

        // Collect changed files
        diff.foreach(
            &mut |delta, _| {
                if let Some(path) = delta.new_file().path() {
                    files.insert(path.to_string_lossy().to_string());
                }
                if let Some(path) = delta.old_file().path() {
                    files.insert(path.to_string_lossy().to_string());
                }
                true
            },
            None,
            None,
            None,
        )?;

        Ok(files.into_iter().collect())
    }

    /// Get the current branch name
    pub fn current_branch(&self) -> Result<Option<String>> {
        let head = self.repo.head()?;
        if head.is_branch() {
            Ok(head.shorthand().map(|s| s.to_string()))
        } else {
            Ok(None)
        }
    }

    /// Get the default branch (main or master)
    pub fn default_branch(&self) -> Result<String> {
        // Try common default branch names
        for name in &["main", "master"] {
            if self.repo.find_branch(name, git2::BranchType::Local).is_ok() {
                return Ok(name.to_string());
            }
        }

        // Fallback to main
        Ok("main".to_string())
    }

    /// Check if the repository is dirty (has uncommitted changes)
    pub fn is_dirty(&self) -> Result<bool> {
        let changes = self.get_uncommitted_changes()?;
        Ok(!changes.is_empty())
    }

    /// Get files changed between two refs
    pub fn get_diff_between(&self, from: &str, to: &str) -> Result<Vec<String>> {
        let mut files: HashSet<String> = HashSet::new();

        let from_obj = self.repo.revparse_single(from)?;
        let to_obj = self.repo.revparse_single(to)?;

        let from_tree = from_obj.peel_to_commit()?.tree()?;
        let to_tree = to_obj.peel_to_commit()?.tree()?;

        let mut diff_opts = DiffOptions::new();
        let diff = self.repo.diff_tree_to_tree(
            Some(&from_tree),
            Some(&to_tree),
            Some(&mut diff_opts),
        )?;

        diff.foreach(
            &mut |delta, _| {
                if let Some(path) = delta.new_file().path() {
                    files.insert(path.to_string_lossy().to_string());
                }
                if let Some(path) = delta.old_file().path() {
                    files.insert(path.to_string_lossy().to_string());
                }
                true
            },
            None,
            None,
            None,
        )?;

        Ok(files.into_iter().collect())
    }
}

/// Categorize changed files by type
pub fn categorize_files(files: &[String]) -> FileCategories {
    let mut categories = FileCategories::default();

    for file in files {
        if file.ends_with(".rs") {
            categories.rust.push(file.clone());
        } else if file.ends_with(".ts") || file.ends_with(".tsx") {
            categories.typescript.push(file.clone());
        } else if file.ends_with(".js") || file.ends_with(".jsx") {
            categories.javascript.push(file.clone());
        } else if file.ends_with(".py") {
            categories.python.push(file.clone());
        } else if file.ends_with(".go") {
            categories.go.push(file.clone());
        } else if file.ends_with(".toml") || file.ends_with(".json") || file.ends_with(".yaml") || file.ends_with(".yml") {
            categories.config.push(file.clone());
        } else if file.contains("test") || file.contains("spec") {
            categories.tests.push(file.clone());
        } else {
            categories.other.push(file.clone());
        }
    }

    categories
}

#[derive(Debug, Default)]
pub struct FileCategories {
    pub rust: Vec<String>,
    pub typescript: Vec<String>,
    pub javascript: Vec<String>,
    pub python: Vec<String>,
    pub go: Vec<String>,
    pub config: Vec<String>,
    pub tests: Vec<String>,
    pub other: Vec<String>,
}

impl FileCategories {
    pub fn total(&self) -> usize {
        self.rust.len()
            + self.typescript.len()
            + self.javascript.len()
            + self.python.len()
            + self.go.len()
            + self.config.len()
            + self.tests.len()
            + self.other.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_files() {
        let files = vec![
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "tests/integration.rs".to_string(),
            "package.json".to_string(),
            "src/app.tsx".to_string(),
        ];

        let categories = categorize_files(&files);
        // tests/integration.rs matches .rs first, so all 3 go to rust
        assert_eq!(categories.rust.len(), 3);
        assert_eq!(categories.typescript.len(), 1);
        assert_eq!(categories.config.len(), 1);
        // No files go to tests category since language extension takes precedence
        assert_eq!(categories.tests.len(), 0);
    }
}
