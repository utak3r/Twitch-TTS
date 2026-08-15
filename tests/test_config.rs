use twitch_tts::config::persistence::ConfigManager;
use twitch_tts::config::AppConfig;

#[test]
fn test_default_config() {
    let cfg = AppConfig::default();
    assert_eq!(cfg.tts.model_path, "./models/voice.onnx");
    assert_eq!(cfg.tts.config_path, "./models/voice.onnx.json");
    assert_eq!(cfg.tts.max_queue_size, 5);
    assert_eq!(cfg.filters.max_characters, 150);
    assert!(cfg.filters.username_aliases.contains_key("utak3r"));
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
