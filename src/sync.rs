use std::fs::{self, File};
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use chrono::Local;
use reqwest::header::CONTENT_TYPE;
use serde_json::Value;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use rand::{RngCore, thread_rng};

use crate::config::Config;

// Get Helium Profile Directory
pub fn get_helium_profile_dir(config: &Config) -> Option<PathBuf> {
    if !config.profile_path.is_empty() {
        let path = PathBuf::from(&config.profile_path);
        if path.exists() {
            return Some(path);
        }
    }
    
    // Auto-detect based on platform
    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").unwrap_or_default();
        if !home.is_empty() {
            let path = Path::new(&home).join(".config").join("net.imput.helium");
            if path.exists() {
                return Some(path);
            }
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        if let Some(local_appdata) = std::env::var("LOCALAPPDATA").ok() {
            let path = Path::new(&local_appdata).join("net.imput.helium");
            if path.exists() {
                return Some(path);
            }
        }
        if let Some(appdata) = std::env::var("APPDATA").ok() {
            let path = Path::new(&appdata).join("net.imput.helium");
            if path.exists() {
                return Some(path);
            }
        }
    }
    
    None
}

// Whitelist: Only sync essential browser profile data.
// This drastically reduces sync size from ~200MB to ~1-2MB, making sync near-instant.
fn is_whitelisted(rel_path: &str, config: &Config) -> bool {
    let lower = rel_path.to_lowercase();

    // Top-level files that store critical state
    if lower == "local state" {
        return true;
    }

    // Allow the Default/ directory entry itself (needed to create the folder in zip)
    if lower == "default" {
        return true;
    }

    // Essential Default/ profile files
    if lower.starts_with("default/") {
        let file_part = &lower["default/".len()..];

        // 1. Bookmarks & History & Logins & Preferences & Cookies
        if config.sync_bookmarks_history {
            let essential_files = [
                "bookmarks",
                "bookmarks.bak",
                "cookies",
                "cookies-journal",
                "login data",
                "login data-journal",
                "login data for account",
                "login data for account-journal",
                "web data",
                "web data-journal",
                "preferences",
                "secure preferences",
                "history",
                "history-journal",
                "favicons",
                "favicons-journal",
                "top sites",
                "top sites-journal",
                "shortcuts",
                "shortcuts-journal",
                "network persistent state",
                "transportsecurity",
                "affiliation database",
                "affiliation database-journal",
            ];

            // Direct file match
            for f in &essential_files {
                if file_part == *f {
                    return true;
                }
            }

            // Allow Local Storage/ directory and its contents
            if file_part == "local storage" || file_part.starts_with("local storage/") {
                return true;
            }
            // Allow Session Storage/ directory and its contents
            if file_part == "session storage" || file_part.starts_with("session storage/") {
                return true;
            }
            // Allow Sessions/ directory and its contents
            if file_part == "sessions" || file_part.starts_with("sessions/") {
                return true;
            }
            // Allow Sync Data/ directory
            if file_part == "sync data" || file_part.starts_with("sync data/") {
                return true;
            }
        }

        // 2. Extension Installation files
        if config.sync_extensions {
            if file_part == "extensions" || file_part.starts_with("extensions/") {
                return true;
            }
        }

        // 3. Extension Settings & Local Databases & Extension Cookies
        if config.sync_extension_databases {
            // Extension cookies
            if file_part == "extension cookies" || file_part == "extension cookies-journal" {
                return true;
            }
            // Extension directories
            let extension_dirs = [
                "extension state",
                "extension rules",
                "extension scripts",
                "local extension settings",
                "sync extension settings",
                "managed extension settings",
            ];
            for d in &extension_dirs {
                if file_part == *d || file_part.starts_with(&format!("{}/", d)) {
                    return true;
                }
            }
            // Extension IndexedDB
            if file_part.starts_with("indexeddb/") {
                let inner = &file_part["indexeddb/".len()..];
                if inner.starts_with("chrome-extension_") {
                    return true;
                }
            }
        }
    }

    false
}

