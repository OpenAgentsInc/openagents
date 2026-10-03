//! Testing a key with the provider's cheapest call.
//!
//! | Provider | Test |
//! | --- | --- |
//! | OpenRouter | `GET https://openrouter.ai/api/v1/key` (free; label and credit state) |
//! | Vercel AI Gateway | `GET https://ai-gateway.vercel.sh/v1/credits` (free; the balance) |
//! | TypeSafe | one minimal `POST /v1/systemone` decision |
//!
//! [`request`] says what to send and [`read`] reads the answer, so the
//! rule is tested without a network; [`Http`] (feature `http`) sends it.

use serde_json::{Value, json};

use crate::{ApiKey, Failure, Provider};

/// One test request. The key goes in an `Authorization: Bearer` header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub method: &'static str,
    pub url: &'static str,
    pub body: Option<Value>,
}

/// The test request for `provider`.
#[must_use]
pub fn request(provider: Provider) -> Request {
    match provider {
        Provider::OpenRouter => Request {
            method: "GET",
            url: "https://openrouter.ai/api/v1/key",
            body: None,
        },
        Provider::Vercel => Request {
            method: "GET",
            url: "https://ai-gateway.vercel.sh/v1/credits",
            body: None,
        },
        Provider::TypeSafe => Request {
            method: "POST",
            url: "https://api.typesafe.ai/v1/systemone",
            body: Some(json!({
                "model": "jev-1.13.0",
                "state": "A key check.",
                "questions": {"ok": {"type": "noul", "instructions": "Is this a key check?"}}
            })),
        },
    }
}

/// One minimal Jev decision on `provider`'s own door, which answers the
/// open question whether any key there may call Jev (OpenRouter serves Jev
/// at its Decisions API as `typesafe/jev-1.13`; the gateway as
/// `typesafe-ai/jev`). `None` for TypeSafe, whose key check is already a
/// decision.
#[must_use]
pub fn jev_request(provider: Provider) -> Option<Request> {
    let (url, model) = match provider {
        Provider::OpenRouter => (jev::doors::OPENROUTER_URL, "typesafe/jev-1.13"),
        Provider::Vercel => (jev::doors::GATEWAY_URL, jev::doors::GATEWAY_MODEL),
        Provider::TypeSafe => return None,
    };
    Some(Request {
        method: "POST",
        url,
        body: Some(json!({
            "model": model,
            "state": "A key check.",
            "questions": {"ok": {"type": "noul", "instructions": "Is this a key check?"}}
        })),
    })
}

/// Whether `key` can call Jev on `provider`'s door: `Some(true)` when a
/// decision came back, `Some(false)` when the door refused it, `None` when
/// the test could not finish or does not apply.
#[must_use]
pub fn jev(sender: &dyn Send, provider: Provider, key: &ApiKey) -> Option<bool> {
    let request = jev_request(provider)?;
    let (status, body) = sender.send(&request, key);
    match status? {
        200..=299 => Some(
            serde_json::from_slice::<Value>(&body)
                .ok()
                .is_some_and(|value| value.get("answers").is_some()),
        ),
        400..=499 => Some(false),
        _ => None,
    }
}

/// What a test found.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// The key works.
    Works {
        /// A safe display label: the provider name, never key material.
        label: Option<String>,
        /// What is left on the account, in US dollars, when it says.
        remaining_usd: Option<f64>,
        /// What the key has spent, in US dollars, when it says.
        spent_usd: Option<f64>,
    },
    /// The key works but its account has no credits; calls on it will fail.
    NoCredits,
    /// The provider refused the key (401 or 403): it is not stored.
    Refused,
    /// The test could not finish (no connection, rate-limited, an outage).
    Unknown(Failure),
}

impl State {
    /// The word a settings row shows: `works`, `no credits`, `refused`, or
    /// `unchecked`.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            State::Works { .. } => "works",
            State::NoCredits => "no credits",
            State::Refused => "refused",
            State::Unknown(_) => "unchecked",
        }
    }

    /// Whether a key in this state may be stored: every state but refused.
    #[must_use]
    pub const fn storable(&self) -> bool {
        !matches!(self, State::Refused)
    }

    /// The line a person reads after a test of `provider`'s key.
    #[must_use]
    pub fn line(&self, provider: Provider) -> String {
        match self {
            State::Works { .. } => format!("Your {} key works.", provider.name()),
            State::NoCredits => format!(
                "This key has no credits; calls on it will fail. Add credits to your {} account at {}.",
                provider.name(),
                match provider {
                    Provider::OpenRouter => "https://openrouter.ai/settings/credits",
                    Provider::Vercel => "https://vercel.com (AI Gateway > Credits)",
                    Provider::TypeSafe => "https://typesafe.ai (sign in to manage your credits)",
                }
            ),
            State::Refused => format!("{} didn't accept that key.", provider.name()),
            State::Unknown(failure) => failure.line(),
        }
    }
}

