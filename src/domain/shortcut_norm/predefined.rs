//! Predefined shortcut parsing lives separately because importer formats already
//! know what their delimiters mean. Keeping this path explicit avoids leaking
//! manual-entry heuristics into VS Code, Zed, and JetBrains imports.

use super::parse::{
    canonical_shortcut_from_chords, canonical_shortcut_from_token_chords, flush_token_chord, named_token,
    symbol_token, word_token, Token,
};

/// Used when the caller already knows the format,
/// so delimiter meaning is fixed by the source format instead of inferred from context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShortcutDelimiter {
    Whitespace,
    Character(char),
}

/// Apps describe the same shortcut in different styles:
/// spaced (`⌘ ⇧ P`), compact (`⌘⇧P`), plus-joined (`cmd+k`), or chorded (`⌘ K, ⌘ R` / `⌘K ⌘R`).
/// We normalize those into one consistent representation.
pub(crate) fn normalize_shortcut(raw: &str) -> String {
    let mut chords = Vec::new();
    let mut current = Vec::new();
    let mut has_primary_key = false;

    for token in lex_tokens(raw) {
        match token {
            Token::ChordBreak => {
                flush_token_chord(&mut chords, &mut current);
                has_primary_key = false;
            }
            Token::Modifier(modifier) => {
                if has_primary_key {
                    flush_token_chord(&mut chords, &mut current);
                    has_primary_key = false;
                }
                current.push(Token::Modifier(modifier));
            }
            Token::Key(key) => {
                current.push(Token::Key(key));
                has_primary_key = true;
            }
        }
    }

    flush_token_chord(&mut chords, &mut current);
    canonical_shortcut_from_token_chords(chords)
}

pub(crate) fn canonical_shortcut_from_delimited_input(
    raw: &str,
    chord_separator: ShortcutDelimiter,
    token_separator: ShortcutDelimiter,
) -> String {
    let chords = split_segments(raw, chord_separator)
        .into_iter()
        .map(|chord| {
            split_segments(chord, token_separator).into_iter().map(str::to_string).collect::<Vec<_>>()
        })
        .filter(|chord| !chord.is_empty())
        .collect::<Vec<_>>();

    canonical_shortcut_from_chords(&chords)
}

fn lex_tokens(raw: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.peek().copied() {
        if ch == ',' {
            chars.next();
            tokens.push(if comma_starts_next_chord(&chars) {
                Token::ChordBreak
            } else {
                Token::Key("comma".to_string())
            });
            continue;
        }

        if ch == '+' || ch.is_whitespace() {
            chars.next();
            continue;
        }

        if let Some(token) = symbol_token(ch) {
            chars.next();
            tokens.push(token);
            continue;
        }

        let mut word = String::new();
        while let Some(ch) = chars.peek().copied() {
            if ch == ',' || ch == '+' || ch.is_whitespace() || symbol_token(ch).is_some() {
                break;
            }
            word.push(ch);
            chars.next();
        }

        if let Some(token) = word_token(&word) {
            tokens.push(token);
        }
    }

    tokens
}

fn comma_starts_next_chord(chars: &std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    let mut lookahead = chars.clone();
    while matches!(lookahead.peek(), Some(ch) if ch.is_whitespace()) {
        lookahead.next();
    }

    let Some(ch) = lookahead.peek().copied() else {
        return false;
    };

    if let Some(token) = symbol_token(ch) {
        return matches!(token, Token::Modifier(_));
    }

    let mut word = String::new();
    while let Some(ch) = lookahead.peek().copied() {
        if ch == ',' || ch == '+' || ch.is_whitespace() || symbol_token(ch).is_some() {
            break;
        }
        word.push(ch);
        lookahead.next();
    }
    matches!(named_token(&word.to_ascii_lowercase()), Some(Token::Modifier(_)))
}

