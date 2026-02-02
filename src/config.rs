use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub project: Project,
    #[serde(default)]
    pub tasks: HashMap<String, Task>,
}

#[derive(Debug, Deserialize)]
pub struct Project {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Task {
    pub cmd: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub watch: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Run this task in background (for servers, etc.)
    #[serde(default)]
    pub background: bool,
    /// Wait for this string in stdout before considering the task ready
    #[serde(default)]
    pub ready_when: Option<String>,
    /// Timeout in seconds for ready_when (default: 30)
    #[serde(default = "default_ready_timeout")]
    pub ready_timeout: u64,
    /// Task category: unit, integration, e2e, build, lint, etc.
    #[serde(default)]
    pub category: Option<String>,
}

fn default_ready_timeout() -> u64 {
    30
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| "Failed to parse runx.toml")?;

        config.validate()?;

        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        // Validate that all depends_on references exist
        for (name, task) in &self.tasks {
            for dep in &task.depends_on {
                if !self.tasks.contains_key(dep) {
                    anyhow::bail!(
                        "Task '{}' depends on '{}' which does not exist",
                        name,
                        dep
                    );
                }
            }
        }

        Ok(())
    }

    pub fn get_task(&self, name: &str) -> Option<&Task> {
        self.tasks.get(name)
    }

    pub fn task_names(&self) -> Vec<&String> {
        self.tasks.keys().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let toml_content = r#"
[project]
name = "test-project"

[tasks.build]
cmd = "cargo build"
cwd = "core"
watch = ["src/**/*.rs"]

[tasks.test]
cmd = "cargo test"
depends_on = ["build"]
"#;

        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.project.name, "test-project");
        assert_eq!(config.tasks.len(), 2);
        assert!(config.tasks.contains_key("build"));
        assert!(config.tasks.contains_key("test"));
    }
}
