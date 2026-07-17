use std::path::Path;
use std::process::Command;

const BETA_EXTENSION_ORIGIN: &str = "chrome-extension://hlingdbecfefhglkbballggindegcmik/";

pub fn register_bundled() -> Result<(), String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("failed to resolve Khukri executable: {error}"))?;
    let directory = executable
        .parent()
        .ok_or_else(|| "Khukri executable has no parent directory".to_string())?;
    let bridge = bridge_names()
        .iter()
        .map(|name| directory.join(name))
        .find(|path| path.is_file())
        .ok_or_else(|| missing_bridge_message(&executable))?;

    let output = Command::new(&bridge)
        .arg("--repair")
        .env("KHUKRI_EXTENSION_ORIGIN", BETA_EXTENSION_ORIGIN)
        .output()
        .map_err(|error| format!("failed to launch {}: {error}", bridge.display()))?;

    if output.status.success() {
        tracing::info!(bridge = %bridge.display(), "native messaging host registered");
        return Ok(());
    }

    Err(format!(
        "native host registration failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn missing_bridge_message(executable: &Path) -> String {
    format!(
        "bundled native bridge was not found next to {}",
        executable.display()
    )
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn bridge_names() -> [&'static str; 2] {
    [
        "khukri-bridge.exe",
        "khukri-bridge-x86_64-pc-windows-msvc.exe",
    ]
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn bridge_names() -> [&'static str; 2] {
    ["khukri-bridge", "khukri-bridge-aarch64-apple-darwin"]
}

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
fn bridge_names() -> [&'static str; 2] {
    ["khukri-bridge", "khukri-bridge-x86_64-apple-darwin"]
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn bridge_names() -> [&'static str; 2] {
    ["khukri-bridge", "khukri-bridge-x86_64-unknown-linux-gnu"]
}

#[cfg(not(any(
    all(target_os = "windows", target_arch = "x86_64"),
    all(
        target_os = "macos",
        any(target_arch = "aarch64", target_arch = "x86_64")
    ),
    all(target_os = "linux", target_arch = "x86_64")
)))]
fn bridge_names() -> [&'static str; 0] {
    []
}
