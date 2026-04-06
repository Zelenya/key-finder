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
        let key = entry.get("key").and_then(Value::as_str).unwrap_or("").trim();
        let command = entry.get("command").and_then(Value::as_str).unwrap_or("").trim();
        if key.is_empty() || command.is_empty() {
            continue;
        }
        // TODO: Consider storing internal/canonical structure
        let canonical_shortcut = canonical_shortcut_from_delimited_input(
            key,
            ShortcutDelimiter::Whitespace,
            ShortcutDelimiter::Character('+'),
        );
        let shortcut_display = render_canonical_shortcut(&canonical_shortcut);
        result.push(ImportShortcut {
            shortcut_display,
            description: command.to_string(),
        });
    }
    Ok(result)
}

pub(crate) fn parse_extension_manifest_file(path: &Path) -> Result<Vec<ImportShortcut>, AppError> {
    let content = fs::read_to_string(path).map_err(|source| AppError::ReadImporterFile {
        path: path.to_path_buf(),
        source,
    })?;

    parse_extension_manifest_content(path, &content)
}

fn parse_extension_manifest_content(path: &Path, content: &str) -> Result<Vec<ImportShortcut>, AppError> {
    let json_value = json::parse_json_loose(path, content)?;
    let keybindings =
        json_value.get("contributes").and_then(|v| v.get("keybindings")).and_then(Value::as_array);
    let Some(entries) = keybindings else {
        return Ok(Vec::new());
    };

    let mut result = Vec::new();
    for entry in entries {
        let key = entry
            .get("mac") // TODO: This is platform-specific leak
            .and_then(Value::as_str)
            .or_else(|| entry.get("key").and_then(Value::as_str))
            .unwrap_or("")
            .trim();
        let command = entry.get("command").and_then(Value::as_str).unwrap_or("").trim();
        if key.is_empty() || command.is_empty() || command.starts_with('-') {
            continue;
        }

        // TODO: Consider storing internal/canonical structure
        let canonical_shortcut = canonical_shortcut_from_delimited_input(
            key,
            ShortcutDelimiter::Whitespace,
            ShortcutDelimiter::Character('+'),
        );
        let shortcut_display = render_canonical_shortcut(&canonical_shortcut);
        result.push(ImportShortcut {
            shortcut_display,
            description: command.to_string(),
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{parse_exported_keybindings_content, parse_extension_manifest_content};
    use std::path::Path;

    #[test]
    fn parses_user_keybindings_jsonc() {
        let parsed = parse_exported_keybindings_content(
            Path::new("keybindings.json"),
            r#"
            [
              // comment
              { "key": "cmd+k cmd+s", "command": "workbench.action.openGlobalKeybindings" }
            ]
            "#,
        )
        .expect("parse keybindings");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].shortcut_display, "⌘ K, ⌘ S");
        assert_eq!(parsed[0].description, "workbench.action.openGlobalKeybindings");
    }

    #[test]
    fn parses_extension_manifest_keybindings() {
        let parsed = parse_extension_manifest_content(
            Path::new("package.json"),
            r#"
            {
              "name": "sample",
              "contributes": {
                "keybindings": [
                  { "command": "sample.run", "mac": "cmd+shift+r" }
                ]
              }
            }
            "#,
        )
        .expect("parse package");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].shortcut_display, "⌘ ⇧ R");
        assert_eq!(parsed[0].description, "sample.run");
    }

    #[test]
    fn normalizes_shared_punctuation_and_arrow_keys_in_imported_keybindings() {
        let parsed = parse_exported_keybindings_content(
            Path::new("keybindings.json"),
            r#"
            [
              { "key": "cmd+grave", "command": "sample.grave" },
              { "key": "cmd+right_arrow", "command": "sample.arrow" },
              { "key": "cmd+[", "command": "sample.left-bracket" }
            ]
            "#,
        )
        .expect("parse keybindings");

        assert_eq!(parsed.len(), 3);
        assert!(parsed
            .iter()
            .any(|entry| entry.shortcut_display == "⌘ `" && entry.description == "sample.grave"));
        assert!(parsed
            .iter()
            .any(|entry| entry.shortcut_display == "⌘ →" && entry.description == "sample.arrow"));
        assert!(parsed
            .iter()
            .any(|entry| entry.shortcut_display == "⌘ [" && entry.description == "sample.left-bracket"));
    }
}
