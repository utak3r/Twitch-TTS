use crate::audio::devices::AudioDeviceManager;
use crate::audio::player::AudioPlayer;
use crate::config::persistence::ConfigManager;
use crate::config::AppConfig;
use crate::domain::models::{ChatEvent, FilterResult, MessageStatus, SpokenItem};
use crate::domain::queue::OverflowQueue;
use crate::filter::profanity::ProfanityFilter;
use crate::filter::TextFilter;
use crate::hotkeys::manager::{HotkeyAction, HotkeysManager};
use crate::tts::piper::PiperEngine;
use crate::tts::{export_wav_file, TTSEngine};
use crate::twitch::auth::OAuthServer;
use crate::twitch::TwitchCoordinator;
use crate::{ActivityItem, AliasItem, IgnoredUserItem, MainWindow};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::fs;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{error, info};

pub struct AppState {
    pub config_manager: ConfigManager,
    pub filter: Arc<Mutex<TextFilter>>,
    pub tts: Arc<Mutex<Box<dyn TTSEngine>>>,
    pub audio_player: Arc<AudioPlayer>,
    pub queue: OverflowQueue,
    pub twitch: Arc<Mutex<TwitchCoordinator>>,
    pub hotkeys: Arc<Mutex<HotkeysManager>>,
    pub activity_history: Arc<Mutex<Vec<SpokenItem>>>,
    pub main_window: slint::Weak<MainWindow>,
    pub chat_tx: mpsc::UnboundedSender<ChatEvent>,
    pub status_tx: mpsc::UnboundedSender<String>,
}

pub fn setup_ui_bridge(
    main_window: &MainWindow,
    config_manager: ConfigManager,
    hotkey_action_rx: mpsc::UnboundedReceiver<HotkeyAction>,
) -> Arc<AppState> {
    let cfg = config_manager.get();

    let (chat_tx, chat_rx) = mpsc::unbounded_channel::<ChatEvent>();
    let (status_tx, status_rx) = mpsc::unbounded_channel::<String>();

    let filter = Arc::new(Mutex::new(TextFilter::new(cfg.filters.clone())));
    let tts_engine: Box<dyn TTSEngine> = Box::new(PiperEngine::new(
        &cfg.tts.model_path,
        &cfg.tts.config_path,
        cfg.tts.speaker_id,
    ));
    let tts = Arc::new(Mutex::new(tts_engine));
    let audio_player = Arc::new(AudioPlayer::new(
        &cfg.tts.audio_device_name,
        cfg.tts.volume,
    ));
    let queue = OverflowQueue::new(cfg.tts.max_queue_size);
    let twitch = Arc::new(Mutex::new(TwitchCoordinator::new(cfg.twitch.clone())));
    let hotkeys = Arc::new(Mutex::new(HotkeysManager::new()));
    let activity_history = Arc::new(Mutex::new(Vec::new()));

    let app_state = Arc::new(AppState {
        config_manager: config_manager.clone(),
        filter: filter.clone(),
        tts: tts.clone(),
        audio_player: audio_player.clone(),
        queue: queue.clone(),
        twitch: twitch.clone(),
        hotkeys: hotkeys.clone(),
        activity_history: activity_history.clone(),
        main_window: main_window.as_weak(),
        chat_tx: chat_tx.clone(),
        status_tx: status_tx.clone(),
    });

    // Populate initial UI properties from config
    populate_ui_from_config(main_window, &cfg);

    // Register Callbacks
    register_ui_callbacks(main_window, app_state.clone());

    // Spawn Background Audio Processing Worker
    spawn_playback_worker(app_state.clone());

    // Spawn Twitch Event Handler Worker
    spawn_twitch_event_handler(app_state.clone(), chat_rx);
    spawn_twitch_status_handler(app_state.clone(), status_rx);

    // Spawn Hotkey Action Handler Worker
    spawn_hotkey_handler(app_state.clone(), hotkey_action_rx);

    app_state
}

