//! Sends cloud computer requests and retries the ones that are safe to retry.
use super::*;
use generated::{Authority, Request, RequestBody};
use reqwest::{
    Method,
    header::{CONTENT_LENGTH, CONTENT_TYPE, HeaderMap},
};
use serde::de::DeserializeOwned;
use tokio::time::Instant;

pub(super) struct Reply {
    pub headers: HeaderMap,
    pub bytes: Vec<u8>,
}
impl Computers<'_> {
    pub(super) async fn json<T: DeserializeOwned>(
        &self,
        plan: Request,
        cancel: &CancellationToken,
    ) -> Result<T, Error> {
        let operation = plan.operation.id;
        let mutation = !matches!(plan.operation.method, "GET" | "HEAD");
        let key = plan
            .headers
            .iter()
            .find(|(name, _)| *name == "Idempotency-Key")
            .map(|(_, value)| value.clone());
        let mut reply = self.request(plan, cancel).await?;
        let decoded = serde_json::from_slice(&reply.bytes).map_err(|_| {
            if mutation {
                Error::Uncertain {
                    operation,
                    key,
                    cause: Box::new(Error::InvalidResponse),
                }
            } else {
                Error::InvalidResponse
            }
        });
        reply.bytes.fill(0);
        decoded
    }
    pub(super) async fn request(
        &self,
        plan: Request,
        cancel: &CancellationToken,
    ) -> Result<Reply, Error> {
        self.client.check_session(self.session)?;
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if plan.operation.authority != Authority::Account {
            return Err(Error::InvalidInput);
        }
        let method = Method::from_bytes(plan.operation.method.as_bytes())
            .map_err(|_| Error::InvalidInput)?;
        let mutation = method != Method::GET && method != Method::HEAD;
        let key = plan
            .headers
            .iter()
            .find(|(name, _)| *name == "Idempotency-Key")
            .map(|(_, v)| v.clone());
        if let Some(key) = &key {
            RequestKey::try_from(key.clone())?;
        }
        let retry = plan.operation.retry == "read"
            || plan.operation.retry == "same_request_key" && key.is_some();
        let mut url = self.client.deployment.api_base().clone();
        {
            let mut path = url.path_segments_mut().map_err(|_| Error::InvalidInput)?;
            path.pop_if_empty();
            for segment in &plan.path_segments {
                if segment.is_empty()
                    || matches!(segment.as_str(), "." | "..")
                    || segment.chars().any(char::is_control)
                {
                    return Err(Error::InvalidInput);
                }
                path.push(segment);
            }
        }
        if !plan.query.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(plan.query.iter().map(|(k, v)| (*k, v.as_str())));
        }
        let (bytes, content_type) = match &plan.body {
            RequestBody::Empty => (vec![], None),
            RequestBody::Json(value) => (
                serde_json::to_vec(value).map_err(|_| Error::InvalidInput)?,
                Some("application/json"),
            ),
            RequestBody::Binary(value) => (value.clone(), Some("application/octet-stream")),
        };
        if plan
            .operation
            .max_request_bytes
            .is_some_and(|limit| bytes.len() > limit)
        {
            return Err(Error::TooLarge);
        }
        let mut headers = HeaderMap::new();
        for (name, value) in &plan.headers {
            if !matches!(
                *name,
                "Idempotency-Key" | "Content-Length" | "X-Content-SHA256"
            ) {
                return Err(Error::InvalidInput);
            }
            if *name == "Content-Length" && value.parse::<usize>().ok() != Some(bytes.len()) {
                return Err(Error::InvalidInput);
            }
            let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| Error::InvalidInput)?;
            let value =
                reqwest::header::HeaderValue::from_str(value).map_err(|_| Error::InvalidInput)?;
            headers.insert(name, value);
        }
        if mutation {
            headers.insert(CONTENT_LENGTH, bytes.len().into());
        }
        if let Some(content_type) = content_type {
            headers.insert(
                CONTENT_TYPE,
                reqwest::header::HeaderValue::from_static(content_type),
            );
        }
        let deadline = Instant::now() + self.operation_timeout;
        let mut attempt = 0u8;
        let mut ambiguous_attempt = false;
        let result = loop {
            let request = self
                .client
                .http
                .request(method.clone(), url.clone())
                .headers(headers.clone())
                .bearer_auth(self.session.token())
                .body(bytes.clone());
            let effect = async {
                let response = request.send().await.map_err(|e| Error::Network {
                    timeout: e.is_timeout(),
                })?;
                let status = response.status().as_u16();
                if (300..400).contains(&status) {
                    return Err(Error::Redirect);
                }
                if status == 402 {
                    return Err(Error::PaymentRequired(
                        crate::PaymentRequired::from_headers(response.headers()),
                    ));
                }
                let reply_headers = response.headers().clone();
                let limit = if plan.operation.success.contains(&status) {
                    plan.operation.max_response_bytes
                } else {
                    1024 * 1024
                };
                let mut body = Client::read_limited(response, limit)
                    .await
                    .map_err(Error::from)?;
                if !plan.operation.success.contains(&status) {
                    let code = serde_json::from_slice::<generated::AppAccountError>(&body)
                        .ok()
                        .map(|value| value.error.code)
                        .filter(|code| {
                            code.len() <= 128
                                && !code.is_empty()
                                && !code.chars().any(char::is_control)
                        });
                    body.fill(0);
                    return Err(Error::Api {
                        status,
                        code,
                        retry_after: retry_after(&reply_headers),
                    });
                }
                Ok(Reply {
                    headers: reply_headers,
                    bytes: body,
                })
            };
            let outcome = tokio::select! {
                biased;
                _=cancel.cancelled()=>Err(Error::Cancelled),
                _=tokio::time::sleep_until(deadline)=>Err(Error::Network{timeout:true}),
                value=effect=>value,
            };
            let error = match outcome {
                Ok(value) => break Ok(value),
                Err(error) => error,
            };
            if mutation && !error.definite_refusal() {
                ambiguous_attempt = true;
            }
            if !retry || !error.retryable() || attempt >= self.retries.max_retries {
                break Err(error);
            }
            let mut sample = [128];
            let _ = getrandom::fill(&mut sample);
            let multiplier = 0.5 + f64::from(sample[0]) / 255.0;
            let delay = error.retry_after().unwrap_or_else(|| {
                self.retries
                    .initial_delay
                    .saturating_mul(1u32 << attempt)
                    .mul_f64(multiplier)
                    .min(self.retries.max_delay)
            });
            if delay > self.retries.max_delay || Instant::now() + delay >= deadline {
                break Err(error);
            }
            tokio::select! {biased;_=cancel.cancelled()=>break Err(Error::Cancelled),_=tokio::time::sleep(delay)=>{}}
            attempt += 1;
        };
        result.map_err(|error| {
            let refused = error.definite_refusal();
            if mutation && (ambiguous_attempt || !refused) {
                Error::Uncertain {
                    operation: plan.operation.id,
                    key,
                    cause: Box::new(error),
                }
            } else {
                error
            }
        })
    }
}

fn retry_after(headers: &HeaderMap) -> Option<Duration> {
    let value = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?;
    value
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
        .or_else(|| {
            httpdate::parse_http_date(value)
                .ok()?
                .duration_since(std::time::SystemTime::now())
                .ok()
        })
}
