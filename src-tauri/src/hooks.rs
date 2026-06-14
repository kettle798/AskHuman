//! User-level AskHuman hooks: executable scripts under `~/.askhuman/hooks/<event>`
//! are invoked at specific lifecycle points.
//!
//! Each event maps to a script with the same name, such as `ask-received`.
//! Short fields are passed through environment variables and the full payload is
//! written to stdin as JSON. Hooks are fire-and-forget: a background thread waits
//! on the child process so the long-lived daemon does not leave zombies behind.
//! Scripts should return quickly and are intended for notifications, sounds, etc.
//!
//! Hooks only run on Unix platforms. Other platforms intentionally no-op.

use crate::models::AskRequest;
use crate::paths;
use std::path::PathBuf;

/// Hooks directory: `~/.askhuman/hooks`.
pub fn hooks_dir() -> PathBuf {
    paths::config_dir().join("hooks")
}

/// Invoke the executable script for an event if it exists. Non-blocking.
///
/// - `event`: event name, mapped to `hooks/<event>` and passed as `ASKHUMAN_EVENT`.
/// - `env`: additional environment variables with short fields.
/// - `stdin_json`: full payload written to the script's stdin.
pub fn fire(event: &str, env: Vec<(String, String)>, stdin_json: String) {
    #[cfg(unix)]
    {
        let script = hooks_dir().join(event);
        if !is_executable_file(&script) {
            return;
        }
        let event = event.to_string();
        std::thread::spawn(move || {
            run_hook(&script, &event, env, &stdin_json);
        });
    }
    #[cfg(not(unix))]
    {
        let _ = (event, env, stdin_json);
    }
}

/// Fire `ask-received` when a question request arrives, regardless of popup state.
pub fn fire_ask_received(request_id: &str, source: &str, project: &str, request: &AskRequest) {
    let env = vec![
        ("ASKHUMAN_REQUEST_ID".to_string(), request_id.to_string()),
        ("ASKHUMAN_SOURCE".to_string(), source.to_string()),
        ("ASKHUMAN_PROJECT".to_string(), project.to_string()),
        (
            "ASKHUMAN_QUESTION_COUNT".to_string(),
            request.questions.len().to_string(),
        ),
    ];
    let payload = serde_json::json!({
        "event": "ask-received",
        "requestId": request_id,
        "source": source,
        "project": project,
        "isMarkdown": request.is_markdown,
        "message": {
            "text": request.message.text,
            "files": request.message.files.iter().map(|f| serde_json::json!({
                "path": f.path,
                "name": f.name,
                "size": f.size,
                "isImage": f.is_image,
            })).collect::<Vec<_>>(),
        },
        "questions": request.questions.iter().map(|q| serde_json::json!({
            "message": q.message,
            "options": q.predefined_options.iter().map(|o| serde_json::json!({
                "text": o.text,
                "recommended": o.recommended,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    });
    let json = serde_json::to_string(&payload).unwrap_or_default();
    fire("ask-received", env, json);
}

/// Ensure the hooks directory exists and write a non-executable sample script.
/// Existing files are preserved.
pub fn ensure_sample() {
    let dir = hooks_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let sample = dir.join("ask-received.sample");
    if sample.exists() {
        return;
    }
    let _ = std::fs::write(&sample, SAMPLE_ASK_RECEIVED);
    // Keep the default non-executable permissions so the sample never fires.
}

#[cfg(unix)]
fn run_hook(script: &std::path::Path, event: &str, env: Vec<(String, String)>, stdin_json: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(script);
    cmd.env("ASKHUMAN_EVENT", event);
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => return,
    };
    if let Some(mut si) = child.stdin.take() {
        let _ = si.write_all(stdin_json.as_bytes());
        // Drop stdin so the script sees EOF.
    }
    let _ = child.wait();
}

/// Return true when the path is a regular executable file.
#[cfg(unix)]
fn is_executable_file(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(p) {
        Ok(m) => m.is_file() && (m.permissions().mode() & 0o111 != 0),
        Err(_) => false,
    }
}

/// Contents of the non-executable `ask-received.sample` reference script.
const SAMPLE_ASK_RECEIVED: &str = r#"#!/usr/bin/env bash
# AskHuman hook example — event: ask-received
#
# To enable: copy this file to "ask-received" (no extension) in the same folder
# and make it executable:
#
#   cp ask-received.sample ask-received
#   chmod +x ask-received
#
# AskHuman runs ~/.askhuman/hooks/<event> whenever the matching event fires.
# "ask-received" fires once per incoming question request, independent of whether
# a popup is shown (it also fires in headless / IM-only setups). The hook runs
# non-blocking; keep it quick (play a sound, send a notification, etc.).
#
# Parameters are provided two ways:
#   1) Environment variables (quick fields):
#        ASKHUMAN_EVENT           e.g. "ask-received"
#        ASKHUMAN_REQUEST_ID      unique id for this request
#        ASKHUMAN_SOURCE          source name (popup/Telegram title)
#        ASKHUMAN_PROJECT         project path (git root or cwd)
#        ASKHUMAN_QUESTION_COUNT  number of questions
#   2) Full JSON payload on stdin, e.g.:
#        { "event": "ask-received", "requestId": "...", "source": "...",
#          "project": "...", "isMarkdown": true,
#          "message": { "text": "...", "files": [ ... ] },
#          "questions": [ { "message": "...", "options": [ ... ] } ] }
#
# Read the JSON with jq if available:
#   payload="$(cat)"
#   text="$(printf '%s' "$payload" | jq -r '.message.text')"

# --- Example: desktop notification ---------------------------------------
# macOS:
#   osascript -e "display notification \"$ASKHUMAN_SOURCE\" with title \"AskHuman\""
# Linux:
#   notify-send "AskHuman" "$ASKHUMAN_SOURCE"

# --- Example: custom sound -----------------------------------------------
# macOS:
#   afplay /System/Library/Sounds/Glass.aiff
# Linux:
#   canberra-gtk-play -i message 2>/dev/null || \
#     paplay /usr/share/sounds/freedesktop/stereo/message.oga 2>/dev/null

exit 0
"#;
