//! Notifications module
//!
//! Provides:
//! - Slack webhook notifications
//! - Discord webhook notifications
//! - GitHub status checks

mod discord;
mod github;
mod slack;

pub use discord::*;
pub use github::*;
pub use slack::*;

use crate::config::NotificationsConfig;
use crate::db::Run;

/// Send notifications based on configuration
pub async fn send_notifications(
    config: &NotificationsConfig,
    run: &Run,
    project_name: &str,
) -> anyhow::Result<()> {
    if !config.enabled {
        return Ok(());
    }

    // Skip if on_failure is true and the run passed
    if config.on_failure && run.status == "passed" {
        return Ok(());
    }

    let mut errors = Vec::new();

    // Send to Slack
    if let Some(ref slack_config) = config.slack {
        if let Err(e) = send_slack_notification(slack_config, run, project_name).await {
            errors.push(format!("Slack: {}", e));
        }
    }

    // Send to Discord
    if let Some(ref discord_config) = config.discord {
        if let Err(e) = send_discord_notification(discord_config, run, project_name).await {
            errors.push(format!("Discord: {}", e));
        }
    }

    // Send to GitHub
    if let Some(ref github_config) = config.github {
        if github_config.enabled {
            if let Err(e) = send_github_status(github_config, run, project_name).await {
                errors.push(format!("GitHub: {}", e));
            }
        }
    }

    if !errors.is_empty() {
        anyhow::bail!("Notification errors: {}", errors.join(", "));
    }

    Ok(())
}

/// Build a summary message for notifications
pub fn build_summary(run: &Run, project_name: &str) -> NotificationSummary {
    let status_emoji = if run.status == "passed" { "✅" } else { "❌" };
    let status_text = if run.status == "passed" { "PASSED" } else { "FAILED" };

    let duration = run.finished_at
        .map(|f| (f - run.started_at).num_seconds())
        .unwrap_or(0);

    NotificationSummary {
        project_name: project_name.to_string(),
        run_id: run.id.clone(),
        status: run.status.clone(),
        status_emoji: status_emoji.to_string(),
        status_text: status_text.to_string(),
        passed: run.passed,
        failed: run.failed,
        total: run.total_tasks,
        duration_seconds: duration,
    }
}

#[derive(Debug, Clone)]
pub struct NotificationSummary {
    pub project_name: String,
    pub run_id: String,
    pub status: String,
    pub status_emoji: String,
    pub status_text: String,
    pub passed: i32,
    pub failed: i32,
    pub total: i32,
    pub duration_seconds: i64,
}
