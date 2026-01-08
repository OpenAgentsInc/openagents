//! RLM Run Durable Object
//!
//! Relays live trace events from Pylon to browser clients.

use worker::*;

#[derive(Debug, Clone, PartialEq)]
enum ConnectionType {
    Browser,
    Pylon,
}

#[durable_object]
pub struct RlmRunDO {
    state: State,
    #[allow(dead_code)]
    env: Env,
}

impl DurableObject for RlmRunDO {
    fn new(state: State, env: Env) -> Self {
        Self { state, env }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let url = req.url()?;
        let path = url.path();

        let conn_type = if path.ends_with("/browser") {
            ConnectionType::Browser
        } else if path.ends_with("/pylon") {
            ConnectionType::Pylon
        } else {
            return Response::error("Invalid WebSocket path", 400);
        };

        let upgrade_header = req.headers().get("Upgrade")?.unwrap_or_default();
        if upgrade_header.to_lowercase() != "websocket" {
            return Response::error("Expected WebSocket upgrade", 426);
        }

        let pair = WebSocketPair::new()?;
        let client = pair.client;
        let server = pair.server;

        server.accept()?;

        let tag = match conn_type {
            ConnectionType::Browser => "browser",
            ConnectionType::Pylon => "pylon",
        };
        self.state.accept_websocket_with_tags(&server, &[tag]);

        Response::from_websocket(client)
    }

    async fn websocket_message(
        &self,
        ws: WebSocket,
        message: WebSocketIncomingMessage,
    ) -> Result<()> {
        let tags = self.state.get_tags(&ws);
        let is_browser = tags.contains(&"browser".to_string());
        let is_pylon = tags.contains(&"pylon".to_string());

        let msg_text = match message {
            WebSocketIncomingMessage::String(s) => s,
            WebSocketIncomingMessage::Binary(b) => {
                String::from_utf8(b).unwrap_or_default()
            }
        };

        if is_pylon {
            self.broadcast_to_browsers(&msg_text);
        } else if is_browser {
            self.broadcast_to_pylons(&msg_text);
        }

        Ok(())
    }

    async fn websocket_close(
        &self,
        _ws: WebSocket,
        _code: usize,
        _reason: String,
        _was_clean: bool,
    ) -> Result<()> {
        Ok(())
    }

    async fn websocket_error(&self, _ws: WebSocket, error: Error) -> Result<()> {
        console_log!("RlmRunDO websocket error: {:?}", error);
        Ok(())
    }
}

impl RlmRunDO {
    fn broadcast_to_browsers(&self, message: &str) {
        let sockets = self.state.get_websockets_with_tag("browser");
        for ws in sockets {
            let _: std::result::Result<(), _> = ws.send_with_str(message);
        }
    }

    fn broadcast_to_pylons(&self, message: &str) {
        let sockets = self.state.get_websockets_with_tag("pylon");
        for ws in sockets {
            let _: std::result::Result<(), _> = ws.send_with_str(message);
        }
    }
}
