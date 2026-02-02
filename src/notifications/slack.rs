//! Slack webhook notifications

use anyhow::Result;
use serde_json::json;

use crate::config::SlackConfig;
use crate::db::Run;

use super::build_summary;

/// Send a notification to Slack
pub async fn send_slack_notification(
    config: &SlackConfig,
    run: &Run,
    project_name: &str,
) -> Result<()> {
    let summary = build_summary(run, project_name);

    // Build Slack Block Kit message
    let blocks = json!({
        "blocks": [
            {
                "type": "header",
                "text": {
                    "type": "plain_text",
                    "text": format!("{} {} - {}", summary.status_emoji, summary.project_name, summary.status_text),
                    "emoji": true
                }
            },
            {
                "type": "section",
                "fields": [
                    {
                        "type": "mrkdwn",
                        "text": format!("*Passed:*\n{}", summary.passed)
                    },
                    {
                        "type": "mrkdwn",
                        "text": format!("*Failed:*\n{}", summary.failed)
                    },
                    {
                        "type": "mrkdwn",
                        "text": format!("*Total:*\n{}", summary.total)
                    },
                    {
                        "type": "mrkdwn",
                        "text": format!("*Duration:*\n{}s", summary.duration_seconds)
                    }
                ]
            },
            {
                "type": "context",
                "elements": [
                    {
                        "type": "mrkdwn",
                        "text": format!("Run ID: `{}`", summary.run_id)
                    }
                ]
            }
        ]
    });

    // Add channel if specified
    let payload = if let Some(ref channel) = config.channel {
        let mut p = blocks;
        p["channel"] = json!(channel);
        p
    } else {
        blocks
    };

    // Send to Slack webhook
    let client = reqwest::Client::new();
    let response = client
        .post(&config.webhook_url)
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("Slack webhook failed: {} - {}", status, text);
    }

    Ok(())
}

/// Build a rich Slack message for failures
pub fn build_failure_message(
    run: &Run,
    project_name: &str,
    failed_tasks: &[String],
) -> serde_json::Value {
    let summary = build_summary(run, project_name);

    let failed_list = failed_tasks
        .iter()
        .take(10) // Limit to 10 items
        .map(|t| format!("• {}", t))
        .collect::<Vec<_>>()
        .join("\n");

    let more_text = if failed_tasks.len() > 10 {
        format!("\n_...and {} more_", failed_tasks.len() - 10)
    } else {
        String::new()
    };

    json!({
        "blocks": [
            {
                "type": "header",
                "text": {
                    "type": "plain_text",
                    "text": format!("❌ {} - BUILD FAILED", project_name),
                    "emoji": true
                }
            },
            {
                "type": "section",
                "fields": [
                    {
                        "type": "mrkdwn",
                        "text": format!("*Passed:* {}", summary.passed)
                    },
                    {
                        "type": "mrkdwn",
                        "text": format!("*Failed:* {}", summary.failed)
                    }
                ]
            },
            {
                "type": "section",
                "text": {
                    "type": "mrkdwn",
                    "text": format!("*Failed tasks:*\n{}{}", failed_list, more_text)
                }
            },
            {
                "type": "divider"
            },
            {
                "type": "context",
                "elements": [
                    {
                        "type": "mrkdwn",
                        "text": format!("Run ID: `{}` | Duration: {}s", summary.run_id, summary.duration_seconds)
                    }
                ]
            }
        ]
    })
}
