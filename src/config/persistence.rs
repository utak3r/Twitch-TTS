use super::AppConfig;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::{error, info, warn};

pub fn get_app_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".twitch-tts"))
        .unwrap_or_else(|| PathBuf::from(".twitch-tts"))
}

pub fn default_config_path() -> PathBuf {
    get_app_dir().join("config.yaml")
}

#[derive(Clone)]
pub struct ConfigManager {
    config_path: PathBuf,
    current: Arc<RwLock<AppConfig>>,
}

impl ConfigManager {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        let config = Self::load_or_default(path.as_ref());
        Self {
            config_path: path.as_ref().to_path_buf(),
            current: Arc::new(RwLock::new(config)),
        }
    }

    pub fn get(&self) -> AppConfig {
        self.current.read().unwrap().clone()
    }

    pub fn update<F>(&self, update_fn: F) -> Result<AppConfig, String>
    where
        F: FnOnce(&mut AppConfig),
    {
        let mut cfg = self.current.write().unwrap();
        update_fn(&mut cfg);
        let cloned = cfg.clone();
        drop(cfg);

        self.save(&cloned)?;
        Ok(cloned)
    }

    pub fn save(&self, config: &AppConfig) -> Result<(), String> {
        let yaml_str = serde_yaml::to_string(config)
            .map_err(|e| format!("Failed to serialize config to YAML: {}", e))?;

        if let Some(parent) = self.config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        fs::write(&self.config_path, yaml_str)
            .map_err(|e| format!("Failed to write config file {}: {}", self.config_path.display(), e))?;

        info!("Configuration saved to {}", self.config_path.display());
        Ok(())
    }

    pub fn load_or_default<P: AsRef<Path>>(path: P) -> AppConfig {
        let path = path.as_ref();
        if !path.exists() {
            info!("Config file {} not found. Creating default configuration.", path.display());
            let default_cfg = AppConfig::default();
            if let Ok(yaml_str) = serde_yaml::to_string(&default_cfg) {
                if let Some(parent) = path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(path, yaml_str);
            }
            return default_cfg;
        }

        match fs::read_to_string(path) {
            Ok(content) => match serde_yaml::from_str::<AppConfig>(&content) {
                Ok(cfg) => {
                    info!("Successfully loaded config from {}", path.display());
                    cfg
                }
                Err(err) => {
                    warn!("Failed to parse {}: {}. Using default config.", path.display(), err);
                    AppConfig::default()
                }
            },
            Err(err) => {
                error!("Failed to read {}: {}. Using default config.", path.display(), err);
                AppConfig::default()
            }
        }
    }
}

