pub mod persistence;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub app: AppSection,
    #[serde(default)]
    pub twitch: TwitchConfig,
    #[serde(default)]
    pub tts: TTSConfig,
    #[serde(default)]
    pub filters: FiltersConfig,
    #[serde(default)]
    pub hotkeys: HotkeysConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            app: AppSection::default(),
            twitch: TwitchConfig::default(),
            tts: TTSConfig::default(),
            filters: FiltersConfig::default(),
            hotkeys: HotkeysConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSection {
    #[serde(default)]
    pub test_mode: bool,
    #[serde(default)]
    pub minimize_to_tray: bool,
}

impl Default for AppSection {
    fn default() -> Self {
        Self {
            test_mode: false,
            minimize_to_tray: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwitchConfig {
    #[serde(default)]
    pub oauth_token: String,
    #[serde(default)]
    pub broadcaster_user_id: String,
    #[serde(default = "default_true")]
    pub read_all_chat: bool,
    #[serde(default)]
    pub reward_id: String,
}

impl Default for TwitchConfig {
    fn default() -> Self {
        Self {
            oauth_token: String::new(),
            broadcaster_user_id: String::new(),
            read_all_chat: true,
            reward_id: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TTSConfig {
    #[serde(default = "default_model_path")]
    pub model_path: String,
    #[serde(default = "default_config_path")]
    pub config_path: String,
    #[serde(default)]
    pub speaker_id: i64,
    #[serde(default = "default_speech_rate")]
    pub speech_rate: f32,
    #[serde(default = "default_max_characters")]
    pub max_characters: usize,
    #[serde(default = "default_max_queue_size")]
    pub max_queue_size: usize,
    #[serde(default = "default_audio_device")]
    pub audio_device_name: String,
    #[serde(default = "default_padding_sec")]
    pub padding_sec: f32,
    #[serde(default = "default_volume")]
    pub volume: f32,
}

fn default_model_path() -> String {
    "./models/pl_zenski_1.onnx".to_string()
}
fn default_config_path() -> String {
    "./models/pl_zenski_1.onnx.json".to_string()
}
fn default_speech_rate() -> f32 {
    1.0
}
fn default_max_characters() -> usize {
    150
}
fn default_max_queue_size() -> usize {
    5
}
fn default_audio_device() -> String {
    "Default".to_string()
}
fn default_padding_sec() -> f32 {
    0.3
}
fn default_volume() -> f32 {
    1.0
}

impl Default for TTSConfig {
    fn default() -> Self {
        Self {
            model_path: default_model_path(),
            config_path: default_config_path(),
            speaker_id: 0,
            speech_rate: default_speech_rate(),
            max_characters: default_max_characters(),
            max_queue_size: default_max_queue_size(),
            audio_device_name: default_audio_device(),
            padding_sec: default_padding_sec(),
            volume: default_volume(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiltersConfig {
    #[serde(default = "default_true")]
    pub announce_username: bool,
    #[serde(default = "default_username_template")]
    pub username_template: String,
    #[serde(default = "default_true")]
    pub enable_profanity_filter: bool,
    #[serde(default = "default_profanity_words_file")]
    pub profanity_words_file: String,
    #[serde(default = "default_true")]
    pub filter_emotes: bool,
    #[serde(default = "default_ignore_users")]
    pub ignore_users: Vec<String>,
    #[serde(default = "default_max_characters")]
    pub max_characters: usize,
    #[serde(default = "default_max_repeated_chars")]
    pub max_repeated_chars: usize,
    #[serde(default = "default_aliases")]
    pub username_aliases: HashMap<String, String>,
}

fn default_true() -> bool {
    true
}
fn default_username_template() -> String {
    "{nick} mówi: {message}".to_string()
}
fn default_profanity_words_file() -> String {
    "profanity_words.txt".to_string()
}
fn default_max_repeated_chars() -> usize {
    3
}
fn default_ignore_users() -> Vec<String> {
    vec![
        "Nightbot".to_string(),
        "StreamElements".to_string(),
        "Moobot".to_string(),
    ]
}
fn default_aliases() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("utak3r".to_string(), "utaker".to_string());
    map.insert("Dr3gu".to_string(), "Dregu".to_string());
    map.insert("masi4m".to_string(), "masiam".to_string());
    map.insert("ok".to_string(), "okej".to_string());
    map.insert("stream".to_string(), "strim".to_string());
    map
}

impl Default for FiltersConfig {
    fn default() -> Self {
        Self {
            announce_username: true,
            username_template: default_username_template(),
            enable_profanity_filter: true,
            profanity_words_file: default_profanity_words_file(),
            filter_emotes: true,
            ignore_users: default_ignore_users(),
            max_characters: default_max_characters(),
            max_repeated_chars: default_max_repeated_chars(),
            username_aliases: default_aliases(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeysConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_mute_hotkey")]
    pub mute_toggle: String,
    #[serde(default = "default_skip_hotkey")]
    pub skip_current: String,
}

fn default_mute_hotkey() -> String {
    "F9".to_string()
}
fn default_skip_hotkey() -> String {
    "F10".to_string()
}

impl Default for HotkeysConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mute_toggle: default_mute_hotkey(),
            skip_current: default_skip_hotkey(),
        }
    }
}
