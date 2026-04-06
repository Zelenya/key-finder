//! Shared shortcut-normalization primitives used by both parser modes.
//! We keep common key names here so manual entry and imported shortcuts can
//! converge on the same canonical representation instead of each parser family
//! inventing its own names for arrows and punctuation keys.

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Token {
    Modifier(&'static str),
    Key(String),
    ChordBreak,
}

pub(super) fn named_token(normalized: &str) -> Option<Token> {
    match normalized {
        "cmd" | "command" | "meta" | "super" => Some(Token::Modifier("cmd")),
        "ctrl" | "control" => Some(Token::Modifier("ctrl")),
        "alt" | "option" => Some(Token::Modifier("alt")),
        "shift" => Some(Token::Modifier("shift")),
        _ => canonical_key_name(normalized).map(|key| Token::Key(key.to_string())),
    }
}

pub(super) fn modifier_rank(modifier: &str) -> usize {
    match modifier {
        "cmd" => 0,
        "ctrl" => 1,
        "alt" => 2,
        "shift" => 3,
        _ => 4,
    }
}

pub(super) fn flush_token_chord(chords: &mut Vec<Vec<Token>>, current: &mut Vec<Token>) {
    if !current.is_empty() {
        chords.push(std::mem::take(current));
    }
}

pub(super) fn canonical_shortcut_from_chords(chords: &[Vec<String>]) -> String {
    let token_chords =
        chords.iter().map(|chord| chord.iter().filter_map(|part| part_token(part)).collect::<Vec<_>>());

    canonical_shortcut_from_token_chords(token_chords)
}

pub(super) fn canonical_shortcut_from_token_chords<I>(chords: I) -> String
where
    I: IntoIterator<Item = Vec<Token>>,
{
    chords
        .into_iter()
        .filter_map(|chord| canonical_chord_from_tokens(&chord))
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn symbol_token(ch: char) -> Option<Token> {
    match ch {
        '⌘' => Some(Token::Modifier("cmd")),
        '⌃' => Some(Token::Modifier("ctrl")),
        '⌥' => Some(Token::Modifier("alt")),
        '⇧' => Some(Token::Modifier("shift")),
        _ => symbol_key_name(ch).map(|key| Token::Key(key.to_string())),
    }
}

pub(super) fn word_token(word: &str) -> Option<Token> {
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

pub(super) fn render_key_name(key: &str) -> Option<&'static str> {
    match key {
        "backtick" => Some("`"),
        "comma" => Some(","),
        "period" => Some("."),
        "slash" => Some("/"),
        "minus" => Some("-"),
        "equal" => Some("="),
        "left_bracket" => Some("["),
        "right_bracket" => Some("]"),
        "backslash" => Some("\\"),
        "semicolon" => Some(";"),
        "quote" => Some("'"),
        "enter" => Some("↩"),
        "escape" => Some("⎋"),
        "left" => Some("←"),
        "right" => Some("→"),
        "up" => Some("↑"),
        "down" => Some("↓"),
        "space" => Some("Space"),
        _ => None,
    }
}

fn canonical_key_name(normalized: &str) -> Option<&'static str> {
    match normalized {
        "enter" | "return" => Some("enter"),
        "esc" | "escape" => Some("escape"),
        "space" => Some("space"),
        "`" | "backtick" | "grave" => Some("backtick"),
        "," | "comma" => Some("comma"),
        "." | "period" | "dot" => Some("period"),
        "/" | "slash" => Some("slash"),
        "-" | "minus" => Some("minus"),
        "=" | "equal" => Some("equal"),
        "[" | "left_bracket" => Some("left_bracket"),
        "]" | "right_bracket" => Some("right_bracket"),
        "\\" | "backslash" => Some("backslash"),
        ";" | "semicolon" => Some("semicolon"),
        "'" | "quote" | "apostrophe" => Some("quote"),
        "left" | "left_arrow" => Some("left"),
        "right" | "right_arrow" => Some("right"),
        "up" | "up_arrow" => Some("up"),
        "down" | "down_arrow" => Some("down"),
        _ => None,
    }
}

fn symbol_key_name(ch: char) -> Option<&'static str> {
    match ch {
        '`' => Some("backtick"),
        '.' => Some("period"),
        '/' => Some("slash"),
        '-' => Some("minus"),
        '=' => Some("equal"),
        '[' => Some("left_bracket"),
        ']' => Some("right_bracket"),
        '\\' => Some("backslash"),
        ';' => Some("semicolon"),
        '\'' => Some("quote"),
        '↩' => Some("enter"),
        '⎋' => Some("escape"),
        '←' => Some("left"),
        '→' => Some("right"),
        '↑' => Some("up"),
        '↓' => Some("down"),
        _ => None,
    }
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
