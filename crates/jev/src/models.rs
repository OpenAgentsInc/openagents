//! The models the account can ask.

use std::time::Duration;

use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::Result;
use crate::client::Client;
use crate::error::Error;
use crate::retry::RetryPolicy;

/// One model the account can ask.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ModelCard {
    /// The name a request names in `model`.
    pub name: String,
    /// What the model is for, as TypeSafe describes it.
    #[serde(default)]
    pub description: String,
    /// The day the model was released, as the API writes it.
    #[serde(default)]
    pub release_date: String,
}

/// What one listing overrides on the client.
#[derive(Debug, Clone, Default)]
pub struct ListOptions {
    pub(crate) retry: Option<RetryPolicy>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) headers: HeaderMap,
}

impl ListOptions {
    /// Options that override nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Retry this listing by another policy.
    #[must_use]
    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = Some(retry);
        self
    }

    /// Give each attempt of this listing another timeout.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Add headers to this listing.
    #[must_use]
    pub fn headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self
    }
}

/// The models resource, reached through [`Client::models`].
#[derive(Debug)]
pub struct Models<'a> {
    client: &'a Client,
}

impl<'a> Models<'a> {
    /// Reach the resource through a client.
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// List the models the account can ask.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Client::system_one`], and
    /// [`Error::ResponseValidation`] when the body does not carry a list of
    /// models.
    pub async fn list(&self, options: ListOptions) -> Result<Vec<ModelCard>> {
        let raw = self.client.list_models(options).await?;
        let envelope: Envelope =
            serde_json::from_slice(&raw.bytes).map_err(|_| Error::ResponseValidation {
                status: raw.status,
                field_path: "models".to_string(),
                body: raw.body().map(Box::new),
                request_id: raw.request_id().map(str::to_string),
            })?;
        Ok(envelope.models)
    }
}

/// The body `GET /v1/models` returns.
#[derive(Deserialize)]
struct Envelope {
    models: Vec<ModelCard>,
}