fn populate_ui_from_config(ui: &MainWindow, cfg: &AppConfig) {
    ui.set_channel_name(cfg.twitch.broadcaster_user_id.clone().into());
    ui.set_volume(cfg.tts.volume);
    ui.set_queue_count(0);
    ui.set_is_muted(false);

    // Filters
    ui.set_enable_profanity(cfg.filters.enable_profanity_filter);
    ui.set_filter_emotes(cfg.filters.filter_emotes);
    ui.set_announce_username(cfg.filters.announce_username);
    ui.set_username_template(cfg.filters.username_template.clone().into());
    ui.set_max_characters(cfg.filters.max_characters as i32);
    ui.set_max_repeated_chars(cfg.filters.max_repeated_chars as i32);

    // Aliases Model
    let mut alias_items: Vec<AliasItem> = cfg
        .filters
        .username_aliases
        .iter()
        .map(|(k, v)| AliasItem {
            key: k.clone().into(),
            value: v.clone().into(),
        })
        .collect();
    alias_items.sort_by(|a, b| a.key.to_lowercase().cmp(&b.key.to_lowercase()));
    ui.set_aliases(ModelRc::new(VecModel::from(alias_items)));

    // Ignored Users Model
    let ignored_items: Vec<IgnoredUserItem> = cfg
        .filters
        .ignore_users
        .iter()
        .map(|u| IgnoredUserItem { name: u.clone().into() })
        .collect();
    ui.set_ignored_users(ModelRc::new(VecModel::from(ignored_items)));

    // Profanity Words Raw
    let words = ProfanityFilter::load_words(&cfg.filters.profanity_words_file);
    ui.set_profanity_words_raw(words.join("\n").into());

    // Audio View
    ui.set_model_path(cfg.tts.model_path.clone().into());
    ui.set_config_path(cfg.tts.config_path.clone().into());
    ui.set_speaker_id(cfg.tts.speaker_id as i32);
    ui.set_speech_rate(cfg.tts.speech_rate);
    ui.set_selected_device_name(cfg.tts.audio_device_name.clone().into());
    ui.set_padding_sec(cfg.tts.padding_sec);

    let devices = AudioDeviceManager::list_output_devices();
    let dev_model: Vec<SharedString> = devices.into_iter().map(|d| d.into()).collect();
    ui.set_available_devices(ModelRc::new(VecModel::from(dev_model)));

    // Twitch View
    ui.set_twitch_oauth_token(cfg.twitch.oauth_token.clone().into());
    ui.set_broadcaster_user_id(cfg.twitch.broadcaster_user_id.clone().into());
    ui.set_read_all_chat(cfg.twitch.read_all_chat);
    ui.set_reward_id(cfg.twitch.reward_id.clone().into());

    // Settings View
    ui.set_hotkeys_enabled(cfg.hotkeys.enabled);
    ui.set_mute_hotkey(cfg.hotkeys.mute_toggle.clone().into());
    ui.set_skip_hotkey(cfg.hotkeys.skip_current.clone().into());
    ui.set_minimize_to_tray(cfg.app.minimize_to_tray);

    // Trigger initial test lab preview
    let filter = TextFilter::new(cfg.filters.clone());
    let (aliased, censored, truncated) = filter.inspect_stages(&ui.get_test_text());
    ui.set_transformed_aliases(aliased.into());
    ui.set_transformed_censored(censored.into());
    ui.set_transformed_final(truncated.into());
}

