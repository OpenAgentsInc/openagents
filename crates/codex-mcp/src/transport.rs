use anyhow::Context;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::protocol::{Request, Response};

#[async_trait::async_trait]
pub trait Transport: Send {
    async fn read_request(&mut self) -> anyhow::Result<Option<Request>>;
    async fn write_response(&mut self, response: &Response) -> anyhow::Result<()>;
}

/// JSON-RPC 2.0 framing over newline-delimited JSON on stdin/stdout.
pub struct StdioTransport {
    stdin: BufReader<tokio::io::Stdin>,
    stdout: tokio::io::Stdout,
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl StdioTransport {
    pub fn new() -> Self {
        Self {
            stdin: BufReader::new(tokio::io::stdin()),
            stdout: tokio::io::stdout(),
        }
    }
}

#[async_trait::async_trait]
impl Transport for StdioTransport {
    async fn read_request(&mut self) -> anyhow::Result<Option<Request>> {
        let mut line = String::new();
        let read = self
            .stdin
            .read_line(&mut line)
            .await
            .context("read JSON-RPC line")?;
        if read == 0 {
            return Ok(None);
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(Some(Request {
                jsonrpc: "2.0".to_string(),
                id: None,
                method: "".to_string(),
                params: serde_json::Value::Null,
            }));
        }

        let request = serde_json::from_str::<Request>(trimmed).context("parse JSON-RPC request")?;
        Ok(Some(request))
    }

    async fn write_response(&mut self, response: &Response) -> anyhow::Result<()> {
        let mut json = serde_json::to_vec(response).context("serialize JSON-RPC response")?;
        json.push(b'\n');
        self.stdout
            .write_all(&json)
            .await
            .context("write JSON-RPC response")?;
        self.stdout.flush().await.context("flush stdout")?;
        Ok(())
    }
}
