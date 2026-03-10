use crate::domain::errors::AppError;
use crate::storage::ImportShortcut;
use csv::{ReaderBuilder, StringRecord};
use std::path::Path;

const CUSTOM_CSV_HEADER: [&str; 2] = ["shortcut", "description"];

pub(crate) fn collect_file(path: &Path) -> Result<Vec<ImportShortcut>, AppError> {
    let content = read_custom_csv(path)?;
    let mut reader = ReaderBuilder::new().has_headers(false).flexible(false).from_reader(content.as_bytes());
    collect_rows(path, &mut reader)
}

fn read_custom_csv(path: &Path) -> Result<String, AppError> {
    let content = std::fs::read_to_string(path).map_err(|source| AppError::ReadImporterFile {
        path: path.to_path_buf(),
        source,
    })?;
    if content.trim().is_empty() {
        return Err(AppError::InvalidImporterSource {
            path: path.to_path_buf(),
            message: "expected rows in 'shortcut,description' format, with an optional header row"
                .to_string(),
        });
    }

    Ok(content)
}

fn collect_rows(path: &Path, reader: &mut csv::Reader<&[u8]>) -> Result<Vec<ImportShortcut>, AppError> {
    let mut shortcuts = Vec::new();

    for (row_idx, row) in reader.records().enumerate() {
        let row = row.map_err(|error| AppError::InvalidImporterSource {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
        if row.iter().all(|value| value.trim().is_empty()) {
            continue;
        }
        if row_idx == 0 && is_header_row(&row) {
            continue;
        }

        let shortcut = parse_row(path, &row, row_idx)?;
        shortcuts.push(shortcut);
    }

    Ok(shortcuts)
}

fn is_header_row(row: &StringRecord) -> bool {
    row.iter().map(normalize_header_cell).collect::<Vec<_>>().as_slice() == CUSTOM_CSV_HEADER
}

fn normalize_header_cell(value: &str) -> String {
    value.trim().trim_start_matches('\u{feff}').to_ascii_lowercase()
}

fn parse_row(path: &Path, row: &StringRecord, row_idx: usize) -> Result<ImportShortcut, AppError> {
    if row.len() != 2 {
        return Err(AppError::InvalidImporterSource {
            path: path.to_path_buf(),
            message: format!("row {row_idx} must contain exactly 2 columns: shortcut,description"),
        });
    }

    let shortcut_display = row[0].trim();
    let description = row[1].trim();
    if shortcut_display.is_empty() || description.is_empty() {
        return Err(AppError::InvalidImporterSource {
            path: path.to_path_buf(),
            message: format!("row {row_idx} must include non-empty shortcut and description"),
        });
    }

    Ok(ImportShortcut {
        shortcut_display: shortcut_display.to_string(),
        description: description.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::collect_file;
    use tempfile::tempdir;

    #[test]
    fn parses_valid_custom_csv() {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("shortcuts.csv");
        std::fs::write(&path, "shortcut,description\n\"cmd+k\",Open command palette\n").expect("write csv");

        let shortcuts = collect_file(&path).expect("parse csv");
        assert_eq!(shortcuts.len(), 1);
        assert_eq!(shortcuts[0].description, "Open command palette");
    }

    #[test]
    fn handles_duplicates_in_custom_csv() {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("shortcuts.csv");
        std::fs::write(&path, "shortcut,description\n\"cmd+k\",Foo\n\"cmd+k\",Foo\n").expect("write csv");

        let shortcuts = collect_file(&path).expect("parse csv");
        assert_eq!(shortcuts.len(), 2);
        assert_eq!(shortcuts[0].description, "Foo");
        assert_eq!(shortcuts[1].description, "Foo");
    }

    #[test]
    fn ignores_blank_rows_in_custom_csv() {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("shortcuts.csv");
        std::fs::write(&path, "shortcut,description\n\ncmd+k,Open command palette\n\n").expect("write csv");

        let shortcuts = collect_file(&path).expect("parse csv");
        assert_eq!(shortcuts.len(), 1);
    }

    #[test]
    fn rejects_missing_invalid_csv_value() {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("shortcuts.csv");
        std::fs::write(&path, "shortcut,description\ncmd+k,\n").expect("write csv");

        let error = collect_file(&path).expect_err("missing value");
        assert!(error.to_string().contains("non-empty shortcut and description"));
    }

    #[test]
    fn parses_headerless_custom_csv() {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("shortcuts.csv");
        std::fs::write(&path, "cmd+k,Open command palette\ncmd+shift+p,Open file\n").expect("write csv");

        let shortcuts = collect_file(&path).expect("parse headerless csv");
        assert_eq!(shortcuts.len(), 2);
        assert_eq!(shortcuts[0].shortcut_display, "cmd+k");
        assert_eq!(shortcuts[1].description, "Open file");
    }

    #[test]
    fn rejects_invalid_custom_csv_header() {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("shortcuts.csv");
        std::fs::write(&path, "keys,action\ncmd+k,Open command palette\n").expect("write csv");

        let shortcuts = collect_file(&path).expect("parse headerless csv");
        assert_eq!(shortcuts.len(), 2);
        assert_eq!(shortcuts[0].shortcut_display, "keys");
        assert_eq!(shortcuts[0].description, "action");
    }

    #[test]
    fn parses_fixture_csv() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/importers/ghostty-shortcuts.csv");

        let shortcuts = collect_file(&path).expect("parse ghostty fixture");
        assert_eq!(shortcuts.len(), 4);
        assert_eq!(shortcuts[0].shortcut_display, "cmd+shift+d");
        assert_eq!(shortcuts[0].description, "split pane down");
        assert_eq!(shortcuts[3].shortcut_display, "cmd+shift+w");
        assert_eq!(shortcuts[3].description, "close surface");
    }
}
