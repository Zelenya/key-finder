use crate::domain::errors::AppError;
use crate::storage::models::AppSettings;
use crate::storage::sqlite::sqlite_db::now_unix;
use crate::storage::sqlite::SqliteDb;
use rusqlite::{params, Connection};

#[derive(Clone, Debug)]
pub(crate) struct SqliteSettingsRepository {
    db: SqliteDb,
}

impl SqliteSettingsRepository {
    pub(crate) fn new(db: SqliteDb) -> Self {
        Self { db }
    }

    pub(crate) fn load_app_settings(&self) -> Result<AppSettings, AppError> {
        self.db.with_connection("load app settings", |conn| {
            let mut settings = AppSettings::default();

            let mut stmt =
                conn.prepare("select key, value from settings").map_err(|source| AppError::Database {
                    operation: "prepare settings query".to_string(),
                    source,
                })?;

            let rows = stmt
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
                .map_err(|source| AppError::Database {
                    operation: "run settings query".to_string(),
                    source,
                })?;

            for row in rows {
                let (key, value) = row.map_err(|source| AppError::Database {
                    operation: "collect settings row".to_string(),
                    source,
                })?;

                let trimmed_value = Some(value.trim())
                    .filter(|v| !v.is_empty())
                    .ok_or(AppError::Config("stored empty value for {key}".to_string()))?;

                if let Some(setting_key) = SettingKey::parse(&key) {
                    setting_key.write(&mut settings, trimmed_value.to_string());
                }
            }
            Ok(settings)
        })
    }

    pub(crate) fn save_app_settings(&self, settings: &AppSettings) -> Result<(), AppError> {
        self.db.with_transaction("save app settings", |tx| {
            for key in SettingKey::ALL {
                upsert_app_setting(tx, key, key.read(settings))?;
            }
            Ok(())
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum SettingKey {
    NotifyInterval,
    TerminalNotifierPath,
}

impl SettingKey {
    const ALL: [Self; 2] = [Self::NotifyInterval, Self::TerminalNotifierPath];

    fn as_str(self) -> &'static str {
        match self {
            Self::NotifyInterval => "notify_interval",
            Self::TerminalNotifierPath => "terminal_notifier_path",
        }
    }

    fn parse(key: &str) -> Option<Self> {
        match key {
            "notify_interval" => Some(Self::NotifyInterval),
            "terminal_notifier_path" => Some(Self::TerminalNotifierPath),
            _ => None,
        }
    }

    fn read(self, settings: &AppSettings) -> Option<&str> {
        match self {
            Self::NotifyInterval => settings.notify_interval.as_deref(),
            Self::TerminalNotifierPath => settings.terminal_notifier_path.as_deref(),
        }
    }

    fn write(self, settings: &mut AppSettings, value: String) {
        match self {
            Self::NotifyInterval => settings.notify_interval = Some(value),
            Self::TerminalNotifierPath => settings.terminal_notifier_path = Some(value),
        }
    }
}

fn upsert_app_setting(conn: &Connection, key: SettingKey, value: Option<&str>) -> Result<(), AppError> {
    match value.map(|v| v.trim()).filter(|v| !v.is_empty()) {
        Some(value) => conn
            .execute(
                "insert into settings(key, value, updated_at)
                 values (?1, ?2, ?3)
                 on conflict(key) do update set
                    value = excluded.value,
                    updated_at = excluded.updated_at",
                params![key.as_str(), value, now_unix()],
            )
            .map(|_| ())
            .map_err(|source| AppError::Database {
                operation: format!("upsert setting '{}'", key.as_str()),
                source,
            }),
        None => conn
            .execute("delete from settings where key = ?1", params![key.as_str()])
            .map(|_| ())
            .map_err(|source| AppError::Database {
                operation: format!("delete setting '{}'", key.as_str()),
                source,
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::{SqliteDb, SqliteSettingsRepository};
    use tempfile::tempdir;

    #[test]
    fn save_and_load_runtime_settings() {
        let dir = tempdir().expect("temp dir");
        let db_path = dir.path().join("library.db");
        let db = SqliteDb::open(&db_path).expect("db");
        let settings_repo = SqliteSettingsRepository::new(db);

        settings_repo
            .save_app_settings(&crate::storage::AppSettings {
                notify_interval: Some("30m".to_string()),
                terminal_notifier_path: Some("/opt/homebrew/bin/terminal-notifier".to_string()),
            })
            .expect("save settings");

        let settings = settings_repo.load_app_settings().expect("load settings");
        assert_eq!(settings.notify_interval.as_deref(), Some("30m"));
        assert_eq!(
            settings.terminal_notifier_path.as_deref(),
            Some("/opt/homebrew/bin/terminal-notifier")
        );
    }
}
