use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug)]
pub(crate) struct NotificationContent {
    pub title: String,
    pub subtitle: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub is_bundled: bool,
    // TODO: Should this be taken out, non-bundled specific?
    pub terminal_notifier_path: Option<String>,
    pub interval: Duration,
    pub database_path: PathBuf,
}
