//! Just enough Chrome DevTools to drive the demo's checks: open a page,
//! evaluate an expression, and hold a key. The browser comes from
//! `openagents browser run`, which sets `OPENAGENTS_CHROME_PORT`.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// One page's DevTools connection.
pub struct Page {
    socket: Socket,
    next: u64,
}

/// The DevTools port `openagents browser run` gave this process.
#[must_use]
pub fn port() -> Option<u16> {
    std::env::var("OPENAGENTS_CHROME_PORT").ok()?.parse().ok()
}

impl Page {
    /// Opens `url` in a new tab and connects to it.
    ///
    /// # Errors
    /// The browser can't be reached.
    pub async fn open(port: u16, url: &str) -> Result<Self> {
        let target: Value = reqwest::Client::new()
            .put(format!("http://127.0.0.1:{port}/json/new?about:blank"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let ws = target["webSocketDebuggerUrl"]
            .as_str()
            .context("the new tab has no DevTools address")?;
        let (socket, _) = tokio_tungstenite::connect_async(ws).await?;
        let mut page = Self { socket, next: 1 };
        // Navigated rather than opened by address, so a `#fragment` reaches
        // the page.
        page.navigate(url).await?;
        Ok(page)
    }

    /// Loads `url` in this tab.
    ///
    /// # Errors
    /// DevTools failed.
    pub async fn navigate(&mut self, url: &str) -> Result<()> {
        self.call("Page.navigate", json!({ "url": url })).await?;
        Ok(())
    }

    /// Evaluates `expression` until it is truthy, every 100 ms, for up to
    /// `timeout`; its last value.
    ///
    /// # Errors
    /// It never became truthy.
    pub async fn until(&mut self, expression: &str, timeout: Duration) -> Result<Value> {
        let started = std::time::Instant::now();
        loop {
            let value = self.eval(expression).await.unwrap_or(Value::Null);
            let truthy =
                !matches!(&value, Value::Null | Value::Bool(false)) && value.as_str() != Some("");
            if truthy {
                return Ok(value);
            }
            if started.elapsed() > timeout {
                bail!("{expression} never held");
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next;
        self.next += 1;
        let message = json!({ "id": id, "method": method, "params": params });
        self.socket
            .send(Message::Text(message.to_string().into()))
            .await?;
        loop {
            let Some(reply) = tokio::time::timeout(Duration::from_secs(30), self.socket.next())
                .await
                .context("DevTools didn't answer")?
            else {
                bail!("DevTools closed");
            };
            let Message::Text(text) = reply? else {
                continue;
            };
            let value: Value = serde_json::from_str(&text)?;
            if value["id"] == id {
                if let Some(error) = value.get("error") {
                    bail!("{method}: {error}");
                }
                return Ok(value["result"].clone());
            }
        }
    }

    /// The value of `expression` in the page, as JSON.
    ///
    /// # Errors
    /// The expression threw, or DevTools failed.
    pub async fn eval(&mut self, expression: &str) -> Result<Value> {
        let result = self
            .call(
                "Runtime.evaluate",
                json!({ "expression": expression, "returnByValue": true, "awaitPromise": true }),
            )
            .await?;
        if let Some(thrown) = result.get("exceptionDetails") {
            bail!("{expression}: {thrown}");
        }
        Ok(result["result"]["value"].clone())
    }

    /// Presses (`down`) or releases a key, as a keyboard would.
    ///
    /// # Errors
    /// DevTools failed.
    pub async fn key(&mut self, key: &str, code: &str, down: bool) -> Result<()> {
        let kind = if down { "keyDown" } else { "keyUp" };
        self.call(
            "Input.dispatchKeyEvent",
            json!({ "type": kind, "key": key, "code": code, "windowsVirtualKeyCode": key.to_ascii_uppercase().chars().next().map_or(0, |c| c as u32) }),
        )
        .await?;
        Ok(())
    }
}
