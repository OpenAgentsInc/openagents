//! A result's `switched` field (`nips/openagents/NIP-CJ.md`, #11132): the
//! model provider a turn went to first did not answer it, and another
//! one did.
//!
//! The worker's chat walks a chain of providers and hands a turn that
//! fails before its first words to the next one, so a provider's error,
//! refusal, or silence never reaches the person while another can answer.
//! The person still hears that it happened: the result names the provider
//! that missed the turn, the model it was running, and why, each as an
//! exact word or a bounded id, and the client says so in one short line
//! beside the answer. The answer's own `model` names who wrote it, so a
//! client never claims a model that wasn't used.
//!
//! Everything here is a closed set. A word this version doesn't know
//! drops the whole field rather than showing a guess.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The longest model id a `switched` field carries, in bytes.
pub const MAX_MODEL_BYTES: usize = 128;

/// Who runs the door that missed the turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Provider {
    /// The OpenAgents inference gateway.
    #[serde(rename = "openagents")]
    OpenAgents,
    /// OpenRouter.
    #[serde(rename = "openrouter")]
    OpenRouter,
    /// The Vercel AI Gateway.
    #[serde(rename = "vercel")]
    Vercel,
    /// Any other door.
    #[serde(rename = "other")]
    Other,
}

impl Provider {
    /// The word the wire carries.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Provider::OpenAgents => "openagents",
            Provider::OpenRouter => "openrouter",
            Provider::Vercel => "vercel",
            Provider::Other => "other",
        }
    }

    /// The provider `word` names, or `None` for a word this version
    /// doesn't know.
    #[must_use]
    pub fn read(word: &str) -> Option<Self> {
        Some(match word {
            "openagents" => Provider::OpenAgents,
            "openrouter" => Provider::OpenRouter,
            "vercel" => Provider::Vercel,
            "other" => Provider::Other,
            _ => return None,
        })
    }

    /// The provider as a person reads it, at the start of a sentence.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Provider::OpenAgents => "Our own model service",
            Provider::OpenRouter => "OpenRouter",
            Provider::Vercel => "The Vercel AI Gateway",
            Provider::Other => "The first model provider",
        }
    }
}

/// Why the door missed the turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Missed {
    /// It answered with an error, or broke before its first words.
    #[serde(rename = "error")]
    Error,
    /// It sent no answer in time.
    #[serde(rename = "timeout")]
    Timeout,
    /// It refused the request: the key, the account, or a rate limit.
    #[serde(rename = "refused")]
    Refused,
}

impl Missed {
    /// The word the wire carries.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Missed::Error => "error",
            Missed::Timeout => "timeout",
            Missed::Refused => "refused",
        }
    }

    /// The reason `word` names, or `None` for a word this version doesn't
    /// know.
    #[must_use]
    pub fn read(word: &str) -> Option<Self> {
        Some(match word {
            "error" => Missed::Error,
            "timeout" => Missed::Timeout,
            "refused" => Missed::Refused,
            _ => return None,
        })
    }

    /// What happened, as a person reads it after the provider's name.
    #[must_use]
    pub const fn phrase(self) -> &'static str {
        match self {
            Missed::Error => "had a problem",
            Missed::Timeout => "didn't answer in time",
            Missed::Refused => "turned the request down",
        }
    }
}

/// A turn the first provider missed and another answered.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Switched {
    /// Who runs the door that missed the turn.
    pub provider: Provider,
    /// The model that door was running.
    pub model: String,
    /// Why it missed the turn.
    pub why: Missed,
    /// The model that answered instead: the result's own `model`, kept
    /// here so the line can name it wherever the field is kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answered: Option<String>,
}

/// Whether `model` is an id a `switched` field may carry: 1 to
/// [`MAX_MODEL_BYTES`] printable ASCII bytes with no spaces.
#[must_use]
pub fn model_like(model: &str) -> bool {
    (1..=MAX_MODEL_BYTES).contains(&model.len()) && model.bytes().all(|b| b.is_ascii_graphic())
}