fn register_ui_callbacks(ui: &MainWindow, state: Arc<AppState>) {
    // Mute Toggle
    let state_c = state.clone();
    ui.on_toggle_mute(move || {
        let new_muted = !state_c.audio_player.is_muted();
        state_c.audio_player.set_muted(new_muted);

        if let Some(w) = state_c.main_window.upgrade() {
            w.set_is_muted(new_muted);
        }
    });

    // Skip Current
    let state_c = state.clone();
    ui.on_skip_current(move || {
        state_c.audio_player.stop();
    });

    // Clear Queue
    let state_c = state.clone();
    ui.on_clear_queue(move || {
        let dropped = state_c.queue.clear();
        for item in dropped {
            add_activity_row(&state_c, item);
        }
        if let Some(w) = state_c.main_window.upgrade() {
            w.set_queue_count(0);
        }
    });

    // Quick Synthesize
    let state_c = state.clone();
    ui.on_quick_synthesize(move |text| {
        let filter = state_c.filter.lock().unwrap();
        let res = filter.process("Streamer", &text, false);
        drop(filter);

        match res {
            FilterResult::Ready(item) => {
                let dropped = state_c.queue.push(item.clone());
                if let Some(d) = dropped {
                    add_activity_row(&state_c, d);
                }
                add_activity_row(&state_c, item);
                if let Some(w) = state_c.main_window.upgrade() {
                    w.set_queue_count(state_c.queue.len() as i32);
                }
            }
            FilterResult::Filtered(item) | FilterResult::Ignored(item) => {
                add_activity_row(&state_c, item);
            }
        }
    });

    // Volume Changed
    let state_c = state.clone();
    ui.on_volume_changed(move |vol| {
        state_c.audio_player.set_volume(vol);
        let _ = state_c.config_manager.update(|cfg| {
            cfg.tts.volume = vol;
        });
    });

    // Replay Item
    let state_c = state.clone();
    ui.on_replay_item(move |id_str| {
        let item_opt = {
            let history = state_c.activity_history.lock().unwrap();
            history.iter().find(|i| i.id.to_string() == id_str.as_str()).cloned()
        };

        if let Some(item) = item_opt {
            let mut replay_item = item;
            replay_item.id = uuid::Uuid::new_v4();
            replay_item.timestamp = chrono::Local::now();
            replay_item.status = MessageStatus::Queued;
            let dropped = state_c.queue.push(replay_item.clone());
            if let Some(d) = dropped {
                add_activity_row(&state_c, d);
            }
            add_activity_row(&state_c, replay_item);
            if let Some(w) = state_c.main_window.upgrade() {
                w.set_queue_count(state_c.queue.len() as i32);
            }
        }
    });

    // Add Alias For User
    let state_c = state.clone();
    ui.on_add_alias_for_user(move |user| {
        if user.is_empty() { return; }
        let _ = state_c.config_manager.update(|cfg| {
            cfg.filters.username_aliases.insert(user.to_string(), user.to_string());
        });
        let mut filter = state_c.filter.lock().unwrap();
        filter.update_config(state_c.config_manager.get().filters);
        if let Some(w) = state_c.main_window.upgrade() {
            let aliases = state_c.config_manager.get().filters.username_aliases;
            let mut alias_items: Vec<AliasItem> = aliases
                .into_iter()
                .map(|(k, v)| AliasItem { key: k.into(), value: v.into() })
                .collect();
            alias_items.sort_by(|a, b| a.key.to_lowercase().cmp(&b.key.to_lowercase()));
            w.set_aliases(ModelRc::new(VecModel::from(alias_items)));
        }
    });

    // Ignore User
    let state_c = state.clone();
    ui.on_ignore_user(move |user| {
        if user.is_empty() { return; }
        let _ = state_c.config_manager.update(|cfg| {
            if !cfg.filters.ignore_users.contains(&user.to_string()) {
                cfg.filters.ignore_users.push(user.to_string());
            }
        });
        let mut filter = state_c.filter.lock().unwrap();
        filter.update_config(state_c.config_manager.get().filters);
        if let Some(w) = state_c.main_window.upgrade() {
            let ignored = state_c.config_manager.get().filters.ignore_users;
            let items: Vec<IgnoredUserItem> = ignored
                .into_iter()
                .map(|u| IgnoredUserItem { name: u.into() })
                .collect();
            w.set_ignored_users(ModelRc::new(VecModel::from(items)));
        }
    });

    // Test Lab: Text Changed
    let state_c = state.clone();
    ui.on_test_text_changed(move |txt| {
        let filter = state_c.filter.lock().unwrap();
        let (aliased, censored, truncated) = filter.inspect_stages(&txt);
        if let Some(w) = state_c.main_window.upgrade() {
            w.set_transformed_aliases(aliased.into());
            w.set_transformed_censored(censored.into());
            w.set_transformed_final(truncated.into());
        }
    });

    // Test Lab: Synthesize & Play
    let state_c = state.clone();
    ui.on_test_synthesize_and_play(move |txt, speed| {
        let state_t = state_c.clone();
        if let Some(w) = state_c.main_window.upgrade() {
            w.set_is_synthesizing(true);
        }

        std::thread::spawn(move || {
            let filter = state_t.filter.lock().unwrap();
            let res = filter.process("Tester", &txt, false);
            drop(filter);

            let spoken_text = match res {
                FilterResult::Ready(item) => Some(item.spoken_text),
                FilterResult::Filtered(_) | FilterResult::Ignored(_) => None,
            };

            let synth_res = if let Some(ref text) = spoken_text {
                let mut tts = state_t.tts.lock().unwrap();
                let r = tts.synthesize(text, speed);
                drop(tts);
                Some(r)
            } else {
                None
            };

            let main_window = state_t.main_window.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = main_window.upgrade() {
                    w.set_is_synthesizing(false);
                }
            });

            if let Some(Ok((sample_rate, samples))) = synth_res {
                let padding = state_t.config_manager.get().tts.padding_sec;
                let _ = state_t.audio_player.play_samples(sample_rate, &samples, padding);
            }
        });
    });

    // Test Lab: Export WAV
    let state_c = state.clone();
    ui.on_test_export_to_wav(move |txt, speed| {
        let state_t = state_c.clone();
        if let Some(file_path) = rfd::FileDialog::new()
            .set_file_name("tts_sample.wav")
            .add_filter("WAV Audio", &["wav"])
            .save_file()
        {
            std::thread::spawn(move || {
                let filter = state_t.filter.lock().unwrap();
                let res = filter.process("Export", &txt, false);
                drop(filter);

                let spoken_text = match res {
                    FilterResult::Ready(item) => Some(item.spoken_text),
                    FilterResult::Filtered(_) | FilterResult::Ignored(_) => None,
                };

                if let Some(ref text) = spoken_text {
                    let mut tts = state_t.tts.lock().unwrap();
                    if let Ok((sample_rate, samples)) = tts.synthesize(text, speed) {
                        if let Err(e) = export_wav_file(file_path, sample_rate, &samples) {
                            error!("Failed to export WAV file: {}", e);
                        } else {
                            info!("Successfully exported WAV file!");
                        }
                    }
                }
            });
        }
    });

    // Filters: Add Alias
    let state_c = state.clone();
    ui.on_add_alias(move |k, v| {
        let k_str = k.trim().to_string();
        let v_str = v.trim().to_string();
        if k_str.is_empty() || v_str.is_empty() {
            return;
        }
        let _ = state_c.config_manager.update(|cfg| {
            cfg.filters.username_aliases.insert(k_str, v_str);
        });
        let mut filter = state_c.filter.lock().unwrap();
        filter.update_config(state_c.config_manager.get().filters);
        if let Some(w) = state_c.main_window.upgrade() {
            let aliases = state_c.config_manager.get().filters.username_aliases;
            let mut items: Vec<AliasItem> = aliases
                .into_iter()
                .map(|(k, v)| AliasItem { key: k.into(), value: v.into() })
                .collect();
            items.sort_by(|a, b| a.key.to_lowercase().cmp(&b.key.to_lowercase()));
            w.set_aliases(ModelRc::new(VecModel::from(items)));

            let (aliased, censored, truncated) = filter.inspect_stages(&w.get_test_text());
            w.set_transformed_aliases(aliased.into());
            w.set_transformed_censored(censored.into());
            w.set_transformed_final(truncated.into());
        }
    });

    // Filters: Edit Alias
    let state_c = state.clone();
    ui.on_edit_alias(move |old_k, new_k, new_v| {
        let old_k_str = old_k.to_string();
        let new_k_str = new_k.trim().to_string();
        let new_v_str = new_v.trim().to_string();
        if new_k_str.is_empty() || new_v_str.is_empty() {
            return;
        }
        let _ = state_c.config_manager.update(|cfg| {
            if old_k_str != new_k_str {
                cfg.filters.username_aliases.remove(old_k_str.as_str());
            }
            cfg.filters.username_aliases.insert(new_k_str, new_v_str);
        });
        let mut filter = state_c.filter.lock().unwrap();
        filter.update_config(state_c.config_manager.get().filters);
        if let Some(w) = state_c.main_window.upgrade() {
            let aliases = state_c.config_manager.get().filters.username_aliases;
            let mut items: Vec<AliasItem> = aliases
                .into_iter()
                .map(|(k, v)| AliasItem { key: k.into(), value: v.into() })
                .collect();
            items.sort_by(|a, b| a.key.to_lowercase().cmp(&b.key.to_lowercase()));
            w.set_aliases(ModelRc::new(VecModel::from(items)));

            let (aliased, censored, truncated) = filter.inspect_stages(&w.get_test_text());
            w.set_transformed_aliases(aliased.into());
            w.set_transformed_censored(censored.into());
            w.set_transformed_final(truncated.into());
        }
    });

    // Filters: Remove Alias
    let state_c = state.clone();
    ui.on_remove_alias(move |k| {
        let _ = state_c.config_manager.update(|cfg| {
            cfg.filters.username_aliases.remove(k.as_str());
        });
        let mut filter = state_c.filter.lock().unwrap();
        filter.update_config(state_c.config_manager.get().filters);
        if let Some(w) = state_c.main_window.upgrade() {
            let aliases = state_c.config_manager.get().filters.username_aliases;
            let mut items: Vec<AliasItem> = aliases
                .into_iter()
                .map(|(k, v)| AliasItem { key: k.into(), value: v.into() })
                .collect();
            items.sort_by(|a, b| a.key.to_lowercase().cmp(&b.key.to_lowercase()));
            w.set_aliases(ModelRc::new(VecModel::from(items)));

            let (aliased, censored, truncated) = filter.inspect_stages(&w.get_test_text());
            w.set_transformed_aliases(aliased.into());
            w.set_transformed_censored(censored.into());
            w.set_transformed_final(truncated.into());
        }
    });

    // Filters: Add Ignored User
    let state_c = state.clone();
    ui.on_add_ignored_user(move |u| {
        let _ = state_c.config_manager.update(|cfg| {
            if !cfg.filters.ignore_users.contains(&u.to_string()) {
                cfg.filters.ignore_users.push(u.to_string());
            }
        });
        let mut filter = state_c.filter.lock().unwrap();
        filter.update_config(state_c.config_manager.get().filters);
        if let Some(w) = state_c.main_window.upgrade() {
            let items: Vec<IgnoredUserItem> = state_c
                .config_manager
                .get()
                .filters
                .ignore_users
                .into_iter()
                .map(|u| IgnoredUserItem { name: u.into() })
                .collect();
            w.set_ignored_users(ModelRc::new(VecModel::from(items)));
        }
    });

    // Filters: Remove Ignored User
    let state_c = state.clone();
    ui.on_remove_ignored_user(move |u| {
        let _ = state_c.config_manager.update(|cfg| {
            cfg.filters.ignore_users.retain(|x| x != u.as_str());
        });
        let mut filter = state_c.filter.lock().unwrap();
        filter.update_config(state_c.config_manager.get().filters);
        if let Some(w) = state_c.main_window.upgrade() {
            let items: Vec<IgnoredUserItem> = state_c
                .config_manager
                .get()
                .filters
                .ignore_users
                .into_iter()
                .map(|u| IgnoredUserItem { name: u.into() })
                .collect();
            w.set_ignored_users(ModelRc::new(VecModel::from(items)));
        }
    });

    // Filters: Save Profanity Words
    let state_c = state.clone();
    ui.on_save_profanity_words(move |raw| {
        let file_path = state_c.config_manager.get().filters.profanity_words_file;
        let _ = fs::write(&file_path, raw.as_str());
        let words: Vec<String> = raw
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        let mut filter = state_c.filter.lock().unwrap();
        filter.update_profanity_words(&words);
        info!("Saved {} profanity words to {}", words.len(), file_path);
    });

    // Filters: Filter Settings Changed
    let state_c = state.clone();
    ui.on_filter_settings_changed(move || {
        if let Some(w) = state_c.main_window.upgrade() {
            let ep = w.get_enable_profanity();
            let fe = w.get_filter_emotes();
            let au = w.get_announce_username();
            let ut = w.get_username_template().to_string();
            let mc = w.get_max_characters() as usize;
            let mr = w.get_max_repeated_chars() as usize;

            let _ = state_c.config_manager.update(|cfg| {
                cfg.filters.enable_profanity_filter = ep;
                cfg.filters.filter_emotes = fe;
                cfg.filters.announce_username = au;
                cfg.filters.username_template = ut;
                cfg.filters.max_characters = mc;
                cfg.filters.max_repeated_chars = mr;
            });

            let mut filter = state_c.filter.lock().unwrap();
            filter.update_config(state_c.config_manager.get().filters);

            let (aliased, censored, truncated) = filter.inspect_stages(&w.get_test_text());
            w.set_transformed_aliases(aliased.into());
            w.set_transformed_censored(censored.into());
            w.set_transformed_final(truncated.into());
        }
    });

    // Audio: Browse Model
    let state_c = state.clone();
    ui.on_browse_model(move || {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("ONNX Model", &["onnx"])
            .pick_file()
        {
            let path_str = path.to_string_lossy().to_string();
            if let Some(w) = state_c.main_window.upgrade() {
                w.set_model_path(path_str.clone().into());
            }
            let _ = state_c.config_manager.update(|cfg| {
                cfg.tts.model_path = path_str;
            });
            reload_tts_engine(&state_c);
        }
    });

    // Audio: Browse Config
    let state_c = state.clone();
    ui.on_browse_config(move || {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("ONNX Metadata Config", &["json"])
            .pick_file()
        {
            let path_str = path.to_string_lossy().to_string();
            if let Some(w) = state_c.main_window.upgrade() {
                w.set_config_path(path_str.clone().into());
            }
            let _ = state_c.config_manager.update(|cfg| {
                cfg.tts.config_path = path_str;
            });
            reload_tts_engine(&state_c);
        }
    });

    // Audio: Refresh Devices
    let state_c = state.clone();
    ui.on_refresh_devices(move || {
        let devices = AudioDeviceManager::list_output_devices();
        if let Some(w) = state_c.main_window.upgrade() {
            let dev_model: Vec<SharedString> = devices.into_iter().map(|d| d.into()).collect();
            w.set_available_devices(ModelRc::new(VecModel::from(dev_model)));
        }
    });

    // Audio: Settings Changed
    let state_c = state.clone();
    ui.on_audio_settings_changed(move || {
        if let Some(w) = state_c.main_window.upgrade() {
            let mp = w.get_model_path().to_string();
            let cp = w.get_config_path().to_string();
            let sid = w.get_speaker_id() as i64;
            let rate = w.get_speech_rate();
            let dev = w.get_selected_device_name().to_string();
            let pad = w.get_padding_sec();
            let vol = w.get_volume();

            let _ = state_c.config_manager.update(|cfg| {
                cfg.tts.model_path = mp;
                cfg.tts.config_path = cp;
                cfg.tts.speaker_id = sid;
                cfg.tts.speech_rate = rate;
                cfg.tts.audio_device_name = dev.clone();
                cfg.tts.padding_sec = pad;
                cfg.tts.volume = vol;
            });

            state_c.audio_player.set_volume(vol);
            state_c.audio_player.set_device(&dev);

            reload_tts_engine(&state_c);
        }
    });

    // Twitch: Start OAuth Flow
    let state_c = state.clone();
    ui.on_start_oauth_flow(move || {
        let state_t = state_c.clone();
        if let Some(w) = state_c.main_window.upgrade() {
            w.set_is_authenticating(true);
            w.set_auth_status_message("Opening browser for Twitch authorization...".into());
        }

        tokio::spawn(async move {
            match OAuthServer::start_oauth_flow().await {
                Ok(res) => {
                    info!("OAuth flow successful for user: {}", res.user_login);
                    let _ = state_t.config_manager.update(|cfg| {
                        cfg.twitch.oauth_token = res.token.clone();
                        cfg.twitch.broadcaster_user_id = res.user_id.clone();
                    });

                    let mut twitch = state_t.twitch.lock().unwrap();
                    twitch.set_config(state_t.config_manager.get().twitch);
                    twitch.connect(state_t.chat_tx.clone(), state_t.status_tx.clone());
                    drop(twitch);

                    let main_window = state_t.main_window.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = main_window.upgrade() {
                            w.set_is_authenticating(false);
                            w.set_auth_status_message("✅ Authenticated & connected!".into());
                            w.set_twitch_oauth_token(res.token.into());
                            w.set_broadcaster_user_id(res.user_id.into());
                            w.set_channel_name(res.user_login.into());
                        }
                    });
                }
                Err(err) => {
                    error!("OAuth flow error: {}", err);
                    let main_window = state_t.main_window.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = main_window.upgrade() {
                            w.set_is_authenticating(false);
                            w.set_auth_status_message(format!("❌ {}", err).into());
                        }
                    });
                }
            }
        });
    });

    // Twitch: Connect
    let state_c = state.clone();
    ui.on_connect_twitch(move || {
        if let Some(w) = state_c.main_window.upgrade() {
            let tok = w.get_twitch_oauth_token().to_string();
            let uid = w.get_broadcaster_user_id().to_string();
            let rac = w.get_read_all_chat();
            let rid = w.get_reward_id().to_string();

            let _ = state_c.config_manager.update(|cfg| {
                cfg.twitch.oauth_token = tok;
                cfg.twitch.broadcaster_user_id = uid;
                cfg.twitch.read_all_chat = rac;
                cfg.twitch.reward_id = rid;
            });
        }
        let cfg = state_c.config_manager.get().twitch;
        let mut twitch = state_c.twitch.lock().unwrap();
        twitch.set_config(cfg);
        twitch.connect(state_c.chat_tx.clone(), state_c.status_tx.clone());
    });

    // Twitch: Disconnect
    let state_c = state.clone();
    ui.on_disconnect_twitch(move || {
        let mut twitch = state_c.twitch.lock().unwrap();
        twitch.disconnect();
        if let Some(w) = state_c.main_window.upgrade() {
            w.set_connection_status("offline".into());
        }
    });

    // Twitch: Settings Changed
    let state_c = state.clone();
    ui.on_twitch_settings_changed(move || {
        if let Some(w) = state_c.main_window.upgrade() {
            let tok = w.get_twitch_oauth_token().to_string();
            let uid = w.get_broadcaster_user_id().to_string();
            let rac = w.get_read_all_chat();
            let rid = w.get_reward_id().to_string();

            let _ = state_c.config_manager.update(|cfg| {
                cfg.twitch.oauth_token = tok;
                cfg.twitch.broadcaster_user_id = uid.clone();
                cfg.twitch.read_all_chat = rac;
                cfg.twitch.reward_id = rid;
            });

            w.set_channel_name(uid.into());
        }
    });

    // Settings: Changed
    let state_c = state.clone();
    ui.on_settings_changed(move || {
        if let Some(w) = state_c.main_window.upgrade() {
            let hke = w.get_hotkeys_enabled();
            let mhk = w.get_mute_hotkey().to_string();
            let shk = w.get_skip_hotkey().to_string();
            let mtt = w.get_minimize_to_tray();

            let _ = state_c.config_manager.update(|cfg| {
                cfg.hotkeys.enabled = hke;
                cfg.hotkeys.mute_toggle = mhk;
                cfg.hotkeys.skip_current = shk;
                cfg.app.minimize_to_tray = mtt;
            });
        }
    });

    // Settings: Save Config Now
    let state_c = state.clone();
    ui.on_save_config_now(move || {
        let cfg = state_c.config_manager.get();
        let _ = state_c.config_manager.save(&cfg);
    });
}

