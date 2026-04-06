use super::super::super::ImportTarget;
use super::spec::VsCodeImportMode;
use crate::application::importers::{self, CollectedImport};
use crate::application::shortcut_center::ShortcutCenterCommandService;
use crate::domain::errors::AppError;
use crate::runtime::macos::platform::import_sources;
use crate::storage::ImportMergeSummary;
use std::path::Path;

pub(super) struct ImportDialogOutcome {
    pub library_status: String,
    pub detail_block: String,
}

pub(super) struct ValidatedFile {
    pub parsed_count: usize,
}

pub(super) fn validate_selected_file(
    target: &ImportTarget,
    vscode_mode: Option<VsCodeImportMode>,
    selected_path: &str,
) -> Result<ValidatedFile, AppError> {
    let collected = prepare_import(target, vscode_mode, Some(selected_path))?;
    if collected.parsed_count == 0 {
        return Err(AppError::StorageOperation(
            "No shortcuts were found in the selected file.".to_string(),
        ));
    }
    Ok(ValidatedFile {
        parsed_count: collected.parsed_count,
    })
}

pub(super) fn run_import(
    command_service: &ShortcutCenterCommandService,
    target: &ImportTarget,
    vscode_mode: Option<VsCodeImportMode>,
    selected_path: Option<&str>,
) -> Result<ImportDialogOutcome, AppError> {
    let collected = prepare_import(target, vscode_mode, selected_path)?;
    let detail_block = format_import_result_block(importer_display_name(target), &collected);
    let summary = command_service.import_shortcuts(target.app_id, collected.shortcuts)?.summary;

    Ok(ImportDialogOutcome {
        library_status: format!(
            "Imported shortcuts for {}. Added {}, unchanged {}, skipped {}.",
            target.app_name, summary.added, summary.unchanged, summary.skipped
        ),
        detail_block: format_import_result_block_with_merge(&detail_block, &summary),
    })
}

fn prepare_import(
    target: &ImportTarget,
    vscode_mode: Option<VsCodeImportMode>,
    selected_path: Option<&str>,
) -> Result<CollectedImport, AppError> {
    match target.importer {
        None => importers::collect_custom_csv_file(Path::new(
            selected_path
                .ok_or_else(|| AppError::StorageOperation("missing selected import file".to_string()))?,
        )),
        Some(crate::domain::known_apps::KnownImporterFamily::VSCode)
            if vscode_mode == Some(VsCodeImportMode::InstalledExtensionShortcuts) =>
        {
            let sources = import_sources::find_vscode_extension_manifest_files()?;
            importers::collect_vscode_extension_manifests(&sources)
        }
        Some(crate::domain::known_apps::KnownImporterFamily::VSCode) => {
            importers::collect_vscode_export_file(Path::new(
                selected_path
                    .ok_or_else(|| AppError::StorageOperation("missing selected import file".to_string()))?,
            ))
        }
        Some(crate::domain::known_apps::KnownImporterFamily::Zed) => {
            importers::collect_zed_keymap_file(Path::new(
                selected_path
                    .ok_or_else(|| AppError::StorageOperation("missing selected import file".to_string()))?,
            ))
        }
        Some(crate::domain::known_apps::KnownImporterFamily::JetBrains) => {
            importers::collect_idea_keymap_file(
                Path::new(
                    selected_path.ok_or_else(|| {
                        AppError::StorageOperation("missing selected import file".to_string())
                    })?,
                ),
                &|parent_name| import_sources::load_idea_parent_keymap(parent_name),
            )
        }
    }
}

fn importer_display_name(target: &ImportTarget) -> &'static str {
    target.importer.map_or("Custom CSV", |importer| importer.display_name())
}

fn format_import_result_block(importer_name: &str, collected: &CollectedImport) -> String {
    format!(
        "Importer: {importer_name}\nParsed: {} shortcut(s)\nSources scanned: {}\nDuplicates merged: {}",
        collected.parsed_count,
        collected.source_files.len(),
        collected.deduped_count,
    )
}

fn format_import_result_block_with_merge(collected_block: &str, summary: &ImportMergeSummary) -> String {
    format!(
        "{collected_block}\nAdded: {}\nUnchanged: {}\nSkipped: {}",
        summary.added, summary.unchanged, summary.skipped
    )
}