// Compression (Zip Profile) — only whitelisted essential files
pub fn zip_profile(profile_dir: &Path, config: &Config) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    {
        let cursor = Cursor::new(&mut buf);
        let mut zip = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        let walk_dir = |zip: &mut ZipWriter<Cursor<&mut Vec<u8>>>| -> Result<(), String> {
            let prefix_len = profile_dir.to_path_buf().to_string_lossy().len();
            let mut stack = vec![profile_dir.to_path_buf()];
            
            while let Some(current_dir) = stack.pop() {
                let entries = fs::read_dir(&current_dir)
                    .map_err(|e| format!("Failed to read folder: {}", e))?;
                
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    
                    let rel_path = &entry_path.to_string_lossy()[prefix_len..];
                    let rel_path = rel_path.trim_start_matches('/').replace('\\', "/");
                    
                    if rel_path.is_empty() {
                        continue;
                    }

                    if entry_path.is_dir() {
                        // Only descend into whitelisted directories
                        if is_whitelisted(&rel_path, config) {
                            zip.add_directory(&rel_path, options)
                                .map_err(|e| format!("Failed to add directory to zip: {}", e))?;
                            stack.push(entry_path);
                        }
                    } else if entry_path.is_file() && is_whitelisted(&rel_path, config) {
                        zip.start_file(&rel_path, options)
                            .map_err(|e| format!("Failed to start file in zip: {}", e))?;
                        let mut file = File::open(&entry_path)
                            .map_err(|e| format!("Failed to open file ({}): {}", rel_path, e))?;
                        let mut buffer = Vec::new();
                        file.read_to_end(&mut buffer)
                            .map_err(|e| format!("Failed to read file ({}): {}", rel_path, e))?;
                        zip.write_all(&buffer)
                            .map_err(|e| format!("Failed to write file to zip ({}): {}", rel_path, e))?;
                    }
                }
            }
            Ok(())
        };

        walk_dir(&mut zip)?;
        zip.finish().map_err(|e| format!("Failed to finish zip: {}", e))?;
    }
    Ok(buf)
}

// Calculate estimated backup size (bytes) based on current whitelist settings
pub fn calculate_estimated_size(config: &Config) -> u64 {
    let profile_dir = match get_helium_profile_dir(config) {
        Some(p) => p,
        None => return 0,
    };
    
    let mut total_size = 0;
    let mut stack = vec![profile_dir.clone()];
    let prefix_len = profile_dir.to_string_lossy().len();
    
    while let Some(current_dir) = stack.pop() {
        if let Ok(entries) = fs::read_dir(&current_dir) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                let rel_path = &entry_path.to_string_lossy()[prefix_len..];
                let rel_path = rel_path.trim_start_matches('/').replace('\\', "/");
                
                if rel_path.is_empty() {
                    continue;
                }
                
                if entry_path.is_dir() {
                    if is_whitelisted(&rel_path, config) {
                        stack.push(entry_path);
                    }
                } else if entry_path.is_file() && is_whitelisted(&rel_path, config) {
                    if let Ok(meta) = entry.metadata() {
                        total_size += meta.len();
                    }
                }
            }
        }
    }
    
    total_size
}

// Extraction (Unzip Profile)
pub fn unzip_profile(zip_bytes: &[u8], dest_dir: &Path) -> Result<(), String> {
    let reader = Cursor::new(zip_bytes);
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| format!("Failed to open zip archive: {}", e))?;
        
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| format!("Failed to access zip file: {}", e))?;
            
        let outpath = match file.enclosed_name() {
            Some(path) => dest_dir.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)
                        .map_err(|e| format!("Failed to create directory: {}", e))?;
                }
            }
            let mut outfile = File::create(&outpath)
                .map_err(|e| format!("Failed to create file: {}", e))?;
            io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to write file: {}", e))?;
        }
    }
    Ok(())
}

// AES-256 Encryption (Salt + Nonce + Ciphertext)
pub fn encrypt_bytes(data: &[u8], password: &str) -> Result<Vec<u8>, String> {
    let mut salt = [0u8; 16];
    thread_rng().fill_bytes(&mut salt);
    
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, 100_000, &mut key);
    
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Failed to initialize encryptor: {}", e))?;
        
    let mut nonce_bytes = [0u8; 12];
    thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    let ciphertext = cipher.encrypt(nonce, data)
        .map_err(|e| format!("Encryption error: {}", e))?;
        
    let mut result = Vec::new();
    result.extend_from_slice(&salt);
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);
    
    Ok(result)
}

