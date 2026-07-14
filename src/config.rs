use serde::{Deserialize, Serialize};
use std::fs::{create_dir_all, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use directories::ProjectDirs;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    pub provider: String,                  // "webdav", "github_gist", "none"
    pub webdav_url: String,
    pub webdav_username: String,
    pub webdav_password: String,
    pub webdav_folder: String,
    pub encryption_active: bool,
    pub encryption_password: String,
    pub profile_path: String,              // Manual override for Helium profile
    pub github_token: String,
    pub github_gist_id: String,
    pub last_sync_time: String,
    pub last_sync_size_bytes: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            provider: "none".to_string(),
            webdav_url: "".to_string(),
            webdav_username: "".to_string(),
            webdav_password: "".to_string(),
            webdav_folder: "helium-sync".to_string(),
            encryption_active: false,
            encryption_password: "".to_string(),
            profile_path: "".to_string(),
            github_token: "".to_string(),
            github_gist_id: "".to_string(),
            last_sync_time: "Never synchronized".to_string(),
            last_sync_size_bytes: 0,
        }
    }
}

pub fn get_config_dir() -> Option<PathBuf> {
    ProjectDirs::from("net", "imput", "Helium-Sync")
        .map(|proj_dirs| proj_dirs.config_dir().to_path_buf())
}

pub fn get_config_path() -> Option<PathBuf> {
    get_config_dir().map(|mut dir| {
        dir.push("config.json");
        dir
    })
}

pub fn load_config() -> Config {
    if let Some(path) = get_config_path() {
        if path.exists() {
            if let Ok(mut file) = File::open(&path) {
                let mut contents = String::new();
                if file.read_to_string(&mut contents).is_ok() {
                    if let Ok(config) = serde_json::from_str::<Config>(&contents) {
                        return config;
                    }
                }
            }
        }
    }
    Config::default()
}

pub fn save_config(config: &Config) -> Result<(), String> {
    let dir = get_config_dir().ok_or("Config directory not found.")?;
    create_dir_all(&dir).map_err(|e| format!("Failed to create directory: {}", e))?;
    
    let path = get_config_path().ok_or("Config path not found.")?;
    let mut file = File::create(&path).map_err(|e| format!("Failed to create file: {}", e))?;
    
    let serialized = serde_json::to_string_pretty(config)
        .map_err(|e| format!("JSON serialization error: {}", e))?;
    
    file.write_all(serialized.as_bytes())
        .map_err(|e| format!("Write error: {}", e))?;
    
    Ok(())
}
