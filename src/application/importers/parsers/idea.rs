use crate::domain::errors::AppError;
use crate::domain::shortcut_norm::{
    canonical_shortcut_from_delimited_input, render_canonical_shortcut, ShortcutDelimiter,
};
use crate::storage::ImportShortcut;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::fs;
use std::path::Path;

pub(crate) fn parse_keybindings_file(
    path: &Path,
    parent_lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<Vec<ImportShortcut>, AppError> {
    let content = fs::read_to_string(path).map_err(|source| AppError::ReadImporterFile {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(parse_idea_keymap(&content, parent_lookup))
}

// Note that we need to recursively parse parent keymaps as well
fn parse_idea_keymap(content: &str, parent_lookup: &dyn Fn(&str) -> Option<String>) -> Vec<ImportShortcut> {
    let mut result = Vec::new();

    if let Some(parent_name) = extract_keymap_attr(content, "parent") {
        if let Some(parent_content) = parent_lookup(&parent_name) {
            result.extend(parse_idea_actions(&parent_content));
        }
    }

    result.extend(parse_idea_actions(content));
    result
}

fn extract_keymap_attr(content: &str, name: &str) -> Option<String> {
    let keymap_start = content.find("<keymap ")?;
    let keymap_end = content[keymap_start..].find('>')? + keymap_start;
    let tag = &content[keymap_start..keymap_end];
    extract_xml_attr(tag, name)
}

fn extract_xml_attr(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=\"");
    let start = tag.find(&needle)?;
    let value_start = start + needle.len();
    let rest = &tag[value_start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn parse_idea_actions(content: &str) -> Vec<ImportShortcut> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    let mut result = Vec::new();
    let mut buffer = Vec::new();
    let mut current_action_id: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                if event.name().as_ref() == b"action" {
                    current_action_id = xml_attr_value(&event, b"id");
                }
            }
            Ok(Event::Empty(event)) => {
                if event.name().as_ref() == b"keyboard-shortcut" {
                    push_shortcut_from_event(&mut result, current_action_id.as_ref(), &event);
                }
            }
            Ok(Event::End(event)) => {
                if event.name().as_ref() == b"action" {
                    current_action_id = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                let _ = error;
                break;
            }
            _ => {}
        }
        buffer.clear();
    }

    result
}

fn push_shortcut_from_event(
    acc: &mut Vec<ImportShortcut>,
    current_action_id: Option<&String>,
    event: &BytesStart<'_>,
) {
    let Some(action_id) = current_action_id else {
        return;
    };

    let first = xml_attr_value(event, b"first-keystroke").unwrap_or_default();
    let second = xml_attr_value(event, b"second-keystroke").unwrap_or_default();
    let raw_shortcut = if second.is_empty() {
        first
    } else {
        format!("{first},{second}")
    };
    let shortcut_display = render_canonical_shortcut(&canonical_shortcut_from_delimited_input(
        &raw_shortcut,
        ShortcutDelimiter::Character(','),
        ShortcutDelimiter::Whitespace,
    ));
    if shortcut_display.is_empty() {
        return;
    }

    acc.push(ImportShortcut {
        shortcut_display,
        description: action_id.clone(),
    });
}

fn xml_attr_value(event: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    event
        .attributes()
        .flatten()
        .find(|attr| attr.key.as_ref() == key)
        .map(|attr| String::from_utf8_lossy(attr.value.as_ref()).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_idea_keymap;

    #[test]
    fn parses_single_shortcut_action() {
        let xml = r#"
        <keymap version="1" name="Test">
          <action id="Format">
            <keyboard-shortcut first-keystroke="ctrl alt L" />
          </action>
        </keymap>
        "#;

        let parsed = parse_idea_keymap(xml, &|_| None);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].shortcut_display, "⌃ ⌥ L");
        assert_eq!(parsed[0].description, "Format");
    }

    #[test]
    fn parses_multiple_actions_sharing_shortcut() {
        let xml = r#"
        <keymap version="1" name="Test">
          <action id="$Delete">
            <keyboard-shortcut first-keystroke="BACK_SPACE" />
          </action>
          <action id="EditorBackSpace">
            <keyboard-shortcut first-keystroke="BACK_SPACE" />
          </action>
        </keymap>
        "#;

        let parsed = parse_idea_keymap(xml, &|_| None);
        assert_eq!(parsed.len(), 2);
        assert!(parsed
            .iter()
            .any(|item| item.shortcut_display == "BACK_SPACE" && item.description == "$Delete"));
        assert!(parsed
            .iter()
            .any(|item| item.shortcut_display == "BACK_SPACE" && item.description == "EditorBackSpace"));
    }

    #[test]
    fn parses_chord_shortcuts() {
        let xml = r#"
        <keymap version="1" name="Test">
          <action id="Format">
            <keyboard-shortcut first-keystroke="ctrl alt L" second-keystroke="ctrl alt F" />
          </action>
        </keymap>
        "#;

        let parsed = parse_idea_keymap(xml, &|_| None);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].shortcut_display, "⌃ ⌥ L, ⌃ ⌥ F");
        assert_eq!(parsed[0].description, "Format");
    }

    #[test]
    fn ignores_actions_without_id() {
        let xml = r#"
        <keymap version="1" name="Test">
          <action>
            <keyboard-shortcut first-keystroke="ctrl alt X" />
          </action>
        </keymap>
        "#;

        let parsed = parse_idea_keymap(xml, &|_| None);
        assert!(parsed.is_empty());
    }

    #[test]
    fn merges_parent_keymap_when_lookup_returns_content() {
        let xml = r#"
        <keymap version="1" name="Test" parent="Mac OS X 10.5+">
          <action id="Format">
            <keyboard-shortcut first-keystroke="ctrl alt L" />
          </action>
        </keymap>
        "#;

        let parsed = parse_idea_keymap(xml, &|name| {
            assert_eq!(name, "Mac OS X 10.5+");
            Some(
                r#"
                    <keymap version="1" name="Parent">
                      <action id="EditorBackSpace">
                        <keyboard-shortcut first-keystroke="BACK_SPACE" />
                      </action>
                    </keymap>
                    "#
                .to_string(),
            )
        });

        assert_eq!(parsed.len(), 2);
        assert!(parsed
            .iter()
            .any(|item| item.shortcut_display == "BACK_SPACE" && item.description == "EditorBackSpace"));
        assert!(parsed.iter().any(|item| item.shortcut_display == "⌃ ⌥ L" && item.description == "Format"));
    }
}
