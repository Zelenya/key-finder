mod parse;
mod render;
mod shared;

pub(crate) use parse::{canonical_shortcut_from_delimited_input, normalize_shortcut, ShortcutDelimiter};
pub(crate) use render::render_canonical_shortcut;
