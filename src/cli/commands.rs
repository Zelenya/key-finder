use crate::domain::errors::AppError;
use crate::domain::models::AppConfig;
use crate::runtime;
use crate::storage::ShortcutMessage;

pub fn run(config: AppConfig, initial_shortcuts: Vec<ShortcutMessage>) -> Result<(), AppError> {
    runtime::run(config, initial_shortcuts)
}
