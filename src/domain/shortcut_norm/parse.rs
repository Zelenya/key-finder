use super::shared::{modifier_rank, named_token, Token};

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

pub(crate) fn canonical_shortcut_from_chords(chords: &[Vec<String>]) -> String {
    let token_chords =
        chords.iter().map(|chord| chord.iter().filter_map(|part| part_token(part)).collect::<Vec<_>>());

    canonical_shortcut_from_token_chords(token_chords)
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
            tokens.push(Token::ChordBreak);
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

fn flush_token_chord(chords: &mut Vec<Vec<Token>>, current: &mut Vec<Token>) {
    if !current.is_empty() {
        chords.push(std::mem::take(current));
    }
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

fn canonical_shortcut_from_token_chords<I>(chords: I) -> String
where
    I: IntoIterator<Item = Vec<Token>>,
{
    chords
        .into_iter()
        .filter_map(|chord| canonical_chord_from_tokens(&chord))
        .collect::<Vec<_>>()
        .join(",")
}

fn canonical_chord_from_tokens(tokens: &[Token]) -> Option<String> {
    let mut modifiers = Vec::new();
    let mut keys = Vec::new();

    for token in tokens {
        match token {
            Token::Modifier(modifier) => modifiers.push((*modifier).to_string()),
            Token::Key(key) => keys.push(key.clone()),
            Token::ChordBreak => {}
        }
    }

    modifiers.sort_by_key(|modifier| modifier_rank(modifier));
    modifiers.extend(keys);

    (!modifiers.is_empty()).then(|| modifiers.join("+"))
}

fn part_token(raw: &str) -> Option<Token> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lowered = trimmed.to_ascii_lowercase();
    if let Some(token) = named_token(&lowered) {
        return Some(token);
    }

    let token = lowered.chars().filter(|ch| !ch.is_whitespace()).collect::<String>();
    (!token.is_empty()).then_some(Token::Key(token))
}

fn symbol_token(ch: char) -> Option<Token> {
    match ch {
        '⌘' => Some(Token::Modifier("cmd")),
        '⌃' => Some(Token::Modifier("ctrl")),
        '⌥' => Some(Token::Modifier("alt")),
        '⇧' => Some(Token::Modifier("shift")),
        '↩' => Some(Token::Key("enter".to_string())),
        '⎋' => Some(Token::Key("escape".to_string())),
        _ => None,
    }
}

fn word_token(word: &str) -> Option<Token> {
    let normalized = word.trim().to_ascii_lowercase();

    if normalized.is_empty() {
        return None;
    }

    if let Some(token) = named_token(&normalized) {
        return Some(token);
    }

    Some(normalized.as_str())
        .map(|s| s.chars().filter(|ch| ch.is_ascii_alphanumeric()).collect::<String>())
        .filter(|s| !s.is_empty())
        .map(Token::Key)
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_shortcut_from_chords, canonical_shortcut_from_delimited_input, normalize_shortcut,
        ShortcutDelimiter,
    };

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
    }
}
