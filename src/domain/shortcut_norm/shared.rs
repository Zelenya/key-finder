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
        "enter" | "return" => Some(Token::Key("enter".to_string())),
        "esc" | "escape" => Some(Token::Key("escape".to_string())),
        "space" => Some(Token::Key("space".to_string())),
        _ => None,
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
