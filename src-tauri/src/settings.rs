use serde::{Deserialize, Serialize};
use std::io::Write;

use crate::bootstrap::app_data_dir;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeneralSettings {
    pub(crate) default_download_path: String,
    pub(crate) max_concurrent: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PerformanceSettings {
    pub(crate) thread_override: Option<u8>,
    pub(crate) bandwidth_cap: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SchedulerSettings {
    pub(crate) enabled: bool,
    pub(crate) start_hour: u8,
    pub(crate) end_hour: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProxySettings {
    pub(crate) enabled: bool,
    pub(crate) url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppearanceSettings {
    pub(crate) theme: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppSettings {
    pub(crate) general: GeneralSettings,
    pub(crate) performance: PerformanceSettings,
    pub(crate) scheduler: SchedulerSettings,
    pub(crate) proxy: ProxySettings,
    pub(crate) appearance: AppearanceSettings,
    #[serde(default, rename = "browser_session")]
    pub(crate) browser_session: Option<String>,
    #[serde(default, rename = "onboarding_complete")]
    pub(crate) onboarding_complete: bool,
    #[serde(default, rename = "ytdlp_auto_update")]
    pub(crate) ytdlp_auto_update: bool,
    #[serde(default, rename = "ytdlp_last_check")]
    pub(crate) ytdlp_last_check: Option<i64>,
    #[serde(default, rename = "ytdlp_version")]
    pub(crate) ytdlp_version: Option<String>,
    #[serde(default, rename = "ytdlp_last_notified_failure")]
    pub(crate) ytdlp_last_notified_failure: Option<String>,
    #[serde(default, rename = "ytdlp_last_rate_limit")]
    pub(crate) ytdlp_last_rate_limit: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            general: GeneralSettings {
                default_download_path: app_data_dir().join("downloads").display().to_string(),
                max_concurrent: 3,
            },
            performance: PerformanceSettings {
                thread_override: None,
                bandwidth_cap: None,
            },
            scheduler: SchedulerSettings {
                enabled: false,
                start_hour: 0,
                end_hour: 23,
            },
            proxy: ProxySettings {
                enabled: false,
                url: String::new(),
            },
            appearance: AppearanceSettings {
                theme: "system".to_string(),
            },
            browser_session: None,
            onboarding_complete: false,
            ytdlp_auto_update: true,
            ytdlp_last_check: None,
            ytdlp_version: None,
            ytdlp_last_notified_failure: None,
            ytdlp_last_rate_limit: false,
        }
    }
}

pub(crate) fn load() -> AppSettings {
    let contents = match std::fs::read_to_string(path()) {
        Ok(contents) => contents,
        Err(_) => return AppSettings::default(),
    };

    let mut settings = serde_json::from_str(&contents).unwrap_or_else(|_| AppSettings::default());
    normalize(&mut settings);
    settings
}

pub(crate) fn save(settings: &AppSettings) -> Result<(), String> {
    validate(settings)?;

    let path = path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let json = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(path.parent().expect("settings path has parent"))
            .map_err(|error| error.to_string())?;
    temporary
        .write_all(json.as_bytes())
        .map_err(|error| error.to_string())?;
    temporary.flush().map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    temporary
        .persist(&path)
        .map_err(|error| format!("failed to replace settings file: {}", error.error))?;
    Ok(())
}

fn validate(settings: &AppSettings) -> Result<(), String> {
    if settings.general.default_download_path.trim().is_empty() {
        return Err("default download path must not be empty".to_string());
    }
    if !(1..=16).contains(&settings.general.max_concurrent) {
        return Err("max concurrent downloads must be between 1 and 16".to_string());
    }
    if let Some(threads) = settings.performance.thread_override {
        if !(1..=128).contains(&threads) {
            return Err("worker threads must be between 1 and 128".to_string());
        }
    }
    if settings.scheduler.start_hour > 23 || settings.scheduler.end_hour > 23 {
        return Err("scheduler hours must be between 0 and 23".to_string());
    }
    if settings.proxy.enabled {
        let url = settings.proxy.url.trim();
        if !(url.starts_with("http://")
            || url.starts_with("https://")
            || url.starts_with("socks5://"))
        {
            return Err("proxy URL must start with http://, https://, or socks5://".to_string());
        }
    }
    if !matches!(
        settings.appearance.theme.as_str(),
        "system" | "light" | "dark"
    ) {
        return Err("unsupported appearance theme".to_string());
    }
    if !supported_browser_session(settings.browser_session.as_deref()) {
        return Err("unsupported browser session selection".to_string());
    }
    Ok(())
}

fn normalize(settings: &mut AppSettings) {
    let defaults = AppSettings::default();
    if settings.general.default_download_path.trim().is_empty() {
        settings.general.default_download_path = defaults.general.default_download_path;
    }
    settings.general.max_concurrent = settings.general.max_concurrent.clamp(1, 16);
    if settings
        .performance
        .thread_override
        .is_some_and(|threads| !(1..=128).contains(&threads))
    {
        settings.performance.thread_override = None;
    }
    settings.scheduler.start_hour = settings.scheduler.start_hour.min(23);
    settings.scheduler.end_hour = settings.scheduler.end_hour.min(23);
    if !matches!(
        settings.appearance.theme.as_str(),
        "system" | "light" | "dark"
    ) {
        settings.appearance.theme = defaults.appearance.theme;
    }
    if !supported_browser_session(settings.browser_session.as_deref()) {
        settings.browser_session = None;
    }
    if settings.proxy.enabled
        && !(settings.proxy.url.starts_with("http://")
            || settings.proxy.url.starts_with("https://")
            || settings.proxy.url.starts_with("socks5://"))
    {
        settings.proxy.enabled = false;
        settings.proxy.url.clear();
    }
}

fn supported_browser_session(value: Option<&str>) -> bool {
    matches!(
        value,
        None | Some(
            "brave"
                | "chrome"
                | "chromium"
                | "edge"
                | "firefox"
                | "opera"
                | "safari"
                | "vivaldi"
                | "whale"
        )
    )
}

#[allow(clippy::items_after_test_module)]
#[cfg(test)]
mod tests {
    use super::{supported_browser_session, validate, AppSettings};

    #[test]
    fn browser_session_values_are_allowlisted() {
        assert!(supported_browser_session(None));
        assert!(supported_browser_session(Some("chrome")));
        assert!(!supported_browser_session(Some("chrome:../../secret")));
    }

    #[test]
    fn browser_session_uses_stable_settings_key() {
        let settings = AppSettings {
            browser_session: Some("chrome".to_string()),
            ..AppSettings::default()
        };
        let json = serde_json::to_value(settings).expect("settings should serialize");
        assert_eq!(json["browser_session"], "chrome");
        assert!(json.get("browserSession").is_none());
    }

    #[test]
    fn invalid_queue_and_proxy_settings_are_rejected() {
        let mut settings = AppSettings::default();
        settings.general.max_concurrent = 0;
        assert!(validate(&settings).is_err());

        settings.general.max_concurrent = 3;
        settings.proxy.enabled = true;
        settings.proxy.url = "not-a-proxy".to_string();
        assert!(validate(&settings).is_err());
    }
}

fn path() -> std::path::PathBuf {
    app_data_dir().join("settings.json")
}
