mod manual;
mod parse;
mod predefined;
mod render;

pub(crate) use manual::normalize_manual_shortcut;
pub(crate) use predefined::{canonical_shortcut_from_delimited_input, normalize_shortcut, ShortcutDelimiter};
pub(crate) use render::render_canonical_shortcut;
