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
    /// models, naming the field at fault the way the Python SDK does:
    /// `models`, or `models[i].field` for one card.
    pub async fn list(&self, options: ListOptions) -> Result<Vec<ModelCard>> {
        let raw = self.client.list_models(options).await?;
        let body: serde_json::Value =
            serde_json::from_slice(&raw.bytes).map_err(|_| invalid(&raw, "models"))?;
        let cards = body
            .get("models")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| invalid(&raw, "models"))?;
        cards
            .iter()
            .enumerate()
            .map(|(at, card)| read_card(&raw, at, card))
            .collect()
    }

    /// List the models the account can ask, handing back the response unread,
    /// the way `models.list().asResponse()` does in the JavaScript SDK and
    /// `raw_http_response` does in the Python SDK.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Models::list`], other than
    /// [`Error::ResponseValidation`].
    pub async fn list_raw(&self, options: ListOptions) -> Result<crate::RawResponse> {
        self.client.list_models(options).await
    }
}

/// One card of the listing, or the field that stopped it.
fn read_card(raw: &crate::RawResponse, at: usize, card: &serde_json::Value) -> Result<ModelCard> {
    let object = card
        .as_object()
        .ok_or_else(|| invalid(raw, format!("models[{at}]")))?;
    let read = |field: &str| {
        object
            .get(field)
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| invalid(raw, format!("models[{at}].{field}")))
    };
    Ok(ModelCard {
        name: read("name")?,
        description: read("description")?,
        release_date: read("release_date")?,
    })
}

/// The error a body that is not a model listing raises.
fn invalid(raw: &crate::RawResponse, field_path: impl Into<String>) -> Error {
    Error::ResponseValidation {
        status: raw.status,
        field_path: field_path.into(),
        body: raw.body().map(Box::new),
        request_id: raw.request_id().map(str::to_string),
    }
}
