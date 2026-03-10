use super::parsers::{custom_csv, idea, vscode, zed};
use crate::domain::errors::AppError;
use crate::domain::shortcut_norm::normalize_shortcut;
use crate::storage::ImportShortcut;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct CollectedImport {
    pub source_files: Vec<PathBuf>,
    pub shortcuts: Vec<ImportShortcut>,
    pub parsed_count: usize,
    pub deduped_count: usize,
}

pub(super) fn collect_custom_csv_file(path: &Path) -> Result<CollectedImport, AppError> {
    let shortcuts = custom_csv::collect_file(path)?;
    Ok(finalize_import_shortcuts(vec![path.to_path_buf()], shortcuts))
}

pub(super) fn collect_zed_keymap_file(path: &Path) -> Result<CollectedImport, AppError> {
    let parsed = zed::parse_exported_keybindings_file(path)?;
    Ok(finalize_import_shortcuts(vec![path.to_path_buf()], parsed))
}

pub(super) fn collect_vscode_export_file(path: &Path) -> Result<CollectedImport, AppError> {
    let parsed = vscode::parse_exported_keybindings_file(path)?;
    Ok(finalize_import_shortcuts(vec![path.to_path_buf()], parsed))
}

pub(super) fn collect_vscode_extension_manifests(paths: &[PathBuf]) -> Result<CollectedImport, AppError> {
    let mut sources = paths.to_vec();
    sources.sort();
    if sources.is_empty() {
        return Err(AppError::ImporterSourceNotFound {
            importer: crate::domain::known_apps::KnownImporterFamily::VSCode.display_name().to_string(),
            hint: crate::domain::known_apps::KnownImporterFamily::VSCode.import_hint().to_string(),
        });
    }

    let mut parsed = Vec::new();

    for source in &sources {
        let items = vscode::parse_extension_manifest_file(source)?;
        parsed.extend(items);
    }

    Ok(finalize_import_shortcuts(sources, parsed))
}

pub(super) fn collect_idea_keymap_file(
    path: &Path,
    parent_lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<CollectedImport, AppError> {
    let parsed = idea::parse_keybindings_file(path, parent_lookup)?;
    Ok(finalize_import_shortcuts(vec![path.to_path_buf()], parsed))
}

fn finalize_import_shortcuts(
    source_files: Vec<PathBuf>,
    mut shortcuts: Vec<ImportShortcut>,
) -> CollectedImport {
    let parsed_count = shortcuts.len();
    let mut seen = HashSet::new();
    let mut deduped_count = 0usize;
    shortcuts.retain(|shortcut| {
        let shortcut_key = normalize_shortcut(&shortcut.shortcut_display);
        let description_key = shortcut.description.trim().to_string();
        if shortcut_key.is_empty() || description_key.is_empty() {
            return false;
        }

        let key = format!("{shortcut_key}|{description_key}");
        seen.insert(key) || {
            deduped_count += 1;
            false
        }
    });

    CollectedImport {
        source_files,
        shortcuts,
        parsed_count,
        deduped_count,
    }
}

#[cfg(test)]
mod tests {
    use super::{collect_vscode_extension_manifests, finalize_import_shortcuts};
    use crate::storage::ImportShortcut;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn finalizes_import_shortcuts_with_dedupe_and_counts() {
        let source = PathBuf::from("/tmp/import.csv");
        let collected = finalize_import_shortcuts(
            vec![source.clone()],
            vec![
                ImportShortcut {
                    shortcut_display: "cmd+b".to_string(),
                    description: "Toggle Sidebar".to_string(),
                },
                ImportShortcut {
                    shortcut_display: "⌘ B".to_string(),
                    description: "Toggle Sidebar".to_string(),
                },
                ImportShortcut {
                    shortcut_display: "cmd+k".to_string(),
                    description: "".to_string(),
                },
                ImportShortcut {
                    shortcut_display: "".to_string(),
                    description: "Ignored".to_string(),
                },
                ImportShortcut {
                    shortcut_display: "cmd+shift+p".to_string(),
                    description: "Command Palette".to_string(),
                },
            ],
        );

        assert_eq!(collected.source_files, vec![source]);
        assert_eq!(collected.parsed_count, 5);
        assert_eq!(collected.deduped_count, 1);
        assert_eq!(collected.shortcuts.len(), 2);
        assert!(collected
            .shortcuts
            .iter()
            .any(|item| { item.shortcut_display == "cmd+b" && item.description == "Toggle Sidebar" }));
        assert!(collected
            .shortcuts
            .iter()
            .any(|item| { item.shortcut_display == "cmd+shift+p" && item.description == "Command Palette" }));
    }

    #[test]
    fn vscode_extension_import_fails_when_no_manifests_are_found() {
        let err = collect_vscode_extension_manifests(&[]).expect_err("missing manifests");
        assert!(matches!(err, AppError::ImporterSourceNotFound { .. }));
    }

    #[test]
    fn collects_vscode_extension_shortcuts_from_multiple_manifests_and_sorts_sources() {
        let dir = tempdir().expect("temp dir");
        let first_manifest = dir.path().join("z-last/package.json");
        let second_manifest = dir.path().join("a-first/package.json");
        fs::create_dir_all(first_manifest.parent().expect("parent")).expect("create extension dir");
        fs::create_dir_all(second_manifest.parent().expect("parent")).expect("create extension dir");
        fs::write(
            &first_manifest,
            r#"{ "contributes": { "keybindings": [{ "command": "sample.run", "mac": "cmd+r" }] } }"#,
        )
        .expect("write first manifest");
        fs::write(
            &second_manifest,
            r#"{ "contributes": { "keybindings": [{ "command": "sample.test", "mac": "cmd+t" }] } }"#,
        )
        .expect("write second manifest");

        let collected = collect_vscode_extension_manifests(&[
            PathBuf::from(&first_manifest),
            PathBuf::from(&second_manifest),
        ])
        .expect("collect extensions");

        assert_eq!(collected.source_files, vec![second_manifest, first_manifest]);
        assert_eq!(collected.shortcuts.len(), 2);
        assert!(collected
            .shortcuts
            .iter()
            .any(|item| item.shortcut_display == "⌘ R" && item.description == "sample.run"));
        assert!(collected
            .shortcuts
            .iter()
            .any(|item| item.shortcut_display == "⌘ T" && item.description == "sample.test"));
    }

    use crate::domain::errors::AppError;
}