impl Switched {
    /// A result's `switched` object (`{provider, model, why}`) with the
    /// result's `model` as the one that `answered`, or `None` when the
    /// field is absent or anything in it is not exactly what this version
    /// knows. A model whose id is the prepared-answer bank's or a
    /// knowledge base's is no model, and is not named.
    #[must_use]
    pub fn parse(value: &Value, answered: Option<&str>) -> Option<Self> {
        let provider = Provider::read(value.get("provider")?.as_str()?)?;
        let why = Missed::read(value.get("why")?.as_str()?)?;
        let model = value
            .get("model")?
            .as_str()
            .filter(|model| model_like(model))?
            .to_owned();
        let answered = answered
            .filter(|model| model_like(model))
            .filter(|model| !model.starts_with("bank:") && !model.starts_with("kb:"))
            .map(str::to_owned);
        Some(Self {
            provider,
            model,
            why,
            answered,
        })
    }

    /// The object a result carries as `switched`.
    #[must_use]
    pub fn value(&self) -> Value {
        json!({
            "provider": self.provider.word(),
            "model": self.model,
            "why": self.why.word(),
        })
    }

    /// The one short line shown beside the answer, such as "OpenRouter
    /// didn't answer in time, so google/gemini-3.8-flash answered
    /// instead."
    #[must_use]
    pub fn line(&self) -> String {
        let answered = self.answered.as_deref().unwrap_or("another model");
        format!(
            "{} {}, so {answered} answered instead.",
            self.provider.name(),
            self.why.phrase()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_switched_field_reads_back_as_written() {
        let switched = Switched {
            provider: Provider::OpenRouter,
            model: "google/gemini-3.8-flash".into(),
            why: Missed::Refused,
            answered: Some("z-ai/glm-5.3-flash".into()),
        };
        let read = Switched::parse(&switched.value(), Some("z-ai/glm-5.3-flash"));
        assert_eq!(read, Some(switched.clone()));
        assert_eq!(
            switched.line(),
            "OpenRouter turned the request down, so z-ai/glm-5.3-flash answered instead."
        );
    }

    #[test]
    fn an_unknown_word_or_a_bad_model_drops_the_field() {
        let good = json!({"provider": "vercel", "model": "m", "why": "timeout"});
        assert!(Switched::parse(&good, None).is_some());
        for bad in [
            json!({"provider": "azure", "model": "m", "why": "timeout"}),
            json!({"provider": "vercel", "model": "m", "why": "slow"}),
            json!({"provider": "vercel", "model": "", "why": "timeout"}),
            json!({"provider": "vercel", "model": "a b", "why": "timeout"}),
            json!({"provider": "vercel", "model": "m".repeat(129), "why": "timeout"}),
            json!({"provider": "vercel", "why": "timeout"}),
            Value::Null,
        ] {
            assert_eq!(Switched::parse(&bad, Some("m")), None, "{bad}");
        }
    }

    #[test]
    fn the_line_never_names_a_model_that_did_not_answer() {
        let value = json!({"provider": "openrouter", "model": "m", "why": "error"});
        for answered in [
            None,
            Some("bank:chat-answers-v1"),
            Some("kb:product"),
            Some(""),
        ] {
            let switched = Switched::parse(&value, answered).unwrap();
            assert_eq!(switched.answered, None);
            assert_eq!(
                switched.line(),
                "OpenRouter had a problem, so another model answered instead."
            );
        }
    }

    #[test]
    fn the_line_is_plain_words() {
        for provider in [
            Provider::OpenAgents,
            Provider::OpenRouter,
            Provider::Vercel,
            Provider::Other,
        ] {
            assert_eq!(Provider::read(provider.word()), Some(provider));
            for why in [Missed::Error, Missed::Timeout, Missed::Refused] {
                assert_eq!(Missed::read(why.word()), Some(why));
                let line = Switched {
                    provider,
                    model: "m".into(),
                    why,
                    answered: Some("m2".into()),
                }
                .line();
                assert!(line.ends_with("so m2 answered instead."), "{line}");
                for word in ["upstream", "door", "lane", "dispatch", "fallback"] {
                    assert!(!line.to_lowercase().contains(word), "{line}");
                }
            }
        }
    }
}
