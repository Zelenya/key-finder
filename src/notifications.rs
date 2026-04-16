pub mod notifier;

use crate::application::notifications::SelectedApp;
use crate::domain::app_norm::{app_matches_any, app_names_match};
use crate::domain::models::NotificationContent;
use crate::storage::ShortcutMessage;
use rand::prelude::*;

pub(crate) fn notification_payload(
    shortcuts: &[ShortcutMessage],
    current_app: SelectedApp,
) -> NotificationContent {
    let matching =
        shortcuts.iter().filter(|shortcut| matches_current_app(shortcut, &current_app)).collect::<Vec<_>>();

    let active = match current_app {
        SelectedApp::FocusedId(_) => matching,
        SelectedApp::GuestimatedName(_) | SelectedApp::Unknown => {
            if matching.is_empty() {
                shortcuts.iter().collect::<Vec<_>>()
            } else {
                matching
            }
        }
    };

    let mut rng = rand::rng();

    if let Some(entry) = active.choose(&mut rng) {
        NotificationContent {
            title: format!("Shortcut for {}", entry.app),
            subtitle: Some(entry.shortcut.clone()),
            message: entry.description.clone(),
        }
    } else {
        let current_app = match current_app {
            SelectedApp::FocusedId(_) => "focused app".to_string(),
            SelectedApp::GuestimatedName(name) => name,
            SelectedApp::Unknown => "unknown app".to_string(),
        };
        NotificationContent {
            title: format!("No shortcuts found for {}", current_app),
            subtitle: None,
            message: "Use the app dashboard to find shortcuts".to_string(),
        }
    }
}

// TODO: We can skip the filtering if the app is unknown
fn matches_current_app(shortcut: &ShortcutMessage, current_app: &SelectedApp) -> bool {
    match current_app {
        SelectedApp::FocusedId(id) => shortcut.app_id == *id,
        SelectedApp::GuestimatedName(name) => {
            if shortcut.match_names.is_empty() {
                app_names_match(&shortcut.app, name)
            } else {
                app_matches_any(&shortcut.match_names, name)
            }
        }
        SelectedApp::Unknown => false,
    }
}

#[cfg(test)]
mod tests {
    use super::notification_payload;
    use crate::application::notifications::SelectedApp;
    use crate::storage::ShortcutMessage;

    #[test]
    fn payload_prefers_current_app_shortcuts() {
        let shortcuts = vec![
            ShortcutMessage {
                app_id: 3.into(),
                app: "Zed".to_string(),
                match_names: vec!["Zed".to_string()],
                shortcut: "⌘ B".to_string(),
                description: "Toggle left bar".to_string(),
            },
            ShortcutMessage {
                app_id: 1.into(),
                app: "Code".to_string(),
                match_names: vec!["Code".to_string(), "Visual Studio Code".to_string()],
                shortcut: "⌘ P".to_string(),
                description: "Go to file".to_string(),
            },
        ];

        let payload = notification_payload(
            &shortcuts,
            SelectedApp::GuestimatedName("Visual Studio Code".to_string()),
        );
        assert!(payload.title.contains("Code"));
    }

    #[test]
    fn payload_matches_alias_names() {
        let shortcuts = vec![ShortcutMessage {
            app_id: 2.into(),
            app: "Acme Studio".to_string(),
            match_names: vec!["Acme Studio".to_string(), "Acme".to_string()],
            shortcut: "⌘ K".to_string(),
            description: "Do the thing".to_string(),
        }];

        let payload = notification_payload(&shortcuts, SelectedApp::GuestimatedName("Acme".to_string()));
        assert!(payload.title.contains("Acme Studio"));
    }

    #[test]
    fn payload_is_empty_when_focus_app_has_no_shortcuts() {
        let shortcuts = vec![ShortcutMessage {
            app_id: 2.into(),
            app: "Foo Studio".to_string(),
            match_names: vec!["Foo Studio".to_string(), "Foo".to_string()],
            shortcut: "⌘ K".to_string(),
            description: "Do the thing".to_string(),
        }];

        let payload = notification_payload(&shortcuts, SelectedApp::FocusedId(1.into()));
        assert_eq!(payload.title, "No shortcuts found for focused app");
        assert!(payload.subtitle.is_none());
    }
}
