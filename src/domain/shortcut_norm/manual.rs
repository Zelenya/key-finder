//! Manual shortcut parsing has special treatment because human-entered input is ambiguous.
//! We need to preserve separators long enough to distinguish
//! whether punctuation was meant as a separator syntax or as the key itself.

use super::parse::{
    canonical_shortcut_from_token_chords, flush_token_chord, named_token, symbol_token, Token,
};

#[derive(Clone, Debug, PartialEq, Eq)]
/// A looser representation for free-form user input.
/// We preserve separators to decide if it's actually a separator or a key.
enum ManualItem {
    Separator(ManualSeparator),
    Token(Token),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Manual input has different separator semantics than importer formats.
///
/// `+` continues the same chord, whitespace usually starts the next chord,
/// and comma is ambiguous:
/// in `cmd+,` it means the comma key,
/// in `cmd+k, cmd+s` it means a chord break.
enum ManualSeparator {
    Plus,
    Whitespace,
    Comma,
}

pub(crate) fn normalize_manual_shortcut(raw: &str) -> String {
    let mut chords = Vec::new();
    let mut current = Vec::new();
    let mut has_primary_key = false;
    let mut pending_plus = false;
    let mut pending_whitespace = false;

    for item in lex_manual_items(raw) {
        match item {
            ManualItem::Separator(ManualSeparator::Plus) => {
                pending_plus = true;
                pending_whitespace = false;
            }
            ManualItem::Separator(ManualSeparator::Whitespace) => {
                if !pending_plus {
                    pending_whitespace = true;
                }
            }
            ManualItem::Separator(ManualSeparator::Comma) => {
                if pending_plus || current.is_empty() || !has_primary_key {
                    current.push(Token::Key("comma".to_string()));
                    has_primary_key = true;
                } else {
                    flush_token_chord(&mut chords, &mut current);
                    has_primary_key = false;
                }
                pending_plus = false;
                pending_whitespace = false;
            }
            ManualItem::Token(Token::Modifier(modifier)) => {
                if has_primary_key {
                    flush_token_chord(&mut chords, &mut current);
                    has_primary_key = false;
                }
                current.push(Token::Modifier(modifier));
                pending_plus = false;
                pending_whitespace = false;
            }
            ManualItem::Token(Token::Key(key)) => {
                if has_primary_key && pending_whitespace && !pending_plus {
                    flush_token_chord(&mut chords, &mut current);
                }
                current.push(Token::Key(key));
                has_primary_key = true;
                pending_plus = false;
                pending_whitespace = false;
            }
            ManualItem::Token(Token::ChordBreak) => {}
        }
    }

    flush_token_chord(&mut chords, &mut current);
    canonical_shortcut_from_token_chords(chords)
}

fn lex_manual_items(raw: &str) -> Vec<ManualItem> {
    let mut items = Vec::new();
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.peek().copied() {
        match ch {
            ',' => {
                chars.next();
                items.push(ManualItem::Separator(ManualSeparator::Comma));
                continue;
            }
            '+' => {
                chars.next();
                items.push(ManualItem::Separator(ManualSeparator::Plus));
                continue;
            }
            _ if ch.is_whitespace() => {
                chars.next();
                while matches!(chars.peek(), Some(next) if next.is_whitespace()) {
                    chars.next();
                }
                items.push(ManualItem::Separator(ManualSeparator::Whitespace));
                continue;
            }
            _ => {}
        }

        if ch == '-' {
            let mut lookahead = chars.clone();
            lookahead.next();
            if matches!(lookahead.next(), Some('>')) {
                chars.next();
                chars.next();
                items.push(ManualItem::Token(Token::Key("right".to_string())));
                continue;
            }
        }

        if let Some(token) = manual_symbol_token(ch) {
            chars.next();
            items.push(ManualItem::Token(token));
            continue;
        }

        let mut word = String::new();
        while let Some(ch) = chars.peek().copied() {
            if ch == ',' || ch == '+' || ch.is_whitespace() || manual_symbol_token(ch).is_some() {
                break;
            }
            word.push(ch);
            chars.next();
        }

        if let Some(token) = manual_word_token(&word) {
            items.push(ManualItem::Token(token));
        }
    }

    items
}

fn manual_symbol_token(ch: char) -> Option<Token> {
    symbol_token(ch)
}

fn manual_word_token(word: &str) -> Option<Token> {
    let normalized = word.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }

    if let Some(token) = manual_named_token(&normalized) {
        return Some(token);
    }

    Some(Token::Key(normalized))
}

// This is useful for manually typed shortcuts only.
// Don't want to reuse those with the shared parser, doesn't hurt but doesn't make sense
fn manual_named_token(normalized: &str) -> Option<Token> {
    match normalized {
        "->" => Some(Token::Key("right".to_string())),
        "<-" => Some(Token::Key("left".to_string())),
        _ => named_token(normalized),
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_manual_shortcut;

    #[test]
    fn normalizes_manual_punctuation_shortcuts() {
        assert_eq!(normalize_manual_shortcut("cmd+,"), "cmd+comma");
        assert_eq!(normalize_manual_shortcut("ctrl+`"), "ctrl+backtick");
        assert_eq!(normalize_manual_shortcut("cmd+/"), "cmd+slash");
        assert_eq!(normalize_manual_shortcut("cmd+["), "cmd+left_bracket");
        assert_eq!(normalize_manual_shortcut("cmd+-"), "cmd+minus");
        assert_eq!(normalize_manual_shortcut("cmd+k ->"), "cmd+k,right");
        assert_eq!(normalize_manual_shortcut("cmd+K, ->"), "cmd+k,right");
        assert_eq!(normalize_manual_shortcut("cmd+left"), "cmd+left");
        assert_eq!(normalize_manual_shortcut("cmd+up_arrow"), "cmd+up");
        assert_eq!(normalize_manual_shortcut("cmd+down"), "cmd+down");
    }

    #[test]
    fn normalizes_manual_shortcuts_with_existing_styles() {
        assert_eq!(normalize_manual_shortcut("⌘ K, ⌘ R"), "cmd+k,cmd+r");
        assert_eq!(normalize_manual_shortcut("cmd+k cmd+s"), "cmd+k,cmd+s");
        assert_eq!(normalize_manual_shortcut("⌘⇧P"), "cmd+shift+p");
    }
}
