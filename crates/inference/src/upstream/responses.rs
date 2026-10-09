//! Open Responses upstreams (OpenRouter, Vercel AI Gateway): the request
//! passes through with the upstream's model id and privacy fields, and the
//! upstream's events pass back.
//!
//! What this layer adds to a pass-through: the request is always a stateless
//! stream (`stream: true`, `store: false`, no `openagents` object), and
//! the events pass the first-token [`Gate`], which also names the public
//! model id on each lifecycle event.

use futures_util::StreamExt;
use serde_json::{Map, Value, json};

use super::gate::Gate;
use super::http::Frames;
use super::{AttemptError, ErrorClass, EventStream};
use crate::request::CreateResponse;
use crate::sse::{StreamItem, decode_frame};

/// The body for `request` to `upstream_model`: the request as it came,
/// stateless and streaming, without our extension object, with `privacy`
/// merged in at the top level (one level deep for objects).
///
/// # Errors
///
/// [`ErrorClass::Unsupported`] for `previous_response_id`, which needs
/// stored responses.
pub fn body(
    request: &CreateResponse,
    upstream_model: &str,
    privacy: Map<String, Value>,
) -> Result<Value, AttemptError> {
    if request.previous_response_id.is_some() {
        return Err(AttemptError::new(
            ErrorClass::Unsupported,
            "previous_response_id needs stored responses",
        ));
    }
    let mut request = request.clone();
    request.model = Some(upstream_model.to_owned());
    request.stream = Some(true);
    request.store = Some(false);
    request.openagents = None;
    request.background = None;
    let mut body = serde_json::to_value(&request).map_err(|_| {
        AttemptError::new(ErrorClass::Unsupported, "the request could not be encoded")
    })?;
    if let Some(fields) = body.as_object_mut() {
        merge(fields, privacy);
    }
    Ok(body)
}

/// Merges `add` into `fields`, one level deep for objects, so a caller's
/// own provider options survive the privacy fields.
pub fn merge(fields: &mut Map<String, Value>, add: Map<String, Value>) {
    for (key, value) in add {
        match (fields.get_mut(&key), value) {
            (Some(Value::Object(existing)), Value::Object(more)) => {
                for (inner_key, inner) in more {
                    match (existing.get_mut(&inner_key), inner) {
                        (Some(Value::Object(deep)), Value::Object(more_deep)) => {
                            deep.extend(more_deep);
                        }
                        (_, inner) => {
                            existing.insert(inner_key, inner);
                        }
                    }
                }
            }
            (_, value) => {
                fields.insert(key, value);
            }
        }
    }
}

/// An Open Responses stream from an upstream, for the caller of `model`
/// (the public id), through a [`Gate`].
#[must_use]
pub fn events(frames: Frames, model: &str, scrub: &'static [&'static str]) -> EventStream {
    let state = (frames, Gate::new(model, scrub));
    Box::pin(futures_util::stream::unfold(
        state,
        |(mut frames, mut gate)| async move {
            loop {
                if let Some(item) = gate.next() {
                    return Some((item, (frames, gate)));
                }
                if gate.closed() {
                    return None;
                }
                match frames.next().await {
                    None => gate.end(),
                    Some(Err(error)) => gate.fail(error),
                    Some(Ok(frame)) => match decode_frame(frame) {
                        Ok(StreamItem::Done) => gate.end(),
                        Ok(StreamItem::Event(event)) => gate.push(event),
                        Err(_) => gate.fail(AttemptError::new(
                            ErrorClass::Decode,
                            "the upstream sent an event we could not read",
                        )),
                    },
                }
            }
        },
    ))
}

/// The settings of one Open Responses upstream.
#[derive(Clone, Debug)]
pub struct ResponsesConfig {
    /// `openrouter`, `vercel`.
    pub name: &'static str,
    /// The full `.../v1/responses` URL.
    pub url: String,
    pub key: Option<super::secret::Secret>,
    /// Extra headers (name, value), such as attribution.
    pub headers: Vec<(String, String)>,
    pub account: super::Account,
    pub privacy: super::PrivacyTerms,
    /// The body fields that carry a privacy level to this upstream.
    pub privacy_fields: fn(&crate::openagents::Privacy) -> Map<String, Value>,
    pub models: Vec<super::ModelRow>,
}

/// An adapter for an Open Responses upstream.
pub struct ResponsesUpstream {
    config: ResponsesConfig,
    http: reqwest::Client,
}

impl ResponsesUpstream {
    #[must_use]
    pub fn new(config: ResponsesConfig) -> Self {
        Self {
            config,
            http: super::http::client(super::http::CONNECT_TIMEOUT),
        }
    }

    /// The settings.
    #[must_use]
    pub fn config(&self) -> &ResponsesConfig {
        &self.config
    }

    /// Adds a model row (the long tail a deployment configures).
    #[must_use]
    pub fn with_model(mut self, row: super::ModelRow) -> Self {
        self.config.models.push(row);
        self
    }
}

impl super::Upstream for ResponsesUpstream {
    fn name(&self) -> &'static str {
        self.config.name
    }

    fn account(&self) -> &super::Account {
        &self.config.account
    }

    fn privacy(&self) -> &super::PrivacyTerms {
        &self.config.privacy
    }

    fn models(&self) -> &[super::ModelRow] {
        &self.config.models
    }

    fn configured(&self) -> bool {
        self.config.key.is_some()
    }

    fn send<'a>(
        &'a self,
        request: &'a CreateResponse,
        model: &'a str,
    ) -> super::BoxFuture<'a, Result<super::Sent, AttemptError>> {
        Box::pin(async move {
            let row = super::check(self, request, model)?;
            let Some(key) = &self.config.key else {
                return Err(super::unconfigured(self.config.name, "API key"));
            };
            let privacy = (self.config.privacy_fields)(&super::privacy_level(request));
            let body = body(request, &row.upstream_model, privacy)?;
            let meter = super::AttemptMeter::start(self, row);
            let mut call = self
                .http
                .post(&self.config.url)
                .bearer_auth(key.expose())
                .header("accept", "text/event-stream")
                .json(&body);
            for (name, value) in &self.config.headers {
                call = call.header(name, value);
            }
            let frames = match super::http::open_stream(call, &[]).await {
                Ok(frames) => frames,
                Err(error) => {
                    meter.fail(&error);
                    return Err(error);
                }
            };
            meter.status(200);
            let events = events(frames, &row.id, &[]);
            Ok(super::Sent {
                events: meter.wrap(events),
                meter,
            })
        })
    }
}

/// `{"provider": {"data_collection": "deny", "zdr": true}}`-style privacy
/// fields as a map, from a JSON object literal.
#[must_use]
pub fn fields(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

/// The empty privacy addition.
#[must_use]
pub fn no_fields() -> Map<String, Value> {
    fields(json!({}))
}
