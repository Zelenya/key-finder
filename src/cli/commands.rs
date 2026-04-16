use crate::domain::errors::AppError;
use crate::domain::models::AppConfig;
use crate::runtime;
use crate::storage::SqliteDb;

pub fn run(config: AppConfig) -> Result<(), AppError> {
    let db = SqliteDb::open(&config.database_path)?;
    let initial_snapshot = db.notification_snapshot_repository().load_notification_snapshot()?;
    runtime::run(config, initial_snapshot)
}
