mod spec;
mod ui;
mod workflow;

use super::super::ImportTarget;
use crate::application::shortcut_center::ShortcutCenterCommandService;
use crate::domain::errors::AppError;
use crate::runtime::macos::platform::import_sources;
use crate::runtime::macos::ui::dialogs::{
    close_sheet, control_tag_from_current_event, control_tag_from_focus,
};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSModalResponse, NSWindow};
use objc2_foundation::NSString;

use spec::{dialog_spec_for_target, presentation_for, ImportDialogState, VsCodeImportMode};
use ui::{apply_dialog_state, build_import_window, prompt_for_import_file, selected_vscode_mode};
use workflow::{run_import, validate_selected_file};

const TAG_CLOSE: i64 = 12_001;
const TAG_CHOOSE_FILE: i64 = 12_002;
const TAG_CLEAR_FILE: i64 = 12_003;
const TAG_IMPORT: i64 = 12_004;
const TAG_MODE_CHANGED: i64 = 12_005;

pub(crate) fn open_import_dialog(
    command_service: &ShortcutCenterCommandService,
    target: ImportTarget,
    parent_window: &NSWindow,
) -> Result<String, AppError> {
    let spec = dialog_spec_for_target(&target);
    let mut vscode_mode = spec.kind.default_vscode_mode();

    let Some(mtm) = MainThreadMarker::new() else {
        return Err(AppError::MainThreadRequired);
    };
    let app = NSApplication::sharedApplication(mtm);
    app.activate();

    let ui = build_import_window(mtm, &app, &target.app_name, &spec)?;
    let (mut selected_path, mut state, mut status_text, mut detail_text) =
        initial_dialog_state(&spec, &target, vscode_mode);
    let mut completed_summary: Option<String> = None;

    ui.import_button.setKeyEquivalent(&NSString::from_str("\r"));
    ui.close_button.setKeyEquivalent(&NSString::from_str("\u{1b}"));
    parent_window.beginSheet_completionHandler(&ui.window, None);

    loop {
        let presentation = presentation_for(&spec, vscode_mode);
        apply_dialog_state(
            &ui,
            &presentation,
            &state,
            selected_path.as_deref(),
            &status_text,
            &detail_text,
        );
        let response = app.runModalForWindow(&ui.window);
        if !ui.window.isVisible() {
            close_sheet(parent_window, &ui.window, objc2_app_kit::NSModalResponseCancel);
            return Ok(completed_summary.unwrap_or_else(|| "Import canceled.".to_string()));
        }
        if response != objc2_app_kit::NSModalResponseOK && response != objc2_app_kit::NSModalResponseStop {
            close_sheet(parent_window, &ui.window, objc2_app_kit::NSModalResponseCancel);
            return Ok(completed_summary.unwrap_or_else(|| "Import canceled.".to_string()));
        }

        let tag = resolved_dialog_tag(
            response,
            control_tag_from_current_event(&app, &ui.window).or_else(|| control_tag_from_focus(&ui.window)),
            selected_vscode_mode(&ui, &spec).or(spec.kind.default_vscode_mode()),
            vscode_mode,
        );
        if response == objc2_app_kit::NSModalResponseStop && tag.is_none() {
            close_sheet(parent_window, &ui.window, objc2_app_kit::NSModalResponseCancel);
            return Ok(completed_summary.unwrap_or_else(|| "Import canceled.".to_string()));
        }

        match tag {
            Some(TAG_MODE_CHANGED) if !matches!(state, ImportDialogState::Importing) => {
                let next_mode = selected_vscode_mode(&ui, &spec).or(spec.kind.default_vscode_mode());
                if next_mode != vscode_mode {
                    vscode_mode = next_mode;
                    selected_path = None;
                    completed_summary = None;
                    (state, status_text, detail_text) = idle_dialog_state(&spec, vscode_mode);
                }
            }
            Some(TAG_CHOOSE_FILE) if !matches!(state, ImportDialogState::Importing) => {
                if let Some(path) =
                    prompt_for_import_file(&presentation, spec.kind, selected_path.as_deref(), &ui.window)?
                {
                    (selected_path, state, status_text, detail_text) =
                        validated_file_state(&target, vscode_mode, &path, "That file couldn't be used.");
                    completed_summary = None;
                }
            }
            Some(TAG_CLEAR_FILE) if !matches!(state, ImportDialogState::Importing) => {
                selected_path = None;
                (state, status_text, detail_text) = idle_dialog_state(&spec, vscode_mode);
                completed_summary = None;
            }
            Some(TAG_IMPORT) if !matches!(state, ImportDialogState::Importing) => {
                let path = if presentation.requires_file {
                    let Some(path) = selected_path.as_ref() else {
                        state = ImportDialogState::Error;
                        status_text = "Choose a file before importing.".to_string();
                        detail_text.clear();
                        continue;
                    };
                    Some(path.as_str())
                } else {
                    None
                };

                if presentation.requires_file && path.is_none() {
                    state = ImportDialogState::Error;
                    status_text = "Choose a file before importing.".to_string();
                    detail_text.clear();
                    continue;
                }

                state = ImportDialogState::Importing;
                status_text = format!("Importing shortcuts for {}...", target.app_name);
                detail_text = "This may take a moment.".to_string();
                completed_summary = None;
                apply_dialog_state(
                    &ui,
                    &presentation,
                    &state,
                    selected_path.as_deref(),
                    &status_text,
                    &detail_text,
                );
                ui.window.displayIfNeeded();

                match run_import(command_service, &target, vscode_mode, path) {
                    Ok(outcome) => {
                        completed_summary = Some(outcome.library_status);
                        state = ImportDialogState::Success;
                        status_text = "Import finished.".to_string();
                        detail_text = outcome.detail_block;
                    }
                    Err(error) => {
                        state = ImportDialogState::Error;
                        status_text = "Import failed.".to_string();
                        detail_text = error.to_string();
                    }
                }
            }
            Some(TAG_CLOSE) => {
                close_sheet(
                    parent_window,
                    &ui.window,
                    if matches!(state, ImportDialogState::Success) {
                        objc2_app_kit::NSModalResponseOK
                    } else {
                        objc2_app_kit::NSModalResponseCancel
                    },
                );
                return Ok(completed_summary.unwrap_or_else(|| "Import canceled.".to_string()));
            }
            _ => {
                close_sheet(
                    parent_window,
                    &ui.window,
                    if matches!(state, ImportDialogState::Success) {
                        objc2_app_kit::NSModalResponseOK
                    } else {
                        objc2_app_kit::NSModalResponseCancel
                    },
                );
                return Ok(completed_summary.unwrap_or_else(|| "Import canceled.".to_string()));
            }
        }
    }
}

