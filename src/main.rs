//#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![windows_subsystem = "windows"]

#[cfg(windows)]
#[link(name = "resource")]
unsafe extern "C" {}

pub mod audio;
pub mod config;
pub mod domain;
pub mod filter;
pub mod hotkeys;
pub mod tts;
pub mod twitch;
pub mod ui;

use config::persistence::{ConfigManager, DEFAULT_CONFIG_PATH};
use hotkeys::manager::HotkeyAction;
use tokio::sync::mpsc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

slint::include_modules!();

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let filter_layer = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "twitch_tts=info,warn,error".into());
    let fmt_layer = tracing_subscriber::fmt::layer().with_target(false);
    
    let _ = tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .try_init();

    info!("Starting Twitch TTS Desktop App...");

    let config_manager = ConfigManager::new(DEFAULT_CONFIG_PATH);
    let cfg = config_manager.get();

    let main_window = MainWindow::new()?;

    let (hotkey_action_tx, hotkey_action_rx) = mpsc::unbounded_channel::<HotkeyAction>();

    let app_state = ui::bridge::setup_ui_bridge(
        &main_window,
        config_manager.clone(),
        hotkey_action_rx,
    );

    if cfg.hotkeys.enabled {
        let mut hotkeys = app_state.hotkeys.lock().unwrap();
        if let Err(err) = hotkeys.start(
            &cfg.hotkeys.mute_toggle,
            &cfg.hotkeys.skip_current,
            hotkey_action_tx.clone(),
        ) {
            error!("Failed to initialize global hotkeys: {}", err);
        }
    }

    if !cfg.twitch.oauth_token.trim().is_empty()
        && !cfg.twitch.broadcaster_user_id.trim().is_empty()
        && !crate::twitch::auth::get_client_id().is_empty()
    {
        let mut twitch = app_state.twitch.lock().unwrap();
        twitch.connect(app_state.chat_tx.clone(), app_state.status_tx.clone());
    }

    info!("Launching Slint event loop...");
    main_window.run()?;
    info!("Twitch TTS exited normally.");

    Ok(())
}
