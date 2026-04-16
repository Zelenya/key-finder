pub mod notifier;

use crate::domain::models::NotificationContent;
use crate::storage::{AppId, NotificationSnapshot};
use rand::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SelectedApp {
    FocusedId(AppId),
    GuestimatedName(String),
    Unknown,
}

pub(crate) fn notification_payload(
    snapshot: &NotificationSnapshot,
    current_app: SelectedApp,
) -> NotificationContent {
    let mut rng = rand::rng();

    let entry = match &current_app {
        SelectedApp::Unknown => snapshot.shortcuts.iter().choose(&mut rng),
        SelectedApp::FocusedId(id) => snapshot.shortcuts_for_app(*id).choose(&mut rng),
        SelectedApp::GuestimatedName(name) => match snapshot.resolve_guessed_app(name) {
            Some(app_id) => snapshot.shortcuts_for_app(app_id).choose(&mut rng),
            None => snapshot.shortcuts.iter().choose(&mut rng),
        },
    };

    if let Some(entry) = entry {
        NotificationContent {
            title: format!("Shortcut for {}", snapshot.app_name(entry.app_id)),
            subtitle: Some(entry.shortcut.clone()),
            message: entry.description.clone(),
        }
    } else {
        let empty_state_name = match &current_app {
            SelectedApp::FocusedId(app_id) => snapshot.app_name(*app_id),
            SelectedApp::GuestimatedName(name) => name.as_str(),
            SelectedApp::Unknown => "unknown app",
        };

        NotificationContent {
            title: format!("No shortcuts found for {}", empty_state_name),
            subtitle: None,
            message: "Use the app dashboard to find shortcuts".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{notification_payload, SelectedApp};
    use crate::storage::{NotificationApp, NotificationShortcut, NotificationSnapshot};

    fn snapshot(apps: Vec<NotificationApp>, shortcuts: Vec<NotificationShortcut>) -> NotificationSnapshot {
        NotificationSnapshot { apps, shortcuts }
    }

    #[test]
    fn payload_prefers_current_app_shortcuts() {
        let snapshot = snapshot(
            vec![
                NotificationApp {
                    app_id: 3.into(),
                    name: "Zed".to_string(),
                    aliases: vec!["Zed".to_string()],
                },
                NotificationApp {
                    app_id: 1.into(),
                    name: "Code".to_string(),
                    aliases: vec!["Code".to_string(), "Visual Studio Code".to_string()],
                },
            ],
            vec![
                NotificationShortcut {
                    app_id: 3.into(),
                    shortcut: "⌘ B".to_string(),
                    description: "Toggle left bar".to_string(),
                },
                NotificationShortcut {
                    app_id: 1.into(),
                    shortcut: "⌘ P".to_string(),
                    description: "Go to file".to_string(),
                },
            ],
        );

        let payload = notification_payload(
            &snapshot,
            SelectedApp::GuestimatedName("Visual Studio Code".to_string()),
        );
        assert!(payload.title.contains("Code"));
    }

    #[test]
    fn payload_matches_alias_names() {
        let snapshot = snapshot(
            vec![NotificationApp {
                app_id: 2.into(),
                name: "Foo Studio".to_string(),
                aliases: vec!["Foo Studio".to_string(), "Foo".to_string()],
            }],
            vec![NotificationShortcut {
                app_id: 2.into(),
                shortcut: "⌘ K".to_string(),
                description: "Do the thing".to_string(),
            }],
        );

        let payload = notification_payload(&snapshot, SelectedApp::GuestimatedName("Foo".to_string()));
        assert!(payload.title.contains("Foo Studio"));
    }

    #[test]
    fn payload_falls_back_to_all_shortcuts_when_guessed_name_has_no_match() {
        let snapshot = snapshot(
            vec![NotificationApp {
                app_id: 2.into(),
                name: "Foo Studio".to_string(),
                aliases: vec!["Foo Studio".to_string(), "Foo".to_string()],
            }],
            vec![NotificationShortcut {
                app_id: 2.into(),
                shortcut: "⌘ K".to_string(),
                description: "Do the thing".to_string(),
            }],
        );

        let payload = notification_payload(&snapshot, SelectedApp::GuestimatedName("Safari".to_string()));
        assert!(payload.title.contains("Foo Studio"));
    }

    #[test]
    fn payload_uses_all_shortcuts_when_current_app_is_unknown() {
        let snapshot = snapshot(
            vec![NotificationApp {
                app_id: 2.into(),
                name: "Foo Studio".to_string(),
                aliases: vec!["Foo".to_string()],
            }],
            vec![NotificationShortcut {
                app_id: 2.into(),
                shortcut: "⌘ K".to_string(),
                description: "Do the thing".to_string(),
            }],
        );

        let payload = notification_payload(&snapshot, SelectedApp::Unknown);
        assert!(payload.title.contains("Foo Studio"));
    }

    #[test]
    fn payload_is_empty_when_focus_app_has_no_shortcuts() {
        let snapshot = snapshot(
            vec![
                NotificationApp {
                    app_id: 1.into(),
                    name: "Empty App".to_string(),
                    aliases: vec!["Empty".to_string()],
                },
                NotificationApp {
                    app_id: 2.into(),
                    name: "Foo Studio".to_string(),
                    aliases: vec!["Foo Studio".to_string(), "Foo".to_string()],
                },
            ],
            vec![NotificationShortcut {
                app_id: 3.into(),
                shortcut: "⌘ K".to_string(),
                description: "Do the thing".to_string(),
            }],
        );

        let payload = notification_payload(&snapshot, SelectedApp::FocusedId(1.into()));
        assert_eq!(payload.title, "No shortcuts found for Empty App");
        assert!(payload.subtitle.is_none());
    }
}
