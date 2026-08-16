use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use twitch_tts::audio::player::AudioPlayer;
use twitch_tts::config::persistence::ConfigManager;
use twitch_tts::domain::models::{ChatEvent, MessageStatus, SpokenItem};
use twitch_tts::domain::queue::OverflowQueue;
use twitch_tts::filter::TextFilter;
use twitch_tts::hotkeys::manager::HotkeysManager;
use twitch_tts::tts::mock::MockTTSEngine;
use twitch_tts::twitch::TwitchCoordinator;
use twitch_tts::ui::bridge::{add_activity_row, AppState};

fn create_test_state() -> Arc<AppState> {
    let (chat_tx, _) = mpsc::unbounded_channel::<ChatEvent>();
    let (status_tx, _) = mpsc::unbounded_channel::<String>();

    Arc::new(AppState {
        config_manager: ConfigManager::new("target/test_activity_config.yaml"),
        filter: Arc::new(Mutex::new(TextFilter::new(Default::default()))),
        tts: Arc::new(Mutex::new(Box::new(MockTTSEngine::new()))),
        audio_player: Arc::new(AudioPlayer::new("default", 1.0)),
        queue: OverflowQueue::new(10),
        twitch: Arc::new(Mutex::new(TwitchCoordinator::new(Default::default()))),
        hotkeys: Arc::new(Mutex::new(HotkeysManager::new())),
        activity_history: Arc::new(Mutex::new(Vec::new())),
        main_window: slint::Weak::default(),
        chat_tx,
        status_tx,
    })
}

#[test]
fn test_add_activity_row_deduplication_and_in_place_update() {
    let state = create_test_state();

    let mut item = SpokenItem::new(
        "alice".into(),
        "hello world".into(),
        "hello world".into(),
        MessageStatus::Queued,
    );

    // 1. Initially added as Queued
    add_activity_row(&state, item.clone());
    {
        let history = state.activity_history.lock().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, item.id);
        assert_eq!(history[0].status, MessageStatus::Queued);
    }

    // 2. Updated to Playing - should update in place, not duplicate
    item.status = MessageStatus::Playing;
    add_activity_row(&state, item.clone());
    {
        let history = state.activity_history.lock().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, item.id);
        assert_eq!(history[0].status, MessageStatus::Playing);
    }

    // 3. Updated to Spoken - should update in place, not duplicate
    item.status = MessageStatus::Spoken;
    add_activity_row(&state, item.clone());
    {
        let history = state.activity_history.lock().unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, item.id);
        assert_eq!(history[0].status, MessageStatus::Spoken);
    }

    // 4. Add another distinct item
    let item2 = SpokenItem::new(
        "bob".into(),
        "second message".into(),
        "second message".into(),
        MessageStatus::Queued,
    );
    add_activity_row(&state, item2.clone());
    {
        let history = state.activity_history.lock().unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].id, item2.id); // Newest is at index 0
        assert_eq!(history[1].id, item.id);
    }
}

#[test]
fn test_activity_history_max_limit_100() {
    let state = create_test_state();

    for i in 0..120 {
        let item = SpokenItem::new(
            format!("user_{}", i),
            format!("msg_{}", i),
            format!("msg_{}", i),
            MessageStatus::Queued,
        );
        add_activity_row(&state, item);
    }

    let history = state.activity_history.lock().unwrap();
    assert_eq!(history.len(), 100);
    // The latest inserted item (user_119) should be at index 0
    assert_eq!(history[0].sender, "user_119");
}
