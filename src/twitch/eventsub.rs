use crate::config::TwitchConfig;
use crate::domain::models::ChatEvent;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{error, info, warn};

pub const EVENTSUB_WS_URL: &str = "wss://eventsub.wss.twitch.tv/ws";

pub struct EventSubClient {
    config: TwitchConfig,
    event_tx: mpsc::UnboundedSender<ChatEvent>,
    status_tx: mpsc::UnboundedSender<String>,
    is_running: Arc<AtomicBool>,
}

impl EventSubClient {
    pub fn new(
        config: TwitchConfig,
        event_tx: mpsc::UnboundedSender<ChatEvent>,
        status_tx: mpsc::UnboundedSender<String>,
    ) -> Self {
        Self {
            config,
            event_tx,
            status_tx,
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&self) -> (Arc<AtomicBool>, tokio::task::JoinHandle<()>) {
        let is_running = self.is_running.clone();
        is_running.store(true, Ordering::SeqCst);

        let config = self.config.clone();
        let event_tx = self.event_tx.clone();
        let status_tx = self.status_tx.clone();
        let running_flag = is_running.clone();

        let handle = tokio::spawn(async move {
            let mut ws_url = EVENTSUB_WS_URL.to_string();
            let mut is_reconnect = false;

            while running_flag.load(Ordering::SeqCst) {
                let _ = status_tx.send("reconnecting".to_string());
                info!("Connecting to Twitch EventSub WebSocket at {}", ws_url);

                match connect_async(&ws_url).await {
                    Ok((ws_stream, _)) => {
                        let _ = status_tx.send("connected".to_string());
                        info!("Connected to Twitch EventSub WebSocket!");

                        let (mut write, mut read) = ws_stream.split();
                        let is_reconnect_session = is_reconnect;
                        is_reconnect = false;
                        ws_url = EVENTSUB_WS_URL.to_string();

                        while running_flag.load(Ordering::SeqCst) {
                            tokio::select! {
                                msg = read.next() => {
                                    match msg {
                                        Some(Ok(Message::Text(text))) => {
                                            if let Ok(json) = serde_json::from_str::<Value>(&text) {
                                                let msg_type = json["metadata"]["message_type"].as_str().unwrap_or("");

                                                match msg_type {
                                                    "session_welcome" => {
                                                        if let Some(session_id) = json["payload"]["session"]["id"].as_str() {
                                                            info!("Received EventSub session ID: {}", session_id);
                                                            if is_reconnect_session {
                                                                info!("Seamless reconnect welcome received for session: {}; skipping duplicate subscription creation", session_id);
                                                            } else if let Err(e) = Self::create_subscriptions(&config, session_id).await {
                                                                error!("Failed to subscribe to Twitch events: {}", e);
                                                            }
                                                        }
                                                    }
                                                    "session_keepalive" => {
                                                        // Heartbeat received, connection healthy
                                                    }
                                                    "notification" => {
                                                        Self::handle_notification(&json, &config, &event_tx);
                                                    }
                                                    "session_reconnect" => {
                                                        if let Some(reconnect_url) = json["payload"]["session"]["reconnect_url"].as_str() {
                                                            info!("EventSub server requested reconnect to: {}", reconnect_url);
                                                            ws_url = reconnect_url.to_string();
                                                            is_reconnect = true;
                                                            break;
                                                        }
                                                    }
                                                    "revocation" => {
                                                        warn!("EventSub subscription revoked: {:?}", json);
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                        Some(Ok(Message::Ping(payload))) => {
                                            let _ = write.send(Message::Pong(payload)).await;
                                        }
                                        Some(Ok(Message::Close(_))) => {
                                            warn!("Twitch EventSub WebSocket closed by server.");
                                            break;
                                        }
                                        Some(Err(err)) => {
                                            error!("WebSocket error: {}", err);
                                            break;
                                        }
                                        None => {
                                            warn!("WebSocket stream ended.");
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                    Err(err) => {
                        error!("Failed to connect to Twitch EventSub: {}", err);
                        is_reconnect = false;
                        ws_url = EVENTSUB_WS_URL.to_string();
                    }
                }

                if !running_flag.load(Ordering::SeqCst) {
                    break;
                }

                let _ = status_tx.send("reconnecting".to_string());
                tokio::time::sleep(Duration::from_secs(5)).await;
            }

            let _ = status_tx.send("offline".to_string());
            info!("Twitch EventSub client stopped.");
        });

        (is_running, handle)
    }

    fn handle_notification(
        json: &Value,
        config: &TwitchConfig,
        event_tx: &mpsc::UnboundedSender<ChatEvent>,
    ) {
        let sub_type = json["payload"]["subscription"]["type"].as_str().unwrap_or("");
        let event = &json["payload"]["event"];

        match sub_type {
            "channel.chat.message" => {
                if config.read_all_chat {
                    let user = event["chatter_user_name"]
                        .as_str()
                        .or_else(|| event["chatter_user_login"].as_str())
                        .unwrap_or("Viewer")
                        .to_string();

                    let text = event["message"]["text"].as_str().unwrap_or("").to_string();

                    let mut emotes = Vec::new();
                    if let Some(fragments) = event["message"]["fragments"].as_array() {
                        for f in fragments {
                            if f["type"].as_str() == Some("emote") {
                                if let Some(t) = f["text"].as_str() {
                                    emotes.push(t.to_string());
                                }
                            }
                        }
                    }

                    if !text.trim().is_empty() {
                        let _ = event_tx.send(ChatEvent {
                            user,
                            text,
                            emotes,
                            is_custom_reward: false,
                        });
                    }
                }
            }
            "channel.channel_points_custom_reward_redemption.add" => {
                let reward_id = event["reward"]["id"].as_str().unwrap_or("");
                let matches_reward = config.reward_id.trim().is_empty() || config.reward_id == reward_id;

                if matches_reward {
                    let user = event["user_name"]
                        .as_str()
                        .or_else(|| event["user_login"].as_str())
                        .unwrap_or("Viewer")
                        .to_string();

                    let text = event["user_input"].as_str().unwrap_or("").to_string();

                    if !text.trim().is_empty() {
                        let _ = event_tx.send(ChatEvent {
                            user,
                            text,
                            emotes: Vec::new(),
                            is_custom_reward: true,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    async fn create_subscriptions(config: &TwitchConfig, session_id: &str) -> Result<(), String> {
        let client = reqwest::Client::new();
        let token = if config.oauth_token.starts_with("oauth:") {
            config.oauth_token.replace("oauth:", "Bearer ")
        } else if !config.oauth_token.starts_with("Bearer ") {
            format!("Bearer {}", config.oauth_token)
        } else {
            config.oauth_token.clone()
        };

        let client_id = crate::twitch::auth::get_client_id();
        if client_id.is_empty() || config.broadcaster_user_id.trim().is_empty() {
            return Err("Missing Twitch client_id or broadcaster_user_id".to_string());
        }

        // 1. Subscribe to channel.chat.message
        let chat_sub_body = serde_json::json!({
            "type": "channel.chat.message",
            "version": "1",
            "condition": {
                "broadcaster_user_id": config.broadcaster_user_id,
                "user_id": config.broadcaster_user_id
            },
            "transport": {
                "method": "websocket",
                "session_id": session_id
            }
        });

        let resp = client
            .post("https://api.twitch.tv/helix/eventsub/subscriptions")
            .header("Client-Id", client_id)
            .header("Authorization", &token)
            .header("Content-Type", "application/json")
            .json(&chat_sub_body)
            .send()
            .await
            .map_err(|e| format!("Subscription request failed: {}", e))?;

        if !resp.status().is_success() && resp.status() != reqwest::StatusCode::CONFLICT {
            warn!("Failed to create chat.message subscription: HTTP {}", resp.status());
        } else {
            info!("Successfully subscribed to channel.chat.message!");
        }

        // 2. Subscribe to channel points redemption
        let mut reward_condition = serde_json::json!({
            "broadcaster_user_id": config.broadcaster_user_id
        });
        if !config.reward_id.trim().is_empty() {
            reward_condition["reward_id"] = serde_json::json!(config.reward_id);
        }

        let reward_sub_body = serde_json::json!({
            "type": "channel.channel_points_custom_reward_redemption.add",
            "version": "1",
            "condition": reward_condition,
            "transport": {
                "method": "websocket",
                "session_id": session_id
            }
        });

        let resp2 = client
            .post("https://api.twitch.tv/helix/eventsub/subscriptions")
            .header("Client-Id", client_id)
            .header("Authorization", &token)
            .header("Content-Type", "application/json")
            .json(&reward_sub_body)
            .send()
            .await
            .map_err(|e| format!("Reward subscription request failed: {}", e))?;

        if !resp2.status().is_success() && resp2.status() != reqwest::StatusCode::CONFLICT {
            warn!("Failed to create points redemption subscription: HTTP {}", resp2.status());
        } else {
            info!("Successfully subscribed to channel_points_custom_reward_redemption!");
        }

        Ok(())
    }
}
