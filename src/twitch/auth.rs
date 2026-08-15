use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tracing::{error, info};

pub const OAUTH_PORT: u16 = 17563;
pub const CLIENT_ID: &str = include_str!("../../.client_id");

pub fn get_client_id() -> &'static str {
    CLIENT_ID.trim()
}

#[derive(Debug, Clone)]
pub struct AuthResult {
    pub token: String,
    pub client_id: String,
    pub user_id: String,
    pub user_login: String,
}

#[derive(Debug, Deserialize)]
struct ValidateResponse {
    client_id: String,
    login: String,
    user_id: String,
}

pub struct OAuthServer;

impl OAuthServer {
    pub async fn start_oauth_flow() -> Result<AuthResult, String> {
        let client_id = get_client_id();

        let redirect_uri = format!("http://localhost:{}/callback", OAUTH_PORT);
        let scopes = "user:read:chat channel:read:redemptions";
        let auth_url = format!(
            "https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri={}&response_type=token&scope={}",
            urlencoding::encode(client_id),
            urlencoding::encode(&redirect_uri),
            urlencoding::encode(scopes)
        );

        let (tx, rx) = oneshot::channel::<String>();
        let tx_arc = Arc::new(tokio::sync::Mutex::new(Some(tx)));

        let addr = SocketAddr::from(([127, 0, 0, 1], OAUTH_PORT));
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| format!("Failed to bind OAuth HTTP listener to port {}: {}", OAUTH_PORT, e))?;

        info!("Local OAuth HTTP listener running on port {}", OAUTH_PORT);

        // Open browser
        if let Err(e) = open::that(&auth_url) {
            error!("Failed to open browser for OAuth flow: {}", e);
        }

        let server_task = tokio::spawn(async move {
            loop {
                if let Ok((mut socket, _)) = listener.accept().await {
                    let mut buffer = [0u8; 4096];
                    let n = match socket.read(&mut buffer).await {
                        Ok(n) if n > 0 => n,
                        _ => continue,
                    };

                    let request = String::from_utf8_lossy(&buffer[..n]);

                    if request.contains("GET /callback") {
                        // Return HTML page with script to extract token from URL fragment
                        let html = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Twitch TTS - Authorization</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0e0e13; color: #f9fafb; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; }
        .card { background: #181822; border: 1px solid #323246; border-radius: 12px; padding: 32px; text-align: center; max-width: 420px; box-shadow: 0 10px 25px rgba(0,0,0,0.5); }
        h2 { color: #9146ff; margin-top: 0; }
        p { color: #9ca3af; line-height: 1.5; }
    </style>
</head>
<body>
    <div class="card">
        <h2>Twitch TTS Authorization</h2>
        <p id="msg">Processing authentication token...</p>
    </div>
    <script>
        const hash = window.location.hash.substring(1);
        const params = new URLSearchParams(hash);
        const token = params.get('access_token');
        if (token) {
            fetch('/token?access_token=' + encodeURIComponent(token))
                .then(() => {
                    document.getElementById('msg').innerHTML = '<b style="color:#10b981">Authorization successful!</b><br>You can close this tab and return to Twitch TTS.';
                })
                .catch(() => {
                    document.getElementById('msg').innerHTML = '<b style="color:#ef4444">Error sending token to app.</b>';
                });
        } else {
            document.getElementById('msg').innerHTML = '<b style="color:#ef4444">No token found in response.</b>';
        }
    </script>
</body>
</html>"#;
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            html.len(),
                            html
                        );
                        let _ = socket.write_all(response.as_bytes()).await;
                    } else if request.contains("GET /token?") {
                        // Extract token from query parameter
                        if let Some(pos) = request.find("access_token=") {
                            let sub = &request[pos + 13..];
                            let token_str = sub.split(&['&', ' '][..]).next().unwrap_or("");
                            let clean_token = urlencoding::decode(token_str).unwrap_or_default().to_string();

                            let mut tx_guard = tx_arc.lock().await;
                            if let Some(sender) = tx_guard.take() {
                                let _ = sender.send(clean_token);
                            }

                            let response = "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: 2\r\n\r\nOK";
                            let _ = socket.write_all(response.as_bytes()).await;
                            break;
                        }
                    }
                }
            }
        });

        // Await token with 120 second timeout
        let raw_token = tokio::time::timeout(tokio::time::Duration::from_secs(120), rx)
            .await
            .map_err(|_| "OAuth login timed out after 120 seconds".to_string())?
            .map_err(|_| "Failed to receive token from local server".to_string())?;

        let _ = server_task.abort();

        // Validate token with Twitch API
        let (cid, uid, login) = Self::validate_token(&raw_token).await?;

        Ok(AuthResult {
            token: raw_token,
            client_id: cid,
            user_id: uid,
            user_login: login,
        })
    }

    pub async fn validate_token(token: &str) -> Result<(String, String, String), String> {
        let client = reqwest::Client::new();
        let auth_header = if token.starts_with("oauth:") {
            token.replace("oauth:", "Bearer ")
        } else if !token.starts_with("Bearer ") {
            format!("Bearer {}", token)
        } else {
            token.to_string()
        };

        let resp = client
            .get("https://id.twitch.tv/oauth2/validate")
            .header("Authorization", auth_header)
            .send()
            .await
            .map_err(|e| format!("Twitch validation request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Invalid Twitch OAuth token (HTTP {})", resp.status()));
        }

        let val: ValidateResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Twitch validation response: {}", e))?;

        Ok((val.client_id, val.user_id, val.login))
    }
}
