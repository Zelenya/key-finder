#[cfg(target_os = "macos")]
mod macos;
mod terminal;

use crate::domain::errors::AppError;
use crate::domain::models::AppConfig;
use crate::storage::NotificationSnapshot;

// If the app is bundled, run the tray UI. Otherwise, run the dev-mode terminal notifier.
pub(crate) fn run(config: AppConfig, initial_snapshot: NotificationSnapshot) -> Result<(), AppError> {
    if config.is_bundled {
        #[cfg(target_os = "macos")]
        {
            macos::ui::tray::run(config, initial_snapshot)
        }

        #[cfg(not(target_os = "macos"))]
        {
            Err(AppError::UnsupportedPlatform)
        }
    } else {
        terminal::run(config, initial_snapshot)
    }
}