fn initial_dialog_state(
    spec: &spec::ImportDialogSpec,
    target: &ImportTarget,
    vscode_mode: Option<VsCodeImportMode>,
) -> (Option<String>, ImportDialogState, String, String) {
    initial_dialog_state_with_keymap_lookup(
        spec,
        target,
        vscode_mode,
        import_sources::preferred_idea_keymap_file,
    )
}

fn initial_dialog_state_with_keymap_lookup<F>(
    spec: &spec::ImportDialogSpec,
    target: &ImportTarget,
    vscode_mode: Option<VsCodeImportMode>,
    preferred_idea_keymap_file: F,
) -> (Option<String>, ImportDialogState, String, String)
where
    F: FnOnce() -> Result<Option<std::path::PathBuf>, AppError>,
{
    let (default_state, default_status, default_detail) = idle_dialog_state(spec, vscode_mode);
    if spec.kind != spec::ImportDialogKind::JetBrains {
        return (None, default_state, default_status, default_detail);
    }

    let Ok(Some(path)) = preferred_idea_keymap_file() else {
        return (None, default_state, default_status, default_detail);
    };
    validated_file_state(
        target,
        vscode_mode,
        &path.display().to_string(),
        "The discovered IntelliJ IDEA keymap couldn't be used.",
    )
}

