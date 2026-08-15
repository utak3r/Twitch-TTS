use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageStatus {
    Queued,
    Playing,
    Spoken,
    FilteredProfanity,
    FilteredEmote,
    IgnoredBot,
    DroppedOverflow,
    Skipped,
    Error(String),
}

impl MessageStatus {
    pub fn display_label(&self) -> String {
        match self {
            MessageStatus::Queued => "Queued".to_string(),
            MessageStatus::Playing => "Playing ⏳".to_string(),
            MessageStatus::Spoken => "Spoken ✓".to_string(),
            MessageStatus::FilteredProfanity => "Filtered [Profanity]".to_string(),
            MessageStatus::FilteredEmote => "Filtered [Emote]".to_string(),
            MessageStatus::IgnoredBot => "Ignored [Bot]".to_string(),
            MessageStatus::DroppedOverflow => "Dropped [Overflow]".to_string(),
            MessageStatus::Skipped => "Skipped".to_string(),
            MessageStatus::Error(err) => format!("Error: {}", err),
        }
    }

    pub fn status_type(&self) -> &'static str {
        match self {
            MessageStatus::Queued => "playing",
            MessageStatus::Playing => "playing",
            MessageStatus::Spoken => "spoken",
            MessageStatus::FilteredProfanity | MessageStatus::FilteredEmote => "filtered",
            MessageStatus::IgnoredBot => "ignored",
            MessageStatus::DroppedOverflow => "dropped",
            MessageStatus::Skipped => "skipped",
            MessageStatus::Error(_) => "dropped",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpokenItem {
    pub id: Uuid,
    pub timestamp: DateTime<Local>,
    pub sender: String,
    pub original_text: String,
    pub spoken_text: String,
    pub status: MessageStatus,
}

impl SpokenItem {
    pub fn new(sender: String, original_text: String, spoken_text: String, status: MessageStatus) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Local::now(),
            sender,
            original_text,
            spoken_text,
            status,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChatEvent {
    pub user: String,
    pub text: String,
    pub emotes: Vec<String>,
    pub is_custom_reward: bool,
}

#[derive(Debug, Clone)]
pub enum FilterResult {
    Ready(SpokenItem),
    Ignored(SpokenItem),
    Filtered(SpokenItem),
}
