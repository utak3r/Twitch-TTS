pub mod auth;
pub mod eventsub;

use crate::config::TwitchConfig;
use crate::domain::models::ChatEvent;
use eventsub::EventSubClient;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

pub struct TwitchCoordinator {
    config: TwitchConfig,
    active_client: Option<(Arc<AtomicBool>, tokio::task::JoinHandle<()>)>,
}

impl TwitchCoordinator {
    pub fn new(config: TwitchConfig) -> Self {
        Self {
            config,
            active_client: None,
        }
    }

    pub fn set_config(&mut self, config: TwitchConfig) {
        self.config = config;
    }

    pub fn connect(
        &mut self,
        event_tx: mpsc::UnboundedSender<ChatEvent>,
        status_tx: mpsc::UnboundedSender<String>,
    ) {
        self.disconnect();
        info!("Starting Twitch coordinator connection...");
        let client = EventSubClient::new(self.config.clone(), event_tx, status_tx);
        let (flag, handle) = client.start();
        self.active_client = Some((flag, handle));
    }

    pub fn disconnect(&mut self) {
        if let Some((flag, handle)) = self.active_client.take() {
            info!("Disconnecting Twitch EventSub...");
            flag.store(false, Ordering::SeqCst);
            handle.abort();
        }
    }

    pub fn is_connected(&self) -> bool {
        self.active_client
            .as_ref()
            .map(|(f, _)| f.load(Ordering::SeqCst))
            .unwrap_or(false)
    }
}