// AES-256 Decryption
pub fn decrypt_bytes(encrypted_data: &[u8], password: &str) -> Result<Vec<u8>, String> {
    if encrypted_data.len() < 28 {
        return Err("Invalid encrypted data size (too short).".to_string());
    }
    
    let salt = &encrypted_data[0..16];
    let nonce_bytes = &encrypted_data[16..28];
    let ciphertext = &encrypted_data[28..];
    
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, 100_000, &mut key);
    
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Failed to initialize decryptor: {}", e))?;
        
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|_| "Failed to decrypt. Password may be incorrect or the data is corrupted.".to_string())?;
        
    Ok(plaintext)
}

// WebDAV API helper to ensure parent directory exists
async fn ensure_webdav_folder(client: &reqwest::Client, config: &Config) -> Result<(), String> {
    if config.webdav_url.is_empty() {
        return Err("WebDAV URL not configured.".to_string());
    }
    
    let folder_url = format!("{}/{}", config.webdav_url.trim_end_matches('/'), config.webdav_folder.trim_matches('/'));
    
    // We send a MKCOL request. Since MKCOL can return 405 (if it already exists), we handle success and 405.
    let res = client.request(reqwest::Method::from_bytes(b"MKCOL").unwrap(), &folder_url)
        .basic_auth(&config.webdav_username, Some(&config.webdav_password))
        .send()
        .await;
        
    match res {
        Ok(response) => {
            let status = response.status();
            if status.is_success() || status.as_u16() == 405 {
                Ok(())
            } else {
                let text = response.text().await.unwrap_or_default();
                Err(format!("Failed to create directory (Status {}): {}", status, text))
            }
        }
        Err(e) => Err(format!("WebDAV connection error: {}", e))
    }
}

// WebDAV Push
pub async fn push_webdav(config: &Config, file_data: &[u8]) -> Result<(), String> {
    let client = reqwest::Client::new();
    
    // Ensure sync folder exists
    let _ = ensure_webdav_folder(&client, config).await;
    
    let file_url = format!(
        "{}/{}/helium_sync_profile.zip",
        config.webdav_url.trim_end_matches('/'),
        config.webdav_folder.trim_matches('/')
    );
    
    let res = client.put(&file_url)
        .basic_auth(&config.webdav_username, Some(&config.webdav_password))
        .header(CONTENT_TYPE, "application/zip")
        .body(file_data.to_owned())
        .send()
        .await
        .map_err(|e| format!("WebDAV upload error: {}", e))?;
        
    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(format!("Failed to upload to WebDAV (Status {}): {}", status, text));
    }
    
    Ok(())
}

// WebDAV Pull
pub async fn pull_webdav(config: &Config) -> Result<Option<Vec<u8>>, String> {
    let client = reqwest::Client::new();
    
    let file_url = format!(
        "{}/{}/helium_sync_profile.zip",
        config.webdav_url.trim_end_matches('/'),
        config.webdav_folder.trim_matches('/')
    );
    
    let res = client.get(&file_url)
        .basic_auth(&config.webdav_username, Some(&config.webdav_password))
        .send()
        .await
        .map_err(|e| format!("WebDAV download error: {}", e))?;
        
    let status = res.status();
    if status.as_u16() == 404 {
        return Ok(None);
    }
    
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(format!("Failed to download from WebDAV (Status {}): {}", status, text));
    }
    
    let data = res.bytes().await.map_err(|e| format!("Failed to read data: {}", e))?;
    Ok(Some(data.to_vec()))
}

