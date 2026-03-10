pub(crate) fn app_names_match(expected_app: &str, current_app: &str) -> bool {
    expected_app.eq_ignore_ascii_case(current_app)
        || normalize_app_name(expected_app) == normalize_app_name(current_app)
}

pub(crate) fn app_matches_any(expected_names: &[String], current_app: &str) -> bool {
    expected_names.iter().any(|expected| app_names_match(expected, current_app))
}

pub(crate) fn normalize_app_name(name: &str) -> String {
    name.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{app_matches_any, app_names_match};

    #[test]
    fn matches_expected_names() {
        assert!(app_names_match("zed", "Zed"));
        assert!(app_names_match("zed", "zed"));
        assert!(app_names_match("Zed", "zed"));
    }

    #[test]
    fn matches_normalized_names() {
        assert!(app_names_match("Visual Studio Code", "visual studio code"));
        assert!(app_names_match("Zed", "zed"));
    }

    #[test]
    fn does_not_match_alias_variants() {
        assert!(!app_names_match("VSCode", "Visual Studio Code"));
        assert!(!app_names_match("Code", "Visual Studio Code"));
        assert!(!app_names_match("PyCharm", "IntelliJ IDEA"));
    }

    #[test]
    fn does_not_match_different_apps() {
        assert!(!app_names_match("zed", "safari"));
        assert!(!app_names_match("chrome", "brave"));
    }

    #[test]
    fn matches_any_alias_name() {
        let names = vec!["Acme Studio".to_string(), "Acme".to_string()];
        assert!(app_matches_any(&names, "Acme"));
        assert!(app_matches_any(&names, "Acme Studio"));
        assert!(!app_matches_any(&names, "Safari"));
    }
}
