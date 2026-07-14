use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::time::sleep;
use sysinfo::System;

use crate::config::load_config;
use crate::sync::{trigger_pull, trigger_push};

// Global log buffer for Web UI
static LOG_BUFFER: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

pub fn get_logs() -> Vec<String> {
    let buffer = LOG_BUFFER.get_or_init(|| Mutex::new(Vec::new()));
    let lock = buffer.lock().unwrap();
    lock.clone()
}

pub fn add_log(message: &str) {
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let formatted = format!("[{}] {}", timestamp, message);
    
    // Print to stdout
    println!("{}", formatted);
    
    let buffer = LOG_BUFFER.get_or_init(|| Mutex::new(Vec::new()));
    let mut lock = buffer.lock().unwrap();
    lock.push(formatted);
    
    // Keep only last 200 logs
    if lock.len() > 200 {
        lock.remove(0);
    }
}

pub fn is_helium_running() -> bool {
    let mut sys = System::new();
    sys.refresh_processes();
    
    for process in sys.processes().values() {
        let name = process.name().to_lowercase();
        if name.contains("helium") {
            return true;
        }
    }
    false
}

pub async fn start_watcher() {
    add_log("Process monitoring service started.");
    
    // Startup Sync (Pull)
    let mut config = load_config();
    if config.provider != "none" {
        add_log(&format!("Startup sync initiated (Provider: {})...", config.provider));
        match trigger_pull(&mut config).await {
            Ok(pulled) => {
                if pulled {
                    add_log("Startup sync completed successfully. Latest profile loaded from cloud.");
                } else {
                    add_log("No profile backup found in cloud. Using local profile.");
                }
            }
            Err(e) => {
                add_log(&format!("[ERROR] Startup sync failed: {}", e));
            }
        }
    } else {
        add_log("No cloud provider configured. Synchronization skipped.");
    }
    
    let mut was_running = is_helium_running();
    if was_running {
        add_log("Helium Browser detected running currently.");
    }

    loop {
        sleep(Duration::from_secs(3)).await;
        
        let is_running = is_helium_running();
        
        if is_running && !was_running {
            add_log("Helium Browser started.");
            was_running = true;
        } else if !is_running && was_running {
            add_log("Helium Browser closed. Initiating sync in 3 seconds...");
            
            // Wait for file locks to release
            sleep(Duration::from_secs(3)).await;
            
            // Reload config for latest credentials / encryption password
            let mut config = load_config();
            if config.provider != "none" {
                add_log("Shutdown sync (Push) initiated...");
                match trigger_push(&mut config).await {
                    Ok(_) => {
                        add_log("Shutdown sync completed successfully. Data uploaded to cloud.");
                    }
                    Err(e) => {
                        add_log(&format!("[ERROR] Synchronization error: {}", e));
                    }
                }
            } else {
                add_log("Synchronization skipped as no cloud provider is set.");
            }
            was_running = false;
        }
    }
}
