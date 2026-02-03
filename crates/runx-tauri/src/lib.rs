//! Tauri command logger for Runx Debug Dashboard
//!
//! This crate provides utilities to log Tauri commands to the Runx debug dashboard,
//! enabling real-time visualization of Frontend ↔ Backend communication.
//!
//! # Usage
//!
//! ```rust,ignore
//! use runx_tauri::{log_command_start, log_command_end};
//! use serde_json::json;
//!
//! #[tauri::command]
//! pub async fn get_user(user_id: String) -> Result<String, String> {
//!     log_command_start("user.getUser", &json!({ "user_id": &user_id }));
//!
//!     let result = // ... your code ...
//!
//!     log_command_end("user.getUser", &result);
//!     result
//! }
//! ```
//!
//! Or use the macro for a more concise syntax:
//!
//! ```rust,ignore
//! use runx_tauri::runx_log;
//!
//! runx_log!("user.getUser", start => &json!({ "user_id": &user_id }));
//! // ... your code ...
//! runx_log!("user.getUser", end => &result);
//! ```

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(true);
const DEFAULT_ENDPOINT: &str = "http://localhost:3000/api/debug";

/// Disable logging (useful for tests)
pub fn disable() {
    ENABLED.store(false, Ordering::SeqCst);
}

/// Enable logging
pub fn enable() {
    ENABLED.store(true, Ordering::SeqCst);
}

/// Check if logging is enabled
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

/// Log the start of a Tauri command
///
/// # Arguments
/// * `name` - Command name (e.g., "doodle.getDoodleVotes")
/// * `args` - Command arguments (will be serialized to JSON)
///
/// # Example
/// ```rust,ignore
/// log_command_start("user.getUser", &json!({ "user_id": "123" }));
/// ```
pub fn log_command_start(name: &str, args: &impl Serialize) {
    log_event(name, "command_received", args);
}

/// Log the end of a Tauri command (success or error)
///
/// # Arguments
/// * `name` - Command name (must match the start log)
/// * `result` - The Result from the command
///
/// # Example
/// ```rust,ignore
/// let result = service.get_user(&user_id).await;
/// log_command_end("user.getUser", &result);
/// ```
pub fn log_command_end<T: Serialize, E: std::fmt::Display>(name: &str, result: &Result<T, E>) {
    match result {
        Ok(val) => log_event(name, "command_success", val),
        Err(e) => log_event(
            name,
            "command_error",
            &serde_json::json!({ "error": e.to_string() }),
        ),
    }
}

/// Log a custom event to Runx
///
/// # Arguments
/// * `name` - Event name
/// * `event_type` - Type of event (e.g., "command_received", "command_success", "custom")
/// * `payload` - Event payload (will be serialized to JSON)
pub fn log_event(name: &str, event_type: &str, payload: &impl Serialize) {
    if !ENABLED.load(Ordering::SeqCst) {
        return;
    }

    let name = name.to_string();
    let event_type = event_type.to_string();
    let payload = serde_json::to_value(payload).unwrap_or(serde_json::Value::Null);
    let timestamp = chrono::Utc::now().to_rfc3339();

    // Fire and forget - don't block the command
    std::thread::spawn(move || {
        let body = serde_json::json!({
            "source": "tauri",
            "name": name,
            "event_type": event_type,
            "payload": payload,
            "timestamp": timestamp
        });

        // Silently ignore errors (Runx not running = no problem)
        let _ = ureq::post(DEFAULT_ENDPOINT)
            .set("Content-Type", "application/json")
            .send_json(&body);
    });
}

/// Log with a custom endpoint
pub fn log_event_to(endpoint: &str, name: &str, event_type: &str, payload: &impl Serialize) {
    if !ENABLED.load(Ordering::SeqCst) {
        return;
    }

    let endpoint = endpoint.to_string();
    let name = name.to_string();
    let event_type = event_type.to_string();
    let payload = serde_json::to_value(payload).unwrap_or(serde_json::Value::Null);
    let timestamp = chrono::Utc::now().to_rfc3339();

    std::thread::spawn(move || {
        let body = serde_json::json!({
            "source": "tauri",
            "name": name,
            "event_type": event_type,
            "payload": payload,
            "timestamp": timestamp
        });

        let _ = ureq::post(&endpoint)
            .set("Content-Type", "application/json")
            .send_json(&body);
    });
}

/// Macro for convenient logging
///
/// # Examples
///
/// Log command start:
/// ```rust,ignore
/// runx_log!("user.getUser", start => &json!({ "user_id": &user_id }));
/// ```
///
/// Log command end:
/// ```rust,ignore
/// runx_log!("user.getUser", end => &result);
/// ```
///
/// Log custom event:
/// ```rust,ignore
/// runx_log!("user.getUser", "processing", &json!({ "step": "validation" }));
/// ```
#[macro_export]
macro_rules! runx_log {
    ($name:expr, start => $args:expr) => {
        $crate::log_command_start($name, $args)
    };
    ($name:expr, end => $result:expr) => {
        $crate::log_command_end($name, $result)
    };
    ($name:expr, $event_type:expr, $payload:expr) => {
        $crate::log_event($name, $event_type, $payload)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enable_disable() {
        enable();
        assert!(is_enabled());

        disable();
        assert!(!is_enabled());

        enable();
        assert!(is_enabled());
    }

    #[test]
    fn test_log_event_when_disabled() {
        disable();
        // Should not panic even when disabled
        log_event("test", "test_event", &serde_json::json!({"foo": "bar"}));
        enable();
    }
}
