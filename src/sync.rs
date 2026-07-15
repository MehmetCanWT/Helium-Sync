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
use base64::prelude::*;

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
fn is_whitelisted(rel_path: &str) -> bool {
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

    false
}

// Compression (Zip Profile) — only whitelisted essential files
pub fn zip_profile(profile_dir: &Path) -> Result<Vec<u8>, String> {
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
                        if is_whitelisted(&rel_path) {
                            zip.add_directory(&rel_path, options)
                                .map_err(|e| format!("Failed to add directory to zip: {}", e))?;
                            stack.push(entry_path);
                        }
                    } else if entry_path.is_file() && is_whitelisted(&rel_path) {
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

// Maximum size per Gist file chunk (~900KB base64 = ~675KB raw, well under the 1MB API limit)
const GIST_CHUNK_SIZE: usize = 900_000;

// GitHub Gist Push (with multi-part chunking for large payloads)
pub async fn push_github_gist(config: &mut Config, file_data: &[u8]) -> Result<(), String> {
    if config.github_token.is_empty() {
        return Err("GitHub Token not configured.".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;
    let b64_content = BASE64_STANDARD.encode(file_data);

    // If Gist ID is empty, search for an existing one on GitHub
    if config.github_gist_id.is_empty() {
        let list_url = "https://api.github.com/gists";
        let res = client.get(list_url)
            .header(reqwest::header::USER_AGENT, "helium-sync-daemon")
            .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
            .bearer_auth(&config.github_token)
            .send()
            .await
            .map_err(|e| format!("Gist list error: {}", e))?;

        if res.status().is_success() {
            if let Ok(gists) = res.json::<Vec<Value>>().await {
                for gist in gists {
                    if gist["description"].as_str() == Some("Helium Sync Backup") {
                        if let Some(id) = gist["id"].as_str() {
                            config.github_gist_id = id.to_string();
                            let _ = crate::config::save_config(config);
                            break;
                        }
                    }
                }
            }
        }
    }

    // Split b64 content into chunks to stay under Gist per-file size limits
    let mut files = serde_json::Map::new();
    if b64_content.len() <= GIST_CHUNK_SIZE {
        files.insert(
            "helium_sync_profile.bin".to_string(),
            serde_json::json!({ "content": b64_content }),
        );
    } else {
        let chunks: Vec<&str> = b64_content
            .as_bytes()
            .chunks(GIST_CHUNK_SIZE)
            .map(|chunk| std::str::from_utf8(chunk).unwrap_or_default())
            .collect();
        // Write a manifest so the pull side knows how many parts exist
        files.insert(
            "helium_sync_manifest.json".to_string(),
            serde_json::json!({ "content": serde_json::json!({ "parts": chunks.len() }).to_string() }),
        );
        for (i, chunk) in chunks.iter().enumerate() {
            files.insert(
                format!("helium_sync_part_{:04}.bin", i),
                serde_json::json!({ "content": *chunk }),
            );
        }
    }

    let body = serde_json::json!({
        "description": "Helium Sync Backup",
        "public": false,
        "files": files
    });

    if config.github_gist_id.is_empty() {
        // Create new private Gist
        let create_url = "https://api.github.com/gists";
        let res = client.post(create_url)
            .header(reqwest::header::USER_AGENT, "helium-sync-daemon")
            .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
            .bearer_auth(&config.github_token)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Gist creation error: {}", e))?;

        let status = res.status();
        if !status.is_success() {
            let text = res.text().await.unwrap_or_default();
            return Err(format!("Failed to create Gist (Status {}): {}", status, text));
        }

        let gist_json: Value = res.json().await.map_err(|e| format!("Failed to read Gist response: {}", e))?;
        if let Some(id) = gist_json["id"].as_str() {
            config.github_gist_id = id.to_string();
            let _ = crate::config::save_config(config);
        } else {
            return Err("Failed to retrieve Gist ID.".to_string());
        }
    } else {
        // Update existing Gist
        let update_url = format!("https://api.github.com/gists/{}", config.github_gist_id);
        let res = client.patch(&update_url)
            .header(reqwest::header::USER_AGENT, "helium-sync-daemon")
            .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
            .bearer_auth(&config.github_token)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Gist update error: {}", e))?;

        let status = res.status();
        if !status.is_success() {
            let text = res.text().await.unwrap_or_default();
            return Err(format!("Failed to update Gist (Status {}): {}", status, text));
        }
    }

    Ok(())
}

// GitHub Gist Pull (supports multi-part chunked payloads)
pub async fn pull_github_gist(config: &mut Config) -> Result<Option<Vec<u8>>, String> {
    if config.github_token.is_empty() {
        return Err("GitHub Token not configured.".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    // If Gist ID is empty, search for it
    if config.github_gist_id.is_empty() {
        let list_url = "https://api.github.com/gists";
        let res = client.get(list_url)
            .header(reqwest::header::USER_AGENT, "helium-sync-daemon")
            .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
            .bearer_auth(&config.github_token)
            .send()
            .await
            .map_err(|e| format!("Gist list error: {}", e))?;

        if res.status().is_success() {
            if let Ok(gists) = res.json::<Vec<Value>>().await {
                for gist in gists {
                    if gist["description"].as_str() == Some("Helium Sync Backup") {
                        if let Some(id) = gist["id"].as_str() {
                            config.github_gist_id = id.to_string();
                            let _ = crate::config::save_config(config);
                            break;
                        }
                    }
                }
            }
        }
    }

    if config.github_gist_id.is_empty() {
        return Ok(None);
    }

    let get_url = format!("https://api.github.com/gists/{}", config.github_gist_id);
    let res = client.get(&get_url)
        .header(reqwest::header::USER_AGENT, "helium-sync-daemon")
        .header(reqwest::header::ACCEPT, "application/vnd.github.v3+json")
        .bearer_auth(&config.github_token)
        .send()
        .await
        .map_err(|e| format!("Gist download error: {}", e))?;

    let status = res.status();
    if status.as_u16() == 404 {
        return Ok(None);
    }

    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(format!("Failed to download Gist (Status {}): {}", status, text));
    }

    let gist_json: Value = res.json().await.map_err(|e| format!("Failed to parse Gist JSON: {}", e))?;
    let files = &gist_json["files"];

    // Check if this is a single-file or multi-part backup
    if let Some(single_content) = files["helium_sync_profile.bin"]["content"].as_str() {
        // Single-file backup (small profile)
        let decoded = BASE64_STANDARD.decode(single_content)
            .map_err(|e| format!("Base64 decoding error: {}", e))?;
        return Ok(Some(decoded));
    }

    // Multi-part backup: read manifest and reassemble chunks
    if let Some(manifest_str) = files["helium_sync_manifest.json"]["content"].as_str() {
        let manifest: Value = serde_json::from_str(manifest_str)
            .map_err(|e| format!("Failed to parse manifest: {}", e))?;
        let parts = manifest["parts"].as_u64()
            .ok_or_else(|| "Invalid manifest: missing parts count.".to_string())? as usize;

        let mut combined_b64 = String::new();
        for i in 0..parts {
            let part_name = format!("helium_sync_part_{:04}.bin", i);
            let part_content = files[&part_name]["content"].as_str()
                .ok_or_else(|| format!("Missing chunk file: {}", part_name))?;
            combined_b64.push_str(part_content);
        }

        let decoded = BASE64_STANDARD.decode(&combined_b64)
            .map_err(|e| format!("Base64 decoding error (multi-part): {}", e))?;
        return Ok(Some(decoded));
    }

    Err("No backup file found in Gist.".to_string())
}

// Main Sync trigger (Push)
pub async fn trigger_push(config: &mut Config) -> Result<(), String> {
    let profile_dir = get_helium_profile_dir(config)
        .ok_or_else(|| "Helium profile directory not found. Please specify it manually in Settings.".to_string())?;
        
    let zip_data = zip_profile(&profile_dir)?;
    
    let payload = if config.encryption_active {
        if config.encryption_password.is_empty() {
            return Err("Encryption is enabled but no passphrase was provided.".to_string());
        }
        encrypt_bytes(&zip_data, &config.encryption_password)?
    } else {
        zip_data
    };
    
    match config.provider.as_str() {
        "webdav" => {
            push_webdav(config, &payload).await?;
        }
        "github_gist" => {
            push_github_gist(config, &payload).await?;
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
        
    let fetched = match config.provider.as_str() {
        "webdav" => pull_webdav(config).await?,
        "github_gist" => pull_github_gist(config).await?,
        _ => return Ok(false),
    };
    
    if let Some(payload) = fetched {
        let zip_data = if config.encryption_active {
            if config.encryption_password.is_empty() {
                return Err("Encryption is enabled but no passphrase was provided.".to_string());
            }
            decrypt_bytes(&payload, &config.encryption_password)?
        } else {
            payload
        };
        
        // Ensure profile dir exists before unzipping
        fs::create_dir_all(&profile_dir)
            .map_err(|e| format!("Failed to create profile directory: {}", e))?;
            
        unzip_profile(&zip_data, &profile_dir)?;
        
        config.last_sync_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        config.last_sync_size_bytes = zip_data.len() as u64; // Use raw size or encrypted size? Raw is fine.
        let _ = crate::config::save_config(config);
        
        return Ok(true);
    }
    
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
        let zipped_data = zip_profile(&temp_dir).unwrap();
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