fn idle_dialog_state(
    spec: &spec::ImportDialogSpec,
    vscode_mode: Option<VsCodeImportMode>,
) -> (ImportDialogState, String, String) {
    let presentation = presentation_for(spec, vscode_mode);
    (
        ImportDialogState::Idle,
        presentation.idle_status.to_string(),
        presentation.idle_detail.to_string(),
    )
}

fn validated_file_state(
    target: &ImportTarget,
    vscode_mode: Option<VsCodeImportMode>,
    path: &str,
    error_status: &str,
) -> (Option<String>, ImportDialogState, String, String) {
    match validate_selected_file(target, vscode_mode, path) {
        Ok(validated) => (
            Some(path.to_string()),
            ImportDialogState::Idle,
            "File looks good. Click Import to continue.".to_string(),
            format!(
                "Ready to import. Parsed {} shortcut(s) from the selected file.",
                validated.parsed_count
            ),
        ),
        Err(error) => (
            None,
            ImportDialogState::Error,
            error_status.to_string(),
            format!("{}\n\nChoose a different XML file and try again.", error),
        ),
    }
}

fn resolved_dialog_tag(
    response: NSModalResponse,
    raw_tag: Option<i64>,
    selected_mode: Option<VsCodeImportMode>,
    current_mode: Option<VsCodeImportMode>,
) -> Option<i64> {
    if let Some(tag) = raw_tag {
        return Some(tag);
    }

    if response == objc2_app_kit::NSModalResponseStop && selected_mode != current_mode {
        return Some(TAG_MODE_CHANGED);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::spec::{ImportDialogKind, ImportDialogSpec, ImportDialogState};
    use super::{initial_dialog_state_with_keymap_lookup, resolved_dialog_tag, TAG_MODE_CHANGED};
    use crate::domain::known_apps::KnownImporterFamily;
    use crate::runtime::macos::ui::shortcut_center::ImportTarget;
    use crate::storage::AppId;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn prefill_state_uses_discovered_idea_keymap() {
        let dir = tempdir().expect("temp dir");
        let home = dir.path();
        let keymaps = home.join("Library/Application Support/JetBrains/IntelliJIdea2025.3/keymaps");
        fs::create_dir_all(&keymaps).expect("create keymaps");
        let path = keymaps.join("macOS copy.xml");
        fs::write(
            &path,
            r#"<keymap version="1" name="Test"><action id="Format"><keyboard-shortcut first-keystroke="ctrl alt L" /></action></keymap>"#,
        )
        .expect("write keymap");
        let spec = ImportDialogSpec {
            kind: ImportDialogKind::JetBrains,
        };
        let target = ImportTarget {
            app_id: AppId::from(1),
            app_name: "IntelliJ IDEA".to_string(),
            importer: Some(KnownImporterFamily::JetBrains),
        };

        let state = initial_dialog_state_with_keymap_lookup(&spec, &target, None, || Ok(Some(path.clone())));

        assert_eq!(state.0.as_deref(), Some(path.to_string_lossy().as_ref()));
        assert_eq!(state.1, ImportDialogState::Idle);
        assert!(state.2.contains("File looks good"));
        assert!(state.3.contains("Parsed 1 shortcut"));
    }

    #[test]
    fn anonymous_modal_stop_is_treated_as_mode_change_when_popup_selection_changed() {
        let tag = resolved_dialog_tag(
            objc2_app_kit::NSModalResponseStop,
            None,
            Some(super::spec::VsCodeImportMode::InstalledExtensionShortcuts),
            Some(super::spec::VsCodeImportMode::ExportedKeybindingsFile),
        );

        assert_eq!(tag, Some(TAG_MODE_CHANGED));
    }

    #[test]
    fn anonymous_modal_stop_without_mode_change_stays_unresolved() {
        let tag = resolved_dialog_tag(
            objc2_app_kit::NSModalResponseStop,
            None,
            Some(super::spec::VsCodeImportMode::ExportedKeybindingsFile),
            Some(super::spec::VsCodeImportMode::ExportedKeybindingsFile),
        );

        assert_eq!(tag, None);
    }
}
