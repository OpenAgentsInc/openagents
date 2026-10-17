use std::fmt;
use std::time::{Duration, SystemTime};

use reqwest::Method;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use url::Url;

/// The two explicit addresses belonging to one deployment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deployment {
    api: Url,
    website: Url,
}

fn allowed(url: &Url) -> bool {
    let loopback = url.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    (url.scheme() == "https" || url.scheme() == "http" && loopback)
        && url.host().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

impl Deployment {
    pub fn new(api_base: &str, website: &str) -> Result<Self, Error> {
        let mut api = Url::parse(api_base).map_err(|_| Error::InvalidDeployment)?;
        let website = Url::parse(website).map_err(|_| Error::InvalidDeployment)?;
        if api.path() == "/v1" {
            api.set_path("/v1/");
        }
        let deployment = Self { api, website };
        deployment.validate()?;
        Ok(deployment)
    }

    #[must_use]
    pub fn production() -> Self {
        Self::new("https://api.openagents.com/v1", "https://openagents.com")
            .expect("fixed production addresses")
    }

    pub(crate) fn validate(&self) -> Result<(), Error> {
        if allowed(&self.api)
            && allowed(&self.website)
            && self.api.path() == "/v1/"
            && self.website.path() == "/"
        {
            Ok(())
        } else {
            Err(Error::InvalidDeployment)
        }
    }

    pub fn api_base(&self) -> &Url {
        &self.api
    }
    pub fn website(&self) -> &Url {
        &self.website
    }

    pub(crate) fn verification(&self, uri: &str, code: &str) -> Result<Url, Error> {
        let expected = self
            .website
            .join("device")
            .map_err(|_| Error::InvalidResponse)?;
        let actual = Url::parse(uri).map_err(|_| Error::InvalidResponse)?;
        if actual != expected
            || code.is_empty()
            || code.len() > 32
            || code.chars().any(char::is_control)
        {
            return Err(Error::InvalidResponse);
        }
        let mut complete = expected;
        complete.query_pairs_mut().append_pair("code", code);
        Ok(complete)
    }
}

/// Error diagnostics omit response bodies, credential values, request URLs,
/// and the HTTP 402 headers available through the explicit accessor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidDeployment,
    InvalidTimeout,
    InvalidCredential,
    WrongDeployment,
    WrongAccount,
    ExpiredSession,
    Unauthorized,
    Forbidden,
    Denied,
    ExpiredLogin,
    FinishedLogin,
    Cancelled,
    /// A poll might have redeemed its single-use grant. Start a new login.
    UncertainLogin(Box<Error>),
    Network {
        timeout: bool,
    },
    InvalidResponse,
    NotFound,
    Changed,
    InvalidCursor,
    InvalidReference,
    TooLarge,
    Redirect,
    PaymentRequired(crate::PaymentRequired),
    RateLimited {
        retry_after: Option<Duration>,
    },
    Unavailable {
        retry_after: Option<Duration>,
    },
    Refused {
        status: u16,
        code: Option<&'static str>,
    },
}

impl Error {
    pub fn payment_required(&self) -> Option<&crate::PaymentRequired> {
        match self {
            Self::PaymentRequired(challenge) => Some(challenge),
            Self::UncertainLogin(cause) => cause.payment_required(),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidDeployment => "Check the website address and try again.",
            Self::InvalidTimeout => "Use positive timeouts up to two minutes, with the connection timeout no longer than the request timeout.",
            Self::InvalidCredential => "Log in again to continue.",
            Self::WrongDeployment => "This login belongs to another site.",
            Self::WrongAccount => "This login belongs to another account.",
            Self::ExpiredSession | Self::Unauthorized => "Log in again to continue.",
            Self::Forbidden => "You don't have access to this.",
            Self::Denied => "Login was denied on the website.",
            Self::ExpiredLogin => "The login code expired. Start again.",
            Self::FinishedLogin => "Start a new login to continue.",
            Self::Cancelled => "Canceled.",
            Self::UncertainLogin(_) => "Login could not be confirmed. Start again.",
            Self::Network { .. } => "Couldn't connect. Check your connection and try again.",
            Self::NotFound => "This chat or file is no longer available.",
            Self::Changed => "This chat changed. Load it again.",
            Self::InvalidCursor => "Start again from the first page.",
            Self::InvalidReference => "This chat or file could not be opened.",
            Self::InvalidResponse => "That response couldn't be opened. Try again.",
            Self::TooLarge => "The response is too large to open.",
            Self::Redirect => "Check the website address and try again.",
            Self::PaymentRequired(_) => "Payment is required. No payment was sent.",
            Self::RateLimited { .. } => "Too many requests. Try again later.",
            Self::Unavailable { .. } => "The service is unavailable. Try again later.",
            Self::Refused { .. } => "The service could not complete this request.",
        })
    }
}
impl std::error::Error for Error {}

