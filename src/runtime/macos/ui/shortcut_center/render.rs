use super::ImportTarget;
use crate::storage::{AppSummary, ManagedShortcut, ShortcutState};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ShortcutActionAvailability {
    pub can_save_description: bool,
    pub visibility_action: Option<VisibilityAction>,
    pub can_delete: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum VisibilityAction {
    Hide,
    ShowAgain,
}

/// Powers inspector title area and hints for selected app
pub(super) fn summarize_app(
    app: &AppSummary,
    aliases: &[String],
    include_dismissed: bool,
    shortcuts: &[ManagedShortcut],
) -> (String, String) {
    let active_count = usize::try_from(app.active_count).unwrap_or(0);
    let total_count = usize::try_from(app.total_count).unwrap_or(shortcuts.len());
    let hidden_count = total_count.saturating_sub(active_count);
    let alias_text = if aliases.is_empty() {
        "No aliases".to_string()
    } else {
        format!("Aliases: {}", aliases.join(", "))
    };
    let summary = format!(
        "{} shortcuts loaded, {} active, {} hidden{}.",
        total_count,
        active_count,
        hidden_count,
        if include_dismissed {
            ", showing hidden shortcuts"
        } else {
            ""
        }
    );
    (summary, alias_text)
}

/// If app has no shortcuts, either shows custom import hint or generic message
pub(super) fn app_hint(target: &ImportTarget) -> String {
    target.importer.map(|importer| importer.import_hint().to_string()).unwrap_or(
        "This is a custom app. Import a CSV with shortcut,description or add shortcuts one by one."
            .to_string(),
    )
}

/// Returns the selection UI overview based on the number of shortcuts selected
pub(super) fn selection_title(selected_shortcuts: &[&ManagedShortcut]) -> String {
    match selected_shortcuts.len() {
        0 => "No shortcuts selected".to_string(),
        1 => "1 shortcut selected".to_string(),
        count => format!("{count} shortcuts selected"),
    }
}

/// Returns the selection UI hint based on the number of shortcuts selected
pub(super) fn selection_hint(selected_shortcuts: &[&ManagedShortcut]) -> String {
    match selected_shortcuts.len() {
        0 => "Select a shortcut to edit its description or change visibility.".to_string(),
        1 => {
            "Edit the description, hide it from notifications, or delete it.".to_string()
        }
        _ => "Bulk actions apply to the selected shortcuts. Description editing is available when a single shortcut is selected.".to_string(),
    }
}

/// Drives the availablity of shortcut actions based on the selected shortcuts.
/// If all selected shortcuts have the same state, we can batch update them.
pub(super) fn shortcut_action_availability(
    selected_shortcuts: &[&ManagedShortcut],
) -> ShortcutActionAvailability {
    let visibility_action = if selected_shortcuts.is_empty() {
        None
    } else if selected_shortcuts.iter().all(|shortcut| shortcut.state == ShortcutState::Active) {
        Some(VisibilityAction::Hide)
    } else if selected_shortcuts.iter().all(|shortcut| shortcut.state == ShortcutState::Dismissed) {
        Some(VisibilityAction::ShowAgain)
    } else {
        None
    };
    let can_delete = !selected_shortcuts.is_empty();

    ShortcutActionAvailability {
        can_save_description: selected_shortcuts.len() == 1,
        visibility_action,
        can_delete,
    }
}

#[cfg(test)]
mod tests {
    use super::app_hint;
    use crate::domain::known_apps::KnownImporterFamily;
    use crate::runtime::macos::ui::shortcut_center::ImportTarget;
    use crate::storage::AppId;

    #[test]
    fn app_hint_uses_importer_family_for_known_apps() {
        let target = ImportTarget {
            app_id: AppId::from(2),
            app_name: "Visual Studio Code".to_string(),
            importer: Some(KnownImporterFamily::VSCode),
        };

        let hint = app_hint(&target);
        assert!(hint.contains("Preferences: Open Default Keyboard Shortcuts (JSON)"));
    }

    #[test]
    fn app_hint_uses_custos_copy_when_no_importer_exists() {
        let target = ImportTarget {
            app_id: AppId::from(99),
            app_name: "Ghostty".to_string(),
            importer: None,
        };

        let hint = app_hint(&target);
        assert!(hint.contains("custom app"));
        assert!(hint.contains("shortcut,description"));
    }
}
