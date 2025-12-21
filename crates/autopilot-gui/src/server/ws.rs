//! WebSocket handler for real-time message streaming

use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_ws::{Message as WsMessage, Session};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

/// WebSocket upgrade endpoint
pub async fn websocket(
    req: HttpRequest,
    stream: web::Payload,
) -> Result<HttpResponse, Error> {
    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    // Spawn WebSocket handler
    actix_web::rt::spawn(async move {
        info!("WebSocket connection established");

        while let Some(msg) = msg_stream.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    debug!("Received message: {}", text);

                    // Parse incoming message
                    match serde_json::from_str::<ClientMessage>(&text) {
                        Ok(client_msg) => {
                            handle_client_message(client_msg, &mut session).await;
                        }
                        Err(e) => {
                            error!("Failed to parse message: {}", e);
                        }
                    }
                }
                Ok(WsMessage::Ping(msg)) => {
                    let _ = session.pong(&msg).await;
                }
                Ok(WsMessage::Close(reason)) => {
                    info!("WebSocket closed: {:?}", reason);
                    break;
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        info!("WebSocket connection closed");
    });

    Ok(response)
}

/// Client -> Server message types
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ClientMessage {
    #[serde(rename = "prompt")]
    Prompt { text: String },

    #[serde(rename = "abort")]
    Abort,
}

/// Server -> Client message types
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
#[allow(dead_code)]
enum ServerMessage {
    #[serde(rename = "message")]
    Message {
        role: String,
        content: String,
    },

    #[serde(rename = "status")]
    Status {
        status: String,
    },

    #[serde(rename = "error")]
    Error {
        message: String,
    },
}

async fn handle_client_message(msg: ClientMessage, session: &mut Session) {
    match msg {
        ClientMessage::Prompt { text } => {
            debug!("Handling prompt: {}", text);

            // Echo back for now (will integrate with agent in future)
            let response = ServerMessage::Message {
                role: "assistant".to_string(),
                content: format!("Echo: {}", text),
            };

            send_message(session, response).await;
        }
        ClientMessage::Abort => {
            debug!("Abort requested");

            let response = ServerMessage::Status {
                status: "aborted".to_string(),
            };

            send_message(session, response).await;
        }
    }
}

async fn send_message(session: &mut Session, msg: ServerMessage) {
    match serde_json::to_string(&msg) {
        Ok(json) => {
            if let Err(e) = session.text(json).await {
                error!("Failed to send message: {}", e);
            }
        }
        Err(e) => {
            error!("Failed to serialize message: {}", e);
        }
    }
}
