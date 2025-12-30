//! Tunnel Relay Durable Object
//!
//! Handles WebSocket connections from both browser clients and tunnel clients,
//! relaying messages bidirectionally.
//!
//! ## Connection Flow
//!
//! 1. User logs in via GitHub OAuth in browser
//! 2. User connects Claude OAuth (gets Claude tokens)
//! 3. Browser creates a tunnel session via POST /api/tunnel/register
//! 4. User runs `openagents connect` on their machine
//! 5. Tunnel client connects to DO via WebSocket with session token
//! 6. Browser connects to same DO via WebSocket
//! 7. DO relays messages between browser and tunnel

use serde::{Deserialize, Serialize};
use worker::*;

/// Connection type for tracking WebSocket clients
#[derive(Debug, Clone, PartialEq)]
enum ConnectionType {
    Browser,
    Tunnel,
}

/// Tunnel Relay Durable Object
///
/// Each instance handles a single session (browser + tunnel pair).
#[durable_object]
pub struct TunnelRelay {
    state: State,
    #[allow(dead_code)]
    env: Env,
}

impl DurableObject for TunnelRelay {
    fn new(state: State, env: Env) -> Self {
        Self { state, env }
    }

    /// Handle incoming HTTP requests (for WebSocket upgrade)
    async fn fetch(&self, req: Request) -> Result<Response> {
        let url = req.url()?;
        let path = url.path();

        // Parse connection type from path
        // /ws/browser - browser client
        // /ws/tunnel - tunnel client
        let conn_type = if path.ends_with("/browser") {
            ConnectionType::Browser
        } else if path.ends_with("/tunnel") {
            ConnectionType::Tunnel
        } else {
            return Response::error("Invalid WebSocket path", 400);
        };

        // Get session ID from query
        let session_id = url
            .query_pairs()
            .find(|(k, _)| k == "session_id")
            .map(|(_, v)| v.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Check for WebSocket upgrade
        let upgrade_header = req.headers().get("Upgrade")?.unwrap_or_default();
        if upgrade_header.to_lowercase() != "websocket" {
            return Response::error("Expected WebSocket upgrade", 426);
        }

        // Create WebSocket pair
        let pair = WebSocketPair::new()?;
        let client = pair.client;
        let server = pair.server;

        // Accept the WebSocket
        server.accept()?;

        // Store connection type as a tag
        let tag = match conn_type {
            ConnectionType::Browser => "browser",
            ConnectionType::Tunnel => "tunnel",
        };

        // Use hibernation API to keep connection alive cheaply
        self.state.accept_websocket_with_tags(&server, &[tag]);

        // If tunnel connected, notify browser clients
        if conn_type == ConnectionType::Tunnel {
            self.broadcast_to_browsers(&serde_json::json!({
                "type": "tunnel_connected",
                "session_id": session_id
            })
            .to_string());
        }

        // Return the client WebSocket to the browser/tunnel
        Response::from_websocket(client)
    }

    /// Handle incoming WebSocket messages (hibernation API)
    async fn websocket_message(
        &self,
        ws: WebSocket,
        message: WebSocketIncomingMessage,
    ) -> Result<()> {
        // Get the tags for this WebSocket to determine sender type
        let tags = self.state.get_tags(&ws);
        let is_browser = tags.contains(&"browser".to_string());
        let is_tunnel = tags.contains(&"tunnel".to_string());

        let msg_text = match message {
            WebSocketIncomingMessage::String(s) => s,
            WebSocketIncomingMessage::Binary(b) => {
                String::from_utf8(b).unwrap_or_else(|_| "".to_string())
            }
        };

        // Relay message to the appropriate recipients
        if is_browser {
            // Browser message -> forward to tunnel
            self.broadcast_to_tunnels(&msg_text);
        } else if is_tunnel {
            // Tunnel message -> forward to browsers
            self.broadcast_to_browsers(&msg_text);
        }

        Ok(())
    }

    /// Handle WebSocket close
    async fn websocket_close(
        &self,
        ws: WebSocket,
        code: usize,
        reason: String,
        _was_clean: bool,
    ) -> Result<()> {
        let tags = self.state.get_tags(&ws);
        let is_tunnel = tags.contains(&"tunnel".to_string());

        // If tunnel disconnected, notify browsers
        if is_tunnel {
            self.broadcast_to_browsers(&serde_json::json!({
                "type": "tunnel_disconnected",
                "code": code,
                "reason": reason
            })
            .to_string());
        }

        Ok(())
    }

    /// Handle WebSocket errors
    async fn websocket_error(&self, _ws: WebSocket, error: Error) -> Result<()> {
        console_log!("WebSocket error: {:?}", error);
        Ok(())
    }
}

impl TunnelRelay {
    /// Send message to all connected browser clients
    fn broadcast_to_browsers(&self, message: &str) {
        let sockets = self.state.get_websockets_with_tag("browser");
        for ws in sockets {
            let _: std::result::Result<(), _> = ws.send_with_str(message);
        }
    }

    /// Send message to all connected tunnel clients
    fn broadcast_to_tunnels(&self, message: &str) {
        let sockets = self.state.get_websockets_with_tag("tunnel");
        for ws in sockets {
            let _: std::result::Result<(), _> = ws.send_with_str(message);
        }
    }

    /// Check if tunnel is connected for this session
    #[allow(dead_code)]
    fn is_tunnel_connected(&self) -> bool {
        !self.state.get_websockets_with_tag("tunnel").is_empty()
    }
}

/// Session data stored in KV
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelSession {
    pub session_id: String,
    pub user_id: String,
    pub repo: String,
    pub created_at: u64,
    /// Token for tunnel client authentication
    pub tunnel_token: String,
}

impl TunnelSession {
    /// Create a new tunnel session
    pub fn new(user_id: &str, repo: &str) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            repo: repo.to_string(),
            created_at: js_sys::Date::now() as u64 / 1000,
            tunnel_token: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Store session in KV
    pub async fn save(&self, kv: &kv::KvStore) -> Result<()> {
        let json = serde_json::to_string(self)?;

        // Store by session ID (for browser lookup)
        kv.put(&format!("tunnel_session:{}", self.session_id), &json)?
            .expiration_ttl(86400) // 24 hours
            .execute()
            .await?;

        // Store by tunnel token (for tunnel client auth)
        kv.put(&format!("tunnel_token:{}", self.tunnel_token), &self.session_id)?
            .expiration_ttl(86400)
            .execute()
            .await?;

        Ok(())
    }

    /// Load session by ID
    pub async fn load(kv: &kv::KvStore, session_id: &str) -> Result<Option<Self>> {
        let result = kv.get(&format!("tunnel_session:{}", session_id)).text().await
            .map_err(|e| Error::RustError(format!("KV error: {:?}", e)))?;
        match result {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }

    /// Validate tunnel token and return session ID
    pub async fn validate_token(kv: &kv::KvStore, token: &str) -> Result<Option<String>> {
        kv.get(&format!("tunnel_token:{}", token)).text().await
            .map_err(|e| Error::RustError(format!("KV error: {:?}", e)))
    }
}
