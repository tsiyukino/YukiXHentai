use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::models::ExhCookies;

/// Top-level application config, persisted as TOML.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub storage: StorageConfig,
}

/// UI preferences section of the config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Detail panel preview thumbnail size in pixels (80–200). Default: 120.
    #[serde(default = "default_detail_preview_size")]
    pub detail_preview_size: u32,
    /// Color theme: "light" or "dark". Default: "light".
    #[serde(default = "default_theme")]
    pub theme: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            detail_preview_size: default_detail_preview_size(),
            theme: default_theme(),
        }
    }
}

fn default_detail_preview_size() -> u32 {
    120
}

fn default_theme() -> String {
    "light".to_string()
}

/// Storage preferences section of the config.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorageConfig {
    /// Custom cache directory. None = platform default.
    pub cache_dir: Option<String>,
    /// Maximum size of the originals read cache in megabytes. Range: 128–4096. Default: 512.
    #[serde(default = "default_read_cache_max_mb")]
    pub read_cache_max_mb: u64,
    /// Custom library directory. None = platform default data_local_dir/yukixhentai/library.
    #[serde(default)]
    pub library_dir: Option<String>,
}

fn default_read_cache_max_mb() -> u64 {
    512
}

/// Authentication section of the config.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    pub ipb_member_id: Option<String>,
    pub ipb_pass_hash: Option<String>,
    pub igneous: Option<String>,
}

impl AuthConfig {
    pub fn has_cookies(&self) -> bool {
        self.ipb_member_id.is_some()
            && self.ipb_pass_hash.is_some()
            && self.igneous.is_some()
    }

    pub fn to_cookies(&self) -> Option<ExhCookies> {
        Some(ExhCookies {
            ipb_member_id: self.ipb_member_id.clone()?,
            ipb_pass_hash: self.ipb_pass_hash.clone()?,
            igneous: self.igneous.clone()?,
        })
    }

    pub fn set_cookies(&mut self, cookies: &ExhCookies) {
        self.ipb_member_id = Some(cookies.ipb_member_id.clone());
        self.ipb_pass_hash = Some(cookies.ipb_pass_hash.clone());
        self.igneous = Some(cookies.igneous.clone());
    }

    pub fn clear(&mut self) {
        self.ipb_member_id = None;
        self.ipb_pass_hash = None;
        self.igneous = None;
    }
}

/// Thread-safe handle to the config, stored as Tauri managed state.
pub struct ConfigState {
    pub config: Mutex<AppConfig>,
    pub path: PathBuf,
}

impl ConfigState {
    /// Load config from disk, or create default if file doesn't exist.
    pub fn load(config_dir: PathBuf) -> Self {
        let path = config_dir.join("config.toml");
        let config = if path.exists() {
            let content = fs::read_to_string(&path).unwrap_or_default();
            toml::from_str(&content).unwrap_or_default()
        } else {
            AppConfig::default()
        };
        Self {
            config: Mutex::new(config),
            path,
        }
    }

    /// Write current config to disk.
    pub fn save(&self) -> Result<(), String> {
        let config = self.config.lock().map_err(|e| e.to_string())?;
        let content = toml::to_string_pretty(&*config).map_err(|e| e.to_string())?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&self.path, content).map_err(|e| e.to_string())?;
        Ok(())
    }
}
