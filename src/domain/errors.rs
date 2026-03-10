use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(String),

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

    #[error("native notification backend failed: {message}")]
    NativeNotificationFailed { message: String },

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

    #[error("failed to initialize tray icon: {message}")]
    TrayInit { message: String },

    #[error("failed to configure tray menu: {message}")]
    TrayMenu { message: String },

    #[error("notification worker thread panicked")]
    WorkerPanic,

    #[error("tray runtime must be initialized on the macOS main thread")]
    MainThreadRequired,
}
