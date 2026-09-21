//! What a client is built from, and how a setting is resolved.
//!
//! Resolution follows both official SDKs: an explicit value, then the
//! environment variable, then the default. An environment value that is empty
//! or holds only whitespace is ignored, so an unset variable and a blank one
//! read the same.

use std::fmt;
use std::time::Duration;

use reqwest::header::HeaderMap;

use crate::error::Error;
use crate::retry::RetryPolicy;
use crate::{Result, defaults, env};

/// An API key that no log line, error, or `Debug` output carries.
///
/// ```
/// use jev::ApiKey;
///
/// let key = ApiKey::new("ts-secret-value");
/// assert_eq!(format!("{key:?}"), "***");
/// assert_eq!(key.to_string(), "***");
/// assert_eq!(key.expose(), "ts-secret-value");
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl ApiKey {
    /// Hold a key.
    #[must_use]
    pub fn new<K: Into<String>>(key: K) -> Self {
        Self(key.into())
    }

    /// The key itself. Call this where the key has to go on the wire, and
    /// nowhere else.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl fmt::Display for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl From<&str> for ApiKey {
    fn from(key: &str) -> Self {
        Self::new(key)
    }
}

impl From<String> for ApiKey {
    fn from(key: String) -> Self {
        Self::new(key)
    }
}

/// The settings a client is built from. Every setting a caller leaves out is
/// read from the environment, and then from the defaults.
///
/// ```
/// use std::time::Duration;
///
/// use jev::Config;
///
/// let config = Config::new()
///     .api_key("ts-secret-value")
///     .base_url("https://api.typesafe.ai")
///     .timeout(Duration::from_secs(5));
/// ```
#[derive(Debug, Clone, Default)]
pub struct Config {
    api_key: Option<ApiKey>,
    local_only: bool,
    base_url: Option<String>,
    default_model: Option<String>,
    timeout: Option<Duration>,
    retry: Option<RetryPolicy>,
    default_headers: HeaderMap,
    http_client: Option<reqwest::Client>,
}

impl Config {
    /// Settings that come from the environment and the defaults alone.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Connect directly to an explicitly named loopback IP without credentials.
    /// This mode ignores provider environment settings and disables proxies and
    /// redirects. It refuses custom HTTP clients and explicit API keys because
    /// either could widen its transport or credential boundary.
    #[must_use]
    pub fn local<U: Into<String>, M: Into<String>>(url: U, model: M) -> Self {
        Self {
            local_only: true,
            base_url: Some(url.into()),
            default_model: Some(model.into()),
            ..Self::default()
        }
    }

    /// The key to send. Read from `TYPESAFE_API_KEY` when left out.
    #[must_use]
    pub fn api_key<K: Into<ApiKey>>(mut self, key: K) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// The API root. Read from `TYPESAFE_BASE_URL` when left out, and
    /// `https://api.typesafe.ai` when neither is set. Trailing slashes are
    /// dropped.
    #[must_use]
    pub fn base_url<U: Into<String>>(mut self, url: U) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// The model a request that names none asks. Read from
    /// `TYPESAFE_DEFAULT_MODEL` when left out, and `jev-latest` when neither is
    /// set.
    #[must_use]
    pub fn default_model<M: Into<String>>(mut self, model: M) -> Self {
        self.default_model = Some(model.into());
        self
    }

    /// How long one attempt may take, including reading its body. Ten seconds
    /// when left out.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// What to retry and how long to wait. The defaults of both official SDKs
    /// when left out.
    #[must_use]
    pub fn retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = Some(retry);
        self
    }

    /// Headers every request carries. A per-call header replaces one of these,
    /// and neither replaces the headers the API reads to identify the request.
    #[must_use]
    pub fn default_headers(mut self, headers: HeaderMap) -> Self {
        self.default_headers = headers;
        self
    }

    /// The HTTP client to send through, for a caller that configures its own
    /// proxy, pool, or TLS. A client of this crate's own when left out.
    #[must_use]
    pub fn http_client(mut self, client: reqwest::Client) -> Self {
        self.http_client = Some(client);
        self
    }

    /// Resolve every setting and check it.
    pub(crate) fn resolve(self) -> Result<Resolved> {
        let api_key = match (self.local_only, self.api_key) {
            (true, Some(_)) => {
                return Err(Error::Config(
                    "a local-only client cannot carry an API key".into(),
                ));
            }
            (true, None) => None,
            (false, Some(key)) => Some(key),
            (false, None) => Some(ApiKey::new(from_env(env::API_KEY).ok_or_else(|| {
                Error::Config(format!(
                    "no API key was provided; pass `Config::api_key` or set {}",
                    env::API_KEY
                ))
            })?)),
        };
        let base_url = self
            .base_url
            .or_else(|| from_env(env::BASE_URL))
            .unwrap_or_else(|| defaults::BASE_URL.to_string());
        let base_url = base_url.trim_end_matches('/').to_string();
        let parsed = url::Url::parse(&base_url)
            .map_err(|error| Error::Config(format!("`base_url` is not a URL: {error}")))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(Error::Config(format!(
                "`base_url` uses the {} scheme, and the API speaks http or https",
                parsed.scheme()
            )));
        }
        if self.local_only {
            let loopback = match parsed.host() {
                Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
                Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
                _ => false,
            };
            if !loopback
                || !parsed.username().is_empty()
                || parsed.password().is_some()
                || parsed.query().is_some()
                || parsed.fragment().is_some()
            {
                return Err(Error::Config(
                    "local-only requires a loopback IP URL without credentials, query, or fragment"
                        .into(),
                ));
            }
            if self.http_client.is_some() {
                return Err(Error::Config(
                    "local-only cannot use a custom HTTP client".into(),
                ));
            }
            crate::transport::headers(&self.default_headers, &HeaderMap::new(), None, false)?;
        }
        let default_model = self
            .default_model
            .or_else(|| from_env(env::DEFAULT_MODEL))
            .unwrap_or_else(|| defaults::MODEL.to_string());
        if self.local_only && default_model.trim().is_empty() {
            return Err(Error::Config(
                "local-only requires an explicit nonempty model".into(),
            ));
        }
        let timeout = self.timeout.unwrap_or(defaults::TIMEOUT);
        if timeout.is_zero() {
            return Err(Error::Config(
                "`timeout` must be a positive duration".to_string(),
            ));
        }
        let retry = self.retry.unwrap_or_default();
        retry.validate()?;
        let http = match self.http_client {
            Some(client) => client,
            None => {
                let builder = reqwest::Client::builder();
                let builder = if self.local_only {
                    builder
                        .no_proxy()
                        .redirect(reqwest::redirect::Policy::none())
                } else {
                    builder
                };
                builder.build().map_err(|error| {
                    Error::Config(format!("the HTTP client failed to build: {error}"))
                })?
            }
        };
        Ok(Resolved {
            api_key,
            base_url,
            default_model,
            timeout,
            retry,
            default_headers: self.default_headers,
            http,
        })
    }
}

/// Every setting, resolved and checked.
pub(crate) struct Resolved {
    pub(crate) api_key: Option<ApiKey>,
    pub(crate) base_url: String,
    pub(crate) default_model: String,
    pub(crate) timeout: Duration,
    pub(crate) retry: RetryPolicy,
    pub(crate) default_headers: HeaderMap,
    pub(crate) http: reqwest::Client,
}

/// One environment value, trimmed. An empty or whitespace-only value reads as
/// unset, the way both official SDKs read it.
fn from_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
