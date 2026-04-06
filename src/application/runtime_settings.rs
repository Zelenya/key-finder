use crate::domain::errors::AppError;
use humantime::parse_duration;
use std::time::Duration;

pub(crate) const DEFAULT_NOTIFY_INTERVAL: Duration = Duration::from_hours(1);

pub(crate) fn resolve_notify_interval(
    cli_value: Option<&str>,
    env_value: Option<&str>,
    db_value: Option<&str>,
) -> Result<Duration, AppError> {
    resolve_runtime_setting(cli_value, env_value, db_value)
        .as_deref()
        .map_or_else(|| Ok(DEFAULT_NOTIFY_INTERVAL), parse_notify_interval)
}

pub(crate) fn resolve_terminal_notifier_path(
    cli_value: Option<&str>,
    env_value: Option<&str>,
    db_value: Option<&str>,
) -> Option<String> {
    resolve_runtime_setting(cli_value, env_value, db_value)
}

pub(crate) fn parse_notify_interval(value: &str) -> Result<Duration, AppError> {
    if let Ok(seconds) = value.parse::<u64>() {
        if seconds == 0 {
            return Err(AppError::Config(
                "notification interval must be greater than 0 seconds".to_string(),
            ));
        }
        return Ok(Duration::from_secs(seconds));
    }

    let parsed = parse_duration(value)
        .map_err(|e| AppError::Config(format!("invalid notification interval '{value}': {e}")))?;
    if parsed.is_zero() {
        return Err(AppError::Config(
            "notification interval must be greater than 0 seconds".to_string(),
        ));
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
    use super::{resolve_notify_interval, resolve_terminal_notifier_path};
    use std::time::Duration;

    #[test]
    fn env_beats_db_when_cli_is_missing() {
        assert_eq!(
            resolve_notify_interval(None, Some("45s"), Some("30m")).expect("interval"),
            Duration::from_secs(45)
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
            resolve_notify_interval(None, Some("45s"), None).expect("interval"),
            Duration::from_secs(45)
        );
        assert_eq!(
            resolve_terminal_notifier_path(None, Some("/env/terminal-notifier"), None).as_deref(),
            Some("/env/terminal-notifier")
        );
    }

    #[test]
    fn cli_beats_db_and_env() {
        assert_eq!(
            resolve_notify_interval(Some("15m"), Some("45s"), Some("30m")).expect("interval"),
            Duration::from_secs(15 * 60)
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
            resolve_notify_interval(None, Some("1h"), Some("   ")).expect("interval"),
            Duration::from_secs(60 * 60)
        );
        assert_eq!(
            resolve_terminal_notifier_path(None, Some("/env/path"), Some("")).as_deref(),
            Some("/env/path")
        );
    }
}
