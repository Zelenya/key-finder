use crate::domain::errors::AppError;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn preferred_export_directory() -> Option<PathBuf> {
    let path = dirs::home_dir()?.join("Library/Application Support/Code/User");
    path.exists().then_some(path)
}

pub(crate) fn find_extension_manifest_files() -> Result<Vec<PathBuf>, AppError> {
    let Some(home) = dirs::home_dir() else {
        return Err(AppError::Config("failed to determine home directory".to_string()));
    };
    find_extension_manifest_files_from_home(&home)
}

fn find_extension_manifest_files_from_home(home: &Path) -> Result<Vec<PathBuf>, AppError> {
    let mut files = user_extension_package_files(home)?;

    for app_root in candidate_vscode_app_roots(home) {
        let ext_root = app_root.join("Contents/Resources/app/extensions");
        if ext_root.exists() {
            files.extend(read_extension_package_files(&ext_root)?);
        }
    }

    files.sort();
    Ok(files)
}

fn user_extension_package_files(home: &Path) -> Result<Vec<PathBuf>, AppError> {
    let mut out = Vec::new();
    let extension_roots = [
        home.join(".vscode/extensions"),
        home.join(".vscode-insiders/extensions"),
        home.join(".vscode-oss/extensions"),
        home.join(".vscodium/extensions"),
    ];

    for root in extension_roots {
        if root.exists() {
            out.extend(read_extension_package_files(&root)?);
        }
    }

    Ok(out)
}

fn candidate_vscode_app_roots(home: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let app_roots = [PathBuf::from("/Applications"), home.join("Applications")];
    let app_names = [
        "Visual Studio Code.app",
        "Visual Studio Code - Insiders.app",
        "VSCodium.app",
    ];

    for root in app_roots {
        for app_name in app_names {
            let path = root.join(app_name);
            if path.exists() {
                roots.push(path);
            }
        }
    }

    roots
}

fn read_extension_package_files(ext_root: &Path) -> Result<Vec<PathBuf>, AppError> {
    let mut result = Vec::new();
    let entries = fs::read_dir(ext_root).map_err(|source| AppError::ReadImporterFile {
        path: ext_root.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| AppError::ReadImporterFile {
            path: ext_root.to_path_buf(),
            source,
        })?;
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let manifest = dir.join("package.json");
        if manifest.exists() && might_have_keybindings(&manifest)? {
            result.push(manifest);
        }
    }
    Ok(result)
}

fn might_have_keybindings(path: &Path) -> Result<bool, AppError> {
    let content = fs::read_to_string(path).map_err(|source| AppError::ReadImporterFile {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(content.contains("\"keybindings\""))
}

#[cfg(test)]
mod tests {
    use super::{find_extension_manifest_files_from_home, user_extension_package_files};
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn finds_only_extension_manifests() {
        let dir = tempdir().expect("temp dir");
        let home = dir.path();
        let user_manifest = home.join(".vscode/extensions/sample/package.json");
        let bundled_manifest = home.join(
            "Applications/Visual Studio Code.app/Contents/Resources/app/extensions/builtin/package.json",
        );
        let local_keybindings = home.join("Library/Application Support/Code/User/keybindings.json");
        let profile_keybindings =
            home.join("Library/Application Support/Code/User/profiles/default/keybindings.json");
        let default_export = home.join("Library/Application Support/Code/User/default-keybindings.json");

        fs::create_dir_all(home.join(".vscode/extensions/sample")).expect("create user extension dir");
        fs::write(
            &user_manifest,
            r#"{ "contributes": { "keybindings": [{ "command": "sample.run", "mac": "cmd+r" }] } }"#,
        )
        .expect("write extension manifest");

        fs::create_dir_all(
            home.join("Applications/Visual Studio Code.app/Contents/Resources/app/extensions/builtin"),
        )
        .expect("create bundled extension dir");
        fs::write(
            &bundled_manifest,
            r#"{ "contributes": { "keybindings": [{ "command": "builtin.run", "mac": "cmd+b" }] } }"#,
        )
        .expect("write bundled manifest");

        fs::create_dir_all(home.join("Library/Application Support/Code/User/profiles/default"))
            .expect("create profile dir");
        fs::write(
            &local_keybindings,
            r#"[{ "key": "cmd+k", "command": "ignored.user" }]"#,
        )
        .expect("write local keybindings");
        fs::write(
            &profile_keybindings,
            r#"[{ "key": "cmd+p", "command": "ignored.profile" }]"#,
        )
        .expect("write profile keybindings");
        fs::write(
            &default_export,
            r#"[{ "key": "cmd+d", "command": "ignored.default" }]"#,
        )
        .expect("write default export");

        let files = find_extension_manifest_files_from_home(home).expect("find extension manifests");
        assert!(files.contains(&user_manifest));
        assert!(files.contains(&bundled_manifest));
        assert!(!files.contains(&local_keybindings));
        assert!(!files.contains(&profile_keybindings));
        assert!(!files.contains(&default_export));
        assert!(files
            .iter()
            .all(|path| path.file_name().and_then(|name| name.to_str()) == Some("package.json")));
    }

    #[test]
    fn user_only_extension_manifest_scan_skips_bundled_apps() {
        let dir = tempdir().expect("temp dir");
        let home = dir.path();
        let user_manifest = home.join(".vscode/extensions/sample/package.json");
        let bundled_manifest = home.join(
            "Applications/Visual Studio Code.app/Contents/Resources/app/extensions/builtin/package.json",
        );

        fs::create_dir_all(user_manifest.parent().expect("parent")).expect("create user extension dir");
        fs::write(
            &user_manifest,
            r#"{ "contributes": { "keybindings": [{ "command": "sample.run", "mac": "cmd+r" }] } }"#,
        )
        .expect("write user manifest");

        fs::create_dir_all(bundled_manifest.parent().expect("parent")).expect("create bundled extension dir");
        fs::write(
            &bundled_manifest,
            r#"{ "contributes": { "keybindings": [{ "command": "builtin.run", "mac": "cmd+b" }] } }"#,
        )
        .expect("write bundled manifest");

        let files = find_extension_manifest_files_user_only(home);

        assert_eq!(files, vec![user_manifest]);
        assert!(!files.contains(&bundled_manifest));
    }

    fn find_extension_manifest_files_user_only(home: &Path) -> Vec<std::path::PathBuf> {
        let mut files = user_extension_package_files(home).expect("find user manifests");
        files.sort();
        files
    }
}
