//! Workspace/monorepo support

use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::{Config, Task};

/// A workspace containing multiple packages
#[derive(Debug)]
pub struct Workspace {
    pub root_dir: PathBuf,
    pub packages: Vec<Package>,
}

/// A package within a workspace
#[derive(Debug)]
pub struct Package {
    pub name: String,
    pub path: PathBuf,
    pub config: Option<Config>,
}

impl Workspace {
    /// Discover packages in a workspace based on glob patterns
    pub fn discover(root_dir: &Path, patterns: &[String]) -> Result<Self> {
        let mut packages = Vec::new();

        for pattern in patterns {
            let full_pattern = root_dir.join(pattern);
            let full_pattern_str = full_pattern.to_string_lossy();

            for entry in glob::glob(&full_pattern_str)? {
                if let Ok(path) = entry {
                    if path.is_dir() {
                        let package_name = path
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "unknown".to_string());

                        // Look for runx.toml in package
                        let config_path = path.join("runx.toml");
                        let config = if config_path.exists() {
                            Config::load(&config_path).ok()
                        } else {
                            None
                        };

                        packages.push(Package {
                            name: package_name,
                            path,
                            config,
                        });
                    }
                }
            }
        }

        Ok(Workspace {
            root_dir: root_dir.to_path_buf(),
            packages,
        })
    }

    /// Get all tasks from all packages, prefixed with package name
    pub fn get_all_tasks(&self) -> HashMap<String, (Task, PathBuf)> {
        let mut tasks = HashMap::new();

        for package in &self.packages {
            if let Some(ref config) = package.config {
                for (task_name, task) in &config.tasks {
                    let prefixed_name = format!("{}:{}", package.name, task_name);
                    tasks.insert(prefixed_name, (task.clone(), package.path.clone()));
                }
            }
        }

        tasks
    }

    /// Get tasks for a specific package
    pub fn get_package_tasks(&self, package_name: &str) -> Option<&Config> {
        self.packages
            .iter()
            .find(|p| p.name == package_name)
            .and_then(|p| p.config.as_ref())
    }

    /// Filter tasks by package pattern
    pub fn filter_tasks(&self, pattern: &str) -> Vec<String> {
        let all_tasks = self.get_all_tasks();

        if let Ok(glob_pattern) = glob::Pattern::new(pattern) {
            all_tasks
                .keys()
                .filter(|name| glob_pattern.matches(name))
                .cloned()
                .collect()
        } else {
            // Treat as prefix match
            all_tasks
                .keys()
                .filter(|name| name.starts_with(pattern))
                .cloned()
                .collect()
        }
    }

    /// Get package names
    pub fn package_names(&self) -> Vec<&str> {
        self.packages.iter().map(|p| p.name.as_str()).collect()
    }
}

/// Build a merged config from workspace packages
pub fn merge_workspace_configs(workspace: &Workspace, root_config: &Config) -> Config {
    let mut merged = root_config.clone();

    for package in &workspace.packages {
        if let Some(ref pkg_config) = package.config {
            for (task_name, task) in &pkg_config.tasks {
                let prefixed_name = format!("{}:{}", package.name, task_name);

                // Adjust task cwd to be relative to root
                let mut adjusted_task = task.clone();
                let pkg_rel_path = package.path
                    .strip_prefix(&workspace.root_dir)
                    .unwrap_or(&package.path);

                adjusted_task.cwd = Some(
                    adjusted_task
                        .cwd
                        .map(|cwd| pkg_rel_path.join(cwd).to_string_lossy().to_string())
                        .unwrap_or_else(|| pkg_rel_path.to_string_lossy().to_string())
                );

                // Adjust depends_on to use prefixed names
                adjusted_task.depends_on = task.depends_on
                    .iter()
                    .map(|dep| {
                        if dep.contains(':') {
                            dep.clone()
                        } else {
                            format!("{}:{}", package.name, dep)
                        }
                    })
                    .collect();

                merged.tasks.insert(prefixed_name, adjusted_task);
            }
        }
    }

    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_workspace_discovery() {
        let dir = tempdir().unwrap();

        // Create package directories
        fs::create_dir_all(dir.path().join("packages/api")).unwrap();
        fs::create_dir_all(dir.path().join("packages/web")).unwrap();

        // Create runx.toml in api package
        fs::write(
            dir.path().join("packages/api/runx.toml"),
            r#"
[project]
name = "api"

[tasks.build]
cmd = "cargo build"
"#,
        ).unwrap();

        let workspace = Workspace::discover(
            dir.path(),
            &["packages/*".to_string()],
        ).unwrap();

        assert_eq!(workspace.packages.len(), 2);

        let package_names: Vec<&str> = workspace.package_names();
        assert!(package_names.contains(&"api"));
        assert!(package_names.contains(&"web"));

        // Check tasks
        let all_tasks = workspace.get_all_tasks();
        assert!(all_tasks.contains_key("api:build"));
    }
}
