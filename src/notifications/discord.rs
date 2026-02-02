//! Discord webhook notifications

use anyhow::Result;
use serde_json::json;

use crate::config::DiscordConfig;
use crate::db::Run;

use super::build_summary;

/// Send a notification to Discord
pub async fn send_discord_notification(
    config: &DiscordConfig,
    run: &Run,
    project_name: &str,
) -> Result<()> {
    let summary = build_summary(run, project_name);

    // Discord embed color: green for pass, red for fail
    let color = if run.status == "passed" { 0x26a69a } else { 0xef5350 };

    // Build Discord embed message
    let payload = json!({
        "embeds": [
            {
                "title": format!("{} {} - {}", summary.status_emoji, summary.project_name, summary.status_text),
                "color": color,
                "fields": [
                    {
                        "name": "Passed",
                        "value": summary.passed.to_string(),
                        "inline": true
                    },
                    {
                        "name": "Failed",
                        "value": summary.failed.to_string(),
                        "inline": true
                    },
                    {
                        "name": "Total",
                        "value": summary.total.to_string(),
                        "inline": true
                    },
                    {
                        "name": "Duration",
                        "value": format!("{}s", summary.duration_seconds),
                        "inline": true
                    }
                ],
                "footer": {
                    "text": format!("Run ID: {}", summary.run_id)
                },
                "timestamp": chrono::Utc::now().to_rfc3339()
            }
        ]
    });

    // Send to Discord webhook
    let client = reqwest::Client::new();
    let response = client
        .post(&config.webhook_url)
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("Discord webhook failed: {} - {}", status, text);
    }

    Ok(())
}

/// Build a rich Discord message for failures
pub fn build_failure_embed(
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
        format!("\n*...and {} more*", failed_tasks.len() - 10)
    } else {
        String::new()
    };

    json!({
        "embeds": [
            {
                "title": format!("❌ {} - BUILD FAILED", project_name),
                "color": 0xef5350,
                "fields": [
                    {
                        "name": "Results",
                        "value": format!("✅ {} passed | ❌ {} failed", summary.passed, summary.failed),
                        "inline": false
                    },
                    {
                        "name": "Failed Tasks",
                        "value": format!("{}{}", failed_list, more_text),
                        "inline": false
                    },
                    {
                        "name": "Duration",
                        "value": format!("{}s", summary.duration_seconds),
                        "inline": true
                    }
                ],
                "footer": {
                    "text": format!("Run ID: {}", summary.run_id)
                },
                "timestamp": chrono::Utc::now().to_rfc3339()
            }
        ]
    })
}
