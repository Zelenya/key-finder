use super::commands::ActionCommand;
use super::dialogs;
use crate::application::shortcut_center::ShortcutCenterCommandService;
use crate::domain::errors::AppError;
use crate::storage::{AppId, ManagedShortcut, ShortcutState};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSApplication, NSWindow};
use objc2_foundation::NSString;

pub(super) fn apply_command(
    command: ActionCommand,
    command_service: &ShortcutCenterCommandService,
    app_id: AppId,
    app_name: &str,
    selected_shortcuts: &[&ManagedShortcut],
    description_input: Option<&str>,
    parent_window: &NSWindow,
) -> Result<String, AppError> {
    match command {
        ActionCommand::AddShortcut => {
            let Some((shortcut, description)) = dialogs::prompt_new_shortcut(app_name, parent_window)? else {
                return Ok("Canceled.".to_string());
            };
            if shortcut.trim().is_empty() {
                return Ok("Shortcut keys can't be empty.".to_string());
            }
            if description.trim().is_empty() {
                return Ok("Description can't be empty.".to_string());
            }
            let _ = command_service.add_shortcut(app_id, shortcut.trim(), description.trim())?;
            Ok(format!("Added a custom shortcut for {app_name}."))
        }
        ActionCommand::SaveDescription => save_description(
            command_service,
            selected_shortcuts,
            description_input.unwrap_or_default(),
        ),
        ActionCommand::ToggleVisibilitySelected => {
            toggle_selected_visibility(command_service, selected_shortcuts)
        }
        ActionCommand::DeleteSelected => delete_selected(command_service, selected_shortcuts),
    }
}

fn save_description(
    command_service: &ShortcutCenterCommandService,
    selected_shortcuts: &[&ManagedShortcut],
    value: &str,
) -> Result<String, AppError> {
    let [shortcut] = selected_shortcuts else {
        return Ok("Select exactly one shortcut to edit its description.".to_string());
    };
    if value.trim().is_empty() {
        return Ok("Description can't be empty.".to_string());
    }
    let _ = command_service.update_description(shortcut.id, value.trim())?;
    Ok("Description updated.".to_string())
}

fn toggle_selected_visibility(
    command_service: &ShortcutCenterCommandService,
    selected_shortcuts: &[&ManagedShortcut],
) -> Result<String, AppError> {
    if selected_shortcuts.is_empty() {
        return Ok("Select one or more shortcuts first.".to_string());
    }

    let all_active = selected_shortcuts.iter().all(|shortcut| shortcut.state == ShortcutState::Active);
    let all_hidden = selected_shortcuts.iter().all(|shortcut| shortcut.state == ShortcutState::Dismissed);

    if all_active {
        set_selected_state(command_service, selected_shortcuts, ShortcutState::Dismissed)
    } else if all_hidden {
        set_selected_state(command_service, selected_shortcuts, ShortcutState::Active)
    } else {
        Ok("Select shortcuts that are all visible or all hidden to change visibility.".to_string())
    }
}

fn set_selected_state(
    command_service: &ShortcutCenterCommandService,
    selected_shortcuts: &[&ManagedShortcut],
    next_state: ShortcutState,
) -> Result<String, AppError> {
    if selected_shortcuts.is_empty() {
        return Ok("Select one or more shortcuts first.".to_string());
    }

    let affected_ids = selected_shortcuts
        .iter()
        .filter(|shortcut| shortcut.state != next_state)
        .map(|shortcut| shortcut.id)
        .collect::<Vec<_>>();
    if affected_ids.is_empty() {
        return Ok(match next_state {
            ShortcutState::Active => "The selected shortcuts are already visible.".to_string(),
            ShortcutState::Dismissed => "The selected shortcuts are already hidden.".to_string(),
        });
    }

    let result = command_service.set_shortcut_state(&affected_ids, next_state)?;
    Ok(match result.target_state {
        ShortcutState::Active => pluralize(result.updated, "Restored", "shortcut"),
        ShortcutState::Dismissed => pluralize(result.updated, "Hidden", "shortcut"),
    })
}

fn delete_selected(
    command_service: &ShortcutCenterCommandService,
    selected_shortcuts: &[&ManagedShortcut],
) -> Result<String, AppError> {
    if selected_shortcuts.is_empty() {
        return Ok("Select one or more shortcuts first.".to_string());
    }

    let count = selected_shortcuts.len();
    let confirmed = confirm_action(
        "Delete Shortcuts",
        &format!("{} permanently?", pluralize(count, "Delete", "shortcut")),
    )?;

    if !confirmed {
        return Ok("Canceled.".to_string());
    }

    let result = command_service
        .delete_shortcuts(&selected_shortcuts.iter().map(|shortcut| shortcut.id).collect::<Vec<_>>())?;
    Ok(pluralize(result.deleted, "Deleted", "shortcut"))
}

fn pluralize(count: usize, verb: &str, singular: &str) -> String {
    if count == 1 {
        format!("{verb} 1 {singular}")
    } else {
        format!("{verb} {count} {singular}s")
    }
}

fn confirm_action(title: &str, prompt: &str) -> Result<bool, AppError> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Err(AppError::MainThreadRequired);
    };

    let app = NSApplication::sharedApplication(mtm);
    app.activate();

    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(prompt));
    alert.addButtonWithTitle(&NSString::from_str("Delete"));
    alert.addButtonWithTitle(&NSString::from_str("Cancel"));
    let response = alert.runModal();
    Ok(response == NSAlertFirstButtonReturn)
}
