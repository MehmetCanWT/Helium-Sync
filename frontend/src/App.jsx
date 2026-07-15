import React, { useState, useEffect, useRef } from 'react';
import { 
  RefreshCw, 
  Shield, 
  Database, 
  Folder, 
  Terminal, 
  Settings as SettingsIcon, 
  ExternalLink, 
  CheckCircle, 
  AlertTriangle, 
  Play, 
  Power,
  Info,
  Server,
  Eye,
  EyeOff
} from 'lucide-react';
import logo from './assets/logo.png';

function App() {
  const [activeTab, setActiveTab] = useState('status');
  const [status, setStatus] = useState({
    provider: 'none',
    github_connected: false,
    github_gist_id: '',
    webdav_url: '',
    last_sync_time: 'Never synchronized',
    last_sync_size_bytes: 0,
    encryption_active: false,
    browser_running: false,
    drm_status: 'missing', // active, missing, unsupported
    profile_path: '',
    platform: 'linux',
    app_version: '0.3.0'
  });

  const [settings, setSettings] = useState({
    provider: 'github_gist',
    webdav_url: '',
    webdav_username: '',
    webdav_password: '',
    webdav_folder: 'helium-sync',
    encryption_active: false,
    encryption_password: '',
    profile_path: '',
    github_token: '',
    github_gist_id: ''
  });

  const [showPassword, setShowPassword] = useState(false);
  const [logs, setLogs] = useState([]);
  const [syncing, setSyncing] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [fixingDrm, setFixingDrm] = useState(false);
  const [saveSuccess, setSaveSuccess] = useState(false);
  
  const logEndRef = useRef(null);

  useEffect(() => {
    // Fetch initial status and settings
    fetchStatus();
    fetchSettings();
    
    // Poll status every 3 seconds
    const statusInterval = setInterval(fetchStatus, 3000);
    // Poll logs every 2 seconds
    const logInterval = setInterval(fetchLogs, 2000);

    return () => {
      clearInterval(statusInterval);
      clearInterval(logInterval);
    };
  }, []);

  useEffect(() => {
    if (activeTab === 'logs') {
      scrollToBottom();
    }
  }, [logs, activeTab]);

  const scrollToBottom = () => {
    logEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  };

  const fetchStatus = async () => {
    try {
      const res = await fetch('/api/status');
      if (res.ok) {
        const data = await res.json();
        setStatus(data);
      }
    } catch (err) {
      console.error("Failed to fetch status:", err);
    }
  };

  const fetchSettings = async () => {
    try {
      const res = await fetch('/api/settings');
      if (res.ok) {
        const data = await res.json();
        setSettings(data);
      }
    } catch (err) {
      console.error("Failed to fetch settings:", err);
    }
  };

  const fetchLogs = async () => {
    try {
      const res = await fetch('/api/logs');
      if (res.ok) {
        const data = await res.json();
        setLogs(data.logs || []);
      }
    } catch (err) {
      console.error("Failed to fetch logs:", err);
    }
  };

  const handleSaveSettings = async (e) => {
    e.preventDefault();
    try {
      const res = await fetch('/api/settings', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(settings)
      });
      if (res.ok) {
        setSaveSuccess(true);
        setTimeout(() => setSaveSuccess(false), 3000);
        fetchStatus();
      }
    } catch (err) {
      console.error("Failed to save settings:", err);
    }
  };

  const handleTriggerSync = async () => {
    if (syncing) return;
    setSyncing(true);
    try {
      const res = await fetch('/api/sync', { method: 'POST' });
      if (res.ok) {
        fetchStatus();
      }
    } catch (err) {
      console.error("Failed to trigger synchronization:", err);
    } finally {
      setSyncing(false);
    }
  };

  const handleTriggerRestore = async () => {
    if (restoring) return;
    if (!confirm("Are you sure you want to download and restore the profile from cloud? This will overwrite your current local browser profile!")) {
      return;
    }
    setRestoring(true);
    try {
      const res = await fetch('/api/restore', { method: 'POST' });
      if (res.ok) {
        fetchStatus();
      }
    } catch (err) {
      console.error("Failed to trigger restore:", err);
    } finally {
      setRestoring(false);
    }
  };

  const handleFixDrm = async () => {
    if (fixingDrm) return;
    setFixingDrm(true);
    try {
      const res = await fetch('/api/fix-drm', { method: 'POST' });
      if (res.ok) {
        fetchStatus();
      }
    } catch (err) {
      console.error("Failed to fix DRM:", err);
    } finally {
      setFixingDrm(false);
    }
  };



  const formatBytes = (bytes) => {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  return (
    <div className="min-h-screen bg-[#040405] bg-glow flex flex-col justify-between py-6 px-4 sm:px-6 lg:px-8">
      {/* Header */}
      <header className="max-w-5xl w-full mx-auto flex flex-col sm:flex-row items-center justify-between border-b border-neutral-900 pb-6 mb-8 gap-4">
        <div className="flex items-center gap-3">
          <img src={logo} alt="Helium Sync Logo" className="h-12 w-12 rounded-xl object-contain shadow-lg shadow-purple-500/10" />
          <div>
            <div className="flex items-center gap-2">
              <h1 className="text-2xl font-bold tracking-tight text-white m-0">Helium Sync</h1>
              <span className="bg-neutral-900 text-purple-400 border border-purple-900/30 text-[10px] px-2 py-0.5 rounded font-mono font-bold">v{status.app_version || '0.3.0'}</span>
            </div>
            <p className="text-xs text-purple-400 font-semibold uppercase tracking-wider">Cloud Synchronizer & DRM Fixer</p>
          </div>
        </div>

        {/* Navigation Tabs */}
        <nav className="flex space-x-1 bg-neutral-950 p-1 rounded-xl border border-neutral-900">
          <button
            onClick={() => setActiveTab('status')}
            className={`px-4 py-2 text-sm font-medium rounded-lg transition-all ${
              activeTab === 'status'
                ? 'bg-purple-600 text-white shadow-md'
                : 'text-gray-400 hover:text-white hover:bg-neutral-900'
            }`}
          >
            Status
          </button>
          <button
            onClick={() => setActiveTab('settings')}
            className={`px-4 py-2 text-sm font-medium rounded-lg transition-all ${
              activeTab === 'settings'
                ? 'bg-purple-600 text-white shadow-md'
                : 'text-gray-400 hover:text-white hover:bg-neutral-900'
            }`}
          >
            Settings
          </button>
          <button
            onClick={() => setActiveTab('logs')}
            className={`px-4 py-2 text-sm font-medium rounded-lg transition-all ${
              activeTab === 'logs'
                ? 'bg-purple-600 text-white shadow-md'
                : 'text-gray-400 hover:text-white hover:bg-neutral-900'
            }`}
          >
            Logs
          </button>
        </nav>
      </header>

      {/* Main Content Area */}
      <main className="max-w-5xl w-full mx-auto flex-grow mb-8">
        {activeTab === 'status' && (
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
            {/* Status Panel 1: Browser & Sync status */}
            <div className="md:col-span-2 space-y-6">
              {/* Connection Status Card */}
              <div className="glass-panel rounded-2xl p-6 relative overflow-hidden">
                <div className="absolute top-0 right-0 p-4 opacity-5">
                  <Database className="h-24 w-24" />
                </div>
                <h3 className="text-lg font-semibold text-white mb-4 flex items-center gap-2">
                  <Database className="h-5 w-5 text-purple-400" /> Synchronization Status
                </h3>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                  <div className="bg-neutral-950 p-4 rounded-xl border border-neutral-900">
                    <p className="text-xs text-gray-500 font-medium">Cloud Provider</p>
                    <p className="text-base font-semibold text-white mt-1 capitalize">
                      {status.provider === 'webdav' ? 'WebDAV' : status.provider === 'github_gist' ? 'GitHub Gist' : 'Not Configured'}
                    </p>
                  </div>
                  <div className="bg-neutral-950 p-4 rounded-xl border border-neutral-900">
                    <p className="text-xs text-gray-500 font-medium">Last Synchronized</p>
                    <p className="text-base font-semibold text-white mt-1">{status.last_sync_time}</p>
                  </div>
                  <div className="bg-neutral-950 p-4 rounded-xl border border-neutral-900">
                    <p className="text-xs text-gray-500 font-medium">Last Backup Size</p>
                    <p className="text-base font-semibold text-white mt-1">{formatBytes(status.last_sync_size_bytes)}</p>
                  </div>
                  <div className="bg-neutral-950 p-4 rounded-xl border border-neutral-900">
                    <p className="text-xs text-gray-500 font-medium">Local Encryption</p>
                    <div className="flex items-center gap-2 mt-1">
                      {status.encryption_active ? (
                        <>
                          <Shield className="h-4 w-4 text-emerald-400" />
                          <span className="text-base font-semibold text-emerald-400">Enabled (AES-256)</span>
                        </>
                      ) : (
                        <>
                          <AlertTriangle className="h-4 w-4 text-amber-500" />
                          <span className="text-base font-semibold text-amber-500">Disabled (Unencrypted)</span>
                        </>
                      )}
                    </div>
                  </div>
                </div>

                <div className="mt-6 flex flex-wrap gap-4 items-center">
                  <button
                    onClick={handleTriggerSync}
                    disabled={syncing || status.provider === 'none'}
                    className={`px-5 py-2.5 rounded-xl text-sm font-semibold flex items-center gap-2 transition-all ${
                      status.provider === 'none' 
                        ? 'bg-neutral-900 text-gray-600 cursor-not-allowed border border-neutral-800'
                        : 'glow-btn-primary text-white cursor-pointer'
                    }`}
                  >
                    <RefreshCw className={`h-4 w-4 ${syncing ? 'animate-spin' : ''}`} />
                    {syncing ? 'Syncing...' : 'Sync Now (Push)'}
                  </button>

                  <button
                    onClick={handleTriggerRestore}
                    disabled={restoring || status.provider === 'none'}
                    className={`px-5 py-2.5 rounded-xl text-sm font-semibold flex items-center gap-2 transition-all ${
                      status.provider === 'none' 
                        ? 'bg-neutral-900 text-gray-600 cursor-not-allowed border border-neutral-800'
                        : 'glow-btn-cyan text-white cursor-pointer border border-cyan-500/30 hover:border-cyan-500'
                    }`}
                  >
                    <RefreshCw className={`h-4 w-4 ${restoring ? 'animate-spin' : ''}`} />
                    {restoring ? 'Restoring...' : 'Restore Now (Pull)'}
                  </button>
                  
                  {status.provider === 'none' && (
                    <p className="text-xs text-amber-400 flex items-center gap-1.5">
                      <Info className="h-4 w-4" /> Please configure a cloud provider in the Settings tab to start synchronization.
                    </p>
                  )}

                  {status.browser_running && (
                    <p className="text-[11px] text-amber-500 font-semibold flex items-center gap-1.5 w-full mt-2 bg-amber-950/20 border border-amber-900/30 p-2.5 rounded-xl">
                      <AlertTriangle className="h-4 w-4 text-amber-500 flex-shrink-0" />
                      Warning: Helium Browser is currently running. Manual sync/restore might fail or cause profile corruption. Please close the browser first!
                    </p>
                  )}
                </div>
              </div>

              {/* Linux DRM Fixer Card */}
              <div className="glass-panel rounded-2xl p-6 relative overflow-hidden">
                <div className="absolute top-0 right-0 p-4 opacity-5">
                  <Shield className="h-24 w-24" />
                </div>
                <div className="flex items-center justify-between mb-4">
                  <h3 className="text-lg font-semibold text-white flex items-center gap-2">
                    <Shield className="h-5 w-5 text-cyan-400" /> Linux DRM (Widevine) Fixer
                  </h3>
                  <span className="bg-cyan-500/10 text-cyan-400 text-xs px-2.5 py-1 rounded-full font-semibold uppercase tracking-wider">Linux Exclusive</span>
                </div>

                {status.platform !== 'linux' ? (
                  <div className="bg-neutral-950 p-4 rounded-xl border border-neutral-900 text-gray-400 text-sm">
                    <p>Widevine DRM Fixer is only applicable for Linux systems. Windows environments support DRM-protected contents by default.</p>
                  </div>
                ) : (
                  <div className="space-y-4">
                    <p className="text-sm text-gray-400">
                      Fix Netflix, Spotify, Prime Video playback issues caused by missing Widevine DRM certificates in Helium Browser. This searches for the Widevine module in Google Chrome or Brave and copies it into Helium's profile directory.
                    </p>
                    
                    <div className="flex items-center gap-3 bg-neutral-950 p-4 rounded-xl border border-neutral-900">
                      {status.drm_status === 'active' ? (
                        <>
                          <CheckCircle className="h-5 w-5 text-emerald-400 flex-shrink-0" />
                          <div>
                            <p className="text-sm font-semibold text-white">Widevine DRM Active</p>
                            <p className="text-xs text-gray-500 mt-0.5">Helium Browser is ready to play DRM-protected media files.</p>
                          </div>
                        </>
                      ) : (
                        <>
                          <AlertTriangle className="h-5 w-5 text-amber-500 flex-shrink-0" />
                          <div>
                            <p className="text-sm font-semibold text-white">Widevine DRM Missing</p>
                            <p className="text-xs text-gray-500 mt-0.5">Protected contents (Netflix, Spotify, etc.) might fail to play on Helium.</p>
                          </div>
                        </>
                      )}
                    </div>

                    <button
                      onClick={handleFixDrm}
                      disabled={fixingDrm}
                      className="px-5 py-2.5 rounded-xl text-sm font-semibold glow-btn-cyan text-white flex items-center gap-2 cursor-pointer"
                    >
                      <Shield className="h-4 w-4" />
                      {fixingDrm ? 'Applying Fix...' : 'Apply DRM Fix (Widevine)'}
                    </button>
                  </div>
                )}
              </div>
            </div>

            {/* Status Panel 2: Browser Status Info */}
            <div className="space-y-6">
              {/* Browser Status */}
              <div className="glass-panel rounded-2xl p-6">
                <h3 className="text-lg font-semibold text-white mb-4 flex items-center gap-2">
                  <Server className="h-5 w-5 text-purple-400" /> Browser Status
                </h3>

                <div className="space-y-4">
                  <div className="flex items-center justify-between bg-neutral-950 p-4 rounded-xl border border-neutral-900">
                    <span className="text-sm text-gray-400">Helium Browser</span>
                    <div className="flex items-center gap-2">
                      <span className={`h-2.5 w-2.5 rounded-full ${status.browser_running ? 'bg-emerald-500 animate-pulse' : 'bg-gray-600'}`}></span>
                      <span className={`text-sm font-semibold ${status.browser_running ? 'text-emerald-400' : 'text-gray-400'}`}>
                        {status.browser_running ? 'Running' : 'Closed'}
                      </span>
                    </div>
                  </div>

                  <div className="bg-neutral-950 p-4 rounded-xl border border-neutral-900">
                    <p className="text-xs text-gray-500 font-medium flex items-center gap-1">
                      <Folder className="h-3 w-3 text-purple-400" /> Profile Directory
                    </p>
                    <p className="text-xs text-gray-300 font-mono mt-1 break-all bg-black/40 p-2 rounded border border-neutral-900">
                      {status.profile_path || 'Not detected'}
                    </p>
                  </div>
                </div>
              </div>

              {/* Quick Info */}
              <div className="glass-accent rounded-2xl p-6 text-gray-400 text-xs leading-relaxed space-y-3">
                <h4 className="text-sm font-semibold text-white flex items-center gap-1.5">
                  <Info className="h-4 w-4 text-purple-400" /> How It Works
                </h4>
                <p>Helium Sync Daemon monitors Helium Browser in the background. When the browser is closed, your profile is zipped, encrypted locally, and pushed to the cloud.</p>
                <p>When the daemon starts or the system boots, it pulls the latest profile package from your cloud provider and restores it locally, resuming your sessions instantly.</p>
              </div>
            </div>
          </div>
        )}

        {activeTab === 'settings' && (
          <form onSubmit={handleSaveSettings} className="glass-panel rounded-2xl p-6 max-w-3xl mx-auto space-y-6">
            <h3 className="text-xl font-bold text-white mb-2 flex items-center gap-2 border-b border-neutral-900 pb-3">
              <SettingsIcon className="h-5 w-5 text-purple-400" /> Synchronization Settings
            </h3>

            {/* Provider Settings */}
            <div className="space-y-3">
              <label className="block text-sm font-semibold text-gray-300">Cloud Storage Provider</label>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                <label className={`flex items-center gap-3 p-4 rounded-xl border cursor-pointer transition-all ${
                  settings.provider === 'webdav' 
                    ? 'bg-purple-600/10 border-purple-500 text-white' 
                    : 'bg-neutral-950 border-neutral-900 text-gray-400 hover:border-neutral-800'
                }`}>
                  <input
                    type="radio"
                    name="provider"
                    value="webdav"
                    checked={settings.provider === 'webdav'}
                    onChange={(e) => setSettings({...settings, provider: e.target.value})}
                    className="sr-only"
                  />
                  <Server className="h-5 w-5" />
                  <span className="font-medium">WebDAV</span>
                </label>

                <label className={`flex items-center gap-3 p-4 rounded-xl border cursor-pointer transition-all ${
                  settings.provider === 'github_gist' 
                    ? 'bg-purple-600/10 border-purple-500 text-white' 
                    : 'bg-neutral-950 border-neutral-900 text-gray-400 hover:border-neutral-800'
                }`}>
                  <input
                    type="radio"
                    name="provider"
                    value="github_gist"
                    checked={settings.provider === 'github_gist'}
                    onChange={(e) => setSettings({...settings, provider: e.target.value})}
                    className="sr-only"
                  />
                  <Shield className="h-5 w-5" />
                  <span className="font-medium">GitHub Gist</span>
                </label>
              </div>
            </div>

            {/* Cloud Provider Specific Fields */}
            {settings.provider === 'github_gist' ? (
              <div className="bg-neutral-950 p-5 rounded-xl border border-neutral-900 space-y-4">
                <h4 className="text-sm font-semibold text-white">GitHub Gist Connection Details</h4>
                <p className="text-xs text-gray-500 leading-relaxed">
                  To sync your profile to a private Gist on your GitHub account, provide a **Personal Access Token (PAT)** with the `gist` permission scope.
                </p>
                <div className="grid grid-cols-1 gap-4">
                  <div className="space-y-1.5">
                    <label className="text-xs font-medium text-gray-400">GitHub Personal Access Token (PAT)</label>
                    <input
                      type="password"
                      value={settings.github_token}
                      onChange={(e) => setSettings({...settings, github_token: e.target.value})}
                      placeholder="ghp_xxxxxxxxxxxxxxxxxxxx"
                      className="w-full bg-[#030406] border border-neutral-900 rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-purple-500"
                    />
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-xs font-medium text-gray-400">Gist ID (Leave empty to create automatically on first sync)</label>
                    <input
                      type="text"
                      value={settings.github_gist_id}
                      onChange={(e) => setSettings({...settings, github_gist_id: e.target.value})}
                      placeholder="Gist ID (optional)"
                      className="w-full bg-[#030406] border border-neutral-900 rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-purple-500"
                    />
                  </div>
                </div>
              </div>
            ) : (
              <div className="bg-neutral-950 p-5 rounded-xl border border-neutral-900 space-y-4">
                <h4 className="text-sm font-semibold text-white">WebDAV Server Details</h4>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                  <div className="sm:col-span-2 space-y-1.5">
                    <label className="text-xs font-medium text-gray-400">Server Endpoint URL</label>
                    <input
                      type="text"
                      value={settings.webdav_url}
                      onChange={(e) => setSettings({...settings, webdav_url: e.target.value})}
                      placeholder="https://nextcloud.example.com/remote.php/dav/files/user/"
                      className="w-full bg-[#030406] border border-neutral-900 rounded-lg px-3 py-2 text-xs text-white placeholder-gray-600 focus:outline-none focus:border-purple-500"
                    />
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-xs font-medium text-gray-400">Username</label>
                    <input
                      type="text"
                      value={settings.webdav_username}
                      onChange={(e) => setSettings({...settings, webdav_username: e.target.value})}
                      className="w-full bg-[#030406] border border-neutral-900 rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-purple-500"
                    />
                  </div>
                  <div className="space-y-1.5">
                    <label className="text-xs font-medium text-gray-400">Password / Application Token</label>
                    <input
                      type="password"
                      value={settings.webdav_password}
                      onChange={(e) => setSettings({...settings, webdav_password: e.target.value})}
                      className="w-full bg-[#030406] border border-neutral-900 rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-purple-500"
                    />
                  </div>
                  <div className="sm:col-span-2 space-y-1.5">
                    <label className="text-xs font-medium text-gray-400">Target Folder Name</label>
                    <input
                      type="text"
                      value={settings.webdav_folder}
                      onChange={(e) => setSettings({...settings, webdav_folder: e.target.value})}
                      placeholder="helium-sync"
                      className="w-full bg-[#030406] border border-neutral-900 rounded-lg px-3 py-2 text-xs text-white focus:outline-none focus:border-purple-500"
                    />
                  </div>
                </div>
              </div>
            )}

            {/* Local Encryption */}
            <div className="bg-neutral-950 p-5 rounded-xl border border-neutral-900 space-y-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Shield className="h-5 w-5 text-purple-400" />
                  <div>
                    <h4 className="text-sm font-semibold text-white">Local Encrypted Backups</h4>
                    <p className="text-xs text-gray-500 mt-0.5">Encrypt files locally with AES-256 before transferring to the cloud.</p>
                  </div>
                </div>
                <div>
                  <label className="relative inline-flex items-center cursor-pointer">
                    <input
                      type="checkbox"
                      checked={settings.encryption_active}
                      onChange={(e) => setSettings({...settings, encryption_active: e.target.checked})}
                      className="sr-only peer"
                    />
                    <div className="w-9 h-5 bg-neutral-800 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-gray-400 after:border-gray-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-purple-600 peer-checked:after:bg-white"></div>
                  </label>
                </div>
              </div>

              {settings.encryption_active && (
                <div className="space-y-1.5 transition-all">
                  <label className="text-xs font-medium text-gray-400">Encryption Password</label>
                  <div className="relative">
                    <input
                      type={showPassword ? 'text' : 'password'}
                      value={settings.encryption_password}
                      onChange={(e) => setSettings({...settings, encryption_password: e.target.value})}
                      placeholder="Enter a strong passphrase"
                      className="w-full bg-[#030406] border border-neutral-900 rounded-lg pl-3 pr-10 py-2 text-xs text-white focus:outline-none focus:border-purple-500"
                    />
                    <button
                      type="button"
                      onClick={() => setShowPassword(!showPassword)}
                      className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-500 hover:text-gray-300"
                    >
                      {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                    </button>
                  </div>
                  <p className="text-[10px] text-amber-500">
                    * IMPORTANT: If you lose this password, you cannot decrypt and restore your profile backups.
                  </p>
                </div>
              )}
            </div>

            {/* Advanced Settings */}
            <div className="bg-neutral-950 p-5 rounded-xl border border-neutral-900 space-y-4">
              <h4 className="text-sm font-semibold text-white flex items-center gap-1">
                Advanced Configurations
              </h4>
              
              <div className="space-y-4">
                <div className="space-y-1.5">
                  <label className="text-xs font-medium text-gray-400">Custom Helium Profile Directory Path (Leave empty to find automatically)</label>
                  <input
                    type="text"
                    value={settings.profile_path}
                    onChange={(e) => setSettings({...settings, profile_path: e.target.value})}
                    placeholder="e.g. /home/user/.config/net.imput.helium"
                    className="w-full bg-[#030406] border border-neutral-900 rounded-lg px-3 py-2 text-xs text-white placeholder-gray-700 focus:outline-none focus:border-purple-500"
                  />
                </div>


              </div>
            </div>

            {/* Save Buttons */}
            <div className="flex items-center gap-4 border-t border-neutral-900 pt-4">
              <button
                type="submit"
                className="px-6 py-2.5 rounded-xl text-sm font-semibold glow-btn-primary text-white cursor-pointer"
              >
                Save Configurations
              </button>
              
              {saveSuccess && (
                <span className="text-sm font-semibold text-emerald-400 flex items-center gap-1.5 animate-fade-in">
                  <CheckCircle className="h-4 w-4" /> Configurations updated successfully!
                </span>
              )}
            </div>
          </form>
        )}

        {activeTab === 'logs' && (
          <div className="glass-panel rounded-2xl p-6 max-w-4xl mx-auto flex flex-col h-[500px]">
            <div className="flex items-center justify-between border-b border-neutral-900 pb-3 mb-4">
              <h3 className="text-lg font-semibold text-white flex items-center gap-2">
                <Terminal className="h-5 w-5 text-purple-400" /> Service Logs Console
              </h3>
              <div className="flex gap-2">
                <button
                  onClick={() => setLogs([])}
                  className="px-3 py-1.5 bg-neutral-950 hover:bg-neutral-900 border border-neutral-900 text-gray-400 hover:text-white rounded-lg text-xs transition-all cursor-pointer"
                >
                  Clear Logs
                </button>
                <button
                  onClick={fetchLogs}
                  className="p-1.5 bg-neutral-950 hover:bg-neutral-900 border border-neutral-900 text-gray-400 hover:text-white rounded-lg text-xs transition-all cursor-pointer"
                >
                  <RefreshCw className="h-4 w-4" />
                </button>
              </div>
            </div>

            {/* Terminal output console */}
            <div className="flex-grow overflow-y-auto rounded-xl p-4 terminal-box text-xs text-[#58e182] font-mono leading-relaxed space-y-1">
              {logs.length === 0 ? (
                <div className="text-gray-600 text-center py-10">No log records found.</div>
              ) : (
                logs.map((log, index) => {
                  let colorClass = 'text-[#58e182]'; // Success / Info
                  if (log.includes('[ERROR]') || log.includes('failed') || log.includes('Error')) {
                    colorClass = 'text-red-400';
                  } else if (log.includes('[WARN]') || log.includes('Warning') || log.includes('skipping')) {
                    colorClass = 'text-amber-400';
                  } else if (log.includes('[DEBUG]')) {
                    colorClass = 'text-gray-500';
                  }
                  
                  return (
                    <div key={index} className={`${colorClass} break-all whitespace-pre-wrap`}>
                      {log}
                    </div>
                  );
                })
              )}
              <div ref={logEndRef} />
            </div>
          </div>
        )}
      </main>

      {/* Footer */}
      <footer className="max-w-5xl w-full mx-auto border-t border-neutral-900 pt-4 flex flex-col sm:flex-row items-center justify-between text-xs text-gray-600 gap-2">
        <p>© 2026 Helium Sync Daemon. All rights reserved.</p>
        <p>Version: 0.2.0 • MIT License</p>
      </footer>
    </div>
  );
}

export default App;
