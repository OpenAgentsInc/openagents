//! NIP-CW `POST /query`.
//!
//! A filter with `top_level: true` is a channel window. Any other filter is
//! served as an ordinary history query. Both require NIP-98 authentication.

use tokio::{net::TcpStream, sync::watch};

use crate::domain::{Event, Filter, RelaySigner, parse_http_authorization};
use nostr::channel_window::{self, WindowRequest};

use super::{
    GatewayConfig, GatewayError,
    db::DbPool,
    server::unix_now,
    socket::{HttpHead, read_http_body, write_http},
};

const MAX_BODY: usize = 65_536;
const SCAN_LIMIT: usize = 4_096;

pub fn is_query_request(head: &HttpHead) -> bool {
    head.method == "POST" && head.path == "/query"
}

pub async fn serve_query(
    mut stream: TcpStream,
    head: &HttpHead,
    config: &GatewayConfig,
    db: &DbPool,
) -> Result<(), GatewayError> {
    if config.relay_url.is_none() || config.relay_signer.is_none() {
        return write_http(
            &mut stream,
            404,
            "Not Found",
            "application/json",
            "{\"error\":\"channel window is not configured\"}",
        )
        .await;
    }
    let body = match read_http_body(&mut stream, head, MAX_BODY).await {
        Ok(body) => body,
        Err(_) => {
            return write_http(
                &mut stream,
                400,
                "Bad Request",
                "application/json",
                "{\"error\":\"invalid request body\"}",
            )
            .await;
        }
    };
    let Some(authorization) = head.header("authorization") else {
        return write_http(
            &mut stream,
            401,
            "Unauthorized",
            "application/json",
            "{\"error\":\"NIP-98 authorization is required\"}",
        )
        .await;
    };
    let absolute = config.absolute_http_url("/query")?;
    let auth = match parse_http_authorization(authorization, "POST", &absolute, &body, unix_now()) {
        Ok(auth) => auth,
        Err(_) => {
            return write_http(
                &mut stream,
                401,
                "Unauthorized",
                "application/json",
                "{\"error\":\"invalid NIP-98 authorization\"}",
            )
            .await;
        }
    };
    let value: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => {
            return write_http(
                &mut stream,
                400,
                "Bad Request",
                "application/json",
                "{\"error\":\"filter is not JSON\"}",
            )
            .await;
        }
    };
    let signer = config.relay_signer.as_ref().expect("signer was checked");
    let events = match channel_window::parse_window(&value) {
        Ok(Some(request)) => {
            match window_events(db, &auth.pubkey, &request, signer, unix_now()).await {
                Ok(events) => events,
                Err(reason) => {
                    let status = if reason == "scan" { 503 } else { 400 };
                    let phrase = if status == 503 {
                        "Service Unavailable"
                    } else {
                        "Bad Request"
                    };
                    return write_http(
                        &mut stream,
                        status,
                        phrase,
                        "application/json",
                        &format!("{{\"error\":\"{reason}\"}}"),
                    )
                    .await;
                }
            }
        }
        Ok(None) => match serde_json::from_value::<Filter>(value) {
            Ok(filter) => ordinary_events(db, &auth.pubkey, filter).await?,
            Err(_) => {
                return write_http(
                    &mut stream,
                    400,
                    "Bad Request",
                    "application/json",
                    "{\"error\":\"filter\"}",
                )
                .await;
            }
        },
        Err(reason) => {
            return write_http(
                &mut stream,
                400,
                "Bad Request",
                "application/json",
                &format!("{{\"error\":\"{reason}\"}}"),
            )
            .await;
        }
    };
    let body = serde_json::to_string(&events)
        .map_err(|error| GatewayError::Internal(format!("query serialization: {error}")))?;
    write_http(&mut stream, 200, "OK", "application/json", &body).await
}

async fn window_events(
    db: &DbPool,
    reader: &str,
    request: &WindowRequest,
    signer: &RelaySigner,
    now: u64,
) -> Result<Vec<Event>, &'static str> {
    if !db
        .channel_window_served(request.channel.clone(), reader.to_owned())
        .await
        .map_err(|_| "store")?
    {
        return Ok(Vec::new());
    }
    let mut raw = serde_json::json!({
        "#h": [request.channel],
        "limit": SCAN_LIMIT + 1,
    });
    if let Some(cursor) = &request.cursor {
        raw["until"] = serde_json::json!(cursor.created_at);
    }
    let filter: Filter = serde_json::from_value(raw).map_err(|_| "filter")?;
    let (_sender, cancel) = watch::channel(false);
    let history = db
        .history(
            vec![filter],
            now,
            SCAN_LIMIT + 1,
            cancel,
            vec![reader.to_owned()],
        )
        .await
        .map_err(|_| "store")?;
    let truncated = history.events.len() > SCAN_LIMIT;
    let events = history
        .events
        .into_iter()
        .map(|stored| stored.event)
        .collect::<Vec<_>>();
    project_window(&events, request, truncated, true, signer, now)
}

async fn ordinary_events(
    db: &DbPool,
    reader: &str,
    filter: Filter,
) -> Result<Vec<Event>, GatewayError> {
    let (_sender, cancel) = watch::channel(false);
    let history = db
        .history(
            vec![filter],
            unix_now(),
            SCAN_LIMIT,
            cancel,
            vec![reader.to_owned()],
        )
        .await?;
    Ok(history
        .events
        .into_iter()
        .map(|stored| stored.event)
        .collect())
}

/// Serve a window from an already authorized event set.
///
/// An unserved channel returns an empty array and no bounds overlay.
///
/// # Errors
///
/// Returns `scan` when the supplied events are a truncated channel and the
/// page cannot prove its cursor.
pub fn project_window(
    events: &[Event],
    request: &WindowRequest,
    truncated: bool,
    served: bool,
    signer: &RelaySigner,
    now: u64,
) -> Result<Vec<Event>, &'static str> {
    if !served {
        return Ok(Vec::new());
    }
    channel_window::render_window(events, request, truncated, signer, now).map(|page| page.events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{RelaySigner, Tag};
    use nostr::channel_window::{BOUNDS_KIND, parse_window};
    use serde_json::json;

    #[test]
    fn a_hidden_channel_has_no_bounds_and_an_empty_one_does() {
        let signer = RelaySigner::from_secret_hex(&"71".repeat(32)).unwrap();
        let request = parse_window(&json!({"#h": ["room"], "top_level": true}))
            .unwrap()
            .unwrap();
        let hidden = project_window(&[], &request, false, false, &signer, 10).unwrap();
        assert!(hidden.is_empty());
        let empty = project_window(&[], &request, false, true, &signer, 10).unwrap();
        assert_eq!(empty.len(), 1);
        assert_eq!(empty[0].kind, BOUNDS_KIND);
        let row = Event {
            id: "12".repeat(32),
            pubkey: "11".repeat(32),
            created_at: 5,
            kind: 1,
            tags: vec![Tag::new(vec!["h".into(), "room".into()])],
            content: String::new(),
            sig: "22".repeat(64),
        };
        let page = project_window(
            std::slice::from_ref(&row),
            &request,
            false,
            true,
            &signer,
            10,
        )
        .unwrap();
        assert!(page.iter().any(|event| event.kind == 1));
        assert!(page.iter().any(|event| event.kind == BOUNDS_KIND));
    }
}
