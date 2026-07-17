use std::fs;
use std::path::Path;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Serialize;

const HOST_ID: &str = "com.khukri.host";
const PLACEHOLDER_ORIGIN: &str = "chrome-extension://replace-with-your-extension-id/";

#[derive(Debug, Serialize)]
struct HostManifest {
    name: String,
    description: String,
    path: String,
    #[serde(rename = "type")]
    host_type: String,
    allowed_origins: Vec<String>,
}

pub fn requested(args: &[String]) -> bool {
    args.iter()
        .any(|arg| arg == "--register" || arg == "--repair")
}

pub fn register(binary_path: &Path) -> Result<()> {
    register_for_platform(binary_path)
}

fn extension_origin_from_env() -> Result<String> {
    let origin =
        std::env::var("KHUKRI_EXTENSION_ORIGIN").unwrap_or_else(|_| PLACEHOLDER_ORIGIN.to_string());
    validate_extension_origin(&origin)?;
    Ok(origin)
}

fn validate_extension_origin(origin: &str) -> Result<()> {
    if origin == PLACEHOLDER_ORIGIN || origin.contains("replace-with-your-extension-id") {
        anyhow::bail!(
            "KHUKRI_EXTENSION_ORIGIN is not set. \
             Set it to your extension's chrome-extension://<id>/ origin before registering."
        );
    }
    if !origin.starts_with("chrome-extension://") && !origin.starts_with("moz-extension://") {
        anyhow::bail!(
            "KHUKRI_EXTENSION_ORIGIN must start with chrome-extension:// or moz-extension://, got: {origin}"
        );
    }
    Ok(())
}

fn native_host_manifest(binary_path: &Path) -> Result<HostManifest> {
    Ok(HostManifest {
        name: HOST_ID.to_string(),
        description: "Khukri Native Messaging Host".to_string(),
        path: binary_path.display().to_string(),
        host_type: "stdio".to_string(),
        allowed_origins: vec![extension_origin_from_env()?],
    })
}

fn write_manifest(path: &Path, binary_path: &Path) -> Result<()> {
    let manifest = serde_json::to_vec_pretty(&native_host_manifest(binary_path)?)?;
    fs::write(path, manifest)?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn register_for_platform(binary_path: &Path) -> Result<()> {
    let bridge_dir = binary_path
        .parent()
        .context("bridge binary has no parent directory")?;
    let manifest_path = bridge_dir.join(format!("{HOST_ID}.json"));
    write_manifest(&manifest_path, binary_path)?;

    let reg_key = format!(r"HKCU\Software\Google\Chrome\NativeMessagingHosts\{HOST_ID}");
    let status = std::process::Command::new("reg")
        .args([
            "add",
            &reg_key,
            "/ve",
            "/t",
            "REG_SZ",
            "/d",
            &manifest_path.display().to_string(),
            "/f",
        ])
        .status()
        .context("failed to launch reg.exe")?;

    if !status.success() {
        anyhow::bail!("failed to register native host in Windows registry");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn register_for_platform(binary_path: &Path) -> Result<()> {
    let config_dir = home_dir()?
        .join(".config")
        .join("google-chrome")
        .join("NativeMessagingHosts");
    fs::create_dir_all(&config_dir)?;
    write_manifest(&config_dir.join(format!("{HOST_ID}.json")), binary_path)
}

#[cfg(target_os = "macos")]
fn register_for_platform(binary_path: &Path) -> Result<()> {
    let config_dir = home_dir()?
        .join("Library")
        .join("Application Support")
        .join("Google")
        .join("Chrome")
        .join("NativeMessagingHosts");
    fs::create_dir_all(&config_dir)?;
    write_manifest(&config_dir.join(format!("{HOST_ID}.json")), binary_path)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn home_dir() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn register_for_platform(_binary_path: &Path) -> Result<()> {
    anyhow::bail!("native host registration is not implemented for this platform")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_origin_is_rejected() {
        assert!(validate_extension_origin(PLACEHOLDER_ORIGIN).is_err());
        assert!(validate_extension_origin(
            "chrome-extension://replace-with-your-extension-id/extra"
        )
        .is_err());
    }

    #[test]
    fn valid_chrome_origin_is_accepted() {
        assert!(
            validate_extension_origin("chrome-extension://abcdefghijklmnopabcdefghijklmnop/")
                .is_ok()
        );
    }

    #[test]
    fn valid_moz_origin_is_accepted() {
        assert!(validate_extension_origin("moz-extension://some-uuid/").is_ok());
    }

    #[test]
    fn http_origin_is_rejected() {
        assert!(validate_extension_origin("https://evil.com/").is_err());
    }
}