fn number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// Read a test's answer: its HTTP status and body. `None` status is no
/// connection.
#[must_use]
pub fn read(provider: Provider, status: Option<u16>, body: &[u8]) -> State {
    let Some(status) = status else {
        return State::Unknown(Failure::NoConnection(provider));
    };
    match status {
        200..=299 => {}
        401 | 403 => return State::Refused,
        402 => return State::NoCredits,
        other => {
            return State::Unknown(
                Failure::of_status(provider, other).unwrap_or(Failure::NoConnection(provider)),
            );
        }
    }
    let value: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    match provider {
        Provider::OpenRouter => {
            let data = value.get("data").unwrap_or(&value);
            let remaining = number(data.get("limit_remaining"));
            let spent = number(data.get("usage"));
            if remaining.is_some_and(|left| left <= 0.0) {
                return State::NoCredits;
            }
            State::Works {
                label: Some(provider.name().to_owned()),
                remaining_usd: remaining,
                spent_usd: spent,
            }
        }
        Provider::Vercel => {
            let balance = number(value.get("balance"));
            if balance.is_some_and(|left| left <= 0.0) {
                return State::NoCredits;
            }
            State::Works {
                label: Some(provider.name().to_owned()),
                remaining_usd: balance,
                spent_usd: number(value.get("total_used")),
            }
        }
        Provider::TypeSafe => State::Works {
            label: Some(provider.name().to_owned()),
            remaining_usd: None,
            spent_usd: number(value.pointer("/usage/cost")),
        },
    }
}

/// Something that sends one test request and returns its status and body,
/// or `None` for no connection.
pub trait Send {
    fn send(&self, request: &Request, key: &ApiKey) -> (Option<u16>, Vec<u8>);
}

/// Test `key` for `provider` through `sender`.
#[must_use]
pub fn test(sender: &dyn Send, provider: Provider, key: &ApiKey) -> State {
    let request = request(provider);
    let (status, body) = sender.send(&request, key);
    read(provider, status, &body)
}

/// The real sender, over HTTPS.
#[cfg(feature = "http")]
#[derive(Clone, Debug, Default)]
pub struct Http;

#[cfg(feature = "http")]
impl Send for Http {
    fn send(&self, request: &Request, key: &ApiKey) -> (Option<u16>, Vec<u8>) {
        let Ok(client) = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
        else {
            return (None, Vec::new());
        };
        let builder = match request.method {
            "POST" => client.post(request.url),
            _ => client.get(request.url),
        };
        let mut builder = builder.bearer_auth(key.expose());
        if let Some(body) = &request.body {
            builder = builder.json(body);
        }
        match builder.send() {
            Ok(response) => {
                let status = response.status().as_u16();
                let body = response.bytes().map(|b| b.to_vec()).unwrap_or_default();
                (Some(status), body)
            }
            Err(_) => (None, Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(Option<u16>, &'static str);

    impl Send for Fake {
        fn send(&self, _: &Request, _: &ApiKey) -> (Option<u16>, Vec<u8>) {
            (self.0, self.1.as_bytes().to_vec())
        }
    }

    #[test]
    fn jev_is_asked_at_each_providers_own_door() {
        let key = ApiKey::new("k");
        let answered = Fake(
            Some(200),
            r#"{"answers":{"ok":{"type":"noul","noul":0.9}}}"#,
        );
        assert_eq!(jev(&answered, Provider::OpenRouter, &key), Some(true));
        assert_eq!(
            jev(&Fake(Some(403), "{}"), Provider::Vercel, &key),
            Some(false)
        );
        assert_eq!(jev(&Fake(None, ""), Provider::OpenRouter, &key), None);
        assert_eq!(jev(&answered, Provider::TypeSafe, &key), None);
        assert_eq!(
            jev_request(Provider::OpenRouter).unwrap().url,
            "https://openrouter.ai/api/alpha/decisions"
        );
    }

    #[test]
    fn each_answer_reads_as_a_state() {
        let key = ApiKey::new("k");
        let works = test(
            &Fake(
                Some(200),
                r#"{"data":{"label":"mine","usage":1.5,"limit_remaining":3}}"#,
            ),
            Provider::OpenRouter,
            &key,
        );
        assert_eq!(
            works,
            State::Works {
                label: Some("OpenRouter".into()),
                remaining_usd: Some(3.0),
                spent_usd: Some(1.5)
            }
        );
        assert_eq!(
            test(&Fake(Some(401), "{}"), Provider::OpenRouter, &key),
            State::Refused
        );
        assert!(!State::Refused.storable());
        assert_eq!(
            test(
                &Fake(Some(200), r#"{"balance":"0","total_used":"9"}"#),
                Provider::Vercel,
                &key
            ),
            State::NoCredits
        );
        assert_eq!(
            test(&Fake(Some(402), "{}"), Provider::TypeSafe, &key),
            State::NoCredits
        );
        assert_eq!(
            test(&Fake(None, ""), Provider::Vercel, &key).line(Provider::Vercel),
            "Couldn't reach Vercel AI Gateway; try again."
        );
        assert_eq!(
            State::Refused.line(Provider::OpenRouter),
            "OpenRouter didn't accept that key."
        );
    }
}
