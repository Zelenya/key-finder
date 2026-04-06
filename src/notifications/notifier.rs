use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Command,
};

use crate::domain::{errors::AppError, models::NotificationContent};

pub(crate) trait Notifier: Send + Sync {
    fn notify(&self, content: &NotificationContent) -> Result<(), AppError>;
}
pub(crate) struct NativeNotifier {}

impl NativeNotifier {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl Default for NativeNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Notifier for NativeNotifier {
    fn notify(&self, content: &NotificationContent) -> Result<(), AppError> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = content;
            return Err(AppError::UnsupportedPlatform);
        }

        #[cfg(target_os = "macos")]
        {
            mac_notification_sys::send_notification(
                &content.title,
                content.subtitle.as_deref(),
                &content.message,
                None,
            )
            .map(|_| ())
            .map_err(|e| AppError::NativeNotificationFailed {
                message: e.to_string(),
            })
        }
    }
}

pub(crate) struct TerminalNotifier {
    terminal_notifier_path: Option<String>,
}

impl TerminalNotifier {
    pub(crate) fn new(terminal_notifier_path: Option<String>) -> Self {
        Self {
            terminal_notifier_path,
        }
    }
}

impl Notifier for TerminalNotifier {
    fn notify(&self, content: &NotificationContent) -> Result<(), AppError> {
        let custom_path = self.terminal_notifier_path.as_deref();
        let candidate = resolve_terminal_notifier(custom_path)?;
        let mut cmd = Command::new(&candidate);
        cmd.args(["-title", &content.title, "-message", &content.message]);
        if let Some(subtitle) = &content.subtitle {
            cmd.args(["-subtitle", subtitle]);
        }

        match cmd.output() {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(AppError::NotifierFailure {
                    candidate,
                    stderr: stderr.trim().to_string(),
                })
            }
            Err(source) if source.kind() == ErrorKind::NotFound => Err(AppError::TerminalNotifierNotFound),
            Err(source) => Err(AppError::NotifierExecution { candidate, source }),
        }
    }
}

fn resolve_terminal_notifier(custom_path: Option<&str>) -> Result<String, AppError> {
    if let Some(path) = custom_path {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err(AppError::TerminalNotifierNotFound);
        }
        if !Path::new(trimmed).exists() {
            return Err(AppError::TerminalNotifierNotFound);
        }
        return Ok(trimmed.to_string());
    }

    find_terminal_notifier_binary().ok_or(AppError::TerminalNotifierNotFound)
}

fn find_terminal_notifier_binary() -> Option<String> {
    const CANDIDATE_PATHS: [&str; 4] = [
        "/opt/homebrew/bin/terminal-notifier",
        "/usr/local/bin/terminal-notifier",
        "/opt/local/bin/terminal-notifier",
        "/usr/bin/terminal-notifier",
    ];

    if let Some(path) = std::env::var_os("PATH").and_then(|raw| {
        std::env::split_paths(&raw)
            .map(|dir| dir.join("terminal-notifier"))
            .find(|candidate| candidate.exists())
    }) {
        return Some(path.to_string_lossy().to_string());
    }

    CANDIDATE_PATHS
        .iter()
        .map(PathBuf::from)
        .find(|path| path.exists())
        .map(|path| path.to_string_lossy().to_string())
}
