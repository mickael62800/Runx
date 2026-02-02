//! GitHub status checks

use anyhow::Result;
use serde_json::json;
use std::env;

use crate::config::GithubConfig;
use crate::db::Run;

/// Send a GitHub status check
pub async fn send_github_status(
    config: &GithubConfig,
    run: &Run,
    project_name: &str,
) -> Result<()> {
    // Get token from config or environment
    let token = config.token.clone()
        .or_else(|| env::var("GITHUB_TOKEN").ok())
        .ok_or_else(|| anyhow::anyhow!("GitHub token not found"))?;

    // Get repository info from environment (set by GitHub Actions)
    let repo = env::var("GITHUB_REPOSITORY")
        .ok()
        .ok_or_else(|| anyhow::anyhow!("GITHUB_REPOSITORY not set"))?;

    // Get commit SHA
    let sha = env::var("GITHUB_SHA")
        .or_else(|_| env::var("GIT_COMMIT"))
        .ok()
        .ok_or_else(|| anyhow::anyhow!("Commit SHA not found"))?;

    // Determine state
    let state = if run.status == "passed" { "success" } else { "failure" };

    // Build description
    let description = format!(
        "{}/{} tasks passed ({}s)",
        run.passed,
        run.total_tasks,
        run.finished_at
            .map(|f| (f - run.started_at).num_seconds())
            .unwrap_or(0)
    );

    // Build status payload
    let payload = json!({
        "state": state,
        "description": description,
        "context": format!("runx/{}", project_name),
    });

    // Send to GitHub API
    let url = format!(
        "https://api.github.com/repos/{}/statuses/{}",
        repo, sha
    );

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("token {}", token))
        .header("User-Agent", "runx")
        .header("Accept", "application/vnd.github.v3+json")
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("GitHub status check failed: {} - {}", status, text);
    }

    Ok(())
}

/// Create a check run (more detailed than status)
pub async fn create_check_run(
    config: &GithubConfig,
    run: &Run,
    project_name: &str,
    failed_tasks: &[String],
) -> Result<()> {
    let token = config.token.clone()
        .or_else(|| env::var("GITHUB_TOKEN").ok())
        .ok_or_else(|| anyhow::anyhow!("GitHub token not found"))?;

    let repo = env::var("GITHUB_REPOSITORY")
        .ok()
        .ok_or_else(|| anyhow::anyhow!("GITHUB_REPOSITORY not set"))?;

    let sha = env::var("GITHUB_SHA")
        .or_else(|_| env::var("GIT_COMMIT"))
        .ok()
        .ok_or_else(|| anyhow::anyhow!("Commit SHA not found"))?;

    let conclusion = if run.status == "passed" { "success" } else { "failure" };

    // Build output
    let mut summary = format!(
        "## Results\n\n✅ **{}** passed | ❌ **{}** failed | Total: **{}**\n",
        run.passed, run.failed, run.total_tasks
    );

    if !failed_tasks.is_empty() {
        summary.push_str("\n### Failed Tasks\n\n");
        for task in failed_tasks.iter().take(20) {
            summary.push_str(&format!("- {}\n", task));
        }
        if failed_tasks.len() > 20 {
            summary.push_str(&format!("\n_...and {} more_\n", failed_tasks.len() - 20));
        }
    }

    let payload = json!({
        "name": format!("runx/{}", project_name),
        "head_sha": sha,
        "status": "completed",
        "conclusion": conclusion,
        "output": {
            "title": format!("Runx - {}", if run.status == "passed" { "All tests passed" } else { "Tests failed" }),
            "summary": summary
        }
    });

    let url = format!(
        "https://api.github.com/repos/{}/check-runs",
        repo
    );

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("token {}", token))
        .header("User-Agent", "runx")
        .header("Accept", "application/vnd.github.v3+json")
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("GitHub check run failed: {} - {}", status, text);
    }

    Ok(())
}