/// A bounded client for one deployment that refuses redirects. Auth and history
/// send once; the Computers facade applies its own operation-specific retry policy.
#[derive(Clone)]
pub struct Client {
    pub(crate) deployment: Deployment,
    pub(crate) http: reqwest::Client,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("deployment", &self.deployment)
            .finish_non_exhaustive()
    }
}

impl Client {
    pub fn new(deployment: Deployment) -> Result<Self, Error> {
        Self::with_timeout(deployment, Duration::from_secs(20))
    }

    pub fn with_timeout(deployment: Deployment, timeout: Duration) -> Result<Self, Error> {
        Self::with_timeouts(deployment, timeout.min(Duration::from_secs(10)), timeout)
    }

    pub fn with_timeouts(
        deployment: Deployment,
        connect_timeout: Duration,
        timeout: Duration,
    ) -> Result<Self, Error> {
        deployment.validate()?;
        if timeout.is_zero()
            || timeout > Duration::from_secs(120)
            || connect_timeout.is_zero()
            || connect_timeout > timeout
        {
            return Err(Error::InvalidTimeout);
        }
        let http = reqwest::Client::builder()
            .pool_max_idle_per_host(8)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(connect_timeout)
            .timeout(timeout)
            .build()
            .map_err(|_| Error::Network { timeout: false })?;
        Ok(Self { deployment, http })
    }

    pub fn deployment(&self) -> &Deployment {
        &self.deployment
    }

