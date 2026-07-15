#[cfg(target_os = "linux")]
use std::fs::{create_dir_all, read_to_string, write};
#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use serde_json::Value;

// DRM Status: "active", "missing", "unsupported"

#[cfg(target_os = "linux")]
const WIDEVINE_PATHS: &[&str] = &[
    "/opt/google/chrome/WidevineCdm",
    "/usr/lib/brave-bin/WidevineCdm",
    "/opt/brave-bin/WidevineCdm",
    "/usr/lib/chromium/WidevineCdm",
];

#[cfg(target_os = "linux")]
fn get_helium_config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    Path::new(&home).join(".config").join("net.imput.helium")
}

#[cfg(target_os = "linux")]
fn find_system_widevine() -> Option<(PathBuf, String)> {
    for path_str in WIDEVINE_PATHS {
        let base_path = Path::new(path_str);
        let manifest_path = base_path.join("manifest.json");
        if manifest_path.exists() {
            if let Ok(content) = read_to_string(&manifest_path) {
                if let Ok(json) = serde_json::from_str::<Value>(&content) {
                    if let Some(version) = json["version"].as_str() {
                        return Some((base_path.to_path_buf(), version.to_string()));
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
pub fn check_drm_status() -> String {
    let helium_dir = get_helium_config_dir();
    let system_wv = find_system_widevine();
    
    if let Some((_, version)) = system_wv {
        let helium_wv_so = helium_dir
            .join("WidevineCdm")
            .join(&version)
            .join("_platform_specific")
            .join("linux_x64")
            .join("libwidevinecdm.so");
            
        if helium_wv_so.exists() {
            return "active".to_string();
        }
    }
    
    // Check if there is any other version folder containing Widevine
    let wv_dir = helium_dir.join("WidevineCdm");
    if wv_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(wv_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let so_path = entry.path()
                        .join("_platform_specific")
                        .join("linux_x64")
                        .join("libwidevinecdm.so");
                    if so_path.exists() {
                        return "active".to_string();
                    }
                }
            }
        }
    }
    
    "missing".to_string()
}

#[cfg(target_os = "linux")]
pub fn fix_drm() -> Result<(), String> {
    let helium_dir = get_helium_config_dir();
    if !helium_dir.exists() {
        return Err("Helium Browser profile directory not found. Please run Helium at least once.".to_string());
    }

    let (src_base, version) = find_system_widevine()
        .ok_or_else(|| "Widevine CDM not found in Google Chrome or Brave directories. Please install Google Chrome or Brave.".to_string())?;

    let dest_wv_dir = helium_dir.join("WidevineCdm");
    create_dir_all(&dest_wv_dir)
        .map_err(|e| format!("Failed to create WidevineCdm directory: {}", e))?;

    let dest_base = dest_wv_dir.join(&version);

    if let Ok(metadata) = dest_base.symlink_metadata() {
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            std::fs::remove_dir_all(&dest_base)
                .map_err(|e| format!("Failed to remove old Widevine directory: {}", e))?;
        } else {
            std::fs::remove_file(&dest_base)
                .map_err(|e| format!("Failed to remove old Widevine file/symlink: {}", e))?;
        }
    }

    // Create a symbolic link pointing to the system's Widevine directory
    std::os::unix::fs::symlink(&src_base, &dest_base)
        .map_err(|e| format!("Failed to create Widevine symbolic link: {}", e))?;

    // Create/update latest-component-updated-widevine-cdm
    let latest_file_path = dest_wv_dir.join("latest-component-updated-widevine-cdm");
    let latest_json = format!("{{\"Path\":\"{}\"}}", dest_base.to_string_lossy().replace('\\', "/"));
    write(&latest_file_path, latest_json)
        .map_err(|e| format!("Failed to write latest-component-updated-widevine-cdm: {}", e))?;

    Ok(())
}

// Windows/macOS fallback
#[cfg(not(target_os = "linux"))]
pub fn check_drm_status() -> String {
    "unsupported".to_string()
}

#[cfg(not(target_os = "linux"))]
pub fn fix_drm() -> Result<(), String> {
    Err("Widevine DRM Fixer is only supported on Linux.".to_string())
}
