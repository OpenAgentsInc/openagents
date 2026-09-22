//! The per-call overrides every service route shares.
//!
//! [`crate::models::ListOptions`] predates the wider surface; new modules
//! take one `CallOptions` so retry, timeout, and header policy reads the
//! same on every route.

use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue};

use crate::Result;
use crate::error::Error;
use crate::retry::RetryPolicy;

/// The `Idempotency-Key` header a retried submission keeps.
const IDEMPOTENCY: &str = "idempotency-key";

/// The `X-Workspace-Id` header account-scoped calls name.
const WORKSPACE: &str = "x-workspace-id";

/// What one call overrides on the client.
#[derive(Debug, Clone, Default)]
pub struct CallOptions {
    pub(crate) retry: Option<RetryPolicy>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) headers: HeaderMap,
}

impl CallOptions {
    /// Options that override nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Retry this call by another policy.
    #[must_use]
    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = Some(retry);
        self
    }

    /// Give each attempt of this call another timeout.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Add headers to this call.
    #[must_use]
    pub fn headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self
    }

    /// Send one `Idempotency-Key` on the call — the dedupe a retried
    /// submission shares.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] when the key is not a header value.
    pub fn idempotency_key(mut self, key: &str) -> Result<Self> {
        let value = HeaderValue::from_str(key)
            .map_err(|_| Error::Config("the idempotency key is not a header value".into()))?;
        self.headers.insert(IDEMPOTENCY, value);
        Ok(self)
    }

    /// Name the workspace an account-scoped call reads under
    /// (`X-Workspace-Id`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] when the id is not a header value.
    pub fn workspace(mut self, workspace: &str) -> Result<Self> {
        let value = HeaderValue::from_str(workspace)
            .map_err(|_| Error::Config("the workspace id is not a header value".into()))?;
        self.headers.insert(WORKSPACE, value);
        Ok(self)
    }
}