    pub(crate) async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        segments: &[&str],
        query: &[(&str, String)],
        body: Option<Value>,
        token: Option<&str>,
    ) -> Result<T, Error> {
        let response = self.send(method, segments, query, body, token).await?;
        let limit = if segments.first() == Some(&"labs") && segments.get(1) == Some(&"agent") {
            16 * 1024 * 1024
        } else if matches!(
            segments.first().copied(),
            Some("agent-definitions" | "agent-assignments")
        ) {
            4 * 1024 * 1024
        } else {
            1024 * 1024
        };
        Self::decode(response, segments, limit).await
    }

    pub(crate) async fn history_json<T: DeserializeOwned>(
        &self,
        segments: &[&str],
        query: &[(&str, String)],
        token: &str,
    ) -> Result<T, Error> {
        let response = self
            .send(Method::GET, segments, query, None, Some(token))
            .await?;
        let limit = if segments.last() == Some(&"messages") {
            17 * 1024 * 1024
        } else {
            1024 * 1024
        };
        Self::decode(response, segments, limit).await
    }

    pub(crate) async fn history_bytes(
        &self,
        segments: &[&str],
        token: &str,
        limit: usize,
    ) -> Result<Vec<u8>, Error> {
        let response = self
            .send(Method::GET, segments, &[], None, Some(token))
            .await?;
        if !response.status().is_success() {
            Self::decode::<Value>(response, segments, 1024 * 1024).await?;
            return Err(Error::InvalidResponse);
        }
        Self::read_limited(response, limit).await
    }

    async fn send(
        &self,
        method: Method,
        segments: &[&str],
        query: &[(&str, String)],
        body: Option<Value>,
        token: Option<&str>,
    ) -> Result<reqwest::Response, Error> {
        self.send_with_timeout(method, segments, query, body, token, None)
            .await
    }

    pub(crate) async fn send_events(
        &self,
        segments: &[&str],
        query: &[(&str, String)],
        token: &str,
    ) -> Result<reqwest::Response, Error> {
        self.send_with_timeout(
            Method::GET,
            segments,
            query,
            None,
            Some(token),
            Some(Duration::from_secs(310)),
        )
        .await
    }

    async fn send_with_timeout(
        &self,
        method: Method,
        segments: &[&str],
        query: &[(&str, String)],
        body: Option<Value>,
        token: Option<&str>,
        timeout: Option<Duration>,
    ) -> Result<reqwest::Response, Error> {
        let mut url = self.deployment.api.clone();
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|_| Error::InvalidDeployment)?;
            path.pop_if_empty();
            for segment in segments {
                path.push(segment);
            }
        }
        if !query.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(query.iter().map(|(k, v)| (*k, v.as_str())));
        }
        let mut request = self.http.request(method, url);
        if let Some(timeout) = timeout {
            request = request.timeout(timeout);
        }
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.map_err(|error| Error::Network {
            timeout: error.is_timeout(),
        })?;
        let status = response.status();
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|h| h.to_str().ok())
            .and_then(|value| {
                value
                    .parse::<u64>()
                    .ok()
                    .map(Duration::from_secs)
                    .or_else(|| {
                        httpdate::parse_http_date(value)
                            .ok()?
                            .duration_since(SystemTime::now())
                            .ok()
                    })
            });
        if status.is_redirection() {
            return Err(Error::Redirect);
        }
        if status.as_u16() == 402 {
            return Err(Error::PaymentRequired(
                crate::PaymentRequired::from_headers(response.headers()),
            ));
        }
        if status.as_u16() == 429 {
            return Err(Error::RateLimited { retry_after });
        }
        if status.is_server_error() || status.as_u16() == 408 {
            return Err(Error::Unavailable { retry_after });
        }
        if status.as_u16() == 401 {
            return Err(Error::Unauthorized);
        }
        if status.as_u16() == 403 {
            return Err(Error::Forbidden);
        }
        Ok(response)
    }
    pub(crate) async fn read_limited(
        mut response: reqwest::Response,
        limit: usize,
    ) -> Result<Vec<u8>, Error> {
        if response
            .content_length()
            .is_some_and(|size| size > limit as u64)
        {
            return Err(Error::TooLarge);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|error| Error::Network {
            timeout: error.is_timeout(),
        })? {
            if chunk.len() > limit.saturating_sub(bytes.len()) {
                return Err(Error::TooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
    pub(crate) async fn decode<T: DeserializeOwned>(
        response: reqwest::Response,
        segments: &[&str],
        limit: usize,
    ) -> Result<T, Error> {
        let status = response.status();
        let mut bytes = Self::read_limited(
            response,
            if status.is_success() {
                limit
            } else {
                1024 * 1024
            },
        )
        .await?;
        let value = serde_json::from_slice::<Value>(&bytes).map_err(|_| Error::InvalidResponse);
        bytes.fill(0);
        let value = value?;
        if !status.is_success() {
            // Device errors use a different envelope and are interpreted by the shared protocol.
            if segments == ["device", "token"]
                && !matches!(
                    openagents_login::protocol::device_error(&value, 0),
                    openagents_login::protocol::DeviceError::Other
                )
            {
                return serde_json::from_value(value).map_err(|_| Error::InvalidResponse);
            }
            let code = match value["error"]["code"].as_str() {
                Some("not_found") => Some("not_found"),
                Some("invalid_cursor") => Some("invalid_cursor"),
                Some("changed") => Some("changed"),
                Some("version_conflict") => Some("version_conflict"),
                Some("idempotency_conflict") => Some("idempotency_conflict"),
                Some("invalid_agent") => Some("invalid_agent"),
                Some("assignment_conflict") => Some("assignment_conflict"),
                Some("invalid_assignment") => Some("invalid_assignment"),
                Some("quota_exhausted") => Some("quota_exhausted"),
                Some("busy") => Some("busy"),
                Some("invalid") => Some("invalid"),
                Some("secret") => Some("secret"),
                Some("computer_chat") => Some("computer_chat"),
                Some("project_role") => Some("project_role"),
                Some("unsupported_turn") => Some("unsupported_turn"),
                _ => None,
            };
            return Err(Error::Refused {
                status: status.as_u16(),
                code,
            });
        }
        serde_json::from_value(value).map_err(|_| Error::InvalidResponse)
    }
}
