use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub project: Project,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
    #[serde(default)]
    pub workspaces: Option<WorkspaceConfig>,
    #[serde(default)]
    pub notifications: Option<NotificationsConfig>,
    #[serde(default)]
    pub cache: Option<CacheConfig>,
    #[serde(default)]
    pub ai: Option<AiConfigFile>,
    #[serde(default)]
    pub tasks: HashMap<String, Task>,
}

/// AI configuration for test annotations
#[derive(Debug, Deserialize, Clone, Default)]
pub struct AiConfigFile {
    /// AI provider: "anthropic" or "openai"
    #[serde(default = "default_ai_provider")]
    pub provider: String,
    /// API key (or env var like ${ANTHROPIC_API_KEY})
    #[serde(default)]
    pub api_key: Option<String>,
    /// Model name to use
    #[serde(default)]
    pub model: Option<String>,
    /// Enable automatic annotation after test runs
    #[serde(default)]
    pub auto_annotate: bool,
    /// Language for annotations (en, fr, es, de)
    #[serde(default = "default_ai_language")]
    pub language: String,
}

fn default_ai_provider() -> String {
    "anthropic".to_string()
}

fn default_ai_language() -> String {
    "en".to_string()
}

#[derive(Debug, Deserialize, Clone)]
pub struct Project {
    pub name: String,
    #[serde(default)]
    pub default_profile: Option<String>,
}

/// Profile configuration for different environments (dev, ci, etc.)
#[derive(Debug, Deserialize, Clone, Default)]
pub struct Profile {
    /// Enable parallel execution
    #[serde(default)]
    pub parallel: bool,
    /// Number of parallel workers
    #[serde(default)]
    pub workers: Option<usize>,
    /// Enable caching
    #[serde(default = "default_cache_enabled")]
    pub cache: bool,
    /// Verbose output
    #[serde(default)]
    pub verbose: bool,
    /// Enable notifications
    #[serde(default)]
    pub notifications: bool,
    /// Stop on first failure
    #[serde(default)]
    pub fail_fast: bool,
    /// Task-specific overrides
    #[serde(default)]
    pub task_overrides: HashMap<String, TaskOverride>,
}

fn default_cache_enabled() -> bool {
    true
}

