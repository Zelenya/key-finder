pub mod notifier;

use crate::domain::app_norm::{app_matches_any, app_names_match};
use crate::domain::models::NotificationContent;
use crate::storage::ShortcutMessage;
use rand::prelude::*;

pub(crate) fn notification_payload(
    shortcuts: &[ShortcutMessage],
    current_app: Option<&str>,
) -> NotificationContent {
    let active = if let Some(app_name) = current_app {
        let matching = shortcuts
            .iter()
            .filter(|shortcut| {
                if shortcut.match_names.is_empty() {
                    app_names_match(&shortcut.app, app_name)
                } else {
                    app_matches_any(&shortcut.match_names, app_name)
                }
            })
            .collect::<Vec<_>>();

        if matching.is_empty() {
            shortcuts.iter().collect::<Vec<_>>()
        } else {
            matching
        }
    } else {
        shortcuts.iter().collect::<Vec<_>>()
    };

    let mut rng = rand::rng();

    if let Some(entry) = active.choose(&mut rng) {
        NotificationContent {
            title: format!("Shortcut for {}", entry.app),
            subtitle: Some(entry.shortcut.clone()),
            message: entry.description.clone(),
        }
    } else {
        let current_app = current_app.unwrap_or("unknown app");
        NotificationContent {
            title: format!("No shortcuts found for {}", current_app),
            subtitle: None,
            message: "Use the app dashboard to find shortcuts".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::notification_payload;
    use crate::storage::ShortcutMessage;

    #[test]
    fn payload_prefers_current_app_shortcuts() {
        let shortcuts = vec![
            ShortcutMessage {
                app: "Zed".to_string(),
                match_names: vec!["Zed".to_string()],
                shortcut: "⌘ B".to_string(),
                description: "Toggle left bar".to_string(),
            },
            ShortcutMessage {
                app: "Code".to_string(),
                match_names: vec!["Code".to_string(), "Visual Studio Code".to_string()],
                shortcut: "⌘ P".to_string(),
                description: "Go to file".to_string(),
            },
        ];

        let payload = notification_payload(&shortcuts, Some("Visual Studio Code"));
        assert!(payload.title.contains("Code"));
    }

    #[test]
    fn payload_matches_alias_names() {
        let shortcuts = vec![ShortcutMessage {
            app: "Acme Studio".to_string(),
            match_names: vec!["Acme Studio".to_string(), "Acme".to_string()],
            shortcut: "⌘ K".to_string(),
            description: "Do the thing".to_string(),
        }];

        let payload = notification_payload(&shortcuts, Some("Acme"));
        assert!(payload.title.contains("Acme Studio"));
    }
}
