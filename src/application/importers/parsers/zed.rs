use super::super::json;
use crate::domain::errors::AppError;
use crate::domain::shortcut_norm::{
    canonical_shortcut_from_delimited_input, render_canonical_shortcut, ShortcutDelimiter,
};
use crate::storage::ImportShortcut;
use serde_json::Value;
use std::fs;
use std::path::Path;

pub(crate) fn parse_exported_keybindings_file(path: &Path) -> Result<Vec<ImportShortcut>, AppError> {
    let content = fs::read_to_string(path).map_err(|source| AppError::ReadImporterFile {
        path: path.to_path_buf(),
        source,
    })?;

    parse_exported_keybindings_content(path, &content)
}

fn parse_exported_keybindings_content(path: &Path, content: &str) -> Result<Vec<ImportShortcut>, AppError> {
    let json_value = json::parse_json_loose(path, content)?;
    let entries = json_value.as_array().ok_or_else(|| AppError::InvalidImporterSource {
        path: path.to_path_buf(),
        message: "expected JSON array".to_string(),
    })?;

    let mut result = Vec::new();
    for entry in entries {
        let Some(bindings) = entry.get("bindings").and_then(Value::as_object) else {
            continue;
        };
        for (key, command_value) in bindings {
            let Some(command) = parse_binding_command(command_value) else {
                continue;
            };

            // TODO: Consider storing internal/canonical structure
            let canonical_shortcut = canonical_shortcut_from_delimited_input(
                key,
                ShortcutDelimiter::Whitespace,
                ShortcutDelimiter::Character('-'),
            );
            let shortcut_display = render_canonical_shortcut(&canonical_shortcut);
            result.push(ImportShortcut {
                shortcut_display,
                description: command,
            });
        }
    }
    Ok(result)
}

fn parse_binding_command(command_value: &Value) -> Option<String> {
    if let Some(command) = command_value.as_str() {
        let trimmed = command.trim();
        return (!trimmed.is_empty()).then(|| trimmed.to_string());
    }

    if let Some(obj) = command_value.as_object() {
        let command = obj.get("command").and_then(Value::as_str)?.trim();
        return (!command.is_empty()).then(|| command.to_string());
    }

    if let Some(items) = command_value.as_array() {
        let command = items.first().and_then(Value::as_str)?.trim();
        if command.is_empty() {
            return None;
        }

        if items.len() >= 2 {
            let args = &items[1];
            if !args.is_null() {
                return Some(format!("{command} {}", render_compact_json(args)));
            }
        }
        return Some(command.to_string());
    }

    None
}

fn render_compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "<args>".to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_exported_keybindings_content;
    use std::path::Path;

    #[test]
    fn parses_string_array_and_object_command_forms() {
        let parsed = parse_exported_keybindings_content(
            Path::new("keymap.json"),
            r#"
            [
              {
                "context": "Workspace",
                "bindings": {
                  "cmd-b": "workspace::ToggleLeftDock",
                  "cmd-k cmd-f": ["editor::Format", {"trigger": "manual"}],
                  "cmd-j": {"command": "workspace::ToggleBottomDock"},
                  "cmd-u": null
                }
              }
            ]
            "#,
        )
        .expect("parse");
        assert_eq!(parsed.len(), 3);
        assert!(parsed
            .iter()
            .any(|m| m.shortcut_display == "⌘ B" && m.description == "workspace::ToggleLeftDock"));
        assert!(parsed
            .iter()
            .any(|m| m.shortcut_display == "⌘ K, ⌘ F" && m.description.contains("editor::Format")));
        assert!(parsed
            .iter()
            .any(|m| m.shortcut_display == "⌘ J" && m.description == "workspace::ToggleBottomDock"));
    }

    #[test]
    fn normalizes_shared_punctuation_and_arrow_keys() {
        let parsed = parse_exported_keybindings_content(
            Path::new("keymap.json"),
            r#"
            [
              {
                "context": "Workspace",
                "bindings": {
                  "cmd-[": "workspace::Back",
                  "cmd-left": "workspace::MoveLeft"
                }
              }
            ]
            "#,
        )
        .expect("parse");

        assert_eq!(parsed.len(), 2);
        assert!(parsed.iter().any(|m| m.shortcut_display == "⌘ [" && m.description == "workspace::Back"));
        assert!(parsed.iter().any(|m| m.shortcut_display == "⌘ ←" && m.description == "workspace::MoveLeft"));
    }
}
