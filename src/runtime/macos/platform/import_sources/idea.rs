use crate::domain::errors::AppError;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use zip::ZipArchive;

/// Finds user's local keymap file
pub(crate) fn preferred_keymap_file() -> Result<Option<PathBuf>, AppError> {
    let Some(home) = dirs::home_dir() else {
        return Err(AppError::Config("failed to determine home directory".to_string()));
    };
    best_local_keymap_file_from_home(&home)
}

/// Finds the expected keymap directory (used if the keymap file is not found)
pub(crate) fn preferred_keymap_directory() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    preferred_keymap_directory_from_home(&home)
}

/// When parsing the selected XML, we may need to look up the parent keymap files
pub(crate) fn load_parent_keymap(parent_name: &str) -> Option<String> {
    let jars = candidate_idea_app_jars_from_system();
    let entry = format!("keymaps/{parent_name}.xml");
    for jar in jars {
        let file = match fs::File::open(jar) {
            Ok(file) => file,
            Err(_) => continue,
        };
        let mut archive = match ZipArchive::new(file) {
            Ok(archive) => archive,
            Err(_) => continue,
        };
        let mut xml = match archive.by_name(&entry) {
            Ok(xml) => xml,
            Err(_) => continue,
        };

        let mut content = String::new();
        if xml.read_to_string(&mut content).is_ok() && !content.is_empty() {
            return Some(content);
        }
    }
    None
}

fn best_local_keymap_file_from_home(home: &Path) -> Result<Option<PathBuf>, AppError> {
    let mut files = find_local_keymap_files(home)?;
    files.sort_by(|left, right| compare_keymap_candidates(left, right));
    Ok(files.into_iter().next())
}