/// Task overrides for profiles
#[derive(Debug, Deserialize, Clone, Default)]
pub struct TaskOverride {
    #[serde(default)]
    pub parallel: Option<bool>,
    #[serde(default)]
    pub workers: Option<usize>,
    #[serde(default)]
    pub retry: Option<u32>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

/// Workspace/monorepo configuration
#[derive(Debug, Deserialize, Clone)]
pub struct WorkspaceConfig {
    /// Glob patterns for package directories
    pub packages: Vec<String>,
}

/// Notifications configuration
#[derive(Debug, Deserialize, Clone)]
pub struct NotificationsConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Notify only on failure
    #[serde(default)]
    pub on_failure: bool,
    #[serde(default)]
    pub slack: Option<SlackConfig>,
    #[serde(default)]
    pub discord: Option<DiscordConfig>,
    #[serde(default)]
    pub github: Option<GithubConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SlackConfig {
    pub webhook_url: String,
    #[serde(default)]
    pub channel: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DiscordConfig {
    pub webhook_url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GithubConfig {
    #[serde(default)]
    pub enabled: bool,
    /// GitHub token (defaults to GITHUB_TOKEN env var)
    #[serde(default)]
    pub token: Option<String>,
}

/// Global cache configuration
#[derive(Debug, Deserialize, Clone)]
pub struct CacheConfig {
    #[serde(default = "default_cache_enabled")]
    pub enabled: bool,
    /// Time-to-live in hours (default: 24)
    #[serde(default = "default_cache_ttl")]
    pub ttl_hours: u32,
}

fn default_cache_ttl() -> u32 {
    24
}

/// Artifact configuration
#[derive(Debug, Deserialize, Clone)]
pub struct ArtifactConfig {
    /// Glob pattern for files
    pub pattern: String,
    /// Artifact type: screenshot, video, log, etc.
    #[serde(rename = "type")]
    pub artifact_type: String,
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
    /// Path to JUnit XML results file (for detailed test parsing)
    #[serde(default)]
    pub results: Option<String>,

    // v0.3.0 - New fields
    /// Can run in parallel with other tasks at the same dependency level
    #[serde(default)]
    pub parallel: bool,
    /// Number of workers for this task (if it supports parallelization)
    #[serde(default)]
    pub workers: Option<usize>,
    /// Number of retries on failure
    #[serde(default)]
    pub retry: u32,
    /// Delay between retries in milliseconds
    #[serde(default = "default_retry_delay")]
    pub retry_delay_ms: u64,
    /// Timeout in seconds for task execution
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    /// Enable coverage collection
    #[serde(default)]
    pub coverage: bool,
    /// Coverage format: lcov, cobertura
    #[serde(default)]
    pub coverage_format: Option<String>,
    /// Path to coverage file
    #[serde(default)]
    pub coverage_path: Option<String>,
    /// Coverage threshold percentage
    #[serde(default)]
    pub coverage_threshold: Option<f64>,
    /// Artifacts to collect
    #[serde(default)]
    pub artifacts: Vec<ArtifactConfig>,
    /// Environment variables for this task
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Input files for cache hash calculation (in addition to watch patterns)
    #[serde(default)]
    pub inputs: Vec<String>,
    /// Output files/directories produced by this task
    #[serde(default)]
    pub outputs: Vec<String>,
}

fn default_ready_timeout() -> u64 {
    30
}

fn default_retry_delay() -> u64 {
    1000
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        // Expand environment variables in the content
        let expanded = shellexpand::env(&content)
            .map(|s| s.into_owned())
            .unwrap_or_else(|_| content.clone());

        let config: Config = toml::from_str(&expanded)
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

        // Validate coverage format if specified
        for (name, task) in &self.tasks {
            if task.coverage {
                if let Some(ref format) = task.coverage_format {
                    if format != "lcov" && format != "cobertura" {
                        anyhow::bail!(
                            "Task '{}' has invalid coverage_format '{}'. Supported: lcov, cobertura",
                            name,
                            format
                        );
                    }
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

    /// Get a profile by name, or return the default profile
    pub fn get_profile(&self, name: Option<&str>) -> Profile {
        let profile_name = name.or(self.project.default_profile.as_deref());

        if let Some(pname) = profile_name {
            self.profiles.get(pname).cloned().unwrap_or_default()
        } else {
            Profile::default()
        }
    }

    /// Apply profile overrides to a task
    pub fn get_task_with_profile(&self, task_name: &str, profile: &Profile) -> Option<Task> {
        let task = self.tasks.get(task_name)?.clone();

        if let Some(overrides) = profile.task_overrides.get(task_name) {
            let mut task = task;
            if let Some(parallel) = overrides.parallel {
                task.parallel = parallel;
            }
            if let Some(workers) = overrides.workers {
                task.workers = Some(workers);
            }
            if let Some(retry) = overrides.retry {
                task.retry = retry;
            }
            if let Some(timeout) = overrides.timeout_seconds {
                task.timeout_seconds = Some(timeout);
            }
            Some(task)
        } else {
            Some(task)
        }
    }

    /// Get global cache TTL in hours
    pub fn get_cache_ttl(&self) -> u32 {
        self.cache.as_ref().map(|c| c.ttl_hours).unwrap_or(24)
    }

    /// Check if caching is enabled globally
    pub fn is_cache_enabled(&self) -> bool {
        self.cache.as_ref().map(|c| c.enabled).unwrap_or(true)
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
default_profile = "dev"

[profiles.dev]
parallel = false
cache = true
verbose = true

[profiles.ci]
parallel = true
workers = 4
cache = true
notifications = true
fail_fast = true

[cache]
enabled = true
ttl_hours = 24

[tasks.build]
cmd = "cargo build"
cwd = "core"
watch = ["src/**/*.rs"]

[tasks.test]
cmd = "cargo test"
depends_on = ["build"]
parallel = true
retry = 3
retry_delay_ms = 1000
timeout_seconds = 300
coverage = true
coverage_format = "lcov"
coverage_path = "coverage/lcov.info"
coverage_threshold = 80
"#;

        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.project.name, "test-project");
        assert_eq!(config.tasks.len(), 2);
        assert!(config.tasks.contains_key("build"));
        assert!(config.tasks.contains_key("test"));

        let test_task = config.tasks.get("test").unwrap();
        assert!(test_task.parallel);
        assert_eq!(test_task.retry, 3);
        assert!(test_task.coverage);
    }

    #[test]
    fn test_profiles() {
        let toml_content = r#"
[project]
name = "test"
default_profile = "dev"

[profiles.dev]
parallel = false
verbose = true

[profiles.ci]
parallel = true
workers = 4

[tasks.test]
cmd = "cargo test"
"#;

        let config: Config = toml::from_str(toml_content).unwrap();

        let dev_profile = config.get_profile(Some("dev"));
        assert!(!dev_profile.parallel);
        assert!(dev_profile.verbose);

        let ci_profile = config.get_profile(Some("ci"));
        assert!(ci_profile.parallel);
        assert_eq!(ci_profile.workers, Some(4));
    }
}
