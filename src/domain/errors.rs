use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("invalid {setting} '{value}': {source}")]
    InvalidDurationSetting {
        setting: String,
        value: String,
        #[source]
        source: humantime::DurationError,
    },

    #[error(
        "unsupported platform: this app currently supports macOS only. \
to extend support, add a target-specific notifier backend in src/core/notifier.rs"
    )]
    UnsupportedPlatform,

    #[error("database error while {operation}: {source}")]
    Database {
        operation: String,
        #[source]
        source: rusqlite::Error,
    },

    #[error("database I/O error while {operation} at '{path}': {source}")]
    DatabaseIo {
        operation: String,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("storage operation failed: {0}")]
    StorageOperation(String),

    #[error("app name cannot be empty")]
    AppNameEmpty,

    #[error("app name must contain at least one letter or number")]
    AppNameInvalid,

    #[error("name '{name}' conflicts with existing app '{existing_app}'")]
    AppNameConflict { name: String, existing_app: String },

    #[error("shortcut cannot be empty")]
    ShortcutEmpty,

    #[error("description cannot be empty")]
    ShortcutDescriptionEmpty,

    #[error("ui operation failed: {0}")]
    UiOperation(String),

    #[error("no shortcut importer found for app '{app}'. supported importers: {supported}")]
    ImporterNotFound { app: String, supported: String },

    #[error("could not find source keymap files for importer '{importer}'. {hint}")]
    ImporterSourceNotFound { importer: String, hint: String },

    #[error("failed to read importer source file '{path}': {source}")]
    ReadImporterFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid importer source in '{path}': {message}")]
    InvalidImporterSource { path: PathBuf, message: String },

    #[cfg(target_os = "macos")]
    #[error("native notification backend failed: {0}")]
    NativeNotificationFailed(#[from] mac_notification_sys::error::Error),

    #[error(
        "terminal-notifier was not found. Install it (e.g. `brew install terminal-notifier`) \
or pass a custom path via `--terminal-notifier-path` / `TERMINAL_NOTIFIER_PATH`"
    )]
    TerminalNotifierNotFound,

    #[error("failed to execute notifier '{candidate}': {source}")]
    NotifierExecution {
        candidate: String,
        #[source]
        source: std::io::Error,
    },

    #[error("notifier '{candidate}' exited with an error: {stderr}")]
    NotifierFailure { candidate: String, stderr: String },

    #[error("both notification backends failed; primary: {primary}; fallback: {fallback}")]
    NotificationBackendsFailed { primary: String, fallback: String },

    #[cfg(target_os = "macos")]
    #[error("failed to build tray icon: {0}")]
    TrayBuild(#[from] tray_icon::Error),

    #[cfg(target_os = "macos")]
    #[error("failed to build tray icon bitmap: {0}")]
    TrayIconBitmap(#[from] tray_icon::BadIcon),

    #[cfg(target_os = "macos")]
    #[error("failed to configure tray menu: {0}")]
    TrayMenu(#[from] tray_icon::menu::Error),

    #[error("notification worker thread panicked")]
    WorkerPanic,

    #[error("tray runtime must be initialized on the macOS main thread")]
    MainThreadRequired,
}
