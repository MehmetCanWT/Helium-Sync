# Helium Sync

<div align="center">
  <img src="aur/helium-sync.png" alt="Helium Sync Logo" width="120" height="120" style="border-radius: 24px;" />
  <h3>Cloud Synchronization Daemon & Widevine DRM Fixer for Helium Browser</h3>
  <p><i>A sleek, lightweight, background service written in Rust with a premium PyQt6 desktop GUI app.</i></p>
</div>

---

## 🌟 Features

- **🔄 Automatic Sync-on-Close:** Helium Sync runs silently in the background. When it detects that you closed your Helium Browser, it automatically zips, encrypts, and pushes your active profile (cookies, history, bookmarks, tabs, extensions) to the cloud.
- **📥 Auto-Restore-on-Start:** When the daemon launches, it automatically pulls the latest profile backup from the cloud and restores it locally, ensuring your browsing state is up-to-date.
- **🔗 Symbolic Link DRM Fixer:** Integrates Widevine DRM on Linux systems by creating symbolic links (`symlinks`) directly to your system's Google Chrome or Brave Widevine directories. **When your system Chrome or Brave gets updated, Helium's Widevine CDM automatically updates with it!**
- **🔒 AES-256-GCM Local Encryption:** Keep your data private. Encrypt your profile backups locally with a password before they leave your machine.
- **🌐 Supported Storage Providers:**
  - **GitHub Gist:** Backup your encrypted profile data directly to a private Gist on your GitHub account.
  - **WebDAV:** Connect to self-hosted Nextcloud, ownCloud, or any WebDAV-compliant storage.
- **⚡ Sleek Desktop GUI:** Manage configurations, run manual backups, check real-time daemon logs, or trigger the DRM fixer with one click via a native dark-mode PyQt6 application launcher. Closing the app window hides it to the system tray, keeping the background sync service active!

---

## 🚨 Critical Security Warning (GitHub Gist)

> [!CAUTION]
> ### KEEP YOUR BACKUPS PRIVATE!
> When configuring the **GitHub Gist** provider, Helium Sync creates a private Gist under your account. 
> 
> **DO NOT** edit this Gist to make it **Public**. If your Gist is set to Public, anyone on the internet can download your backup file. If you have disabled local encryption, they can easily steal your browser session cookies, active logins, history, and passwords.
> 
> Always keep **Local Encryption** enabled with a strong passphrase for maximum security!

---

## 🛠️ Configuration Guides

### 🐱 GitHub Gist Configuration
1. Go to your GitHub account: **Settings** -> **Developer Settings** -> **Personal Access Tokens** -> **Tokens (classic)**.
2. Click **Generate new token (classic)**.
3. Give it a name (e.g., `Helium Sync Token`) and select the **`gist`** scope checkbox.
4. Click **Generate token** and copy the resulting string (`ghp_...`).
5. Launch the **Helium Sync** desktop app, navigate to the **Settings** tab, and select **GitHub Gist**.
6. Paste your Personal Access Token (PAT) into the field.
7. *Optional:* Leave the **Gist ID** field empty. Helium Sync will automatically create a new private Gist for you on the first synchronization and save the Gist ID to your configuration.
8. Click **Save Configurations**.

---

### ☁️ WebDAV Configuration (Nextcloud / ownCloud)
1. Open your cloud interface (e.g., Nextcloud) and navigate to **Personal Settings** -> **Security**.
2. Under **Devices & sessions**, generate an **App password** (e.g., named `Helium Sync`). Copy the username and generated password.
3. Launch the **Helium Sync** desktop app -> **Settings** tab -> select **WebDAV**.
4. Fill in the following fields:
   - **Server Endpoint URL:** The WebDAV URL provided by your server (e.g., `https://nextcloud.example.com/remote.php/dav/files/username/`).
   - **Username:** Your cloud account username.
   - **Password / Application Token:** The app password you generated.
   - **Target Folder Name:** The folder in your cloud where backups will be stored (defaults to `helium-sync`).
5. Click **Save Configurations**.

---

## 📦 Installation & Autostart Setup

### 🐧 Linux Installation

#### Option 1: Install from AUR (Arch Linux / CachyOS / Manjaro)
If you are on an Arch-based system, install using your favorite AUR helper:
```bash
paru -S helium-sync
```
This automatically installs the daemon executable, PyQt6 GUI application, desktop launcher entry, and registers the user systemd service.

#### Option 2: Build from Source
```bash
# Clone the repository
git clone https://github.com/MehmetCanWT/Helium-Sync.git
cd Helium-Sync

# Build backend
cargo build --release
cp target/release/helium-sync-daemon ~/.local/bin/
cp helium-sync-gui ~/.local/bin/
```

#### Autostart & Managing with systemd (User Space)
To run the sync daemon automatically in the background when your user logs in:
```bash
# Enable and start the user service
systemctl --user enable --now helium-sync

# Check if the service is running successfully
systemctl --user status helium-sync

# View live logs to verify it is working
journalctl --user -u helium-sync -f

# Restart the service
systemctl --user restart helium-sync

# Stop the service
systemctl --user stop helium-sync
```

---

## 🙋 Troubleshooting & Feedback

If you encounter any bugs, synchronization errors, or have feature requests:
- Please check the **Logs** tab in the desktop application for descriptive error readouts.
- Open a detailed issue on our **[GitHub Issue Tracker](https://github.com/MehmetCanWT/Helium-Sync/issues)**. 

Contributions and PRs are always welcome!