fn reload_tts_engine(state: &Arc<AppState>) {
    let cfg = state.config_manager.get().tts;
    let mut tts = state.tts.lock().unwrap();
    let _ = tts.reload(&cfg.model_path, &cfg.config_path, cfg.speaker_id);
}

pub fn add_activity_row(state: &Arc<AppState>, item: SpokenItem) {
    let mut history = state.activity_history.lock().unwrap();
    if let Some(existing) = history.iter_mut().find(|i| i.id == item.id) {
        existing.status = item.status;
    } else {
        // Prepend to history (newest first) and limit to 100 items
        history.insert(0, item);
        if history.len() > 100 {
            history.truncate(100);
        }
    }

    let items: Vec<ActivityItem> = history
        .iter()
        .map(|i| ActivityItem {
            id: i.id.to_string().into(),
            time: i.timestamp.format("%H:%M:%S").to_string().into(),
            user: i.sender.clone().into(),
            text: i.spoken_text.clone().into(),
            status: i.status.display_label().into(),
            status_type: i.status.status_type().into(),
        })
        .collect();

    let main_window = state.main_window.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(w) = main_window.upgrade() {
            w.set_activities(ModelRc::new(VecModel::from(items)));
        }
    });
}

fn spawn_playback_worker(state: Arc<AppState>) {
    std::thread::spawn(move || {
        info!("Audio playback worker thread started.");
        loop {
            // Check for next queued speech item
            if let Some(mut item) = state.queue.pop() {
                item.status = MessageStatus::Playing;
                add_activity_row(&state, item.clone());

                let queue_len = state.queue.len() as i32;
                let main_window = state.main_window.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = main_window.upgrade() {
                        w.set_queue_count(queue_len);
                        w.set_is_speaking(true);
                    }
                });

                // Synthesize PCM
                let speech_rate = state.config_manager.get().tts.speech_rate;
                let mut tts = state.tts.lock().unwrap();
                let synth_res = tts.synthesize(&item.spoken_text, speech_rate);
                drop(tts);

                match synth_res {
                    Ok((sample_rate, samples)) => {
                        let padding = state.config_manager.get().tts.padding_sec;
                        let _ = state.audio_player.play_samples(sample_rate, &samples, padding);

                        item.status = MessageStatus::Spoken;
                        add_activity_row(&state, item);
                    }
                    Err(err) => {
                        error!("TTS Synthesis failed: {}", err);
                        item.status = MessageStatus::Error(err);
                        add_activity_row(&state, item);
                    }
                }

                let main_window = state.main_window.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = main_window.upgrade() {
                        w.set_is_speaking(false);
                    }
                });
            } else {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    });
}

