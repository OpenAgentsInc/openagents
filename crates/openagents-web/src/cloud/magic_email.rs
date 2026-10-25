//! User-requested login codes, delivered by us with WorkOS automatic mail off.
//! Key-to-WorkOS sign-ins never use this sender. Neither API errors nor logs
//! contain a code, an email body, or a mail credential.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::http::HeaderMap;
use oa_auth::workos::Workos;
use oa_auth::workos::client::Refusal;
use oa_auth::workos::sdk::SecretString;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::private::ProtectedFile;

const WINDOW: Duration = Duration::from_secs(600);
const MAX_BUCKETS: usize = 16_384;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Settings {
    /// Defaults to OPENAGENTS_RESEND_API_KEY (Secret Manager
    /// openagents-resend-api-key). Alternatively use a private mounted file.
    api_key_env: Option<String>,
    api_key_file: Option<PathBuf>,
    #[cfg(test)]
    endpoint: Option<String>,
}

pub(crate) struct Sender {
    key: Option<SecretString>,
    file: Option<ProtectedFile>,
    client: reqwest::Client,
    endpoint: String,
    cloud_run: bool,
    limits: Mutex<Limits>,
}

impl Sender {
    pub(crate) fn load(settings: Settings) -> Result<Self, String> {
        if settings.api_key_file.is_some() && settings.api_key_env.is_some() {
            return Err("magic_auth_email needs one mail key source.".into());
        }
        let path = settings.api_key_file.or_else(|| {
            (settings.api_key_env.is_none())
                .then(|| std::env::var_os("OPENAGENTS_RESEND_API_KEY_FILE").map(PathBuf::from))
                .flatten()
        });
        let (file, key) = if let Some(path) = path {
            let (file, mut bytes) = ProtectedFile::open(&path, 1024)?;
            let key = std::str::from_utf8(&bytes)
                .map_err(|_| "The mail key isn't text.")?
                .trim()
                .to_string();
            bytes.fill(0);
            (Some(file), Some(key))
        } else {
            let name = settings
                .api_key_env
                .as_deref()
                .unwrap_or("OPENAGENTS_RESEND_API_KEY");
            (
                None,
                std::env::var(name).ok().map(|key| key.trim().to_string()),
            )
        };
        let key = key
            .filter(|key| {
                key.starts_with("re_")
                    && key.len() <= 512
                    && !key
                        .bytes()
                        .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
            })
            .map(Into::into);
        let endpoint = "https://api.resend.com/emails".to_string();
        #[cfg(test)]
        let endpoint = settings.endpoint.unwrap_or(endpoint);
        Ok(Self {
            key,
            file,
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(5))
                .build()
                .map_err(|_| "The mail sender isn't available.")?,
            endpoint,
            cloud_run: std::env::var_os("K_SERVICE").is_some(),
            limits: Mutex::new(Limits::default()),
        })
    }

    pub(crate) fn client_ip(&self, peer: Option<IpAddr>, headers: &HeaderMap) -> IpAddr {
        client_ip(peer, headers, self.cloud_run)
    }

    pub(crate) async fn send(
        &self,
        workos: &Workos,
        email: &str,
        ip: IpAddr,
    ) -> Result<(), Refusal> {
        let email = email.trim().to_ascii_lowercase();
        let allowed =
            self.limits
                .lock()
                .map_err(|_| Refusal::Unavailable)?
                .take(&email, ip, Instant::now());
        if !allowed {
            return Err(Refusal::RateLimited(WINDOW.as_secs()));
        }
        let key = self.key.as_ref().ok_or(Refusal::Unavailable)?;
        if let Some(file) = &self.file {
            file.check().map_err(|_| Refusal::Unavailable)?;
        }
        let code = workos.send_code(&email).await?;
        let response = self.client.post(&self.endpoint)
            .bearer_auth(key.expose())
            .json(&serde_json::json!({
                "from": "OpenAgents <no-reply@openagents.com>",
                "to": [email],
                "subject": "Your OpenAgents login code",
                "text": format!("Your OpenAgents login code is {}.\n\nEnter it on the login page. If you didn't request this code, you can ignore this email.", code.expose()),
            }))
            .send().await.map_err(|_| Refusal::Unavailable)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(Refusal::Unavailable)
        }
    }
}

pub(crate) fn client_ip(peer: Option<IpAddr>, headers: &HeaderMap, cloud_run: bool) -> IpAddr {
    let peer = peer.unwrap_or(IpAddr::from([0, 0, 0, 0])).to_canonical();
    let local = match peer {
        IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unicast_link_local(),
    };
    // Cloud Run appends its observed client after any caller-supplied values.
    if cloud_run && local {
        if let Some(ip) = headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.rsplit(',').next())
            .and_then(|value| value.trim().parse::<IpAddr>().ok())
        {
            return ip.to_canonical();
        }
    }
    peer
}

#[derive(Hash, PartialEq, Eq)]
enum Bucket {
    Email([u8; 32]),
    Ip(IpAddr),
}

#[derive(Default)]
struct Limits(HashMap<Bucket, (Instant, u8)>);

impl Limits {
    /// Three requests per email and ten per IP in ten minutes, including
    /// failed deliveries. The counters are bounded and shared by this server.
    fn take(&mut self, email: &str, ip: IpAddr, now: Instant) -> bool {
        self.0
            .retain(|_, (start, _)| now.duration_since(*start) < WINDOW);
        let buckets = [
            (
                Bucket::Email(Sha256::digest(email.trim().to_ascii_lowercase()).into()),
                3,
            ),
            (Bucket::Ip(ip), 10),
        ];
        let new = buckets
            .iter()
            .filter(|(key, _)| !self.0.contains_key(key))
            .count();
        if self.0.len() + new > MAX_BUCKETS
            || buckets
                .iter()
                .any(|(key, max)| self.0.get(key).is_some_and(|(_, count)| count >= max))
        {
            return false;
        }
        for (key, _) in buckets {
            let (_, count) = self.0.entry(key).or_insert((now, 0));
            *count += 1;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_and_ip_limits_are_independent_normalized_and_expire() {
        let mut limits = Limits::default();
        let now = Instant::now();
        let ip = "192.0.2.1".parse().unwrap();
        for _ in 0..3 {
            assert!(limits.take(" Person@Example.com ", ip, now));
        }
        assert!(!limits.take("person@example.com", "192.0.2.2".parse().unwrap(), now));
        for n in 0..7 {
            assert!(limits.take(&format!("person{n}@example.com"), ip, now));
        }
        assert!(!limits.take("another@example.com", ip, now));
        assert!(limits.take("person@example.com", ip, now + WINDOW));
    }

    #[test]
    fn ip_limit_does_not_trust_caller_forwarding_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "198.51.100.9, 203.0.113.2".parse().unwrap(),
        );
        let remote = "192.0.2.1".parse().unwrap();
        let proxy = "127.0.0.1".parse().unwrap();
        assert_eq!(client_ip(Some(remote), &headers, true), remote);
        assert_eq!(client_ip(Some(proxy), &headers, false), proxy);
        assert_eq!(
            client_ip(Some(proxy), &headers, true),
            "203.0.113.2".parse::<IpAddr>().unwrap()
        );
    }
}
