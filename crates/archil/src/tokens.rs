//! API key management.

use std::sync::Arc;

use reqwest::Method;

use crate::error::Result;
use crate::http::{Http, Req, enc};
use crate::retry::RetryMode;
use crate::types::{ApiTokenResponse, CreateApiTokenRequest, CreatedApiToken};

/// Options for [`Tokens::list`].
#[derive(Debug, Clone, Default)]
pub struct ListTokensOptions {
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

/// API key operations, from [`Archil::tokens`](crate::Archil::tokens).
#[derive(Debug, Clone)]
pub struct Tokens {
    http: Arc<Http>,
}

impl Tokens {
    pub(crate) fn new(http: Arc<Http>) -> Self {
        Self { http }
    }

    pub async fn list(&self, opts: ListTokensOptions) -> Result<Vec<ApiTokenResponse>> {
        #[derive(serde::Deserialize)]
        struct Data {
            tokens: Option<Vec<ApiTokenResponse>>,
        }
        let req = Req::get("/api/tokens")
            .query_opt("limit", opts.limit)
            .query_opt("cursor", opts.cursor);
        let data: Data = self.http.call(req).await?;
        Ok(data.tokens.unwrap_or_default())
    }

    /// Create an API key. The full secret is returned once, in `token`.
    pub async fn create(&self, req: CreateApiTokenRequest) -> Result<CreatedApiToken> {
        self.http
            .call(Req::new(Method::POST, "/api/tokens", RetryMode::Connect).json(req))
            .await
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        let req = Req::new(
            Method::DELETE,
            format!("/api/tokens/{}", enc(id)),
            RetryMode::Connect,
        );
        self.http.call_empty(req).await
    }
}
