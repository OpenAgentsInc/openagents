//! Thin client for a Coder `/v1/responses` Open Responses door.
//!
//! This first slice exposes only the pieces the CLI needs to post a request
//! and read the first event from the returned Server-Sent Events stream.

use clap::Args;
use serde_json::{Value, json};

/// Command-line arguments for `openagents responses`.
#[derive(Args, Debug)]
pub struct ResponsesArgs {
    /// Prompt to send to the Coder Responses endpoint.
    pub prompt: String,

    /// Coder origin to post to.
    #[arg(long, default_value = "http://127.0.0.1:4000")]
    pub origin: String,
}

/// Post a prompt to the Coder Responses endpoint and print the returned text.
pub async fn run(args: ResponsesArgs, token: Option<String>) -> Result<(), reqwest::Error> {
    let client = Client::new(args.origin, token);
    let body = json!({
        "input": [{"role": "user", "content": args.prompt}]
    });
    let response = client.post(&body).await?;
    let text = response.text().await?;
    println!("{text}");
    Ok(())
}

/// Client for the Coder Responses endpoint.
pub struct Client {
    http: reqwest::Client,
    origin: String,
    api_key: Option<String>,
}

impl Client {
    /// Make a new client for `origin` with an optional bearer token.
    #[allow(dead_code)]
    pub fn new(origin: String, api_key: Option<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            origin,
            api_key,
        }
    }

    /// Post a request body to `/v1/responses` and return the raw response.
    #[allow(dead_code)]
    pub async fn post(&self, body: &Value) -> Result<reqwest::Response, reqwest::Error> {
        let mut request = self
            .http
            .post(format!("{}/v1/responses", self.origin))
            .json(body);

        if let Some(key) = &self.api_key {
            request = request.header("authorization", format!("Bearer {key}"));
        }

        request.send().await
    }
}
