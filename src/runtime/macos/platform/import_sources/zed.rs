use std::path::PathBuf;

pub(crate) fn preferred_keymap_directory() -> Option<PathBuf> {
    let path = dirs::home_dir()?.join(".config/zed");
    path.exists().then_some(path)
}
