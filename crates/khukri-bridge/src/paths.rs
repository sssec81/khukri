use std::path::{Path, PathBuf};

pub fn downloads_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            return PathBuf::from(profile).join("Downloads");
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join("Downloads");
        }
    }

    std::env::temp_dir().join("khukri-downloads")
}

pub fn app_data_dir() -> PathBuf {
    if let Some(explicit) = std::env::var_os("KHUKRI_DATA_DIR") {
        return PathBuf::from(explicit);
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local_app_data).join("Khukri");
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(data_home).join("khukri");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("khukri");
        }
    }

    std::env::temp_dir().join("khukri-data")
}

pub fn sqlite_url(path: &Path) -> String {
    format!("sqlite:{}?mode=rwc", path.display())
}
