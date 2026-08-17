use twitch_tts::config::persistence::{default_config_path, get_app_dir, ConfigManager};
use twitch_tts::config::AppConfig;

#[test]
fn test_default_config() {
    let cfg = AppConfig::default();
    assert_eq!(cfg.tts.model_path, "./models/pl_zenski_1.onnx");
    assert_eq!(cfg.tts.config_path, "./models/pl_zenski_1.onnx.json");
    assert_eq!(cfg.tts.max_queue_size, 5);
    assert_eq!(cfg.filters.max_characters, 150);
    assert!(cfg.filters.username_aliases.contains_key("utak3r"));
}

#[test]
fn test_app_dir_and_default_config_path() {
    let app_dir = get_app_dir();
    assert!(app_dir.ends_with(".twitch-tts"));

    let config_path = default_config_path();
    assert_eq!(config_path, app_dir.join("config.yaml"));
}

#[test]
fn test_config_serialization() {
    let temp_path = "target/test_config.yaml";
    let _ = std::fs::create_dir_all("target");

    let manager = ConfigManager::new(temp_path);
    let mut config = manager.get();
    config.twitch.oauth_token = "test_oauth_token_123".to_string();
    manager.save(&config).expect("Failed to save config");

    let loaded = ConfigManager::load_or_default(temp_path);
    assert_eq!(loaded.twitch.oauth_token, "test_oauth_token_123");

    let _ = std::fs::remove_file(temp_path);
}
