mod collect;
mod json;
mod parsers;

pub(crate) use collect::CollectedImport;

use crate::domain::errors::AppError;
use std::path::{Path, PathBuf};

pub(crate) fn collect_custom_csv_file(path: &Path) -> Result<CollectedImport, AppError> {
    collect::collect_custom_csv_file(path)
}

pub(crate) fn collect_zed_keymap_file(path: &Path) -> Result<CollectedImport, AppError> {
    collect::collect_zed_keymap_file(path)
}

pub(crate) fn collect_vscode_export_file(path: &Path) -> Result<CollectedImport, AppError> {
    collect::collect_vscode_export_file(path)
}

pub(crate) fn collect_vscode_extension_manifests(paths: &[PathBuf]) -> Result<CollectedImport, AppError> {
    collect::collect_vscode_extension_manifests(paths)
}

pub(crate) fn collect_idea_keymap_file(
    path: &Path,
    parent_lookup: &dyn Fn(&str) -> Option<String>,
) -> Result<CollectedImport, AppError> {
    collect::collect_idea_keymap_file(path, parent_lookup)
}
