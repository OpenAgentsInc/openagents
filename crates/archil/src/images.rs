//! Sandbox image builds.

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::error::{Error, ImageBuildError, Result};
use crate::http::{Http, Req, enc};
use crate::retry::RetryMode;

/// Build state of an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageStatus {
    Building,
    Ready,
    Failed,
    #[serde(other)]
    Unknown,
}

/// Why a build failed. `InvalidSource`: the registry reported the image
/// missing or denied access, so retrying won't help. `Interrupted`: the build
/// did not finish; requesting it again starts a new build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFailureReason {
    InvalidSource,
    Interrupted,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
struct ImageWire {
    image_id: String,
    source: String,
    #[serde(default)]
    private: bool,
    status: ImageStatus,
    digest: Option<String>,
    canonical_source: Option<String>,
    failure_reason: Option<ImageFailureReason>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

/// A sandbox image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// 64-character hex id; pass it to `CreateSandboxRequest::image_id`.
    pub image_id: String,
    /// Normalized OCI reference.
    pub source: String,
    pub private: bool,
    pub status: ImageStatus,
    /// `sha256:` digest of the last successful build.
    pub digest: Option<String>,
    /// `repo@sha256:` reference of the last successful build.
    pub canonical_source: Option<String>,
    /// Present only when `status` is `Failed`.
    pub failure_reason: Option<ImageFailureReason>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ImageWire> for Image {
    fn from(w: ImageWire) -> Self {
        Self {
            image_id: w.image_id,
            source: w.source,
            private: w.private,
            status: w.status,
            digest: w.digest,
            canonical_source: w.canonical_source,
            failure_reason: w.failure_reason,
            created_at: w.created_at,
            updated_at: w.updated_at,
        }
    }
}

/// Registry credentials for a private image. Archil never stores them.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryAuth {
    pub username: String,
    /// Password or access token.
    pub password: String,
}

impl std::fmt::Debug for RegistryAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegistryAuth")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// Request to build an image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildImageRequest {
    /// OCI reference such as `node:24-bookworm` or `ghcr.io/owner/app@sha256:<digest>`.
    pub source: String,
    /// Required on every build of a private image; without it `source` names
    /// the public image.
    pub registry_auth: Option<RegistryAuth>,
}

impl BuildImageRequest {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            registry_auth: None,
        }
    }
    pub fn registry_auth(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.registry_auth = Some(RegistryAuth {
            username: username.into(),
            password: password.into(),
        });
        self
    }
}

/// Options for [`Images::build_with`].
#[derive(Debug, Clone, Copy)]
pub struct BuildImageOptions {
    /// Seconds to wait for the build. Defaults to 1800 (30 minutes).
    pub timeout_seconds: u64,
}

impl Default for BuildImageOptions {
    fn default() -> Self {
        Self {
            timeout_seconds: 30 * 60,
        }
    }
}

const MAX_INTERRUPTED_RETRIES: u32 = 3;
const REQUEST_AGAIN: Duration = Duration::from_secs(2 * 60);

/// Image operations, from [`Archil::images`](crate::Archil::images).
#[derive(Debug, Clone)]
pub struct Images {
    http: Arc<Http>,
}

impl Images {
    pub(crate) fn new(http: Arc<Http>) -> Self {
        Self { http }
    }

    pub async fn get(&self, image_id: &str) -> Result<Image> {
        let wire: ImageWire = self
            .http
            .call(Req::get(format!("/api/images/{}", enc(image_id))))
            .await?;
        Ok(wire.into())
    }

    /// Build an image and wait until it is ready, with the default 30 minute budget.
    pub async fn build(&self, request: BuildImageRequest) -> Result<Image> {
        self.build_with(request, BuildImageOptions::default()).await
    }

    /// Build a sandbox image from an OCI reference and wait until it is ready.
    ///
    /// Returns at once when the image is already built from a digest
    /// reference and joins a build that is already running. A tag is rebuilt
    /// on each call in case it moved, so call this once and reuse `image_id`.
    /// Interrupted builds are requested again up to three times; other
    /// failures return [`ImageBuildError`]. Running out of time returns an
    /// [`Error::Archil`] with code `IMAGE_BUILD_TIMEOUT`.
    pub async fn build_with(
        &self,
        request: BuildImageRequest,
        options: BuildImageOptions,
    ) -> Result<Image> {
        let poll = self.http.poll_interval;
        let first_poll = poll * 2;
        let max_poll = poll * 10;
        let deadline = Instant::now() + Duration::from_secs(options.timeout_seconds);
        let mut image = self.request(&request).await?;
        let mut last_request = Instant::now();
        let mut retries = 0;
        let mut poll_wait = first_poll;
        loop {
            match image.status {
                ImageStatus::Ready => return Ok(image),
                ImageStatus::Failed => {
                    if image.failure_reason != Some(ImageFailureReason::Interrupted)
                        || retries >= MAX_INTERRUPTED_RETRIES
                    {
                        return Err(ImageBuildError {
                            latest: Box::new(image),
                        }
                        .into());
                    }
                    retries += 1;
                    image = self.request(&request).await?;
                    last_request = Instant::now();
                    continue;
                }
                _ => {}
            }
            if Instant::now() >= deadline {
                return Err(Error::Archil {
                    message: format!(
                        "Image {} was not ready after {} seconds",
                        image.image_id, options.timeout_seconds
                    ),
                    status: 408,
                    code: Some("IMAGE_BUILD_TIMEOUT".into()),
                });
            }
            tokio::time::sleep(poll_wait).await;
            poll_wait = (poll_wait * 2).min(max_poll);
            image = self.get(&image.image_id).await?;
            // Requesting a still-building image again joins a healthy build
            // and restarts one whose runtime host is gone.
            if image.status == ImageStatus::Building && last_request.elapsed() >= REQUEST_AGAIN {
                image = self.request(&request).await?;
                last_request = Instant::now();
            }
        }
    }

    async fn request(&self, request: &BuildImageRequest) -> Result<Image> {
        let mut body = serde_json::json!({ "source": request.source });
        if let Some(auth) = &request.registry_auth {
            body["registry_auth"] = serde_json::to_value(auth).expect("serializes");
        }
        let wire: ImageWire = self
            .http
            .call(Req::new(Method::POST, "/api/images", RetryMode::Transient).json(body))
            .await?;
        Ok(wire.into())
    }
}
