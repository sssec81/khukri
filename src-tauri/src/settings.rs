use serde::{Deserialize, Serialize};

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
    if !supported_browser_session(settings.browser_session.as_deref()) {
        settings.browser_session = None;
    }
    settings
}

pub(crate) fn save(settings: &AppSettings) -> Result<(), String> {
    if !supported_browser_session(settings.browser_session.as_deref()) {
        return Err("unsupported browser session selection".to_string());
    }

    let path = path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let json = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    std::fs::write(path, json).map_err(|error| error.to_string())
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

#[cfg(test)]
mod tests {
    use super::{supported_browser_session, AppSettings};

    #[test]
    fn browser_session_values_are_allowlisted() {
        assert!(supported_browser_session(None));
        assert!(supported_browser_session(Some("chrome")));
        assert!(!supported_browser_session(Some("chrome:../../secret")));
    }

    #[test]
    fn browser_session_uses_stable_settings_key() {
        let mut settings = AppSettings::default();
        settings.browser_session = Some("chrome".to_string());
        let json = serde_json::to_value(settings).expect("settings should serialize");
        assert_eq!(json["browser_session"], "chrome");
        assert!(json.get("browserSession").is_none());
    }
}

fn path() -> std::path::PathBuf {
    app_data_dir().join("settings.json")
}