// GitHub Releases Push (streams raw binary data, no size limit, extremely fast)
pub async fn push_github_releases(config: &mut Config, file_data: &[u8]) -> Result<(), String> {
    if config.github_token.is_empty() {
        return Err("GitHub Token not configured.".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    // 1. Get authenticated user login name
    crate::watcher::add_log("Fetching GitHub username...");
    let user_url = "https://api.github.com/user";
    let res = client.get(user_url)
        .header(reqwest::header::USER_AGENT, "helium-sync-daemon")
        .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
        .bearer_auth(&config.github_token)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch user details: {}", e))?;
    
    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(format!("Failed to authenticate with GitHub (Status {}): {}", status, text));
    }
    
    let user_json: Value = res.json().await.map_err(|e| format!("Failed to parse user details JSON: {}", e))?;
    let owner = user_json["login"].as_str()
        .ok_or_else(|| "Failed to retrieve login name from GitHub".to_string())?;

    let repo = "helium-sync-backups";

    // 2. Check if repository exists. If not, create it as private.
    crate::watcher::add_log(&format!("Checking if repository {}/{} exists...", owner, repo));
    let repo_url = format!("https://api.github.com/repos/{}/{}", owner, repo);
    let res = client.get(&repo_url)
        .header(reqwest::header::USER_AGENT, "helium-sync-daemon")
        .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
        .bearer_auth(&config.github_token)
        .send()
        .await
        .map_err(|e| format!("Failed to check repository existence: {}", e))?;

    let repo_status = res.status();
    if repo_status.as_u16() == 404 {
        crate::watcher::add_log(&format!("Repository does not exist. Creating private repository {}/{}...", owner, repo));
        let create_repo_url = "https://api.github.com/user/repos";
        let create_body = serde_json::json!({
            "name": repo,
            "private": true,
            "description": "Helium Sync Private Profile Backups",
            "auto_init": true
        });
        let res = client.post(create_repo_url)
            .header(reqwest::header::USER_AGENT, "helium-sync-daemon")
            .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
            .bearer_auth(&config.github_token)
            .json(&create_body)
            .send()
            .await
            .map_err(|e| format!("Failed to create repository: {}", e))?;

        let status = res.status();
        // 422 means repo already exists (race condition or stale cache), treat as success
        if !status.is_success() && status.as_u16() != 422 {
            let text = res.text().await.unwrap_or_default();
            return Err(format!(
                "Failed to create private repository (Status {}): {}. Please ensure your GitHub Personal Access Token (PAT) has the 'repo' scope enabled.",
                status, text
            ));
        }
        crate::watcher::add_log("Repository created successfully.");
    } else if !repo_status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(format!("Failed to check repository (Status {}): {}", repo_status, text));
    }

    // 2.5 Ensure repository has at least one branch/commit before creating a release (Fixes GitHub API 422 error on empty repos)
    let branches_url = format!("https://api.github.com/repos/{}/{}/branches", owner, repo);
    let branches_res = client.get(&branches_url)
        .header(reqwest::header::USER_AGENT, "helium-sync-daemon")
        .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
        .bearer_auth(&config.github_token)
        .send()
        .await;
    if let Ok(b_res) = branches_res {
        if let Ok(branches_json) = b_res.json::<Vec<Value>>().await {
            if branches_json.is_empty() {
                crate::watcher::add_log("Repository is currently empty. Creating initial README.md commit...");
                let readme_url = format!("https://api.github.com/repos/{}/{}/contents/README.md", owner, repo);
                let readme_body = serde_json::json!({
                    "message": "Initialize Helium Sync backup repository",
                    "content": "IyBIZWxpdW0gU3luYyBCYWNrdXBzCgpUaGlzIHJlcG9zaXRvcnkgc3RvcmVzIHlvdXIgSGVsaXVtIHByb2ZpbGUgc3luYyBkYXRhLgo="
                });
                let put_res = client.put(&readme_url)
                    .header(reqwest::header::USER_AGENT, "helium-sync-daemon")
                    .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
                    .bearer_auth(&config.github_token)
                    .json(&readme_body)
                    .send()
                    .await;
                if let Ok(r) = put_res {
                    if !r.status().is_success() {
                        crate::watcher::add_log(&format!("[WARN] Initial commit status: {}", r.status()));
                    } else {
                        crate::watcher::add_log("Initial README.md commit created.");
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        }
    }

    // 3. Get or create release tagged "latest"
    crate::watcher::add_log("Fetching 'latest' release metadata...");
    let release_url = format!("https://api.github.com/repos/{}/{}/releases/tags/latest", owner, repo);
    let res = client.get(&release_url)
        .header(reqwest::header::USER_AGENT, "helium-sync-daemon")
        .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
        .bearer_auth(&config.github_token)
        .send()
        .await
        .map_err(|e| format!("Failed to check release status: {}", e))?;

    let release_json: Value = if res.status().is_success() {
        res.json().await.map_err(|e| format!("Failed to parse release JSON: {}", e))?
    } else {
        crate::watcher::add_log("Release does not exist. Creating new release tagged 'latest'...");
        let create_release_url = format!("https://api.github.com/repos/{}/{}/releases", owner, repo);
        let create_body = serde_json::json!({
            "tag_name": "latest",
            "name": "Latest Profile Backup",
            "draft": false,
            "prerelease": false
        });
        let res = client.post(&create_release_url)
            .header(reqwest::header::USER_AGENT, "helium-sync-daemon")
            .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
            .bearer_auth(&config.github_token)
            .json(&create_body)
            .send()
            .await
            .map_err(|e| format!("Failed to create release: {}", e))?;

        let status = res.status();
        if !status.is_success() {
            let text = res.text().await.unwrap_or_default();
            return Err(format!("Failed to create release (Status {}): {}", status, text));
        }
        res.json().await.map_err(|e| format!("Failed to parse created release JSON: {}", e))?
    };

    let release_id = release_json["id"].as_i64()
        .ok_or_else(|| "Failed to get release ID from GitHub response".to_string())?;

    // 4. Delete existing asset named "helium_sync_profile.bin" if it exists
    if let Some(assets) = release_json["assets"].as_array() {
        for asset in assets {
            if asset["name"].as_str() == Some("helium_sync_profile.bin") {
                if let Some(asset_id) = asset["id"].as_i64() {
                    crate::watcher::add_log("Deleting old profile asset from GitHub release...");
                    let delete_url = format!("https://api.github.com/repos/{}/{}/releases/assets/{}", owner, repo, asset_id);
                    let res = client.delete(&delete_url)
                        .header(reqwest::header::USER_AGENT, "helium-sync-daemon")
                        .bearer_auth(&config.github_token)
                        .send()
                        .await;
                    if let Ok(resp) = res {
                        if !resp.status().is_success() {
                            crate::watcher::add_log(&format!("[WARN] Failed to delete old asset: {}", resp.status()));
                        }
                    }
                }
            }
        }
    }

    // 5. Upload the new asset to the release using raw binary PUT/POST
    crate::watcher::add_log("Uploading raw profile package as release asset...");
    let upload_url = format!(
        "https://uploads.github.com/repos/{}/{}/releases/{}/assets?name=helium_sync_profile.bin",
        owner, repo, release_id
    );

    let res = client.post(&upload_url)
        .header(reqwest::header::USER_AGENT, "helium-sync-daemon")
        .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
        .header(reqwest::header::CONTENT_LENGTH, file_data.len())
        .bearer_auth(&config.github_token)
        .body(file_data.to_owned())
        .send()
        .await
        .map_err(|e| format!("Asset upload error: {}", e))?;

    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(format!("Failed to upload asset (Status {}): {}", status, text));
    }

    Ok(())
}

// GitHub Releases Pull (downloads raw binary data directly)
pub async fn pull_github_releases(config: &mut Config) -> Result<Option<Vec<u8>>, String> {
    if config.github_token.is_empty() {
        return Err("GitHub Token not configured.".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    // 1. Get authenticated user login name
    let user_url = "https://api.github.com/user";
    let res = client.get(user_url)
        .header(reqwest::header::USER_AGENT, "helium-sync-daemon")
        .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
        .bearer_auth(&config.github_token)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch user details: {}", e))?;

    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(format!("GitHub authentication failed (Status {}): {}. Check your Personal Access Token.", status, text));
    }
    
    let user_json: Value = res.json().await.map_err(|e| format!("Failed to parse user JSON: {}", e))?;
    let owner = user_json["login"].as_str()
        .ok_or_else(|| "Failed to retrieve login name".to_string())?;

    let repo = "helium-sync-backups";

    // 2. Fetch the latest release to get assets
    let release_url = format!("https://api.github.com/repos/{}/{}/releases/tags/latest", owner, repo);
    let res = client.get(&release_url)
        .header(reqwest::header::USER_AGENT, "helium-sync-daemon")
        .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
        .bearer_auth(&config.github_token)
        .send()
        .await
        .map_err(|e| format!("Failed to check release: {}", e))?;

    let status = res.status();
    if status.as_u16() == 404 {
        return Ok(None);
    }
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(format!("Failed to fetch release (Status {}): {}", status, text));
    }

    let release_json: Value = res.json().await.map_err(|e| format!("Failed to parse release JSON: {}", e))?;
    
    // Find the helium_sync_profile.bin asset
    let mut asset_url = None;
    if let Some(assets) = release_json["assets"].as_array() {
        for asset in assets {
            if asset["name"].as_str() == Some("helium_sync_profile.bin") {
                asset_url = asset["url"].as_str().map(|s| s.to_string());
                break;
            }
        }
    }

    let download_url = match asset_url {
        Some(url) => url,
        None => return Ok(None),
    };

    // 3. Download the asset file
    crate::watcher::add_log("Downloading profile backup asset from GitHub release...");
    let res = client.get(&download_url)
        .header(reqwest::header::USER_AGENT, "helium-sync-daemon")
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .bearer_auth(&config.github_token)
        .send()
        .await
        .map_err(|e| format!("Asset download error: {}", e))?;

    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(format!("Failed to download asset data (Status {}): {}", status, text));
    }

    let data = res.bytes().await.map_err(|e| format!("Failed to read asset bytes: {}", e))?;
    Ok(Some(data.to_vec()))
}

// Main Sync trigger (Push)
pub async fn trigger_push(config: &mut Config) -> Result<(), String> {
    let profile_dir = get_helium_profile_dir(config)
        .ok_or_else(|| "Helium profile directory not found. Please specify it manually in Settings.".to_string())?;
        
    crate::watcher::add_log("Starting profile compression (Zipping)...");
    let zip_data = zip_profile(&profile_dir, config)?;
    crate::watcher::add_log(&format!("Profile compressed successfully. Zip package size: {:.2} KB", zip_data.len() as f64 / 1024.0));
    
    let payload = if config.encryption_active {
        if config.encryption_password.is_empty() {
            return Err("Encryption is enabled but no passphrase was provided.".to_string());
        }
        crate::watcher::add_log("AES-256-GCM local encryption activated. Encrypting package...");
        let encrypted = encrypt_bytes(&zip_data, &config.encryption_password)?;
        crate::watcher::add_log(&format!("Package encrypted successfully. Encrypted size: {:.2} KB", encrypted.len() as f64 / 1024.0));
        encrypted
    } else {
        crate::watcher::add_log("Encryption disabled. Proceeding with raw zip package.");
        zip_data
    };
    
    crate::watcher::add_log(&format!("Uploading backup to provider: {}...", config.provider));
    match config.provider.as_str() {
        "webdav" => {
            push_webdav(config, &payload).await?;
            crate::watcher::add_log("WebDAV upload completed successfully.");
        }
        "github_releases" => {
            push_github_releases(config, &payload).await?;
            crate::watcher::add_log("GitHub Releases upload completed successfully.");
        }
        _ => return Err("No cloud provider configured.".to_string()),
    }
    
    config.last_sync_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    config.last_sync_size_bytes = payload.len() as u64;
    
    let _ = crate::config::save_config(config);
    
    Ok(())
}

// Main Sync trigger (Pull)
pub async fn trigger_pull(config: &mut Config) -> Result<bool, String> {
    let profile_dir = get_helium_profile_dir(config)
        .ok_or_else(|| "Helium profile directory not found. Please specify it manually in Settings.".to_string())?;
        
    crate::watcher::add_log(&format!("Downloading backup from provider: {}...", config.provider));
    let fetched = match config.provider.as_str() {
        "webdav" => pull_webdav(config).await?,
        "github_releases" => pull_github_releases(config).await?,
        _ => return Ok(false),
    };
    
    if let Some(payload) = fetched {
        crate::watcher::add_log(&format!("Download completed successfully. Size: {:.2} KB", payload.len() as f64 / 1024.0));
        
        let zip_data = if config.encryption_active {
            if config.encryption_password.is_empty() {
                return Err("Encryption is enabled but no passphrase was provided.".to_string());
            }
            crate::watcher::add_log("Decrypting local backup package using AES-256-GCM...");
            decrypt_bytes(&payload, &config.encryption_password)?
        } else {
            payload
        };
        
        crate::watcher::add_log("Extracting profile files (Unzipping)...");
        // Ensure profile dir exists before unzipping
        fs::create_dir_all(&profile_dir)
            .map_err(|e| format!("Failed to create profile directory: {}", e))?;
            
        unzip_profile(&zip_data, &profile_dir)?;
        crate::watcher::add_log("Extraction completed. Profile restored successfully!");
        
        config.last_sync_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        config.last_sync_size_bytes = zip_data.len() as u64;
        let _ = crate::config::save_config(config);
        
        return Ok(true);
    }
    
    crate::watcher::add_log("No backup file found in cloud provider.");
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    // We can just use standard std::env::temp_dir() and random folders to avoid extra crate dependency.
    use std::fs::{self, File};
    use std::io::Write;

    #[test]
    fn test_encrypt_decrypt() {
        let original_data = b"Helium Sync Premium AES test data string";
        let password = "super-secure-passphrase";
        
        let encrypted = encrypt_bytes(original_data, password).unwrap();
        assert_ne!(original_data.to_vec(), encrypted);
        
        let decrypted = decrypt_bytes(&encrypted, password).unwrap();
        assert_eq!(original_data.to_vec(), decrypted);
    }
    
    #[test]
    fn test_zip_unzip() {
        let rand_val = rand::thread_rng().next_u32();
        let temp_dir = std::env::temp_dir().join(format!("helium_test_src_{}", rand_val));
        let extract_dir = std::env::temp_dir().join(format!("helium_test_dest_{}", rand_val));
        
        fs::create_dir_all(&temp_dir).unwrap();
        fs::create_dir_all(&extract_dir).unwrap();
        
        // Create whitelisted files that match the profile structure
        let default_dir = temp_dir.join("Default");
        fs::create_dir_all(&default_dir).unwrap();

        let bookmarks_path = default_dir.join("Bookmarks");
        let mut f1 = File::create(&bookmarks_path).unwrap();
        f1.write_all(b"{\"bookmarks\": []}").unwrap();
        
        let sessions_dir = default_dir.join("Sessions");
        fs::create_dir_all(&sessions_dir).unwrap();
        let session_path = sessions_dir.join("current_session");
        let mut f2 = File::create(&session_path).unwrap();
        f2.write_all(b"session data here").unwrap();

        // Create a non-whitelisted file (should be excluded)
        let cache_path = default_dir.join("random_cache_file.bin");
        let mut f3 = File::create(&cache_path).unwrap();
        f3.write_all(b"this should not be included").unwrap();
        
        // Zip
        let zipped_data = zip_profile(&temp_dir, &Config::default()).unwrap();
        assert!(!zipped_data.is_empty());
        
        // Unzip
        unzip_profile(&zipped_data, &extract_dir).unwrap();
        
        // Verify whitelisted files exist
        let ext_bookmarks = extract_dir.join("Default").join("Bookmarks");
        let ext_session = extract_dir.join("Default").join("Sessions").join("current_session");
        
        assert!(ext_bookmarks.exists(), "Bookmarks should be synced");
        assert!(ext_session.exists(), "Session files should be synced");
        
        // Verify non-whitelisted file was excluded
        let ext_cache = extract_dir.join("Default").join("random_cache_file.bin");
        assert!(!ext_cache.exists(), "Non-whitelisted files should be excluded");
        
        let mut c1 = String::new();
        File::open(ext_bookmarks).unwrap().read_to_string(&mut c1).unwrap();
        assert_eq!(c1, "{\"bookmarks\": []}");
        
        let mut c2 = String::new();
        File::open(ext_session).unwrap().read_to_string(&mut c2).unwrap();
        assert_eq!(c2, "session data here");
        
        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::remove_dir_all(&extract_dir);
    }
}
