use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KnownImporterFamily {
    JetBrains,
    VSCode,
    Zed,
}

impl KnownImporterFamily {
    fn from_storage_str(value: &str) -> Option<Self> {
        match value {
            "JetBrains" => Some(KnownImporterFamily::JetBrains),
            "VSCode" => Some(KnownImporterFamily::VSCode),
            "Zed" => Some(KnownImporterFamily::Zed),
            _ => None,
        }
    }

    pub(crate) fn display_name(self) -> &'static str {
        match self {
            KnownImporterFamily::JetBrains => "JetBrains",
            KnownImporterFamily::VSCode => "VS Code",
            KnownImporterFamily::Zed => "Zed",
        }
    }

    pub(crate) fn import_hint(self) -> &'static str {
        match self {
            KnownImporterFamily::JetBrains => {
                "Locate or export an IntelliJ IDEA keymap XML file, then import it in Shortcuts."
            }
            KnownImporterFamily::VSCode => {
                "In VS Code, either run 'Preferences: Open Default Keyboard Shortcuts (JSON)', save the file, and import it in Shortcuts, or use Installed extension shortcuts in the import dialog."
            }
            KnownImporterFamily::Zed => {
                "In Zed, run 'zed: open default keymap', save the JSON file, then import it in Shortcuts."
            }
        }
    }
}

impl ToSql for KnownImporterFamily {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        match self {
            KnownImporterFamily::JetBrains => "JetBrains",
            KnownImporterFamily::VSCode => "VSCode",
            KnownImporterFamily::Zed => "Zed",
        }
        .to_sql()
    }
}

impl FromSql for KnownImporterFamily {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Text(raw) => std::str::from_utf8(raw)
                .ok()
                .and_then(Self::from_storage_str)
                .ok_or(FromSqlError::InvalidType),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::KnownImporterFamily;

    #[test]
    fn parses_storage_values() {
        assert_eq!(
            KnownImporterFamily::from_storage_str("JetBrains"),
            Some(KnownImporterFamily::JetBrains)
        );
        assert_eq!(
            KnownImporterFamily::from_storage_str("VSCode"),
            Some(KnownImporterFamily::VSCode)
        );
        assert_eq!(
            KnownImporterFamily::from_storage_str("Zed"),
            Some(KnownImporterFamily::Zed)
        );
        assert_eq!(KnownImporterFamily::from_storage_str("Code"), None);
    }

    #[test]
    fn lists_supported_importers() {
        let importer_names = [
            KnownImporterFamily::JetBrains,
            KnownImporterFamily::VSCode,
            KnownImporterFamily::Zed,
        ]
        .iter()
        .map(|family| family.display_name())
        .collect::<Vec<_>>();
        assert_eq!(importer_names, vec!["JetBrains", "VS Code", "Zed"]);
    }
}
