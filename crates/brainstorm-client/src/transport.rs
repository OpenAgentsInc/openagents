use crate::{Algorithm, Cancellation, Error, ResponseEvidence, digest, now_ms};
use reqwest::{Method, header};
use std::{
    collections::HashMap,
    future::Future,
    sync::Mutex,
    task::Poll,
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::watch;
use url::Url;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Gate {
    pub enabled: bool,
    pub epoch: u64,
}

pub(crate) struct Transport {
    http: reqwest::Client,
    pub origin: Url,
    gate: watch::Sender<Gate>,
    backoff: Mutex<HashMap<&'static str, (Instant, Duration, Error)>>,
    bytes: usize,
    ttl: u64,
}

pub(crate) struct Response {
    pub bytes: Vec<u8>,
    pub evidence: ResponseEvidence,
    pub error: Option<Error>,
}

impl Transport {
    pub fn new(
        origin: Url,
        gate: watch::Sender<Gate>,
        bytes: usize,
        ttl: u64,
    ) -> Result<Self, Error> {
        Ok(Self {
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .build()
                .map_err(|_| Error::Transport)?,
            origin,
            gate,
            backoff: Mutex::new(HashMap::new()),
            bytes,
            ttl,
        })
    }

    pub async fn request(
        &self,
        path: &'static str,
        body: Option<Vec<u8>>,
        algorithm: Option<Algorithm>,
        epoch: u64,
        cancellation: &Cancellation,
    ) -> Result<Response, Error> {
        self.check(epoch, cancellation)?;
        if let Some((started, duration, error)) =
            self.backoff.lock().expect("backoff lock").get(path)
        {
            if started.elapsed() < *duration {
                return Err(error.clone());
            }
        }
        let url = self
            .origin
            .join(path)
            .map_err(|_| Error::InvalidConfiguration {
                field: "origin".into(),
            })?;
        // Only fixed paths reach this method. Redirects never expand the origin.
        if url.origin() != self.origin.origin() {
            return Err(Error::RedirectRefused);
        }
        let input = body.unwrap_or_default();
        let method = if input.is_empty() {
            Method::GET
        } else {
            Method::POST
        };
        let mut request = self
            .http
            .request(method, url)
            .header(header::ACCEPT, "application/json");
        if !input.is_empty() {
            request = request
                .header(header::CONTENT_TYPE, "application/json")
                .body(input.clone());
        }
        let sending = request.send();
        tokio::pin!(sending);
        // Hold the watch read guard through each poll. Disabling cannot finish
        // between this check and a poll that starts the network request.
        let mut response = std::future::poll_fn(|cx| {
            let gate = self.gate.borrow();
            let cancelled = cancellation.0.borrow();
            if !gate.enabled || gate.epoch != epoch {
                return Poll::Ready(Err(Error::Disabled));
            }
            if *cancelled {
                return Poll::Ready(Err(Error::Cancelled));
            }
            sending
                .as_mut()
                .poll(cx)
                .map(|result| result.map_err(|_| Error::Transport))
        })
        .await?;
        let status = response.status().as_u16();
        if (300..400).contains(&status) {
            return Err(Error::RedirectRefused);
        }
        if response
            .content_length()
            .is_some_and(|size| size > self.bytes as u64)
        {
            return Err(Error::ResponseTooLarge);
        }
        let retry = retry_after(response.headers(), SystemTime::now());
        let header_ttl = cache_ttl(response.headers());
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| Error::Transport)? {
            self.check(epoch, cancellation)?;
            if chunk.len() > self.bytes.saturating_sub(bytes.len()) {
                return Err(Error::ResponseTooLarge);
            }
            bytes.reserve_exact(chunk.len());
            bytes.extend_from_slice(&chunk);
        }
        let fetched = now_ms();
        let error = match status {
            200 => None,
            401 | 403 => Some(Error::AuthenticationRequired { status }),
            429 => Some(Error::RateLimited {
                retry_after_seconds: retry,
            }),
            202 => {
                let body_retry = serde_json::from_slice::<serde_json::Value>(&bytes)
                    .ok()
                    .and_then(|value| value.get("retry_after").and_then(|value| value.as_u64()));
                Some(Error::Computing {
                    retry_after_seconds: retry.or(body_retry),
                })
            }
            422 => Some(Error::PerspectiveUnavailable),
            _ => Some(Error::Service { status }),
        };
        if let Some(error) = &error {
            let seconds = match &error {
                Error::RateLimited {
                    retry_after_seconds,
                }
                | Error::Computing {
                    retry_after_seconds,
                } => *retry_after_seconds,
                _ => retry,
            };
            if let Some(seconds) = seconds {
                self.backoff.lock().expect("backoff lock").insert(
                    path,
                    (Instant::now(), Duration::from_secs(seconds), error.clone()),
                );
            }
        }
        let service_ttl = if algorithm.is_some() {
            serde_json::from_slice::<serde_json::Value>(&bytes)
                .ok()
                .and_then(|value| value.get("ttl").and_then(|value| value.as_u64()))
                .unwrap_or(0)
        } else {
            // Discovery freshness is a host policy when HTTP supplies no TTL.
            header_ttl.unwrap_or(self.ttl)
        };
        let ttl = if error.is_some() {
            0
        } else {
            service_ttl
                .min(header_ttl.unwrap_or(u64::MAX))
                .min(self.ttl)
        };
        Ok(Response {
            evidence: ResponseEvidence {
                origin: self.origin.origin().ascii_serialization(),
                endpoint: path.into(),
                status,
                requested_algorithm: algorithm,
                fetched_at_ms: fetched,
                expires_at_ms: fetched.saturating_add(ttl.saturating_mul(1000)),
                input_digest: digest(&input),
                output_digest: digest(&bytes),
            },
            bytes,
            error,
        })
    }

    fn check(&self, epoch: u64, cancellation: &Cancellation) -> Result<(), Error> {
        let gate = self.gate.borrow();
        if !gate.enabled || gate.epoch != epoch {
            return Err(Error::Disabled);
        }
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        Ok(())
    }
}

fn retry_after(headers: &header::HeaderMap, now: SystemTime) -> Option<u64> {
    let value = headers.get(header::RETRY_AFTER)?.to_str().ok()?.trim();
    if let Ok(seconds) = value.parse() {
        return Some(seconds);
    }
    let date = httpdate::parse_http_date(value).ok()?;
    Some(
        date.duration_since(now)
            .unwrap_or_default()
            .as_secs()
            .saturating_add(u64::from(
                date.duration_since(now).unwrap_or_default().subsec_nanos() != 0,
            )),
    )
}

fn cache_ttl(headers: &header::HeaderMap) -> Option<u64> {
    let mut ttl = None;
    for value in headers.get_all(header::CACHE_CONTROL) {
        for directive in value.to_str().ok()?.split(',').map(str::trim) {
            if directive.eq_ignore_ascii_case("no-store")
                || directive.eq_ignore_ascii_case("no-cache")
            {
                return Some(0);
            }
            if let Some((name, value)) = directive.split_once('=') {
                if name.trim().eq_ignore_ascii_case("max-age") {
                    let seconds: u64 = value.trim().trim_matches('"').parse().unwrap_or(0);
                    ttl = Some(ttl.unwrap_or(u64::MAX).min(seconds));
                }
            }
        }
    }
    ttl
}
