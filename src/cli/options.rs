use crate::application::runtime_settings::{resolve_notify_interval, resolve_terminal_notifier_path};
use crate::domain::errors::AppError;
use crate::domain::models::AppConfig;
use crate::storage::ShortcutMessage;
use crate::storage::SqliteDb;
use clap::Parser;
use std::env;
use std::path::PathBuf;

const APP_DIR_NAME: &str = "Key Finder";
const DATABASE_FILE_NAME: &str = "library.db";

#[derive(Debug, Parser)]
#[command(
    name = "key-finder",
    version,
    about = "A friendly reminder to use keyboard shortcuts"
)]
pub struct Cli {
    #[arg(long)]
    pub terminal_notifier_path: Option<String>,

    #[arg(long)]
    pub notify_interval: Option<String>,

    #[arg(long)]
    pub database_path: Option<PathBuf>,
}

impl Cli {
    pub fn into_runtime_inputs(self) -> Result<(AppConfig, Vec<ShortcutMessage>), AppError> {
        let database_path = resolve_database_path(self.database_path)?;
        let db = SqliteDb::open(&database_path)?;
        let settings_repo = db.settings_repository();
        let notification_snapshot_repo = db.notification_snapshot_repository();
        let db_settings = settings_repo.load_app_settings()?;
        let env_terminal_notifier_path = env::var("TERMINAL_NOTIFIER_PATH").ok();
        let env_notify_interval = env::var("NOTIFY_INTERVAL").ok();

        let terminal_notifier_path = resolve_terminal_notifier_path(
            self.terminal_notifier_path.as_deref(),
            env_terminal_notifier_path.as_deref(),
            db_settings.terminal_notifier_path.as_deref(),
        );
        let interval = resolve_notify_interval(
            self.notify_interval.as_deref(),
            env_notify_interval.as_deref(),
            db_settings.notify_interval.as_deref(),
        )?;

        let shortcuts = notification_snapshot_repo.load_notification_shortcuts()?;
        let is_bundled = detect_bundled_app();

        Ok((
            AppConfig {
                is_bundled,
                terminal_notifier_path,
                interval,
                database_path,
            },
            shortcuts,
        ))
    }
}

fn resolve_database_path(cli_database_path: Option<PathBuf>) -> Result<PathBuf, AppError> {
    cli_database_path
        .or_else(|| env::var("DATABASE_PATH").ok().filter(|s| !s.trim().is_empty()).map(PathBuf::from))
        .map_or_else(|| Ok(get_app_support_dir()?.join(DATABASE_FILE_NAME)), Ok)
}

fn get_app_support_dir() -> Result<PathBuf, AppError> {
    dirs::config_dir()
        .map(|dir| dir.join(APP_DIR_NAME))
        .ok_or_else(|| AppError::Config("failed to determine user config directory".to_string()))
}

fn detect_bundled_app() -> bool {
    #[cfg(target_os = "macos")]
    {
        if let Ok(exe) = env::current_exe() {
            let exe_str = exe.to_string_lossy();
            return exe_str.contains(".app/Contents/MacOS/");
        }
    }
    false
}
