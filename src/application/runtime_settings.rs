use crate::domain::errors::AppError;
use humantime::parse_duration;
use std::time::Duration;

pub(crate) const DEFAULT_COOLDOWN: Duration = Duration::from_secs(10 * 60);
pub(crate) const DEFAULT_APP_SWITCH_BOUNCE: Duration = Duration::from_secs(30);

pub(crate) fn resolve_cooldown(
    cli_value: Option<&str>,
    env_value: Option<&str>,
    db_value: Option<&str>,
) -> Result<Duration, AppError> {
    resolve_runtime_setting(cli_value, env_value, db_value).as_deref().map_or_else(
        || Ok(DEFAULT_COOLDOWN),
        |value| parse_duration_setting("cooldown", value),
    )
}

pub(crate) fn resolve_app_switch_bounce(
    cli_value: Option<&str>,
    env_value: Option<&str>,
    db_value: Option<&str>,
) -> Result<Duration, AppError> {
    resolve_runtime_setting(cli_value, env_value, db_value).as_deref().map_or_else(
        || Ok(DEFAULT_APP_SWITCH_BOUNCE),
        |value| parse_duration_setting("app switch bounce", value),
    )
}

pub(crate) fn resolve_terminal_notifier_path(
    cli_value: Option<&str>,
    env_value: Option<&str>,
    db_value: Option<&str>,
) -> Option<String> {
    resolve_runtime_setting(cli_value, env_value, db_value)
}

pub(crate) fn parse_duration_setting(setting_name: &str, value: &str) -> Result<Duration, AppError> {
    if let Ok(seconds) = value.parse::<u64>() {
        if seconds == 0 {
            return Err(AppError::Config(format!(
                "{setting_name} must be greater than 0 seconds"
            )));
        }
        return Ok(Duration::from_secs(seconds));
    }

    let parsed = parse_duration(value).map_err(|source| AppError::InvalidDurationSetting {
        setting: setting_name.to_string(),
        value: value.to_string(),
        source,
    })?;
    if parsed.is_zero() {
        return Err(AppError::Config(format!(
            "{setting_name} must be greater than 0 seconds"
        )));
    }
    Ok(parsed)
}

fn resolve_runtime_setting(
    cli_value: Option<&str>,
    env_value: Option<&str>,
    db_value: Option<&str>,
) -> Option<String> {
    sanitize_setting_value(cli_value)
        .or_else(|| sanitize_setting_value(env_value))
        .or_else(|| sanitize_setting_value(db_value))
}

fn sanitize_setting_value(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|value| !value.is_empty()).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::{resolve_app_switch_bounce, resolve_cooldown, resolve_terminal_notifier_path};
    use std::time::Duration;

    #[test]
    fn env_beats_db_when_cli_is_missing() {
        assert_eq!(
            resolve_cooldown(None, Some("45s"), Some("30m")).expect("cooldown"),
            Duration::from_secs(45)
        );
        assert_eq!(
            resolve_app_switch_bounce(None, Some("15s"), Some("30s")).expect("bounce"),
            Duration::from_secs(15)
        );
        assert_eq!(
            resolve_terminal_notifier_path(
                None,
                Some("/env/terminal-notifier"),
                Some("/db/terminal-notifier")
            )
            .as_deref(),
            Some("/env/terminal-notifier")
        );
    }

    #[test]
    fn env_applies_when_db_setting_is_missing() {
        assert_eq!(
            resolve_cooldown(None, Some("45s"), None).expect("cooldown"),
            Duration::from_secs(45)
        );
        assert_eq!(
            resolve_app_switch_bounce(None, Some("15s"), None).expect("bounce"),
            Duration::from_secs(15)
        );
        assert_eq!(
            resolve_terminal_notifier_path(None, Some("/env/terminal-notifier"), None).as_deref(),
            Some("/env/terminal-notifier")
        );
    }

    #[test]
    fn cli_beats_db_and_env() {
        assert_eq!(
            resolve_cooldown(Some("15m"), Some("45s"), Some("30m")).expect("cooldown"),
            Duration::from_secs(15 * 60)
        );
        assert_eq!(
            resolve_app_switch_bounce(Some("20s"), Some("45s"), Some("30s")).expect("bounce"),
            Duration::from_secs(20)
        );
        assert_eq!(
            resolve_terminal_notifier_path(
                Some("/cli/terminal-notifier"),
                Some("/env/terminal-notifier"),
                Some("/db/terminal-notifier"),
            )
            .as_deref(),
            Some("/cli/terminal-notifier")
        );
    }

    #[test]
    fn empty_strings_are_treated_as_missing() {
        assert_eq!(
            resolve_cooldown(None, Some("1h"), Some("   ")).expect("cooldown"),
            Duration::from_secs(60 * 60)
        );
        assert_eq!(
            resolve_app_switch_bounce(None, Some("30s"), Some("   ")).expect("bounce"),
            Duration::from_secs(30)
        );
        assert_eq!(
            resolve_terminal_notifier_path(None, Some("/env/path"), Some("")).as_deref(),
            Some("/env/path")
        );
    }
}
