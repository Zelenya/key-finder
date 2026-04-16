use crate::domain::app_norm::{app_matches_any, app_names_match};
use crate::domain::known_apps::KnownImporterFamily;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct AppId(i64);

impl From<i64> for AppId {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl From<AppId> for i64 {
    fn from(value: AppId) -> Self {
        value.0
    }
}

impl ToSql for AppId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl FromSql for AppId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Integer(raw) => Ok(Self(raw)),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ShortcutId(i64);

impl From<i64> for ShortcutId {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl From<ShortcutId> for i64 {
    fn from(value: ShortcutId) -> Self {
        value.0
    }
}

impl ToSql for ShortcutId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

impl FromSql for ShortcutId {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Integer(raw) => Ok(Self(raw)),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NotificationShortcut {
    pub(crate) app_id: AppId,
    pub(crate) shortcut: String,
    pub(crate) description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NotificationApp {
    pub(crate) app_id: AppId,
    pub(crate) name: String,
    pub(crate) aliases: Vec<String>,
}

impl NotificationApp {
    fn matches_given_name(&self, name: &str) -> bool {
        app_names_match(&self.name, name) || app_matches_any(&self.aliases, name)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct NotificationSnapshot {
    pub(crate) shortcuts: Vec<NotificationShortcut>,
    pub(crate) apps: Vec<NotificationApp>,
}

impl NotificationSnapshot {
    pub(crate) fn shortcuts_for_app(
        &self,
        app_id: AppId,
    ) -> impl Iterator<Item = &NotificationShortcut> + '_ {
        self.shortcuts.iter().filter(move |shortcut| shortcut.app_id == app_id)
    }

    pub(crate) fn resolve_guessed_app(&self, app_name: &str) -> Option<AppId> {
        self.apps.iter().find(|app| app.matches_given_name(app_name)).map(|app| app.app_id)
    }

    pub(crate) fn app_name(&self, app_id: AppId) -> &str {
        self.apps.iter().find(|app| app.app_id == app_id).map_or("Unknown App", |app| app.name.as_str())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AppSettings {
    pub notify_interval: Option<String>,
    pub terminal_notifier_path: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShortcutState {
    Active,
    Dismissed,
}

impl ShortcutState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ShortcutState::Active => "active",
            ShortcutState::Dismissed => "dismissed",
        }
    }

    pub(crate) fn from_db(value: &str) -> Self {
        match value {
            "dismissed" => ShortcutState::Dismissed,
            _ => ShortcutState::Active,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ManagedShortcut {
    pub id: ShortcutId,
    pub shortcut_display: String,
    pub description: String,
    pub state: ShortcutState,
}

#[derive(Clone, Debug)]
pub(crate) struct AppSummary {
    pub app_id: AppId,
    pub name: String,
    pub importer: Option<KnownImporterFamily>,
    pub total_count: i64,
    pub active_count: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct ImportShortcut {
    pub shortcut_display: String,
    pub description: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ImportMergeSummary {
    pub added: usize,
    pub unchanged: usize,
    pub deduped: usize,
    pub skipped: usize,
}
