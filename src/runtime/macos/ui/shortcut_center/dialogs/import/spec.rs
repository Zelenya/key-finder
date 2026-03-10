use super::super::super::ImportTarget;
use crate::domain::known_apps::KnownImporterFamily;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ImportDialogKind {
    CustomCsv,
    JetBrains,
    VSCode,
    Zed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum VsCodeImportMode {
    ExportedKeybindingsFile,
    InstalledExtensionShortcuts,
}

impl VsCodeImportMode {
    pub(super) fn title(self) -> &'static str {
        match self {
            Self::ExportedKeybindingsFile => "Exported keybindings file",
            Self::InstalledExtensionShortcuts => "Installed extension shortcuts",
        }
    }
}

pub(super) struct ImportDialogSpec {
    pub kind: ImportDialogKind,
}

impl ImportDialogKind {
    pub(super) fn vscode_modes(self) -> &'static [VsCodeImportMode] {
        match self {
            Self::VSCode => &[
                VsCodeImportMode::ExportedKeybindingsFile,
                VsCodeImportMode::InstalledExtensionShortcuts,
            ] as &[VsCodeImportMode],
            _ => &[] as &[VsCodeImportMode],
        }
    }

    pub(super) fn default_vscode_mode(self) -> Option<VsCodeImportMode> {
        match self {
            Self::VSCode => Some(VsCodeImportMode::ExportedKeybindingsFile),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ImportDialogPresentation {
    pub intro_text: &'static str,
    pub file_label: &'static str,
    pub file_description: &'static str,
    pub allowed_extensions: &'static [&'static str],
    pub requires_file: bool,
    pub import_button_label: &'static str,
    pub idle_status: &'static str,
    pub idle_detail: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ImportDialogState {
    Idle,
    Importing,
    Success,
    Error,
}

pub(super) fn dialog_spec_for_target(target: &ImportTarget) -> ImportDialogSpec {
    ImportDialogSpec {
        kind: match target.importer {
            None => ImportDialogKind::CustomCsv,
            Some(KnownImporterFamily::VSCode) => ImportDialogKind::VSCode,
            Some(KnownImporterFamily::Zed) => ImportDialogKind::Zed,
            Some(KnownImporterFamily::JetBrains) => ImportDialogKind::JetBrains,
        },
    }
}

pub(super) fn presentation_for(
    spec: &ImportDialogSpec,
    vscode_mode: Option<VsCodeImportMode>,
) -> ImportDialogPresentation {
    match spec.kind {
        ImportDialogKind::CustomCsv => ImportDialogPresentation {
            intro_text:
                "Choose a CSV file with the header shortcut,description for this custom app",
            file_label: "Shortcut CSV file",
            file_description: "shortcut CSV",
            allowed_extensions: &["csv"],
            requires_file: true,
            import_button_label: "Import",
            idle_status: "Choose a shortcut CSV file to import. Imported shortcuts start hidden.",
            idle_detail: "",
        },
        ImportDialogKind::VSCode => match vscode_mode.unwrap_or(VsCodeImportMode::ExportedKeybindingsFile) {
            VsCodeImportMode::ExportedKeybindingsFile => ImportDialogPresentation {
                intro_text:
                    "In VS Code, run 'Preferences: Open Default Keyboard Shortcuts (JSON)', save the file, then choose it here",
                file_label: "VS Code exported shortcuts file",
                file_description: "VS Code exported shortcuts",
                allowed_extensions: &["json", "jsonc"],
                requires_file: true,
                import_button_label: "Import",
                idle_status:
                    "Choose a VS Code exported shortcuts file to import. Imported shortcuts start hidden.",
                idle_detail: "",
            },
            VsCodeImportMode::InstalledExtensionShortcuts => ImportDialogPresentation {
                intro_text:
                    "Key Finder will scan installed VS Code extension manifests from the app bundle and your user extensions when you click Import",
                file_label: "VS Code extension shortcuts",
                file_description: "VS Code extension shortcuts",
                allowed_extensions: &[],
                requires_file: false,
                import_button_label: "Scan Extensions",
                idle_status: "Ready to scan installed VS Code extension shortcuts.",
                idle_detail: "No file is needed in this mode. Click Scan Extensions to import shortcuts from installed VS Code extension manifests. Imported shortcuts start hidden.",
            },
        },
        ImportDialogKind::Zed => ImportDialogPresentation {
            intro_text:
                "In Zed, run 'zed: open default keymap', save the JSON file, then choose it here",
            file_label: "Zed keymap file",
            file_description: "Zed keymap",
            allowed_extensions: &["json"],
            requires_file: true,
            import_button_label: "Import",
            idle_status: "Choose a Zed keymap file to import. Imported shortcuts start hidden.",
            idle_detail: "",
        },
        ImportDialogKind::JetBrains => ImportDialogPresentation {
            intro_text:
                "Choose an IntelliJ IDEA keymap XML file to import. If Key Finder finds one locally, it will prefill it here",
            file_label: "JetBrains keymap XML file",
            file_description: "JetBrains keymap",
            allowed_extensions: &["xml"],
            requires_file: true,
            import_button_label: "Import",
            idle_status: "Choose a JetBrains keymap file to import. Imported shortcuts start hidden.",
            idle_detail: "",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{dialog_spec_for_target, presentation_for, ImportDialogKind, VsCodeImportMode};
    use crate::domain::known_apps::KnownImporterFamily;
    use crate::runtime::macos::ui::shortcut_center::ImportTarget;
    use crate::storage::AppId;

    fn target(app_name: &str, importer: Option<KnownImporterFamily>) -> ImportTarget {
        ImportTarget {
            app_id: AppId::from(1),
            app_name: app_name.to_string(),
            importer,
        }
    }

    #[test]
    fn zed_dialog_spec_includes_export_instructions() {
        let spec = dialog_spec_for_target(&target("Zed", Some(KnownImporterFamily::Zed)));
        assert_eq!(spec.kind, ImportDialogKind::Zed);
        let presentation = presentation_for(&spec, None);
        assert!(presentation.intro_text.contains("zed: open default keymap"));
        assert!(presentation.intro_text.contains("save the JSON file"));
    }

    #[test]
    fn vscode_dialog_spec_includes_export_instructions_and_modes() {
        let spec = dialog_spec_for_target(&target("Visual Studio Code", Some(KnownImporterFamily::VSCode)));
        assert_eq!(spec.kind, ImportDialogKind::VSCode);
        assert_eq!(
            spec.kind.vscode_modes(),
            &[
                VsCodeImportMode::ExportedKeybindingsFile,
                VsCodeImportMode::InstalledExtensionShortcuts
            ]
        );
        assert_eq!(
            spec.kind.default_vscode_mode(),
            Some(VsCodeImportMode::ExportedKeybindingsFile)
        );

        let file_presentation = presentation_for(&spec, Some(VsCodeImportMode::ExportedKeybindingsFile));
        assert!(file_presentation.intro_text.contains("Preferences: Open Default Keyboard Shortcuts (JSON)"));
        assert!(file_presentation.intro_text.contains("save the file"));
        assert!(file_presentation.requires_file);
        assert_eq!(file_presentation.import_button_label, "Import");

        let extension_presentation =
            presentation_for(&spec, Some(VsCodeImportMode::InstalledExtensionShortcuts));
        assert!(extension_presentation.intro_text.contains("installed VS Code extension manifests"));
        assert!(!extension_presentation.requires_file);
        assert_eq!(extension_presentation.import_button_label, "Scan Extensions");
        assert!(extension_presentation.idle_detail.contains("No file is needed"));
    }

    #[test]
    fn idea_dialog_spec_mentions_prefill() {
        let spec = dialog_spec_for_target(&target("IntelliJ IDEA", Some(KnownImporterFamily::JetBrains)));
        assert_eq!(spec.kind, ImportDialogKind::JetBrains);
        let presentation = presentation_for(&spec, None);
        assert!(presentation.intro_text.contains("IntelliJ IDEA keymap XML"));
        assert!(presentation.intro_text.contains("prefill"));
    }

    #[test]
    fn custom_app_dialog_spec_uses_csv_import() {
        let spec = dialog_spec_for_target(&target("Ghostty", None));
        assert_eq!(spec.kind, ImportDialogKind::CustomCsv);
        let presentation = presentation_for(&spec, None);
        assert!(presentation.intro_text.contains("shortcut,description"));
        assert!(presentation.intro_text.contains("start hidden"));
    }
}
