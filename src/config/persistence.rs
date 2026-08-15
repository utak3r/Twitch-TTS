use super::AppConfig;
use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};
use tracing::{error, info, warn};

pub const DEFAULT_CONFIG_PATH: &str = "config.yaml";

#[derive(Clone)]
pub struct ConfigManager {
    config_path: String,
    current: Arc<RwLock<AppConfig>>,
}

impl ConfigManager {
    pub fn new(path: &str) -> Self {
        let config = Self::load_or_default(path);
        Self {
            config_path: path.to_string(),
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
        
        fs::write(&self.config_path, yaml_str)
            .map_err(|e| format!("Failed to write config file {}: {}", self.config_path, e))?;
        
        info!("Configuration saved to {}", self.config_path);
        Ok(())
    }

    pub fn load_or_default(path: &str) -> AppConfig {
        if !Path::new(path).exists() {
            info!("Config file {} not found. Creating default configuration.", path);
            let default_cfg = AppConfig::default();
            if let Ok(yaml_str) = serde_yaml::to_string(&default_cfg) {
                let _ = fs::write(path, yaml_str);
            }
            return default_cfg;
        }

        match fs::read_to_string(path) {
            Ok(content) => match serde_yaml::from_str::<AppConfig>(&content) {
                Ok(cfg) => {
                    info!("Successfully loaded config from {}", path);
                    cfg
                }
                Err(err) => {
                    warn!("Failed to parse {}: {}. Using default config.", path, err);
                    AppConfig::default()
                }
            },
            Err(err) => {
                error!("Failed to read {}: {}. Using default config.", path, err);
                AppConfig::default()
            }
        }
    }
}