fn spawn_twitch_event_handler(
    state: Arc<AppState>,
    mut rx: mpsc::UnboundedReceiver<ChatEvent>,
) {
    let state_t = state.clone();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            handle_incoming_chat_event(&state_t, event);
        }
    });
}

fn handle_incoming_chat_event(state: &Arc<AppState>, event: ChatEvent) {
    let filter = state.filter.lock().unwrap();
    let res = filter.process_with_emotes(&event.user, &event.text, &event.emotes, event.is_custom_reward);
    drop(filter);

    match res {
        FilterResult::Ready(item) => {
            let dropped = state.queue.push(item.clone());
            if let Some(d) = dropped {
                add_activity_row(state, d);
            }
            add_activity_row(state, item);
            let queue_len = state.queue.len() as i32;
            let main_window = state.main_window.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = main_window.upgrade() {
                    w.set_queue_count(queue_len);
                }
            });
        }
        FilterResult::Filtered(item) | FilterResult::Ignored(item) => {
            add_activity_row(state, item);
        }
    }
}

fn spawn_twitch_status_handler(
    state: Arc<AppState>,
    mut rx: mpsc::UnboundedReceiver<String>,
) {
    let state_t = state.clone();
    tokio::spawn(async move {
        while let Some(status) = rx.recv().await {
            update_twitch_status_ui(&state_t, status);
        }
    });
}

fn update_twitch_status_ui(state: &Arc<AppState>, status: String) {
    let main_window = state.main_window.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(w) = main_window.upgrade() {
            w.set_connection_status(status.into());
        }
    });
}

fn spawn_hotkey_handler(state: Arc<AppState>, mut rx: mpsc::UnboundedReceiver<HotkeyAction>) {
    let state_t = state.clone();
    tokio::spawn(async move {
        while let Some(action) = rx.recv().await {
            match action {
                HotkeyAction::ToggleMute => {
                    let new_muted = !state_t.audio_player.is_muted();
                    state_t.audio_player.set_muted(new_muted);

                    let main_window = state_t.main_window.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = main_window.upgrade() {
                            w.set_is_muted(new_muted);
                        }
                    });
                }
                HotkeyAction::SkipCurrent => {
                    state_t.audio_player.stop();
                }
            }
        }
    });
}