fn split_segments(raw: &str, delimiter: ShortcutDelimiter) -> Vec<&str> {
    match delimiter {
        ShortcutDelimiter::Whitespace => {
            raw.split_whitespace().filter(|segment| !segment.is_empty()).collect()
        }
        ShortcutDelimiter::Character(ch) => {
            raw.split(ch).map(str::trim).filter(|segment| !segment.is_empty()).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{canonical_shortcut_from_delimited_input, normalize_shortcut, ShortcutDelimiter};
    use crate::domain::shortcut_norm::parse::canonical_shortcut_from_chords;
    use crate::domain::shortcut_norm::render_canonical_shortcut;
    use proptest::prelude::*;

    #[test]
    fn normalizes_symbols_and_names() {
        assert_eq!(normalize_shortcut("⌘ ⇧ B"), "cmd+shift+b");
        assert_eq!(normalize_shortcut("cmd+shift+b"), "cmd+shift+b");
        assert_eq!(normalize_shortcut("Command Shift B"), "cmd+shift+b");
        assert_eq!(normalize_shortcut("⌘⇧P"), "cmd+shift+p");
        assert_eq!(normalize_shortcut("⌘ Enter"), "cmd+enter");
        assert_eq!(normalize_shortcut("⌘ ↩"), "cmd+enter");
        assert_eq!(normalize_shortcut("⌘ Esc"), "cmd+escape");
        assert_eq!(normalize_shortcut("⌘ Space"), "cmd+space");
        assert_eq!(normalize_shortcut("⌘ ["), "cmd+left_bracket");
        assert_eq!(normalize_shortcut("⌘ ]"), "cmd+right_bracket");
        assert_eq!(normalize_shortcut("⌘ /"), "cmd+slash");
        assert_eq!(normalize_shortcut("⌘ -"), "cmd+minus");
        assert_eq!(normalize_shortcut("⌘ ="), "cmd+equal");
        assert_eq!(normalize_shortcut("⌘ ;"), "cmd+semicolon");
        assert_eq!(normalize_shortcut("⌘ '"), "cmd+quote");
        assert_eq!(normalize_shortcut("⌘ ←"), "cmd+left");
        assert_eq!(normalize_shortcut("⌘ ↑"), "cmd+up");
        assert_eq!(normalize_shortcut("⌘ ↓"), "cmd+down");
        assert_eq!(normalize_shortcut("⌘ ,"), "cmd+comma");
    }

    #[test]
    fn normalizes_chords() {
        assert_eq!(normalize_shortcut("⌘ K, ⌘ R"), "cmd+k,cmd+r");
        assert_eq!(normalize_shortcut("⌘+K ⌘+R"), "cmd+k,cmd+r");
        assert_eq!(normalize_shortcut("⌘K ⌘R"), "cmd+k,cmd+r");
        assert_eq!(normalize_shortcut("cmd+k cmd+s"), "cmd+k,cmd+s");
        assert_eq!(normalize_shortcut("ctrl+k, ctrl+c"), "ctrl+k,ctrl+c");
        assert_eq!(normalize_shortcut("⌘⇧P ⌘R"), "cmd+shift+p,cmd+r");
    }

    #[test]
    fn builds_canonical_shortcuts_from_chords() {
        let chords = vec![
            vec!["shift".to_string(), "cmd".to_string(), "r".to_string()],
            vec!["BACK_SPACE".to_string()],
            vec!["ctrl".to_string(), "alt".to_string(), "space".to_string()],
        ];

        assert_eq!(
            canonical_shortcut_from_chords(&chords),
            "cmd+shift+r,back_space,ctrl+alt+space"
        );
    }

    #[test]
    fn parses_vscode_style_delimited_input() {
        assert_eq!(
            canonical_shortcut_from_delimited_input(
                "cmd+k cmd+s",
                ShortcutDelimiter::Whitespace,
                ShortcutDelimiter::Character('+'),
            ),
            "cmd+k,cmd+s"
        );
        assert_eq!(
            canonical_shortcut_from_delimited_input(
                "cmd+[",
                ShortcutDelimiter::Whitespace,
                ShortcutDelimiter::Character('+'),
            ),
            "cmd+left_bracket"
        );
        assert_eq!(
            canonical_shortcut_from_delimited_input(
                "cmd+right_arrow",
                ShortcutDelimiter::Whitespace,
                ShortcutDelimiter::Character('+'),
            ),
            "cmd+right"
        );
    }

    #[test]
    fn parses_zed_style_delimited_input() {
        assert_eq!(
            canonical_shortcut_from_delimited_input(
                "cmd-k cmd-f",
                ShortcutDelimiter::Whitespace,
                ShortcutDelimiter::Character('-'),
            ),
            "cmd+k,cmd+f"
        );
        assert_eq!(
            canonical_shortcut_from_delimited_input(
                "cmd-[",
                ShortcutDelimiter::Whitespace,
                ShortcutDelimiter::Character('-'),
            ),
            "cmd+left_bracket"
        );
        assert_eq!(
            canonical_shortcut_from_delimited_input(
                "cmd-left",
                ShortcutDelimiter::Whitespace,
                ShortcutDelimiter::Character('-'),
            ),
            "cmd+left"
        );
    }

    #[test]
    fn parses_idea_style_delimited_input_and_preserves_special_keys() {
        assert_eq!(
            canonical_shortcut_from_delimited_input(
                "ctrl alt L,ctrl alt F",
                ShortcutDelimiter::Character(','),
                ShortcutDelimiter::Whitespace,
            ),
            "ctrl+alt+l,ctrl+alt+f"
        );
        assert_eq!(
            canonical_shortcut_from_delimited_input(
                "BACK_SPACE",
                ShortcutDelimiter::Character(','),
                ShortcutDelimiter::Whitespace,
            ),
            "back_space"
        );
        assert_eq!(
            canonical_shortcut_from_delimited_input(
                "cmd+grave",
                ShortcutDelimiter::Whitespace,
                ShortcutDelimiter::Character('+'),
            ),
            "cmd+backtick"
        );
        assert_eq!(
            canonical_shortcut_from_delimited_input(
                "cmd+right_arrow",
                ShortcutDelimiter::Whitespace,
                ShortcutDelimiter::Character('+'),
            ),
            "cmd+right"
        );
    }

    proptest! {
        #[test]
        fn normalize_shortcut_is_idempotent(raw in any::<String>()) {
            let once = normalize_shortcut(&raw);
            let twice = normalize_shortcut(&once);
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn canonical_shortcut_round_trips_through_render(canonical in arb_canonical_shortcut()) {
            let rendered = render_canonical_shortcut(&canonical);
            prop_assert_eq!(normalize_shortcut(&rendered), canonical);
        }
    }

    // Note: Each chord carries at least one modifier — a bare `comma` chord renders to ", "
    // which is indistinguishable from a chord break, and that ambiguity is real,
    // not a behavior the property should worry about.
    fn arb_canonical_shortcut() -> impl Strategy<Value = String> {
        proptest::collection::vec(arb_canonical_chord(), 1..=3).prop_map(|chords| chords.join(","))
    }

    fn arb_canonical_chord() -> impl Strategy<Value = String> {
        (arb_modifiers(), arb_canonical_key()).prop_map(|(mods, key)| {
            // This won't happen with the current arb_modifiers, but just in case
            if mods.is_empty() {
                key
            } else {
                format!("{}+{}", mods.join("+"), key)
            }
        })
    }

    fn arb_modifiers() -> impl Strategy<Value = Vec<&'static str>> {
        proptest::sample::subsequence(vec!["cmd", "ctrl", "alt", "shift"], 1..=4)
    }

    fn arb_canonical_key() -> impl Strategy<Value = String> {
        static KEYS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        let keys = KEYS.get_or_init(|| {
            let letters = (b'a'..=b'z').map(|c| (c as char).to_string());
            let keywords = [
                "left",
                "right",
                "up",
                "down",
                "enter",
                "escape",
                "space",
                "comma",
                "period",
                "slash",
                "minus",
                "equal",
                "semicolon",
                "quote",
                "backtick",
                "left_bracket",
                "right_bracket",
                "backslash",
            ]
            .into_iter()
            .map(String::from);
            letters.chain(keywords).collect()
        });
        proptest::sample::select(keys.clone())
    }
}
