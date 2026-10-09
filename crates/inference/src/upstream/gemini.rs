//! Gemini Developer API on the caller's key, sharing Vertex's native codec.

use super::secret::Secret;
use super::vertex::{self, Thinking};
use super::{
    Account, AttemptError, AttemptMeter, BoxFuture, CostBasis, ModelRow, PrivacyTerms, Sent,
    Upstream, check,
};

pub struct Gemini {
    key: Secret,
    pub base_url: String,
    account: Account,
    privacy: PrivacyTerms,
    rows: Vec<ModelRow>,
    http: reqwest::Client,
}

impl Gemini {
    pub fn new(key: Secret) -> Self {
        Self {
            key,
            base_url: "https://generativelanguage.googleapis.com".into(),
            account: Account {
                id: crate::run::CALLER_KEY.into(),
                basis: CostBasis::PayAsYouGo,
            },
            privacy: PrivacyTerms::unverified(
                "Gemini: the caller's billing and data terms are not verified",
            ),
            rows: vertex::default_models()
                .into_iter()
                .map(|(row, _)| row)
                .collect(),
            http: super::http::client(super::http::CONNECT_TIMEOUT),
        }
    }
}

impl Upstream for Gemini {
    fn name(&self) -> &str {
        "google"
    }
    fn account(&self) -> &Account {
        &self.account
    }
    fn privacy(&self) -> &PrivacyTerms {
        &self.privacy
    }
    fn models(&self) -> &[ModelRow] {
        &self.rows
    }
    fn configured(&self) -> bool {
        true
    }
    fn send<'a>(
        &'a self,
        request: &'a crate::CreateResponse,
        model: &'a str,
    ) -> BoxFuture<'a, Result<Sent, AttemptError>> {
        Box::pin(async move {
            let row = check(self, request, model)?;
            let thinking = if row.upstream_model.starts_with("gemini-2.5") {
                Thinking::Budget
            } else {
                Thinking::Level
            };
            let body = vertex::body(request, row, thinking)?;
            let meter = AttemptMeter::start(self, row);
            let url = format!(
                "{}/v1beta/models/{}:streamGenerateContent?alt=sse",
                self.base_url.trim_end_matches('/'),
                row.upstream_model
            );
            let call = self
                .http
                .post(url)
                .header("x-goog-api-key", self.key.expose())
                .header("accept", "text/event-stream")
                .json(&body);
            let frames = match super::http::open_stream(call, &[self.key.expose()]).await {
                Ok(frames) => frames,
                Err(error) => {
                    meter.fail(&error);
                    return Err(error);
                }
            };
            meter.status(200);
            let events = vertex::events(frames, super::emit::Emitter::new(model, request), model);
            Ok(Sent {
                events: meter.wrap(events),
                meter,
            })
        })
    }
}