fn find_local_keymap_files(home: &Path) -> Result<Vec<PathBuf>, AppError> {
    let mut files = Vec::new();
    for keymaps in local_keymap_directories(home)? {
        let entries = fs::read_dir(&keymaps).map_err(|source| AppError::ReadImporterFile {
            path: keymaps.to_path_buf(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| AppError::ReadImporterFile {
                path: keymaps.to_path_buf(),
                source,
            })?;
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "xml") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn preferred_keymap_directory_from_home(home: &Path) -> Option<PathBuf> {
    if let Ok(Some(path)) = best_local_keymap_file_from_home(home) {
        return path.parent().map(Path::to_path_buf);
    }

    local_keymap_directories(home)
        .ok()?
        .into_iter()
        .next()
        .or_else(|| Some(jetbrains_root_directory(home)))
}

fn jetbrains_root_directory(home: &Path) -> PathBuf {
    home.join("Library/Application Support/JetBrains")
}

fn candidate_idea_app_jars_from_system() -> Vec<PathBuf> {
    let mut app_roots = vec![PathBuf::from("/Applications")];
    if let Some(home) = dirs::home_dir() {
        app_roots.push(home.join("Applications"));
    }

    candidate_idea_app_jars(&app_roots)
}

fn candidate_idea_app_jars(app_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut jars = Vec::new();

    for root in app_roots {
        for app_name in ["IntelliJ IDEA.app", "IntelliJ IDEA CE.app"] {
            let jar = root.join(app_name).join("Contents/lib/app.jar");
            if jar.exists() {
                jars.push(jar);
            }
        }
    }
    jars
}

fn local_keymap_directories(home: &Path) -> Result<Vec<PathBuf>, AppError> {
    let root = home.join("Library/Application Support/JetBrains");
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut result = Vec::new();
    let entries = fs::read_dir(&root).map_err(|source| AppError::ReadImporterFile {
        path: root.clone(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| AppError::ReadImporterFile {
            path: root.clone(),
            source,
        })?;
        let product = entry.path();
        if !product.is_dir() {
            continue;
        }
        let Some(name) = product.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !["IntelliJIdea", "IdeaIC"].iter().any(|prefix| name.starts_with(prefix)) {
            continue;
        }

        let keymaps = product.join("keymaps");
        if keymaps.exists() {
            result.push(keymaps);
        }
    }
    result.sort();
    Ok(result)
}

fn compare_keymap_candidates(left: &Path, right: &Path) -> std::cmp::Ordering {
    let left_modified =
        fs::metadata(left).and_then(|metadata| metadata.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
    let right_modified =
        fs::metadata(right).and_then(|metadata| metadata.modified()).unwrap_or(SystemTime::UNIX_EPOCH);

    right_modified
        .cmp(&left_modified)
        .then_with(|| left.to_string_lossy().cmp(&right.to_string_lossy()))
}

#[cfg(test)]
mod tests {
    use super::{
        best_local_keymap_file_from_home, candidate_idea_app_jars, find_local_keymap_files,
        preferred_keymap_directory_from_home,
    };
    use std::fs;
    use std::thread;
    use std::time::Duration;
    use tempfile::tempdir;

    #[test]
    fn finds_parent_keymap_jars_for_idea_only() {
        let dir = tempdir().expect("temp dir");
        let apps_root = dir.path().join("Applications");

        fs::create_dir_all(apps_root.join("IntelliJ IDEA.app/Contents/lib")).expect("create idea app");
        fs::write(apps_root.join("IntelliJ IDEA.app/Contents/lib/app.jar"), "").expect("write idea jar");

        fs::create_dir_all(apps_root.join("DataGrip.app/Contents/lib")).expect("create datagrip app");
        fs::write(apps_root.join("DataGrip.app/Contents/lib/app.jar"), "").expect("write datagrip jar");

        fs::create_dir_all(apps_root.join("PyCharm.app/Contents/lib")).expect("create pycharm app");
        fs::write(apps_root.join("PyCharm.app/Contents/lib/app.jar"), "").expect("write pycharm jar");

        let jars = candidate_idea_app_jars(&[apps_root]);
        assert_eq!(jars.len(), 1);
        assert!(jars[0].ends_with("IntelliJ IDEA.app/Contents/lib/app.jar"));
    }

    #[test]
    fn finds_local_keymaps_for_idea_only() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("Library/Application Support/JetBrains");

        fs::create_dir_all(root.join("IntelliJIdea2025.3/keymaps")).expect("create idea dir");
        fs::write(
            root.join("IntelliJIdea2025.3/keymaps/macOS copy.xml"),
            "<keymap />",
        )
        .expect("write idea keymap");

        fs::create_dir_all(root.join("IdeaIC2025.2/keymaps")).expect("create idea ce dir");
        fs::write(root.join("IdeaIC2025.2/keymaps/Default.xml"), "<keymap />").expect("write idea ce keymap");

        fs::create_dir_all(root.join("PyCharm2025.1/keymaps")).expect("create pycharm dir");
        fs::write(root.join("PyCharm2025.1/keymaps/PyCharm.xml"), "<keymap />")
            .expect("write pycharm keymap");

        let files = find_local_keymap_files(dir.path()).expect("find local keymaps");
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|path| path.ends_with("IntelliJIdea2025.3/keymaps/macOS copy.xml")));
        assert!(files.iter().any(|path| path.ends_with("IdeaIC2025.2/keymaps/Default.xml")));
        assert!(!files.iter().any(|path| path.ends_with("PyCharm2025.1/keymaps/PyCharm.xml")));
    }

    #[test]
    fn prefers_most_recent_local_keymap_file() {
        let dir = tempdir().expect("temp dir");
        let keymaps = dir.path().join("Library/Application Support/JetBrains/IntelliJIdea2025.3/keymaps");
        fs::create_dir_all(&keymaps).expect("create keymaps dir");

        let older = keymaps.join("older.xml");
        fs::write(&older, "<keymap />").expect("write older");
        thread::sleep(Duration::from_millis(20));
        let newer = keymaps.join("newer.xml");
        fs::write(&newer, "<keymap />").expect("write newer");

        let best = best_local_keymap_file_from_home(dir.path()).expect("best keymap");
        assert_eq!(best, Some(newer));
    }

    #[test]
    fn preferred_directory_uses_best_local_keymap_parent() {
        let dir = tempdir().expect("temp dir");
        let keymaps = dir.path().join("Library/Application Support/JetBrains/IdeaIC2025.2/keymaps");
        fs::create_dir_all(&keymaps).expect("create keymaps dir");
        fs::write(keymaps.join("Default.xml"), "<keymap />").expect("write keymap");

        let preferred = preferred_keymap_directory_from_home(dir.path()).expect("preferred keymap dir");
        assert_eq!(preferred, keymaps);
    }

    #[test]
    fn preferred_directory_falls_back_to_jetbrains_root() {
        let dir = tempdir().expect("temp dir");
        let root = dir.path().join("Library/Application Support/JetBrains");
        fs::create_dir_all(&root).expect("create jetbrains root");

        let preferred = preferred_keymap_directory_from_home(dir.path()).expect("preferred keymap dir");
        assert_eq!(preferred, root);
    }
}
