//! A request for one value a view cannot collect itself.
//!
//! The application publishes an [`InputRequest`] beside its view. The
//! platform adapter shows one native field, or a scanner, and answers with
//! the request's token and the value. The application checks the answer with
//! [`InputRequest::accept`] and then validates the value's meaning itself.
//!
//! A request marked `secret` asks the adapter for a masked field: the value
//! is hidden while it is typed, and the adapter never echoes, logs, persists,
//! autofills, or suggests it. The adapter passes it only to the application.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The largest value any input request may accept, in bytes.
pub const MAX_INPUT_VALUE_BYTES: usize = 64 * 1024;

/// One value the application asks the adapter to collect. `P` is the
/// application's closed purpose type, which says what the value is for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputRequest<P> {
    /// Names this request. A new request gets a new token.
    pub token: String,
    pub purpose: P,
    /// The field's accessible label.
    pub label: String,
    /// One sentence that says what to enter.
    pub prompt: String,
    /// Open the scanner first.
    pub scan: bool,
    /// The value is a secret: mask it while typing and never echo, log,
    /// persist, autofill, or suggest it.
    pub secret: bool,
    /// The largest value the application accepts, in UTF-8 bytes.
    pub max_bytes: usize,
}

/// Why an input request or an answer to it is not acceptable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputError {
    /// The token is not a bounded identifier.
    Identity,
    /// The label is empty.
    MissingLabel,
    /// The label or prompt is over the text bound.
    TextLimit,
    /// `max_bytes` is zero or over [`MAX_INPUT_VALUE_BYTES`].
    ValueLimit,
    /// The answer names a request that is not current.
    Stale,
    /// The answer is longer than the request's `max_bytes`.
    TooLong,
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Identity => "input request token must be a bounded identifier",
            Self::MissingLabel => "input request requires a nonempty label",
            Self::TextLimit => "input request text exceeds its byte bound",
            Self::ValueLimit => "input request value bound must be 1 to 64 KiB",
            Self::Stale => "input answer does not name the current request",
            Self::TooLong => "input answer is longer than the request accepts",
        })
    }
}

impl std::error::Error for InputError {}

impl<P> InputRequest<P> {
    /// Check the request's structure. It does not check the purpose, which
    /// belongs to the application.
    pub fn validate(&self) -> Result<(), InputError> {
        if !crate::valid_id(&self.token) {
            return Err(InputError::Identity);
        }
        if self.label.trim().is_empty() {
            return Err(InputError::MissingLabel);
        }
        if self.label.len() > crate::view::MAX_TEXT_BYTES
            || self.prompt.len() > crate::view::MAX_TEXT_BYTES
        {
            return Err(InputError::TextLimit);
        }
        if self.max_bytes == 0 || self.max_bytes > MAX_INPUT_VALUE_BYTES {
            return Err(InputError::ValueLimit);
        }
        Ok(())
    }

    /// Check that an answer names this request and fits its bound. The value
    /// is returned unchanged; its meaning is the application's to validate.
    /// Errors never contain the value, so a secret can't leak through them.
    pub fn accept<'a>(&self, token: &str, value: &'a str) -> Result<&'a str, InputError> {
        if token != self.token {
            return Err(InputError::Stale);
        }
        if value.len() > self.max_bytes {
            return Err(InputError::TooLong);
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(secret: bool) -> InputRequest<&'static str> {
        InputRequest {
            token: "surface-input-1".into(),
            purpose: "key",
            label: "Key".into(),
            prompt: "Enter the key.".into(),
            scan: false,
            secret,
            max_bytes: 128,
        }
    }

    #[test]
    fn a_secret_request_serializes_its_flag() {
        let secret = request(true);
        assert_eq!(secret.validate(), Ok(()));
        let json = serde_json::to_value(&secret).unwrap();
        assert_eq!(json["secret"], true);
        assert_eq!(json["token"], "surface-input-1");
        let back: InputRequest<String> = serde_json::from_value(json).unwrap();
        assert!(back.secret);
        assert!(
            !serde_json::to_value(request(false)).unwrap()["secret"]
                .as_bool()
                .unwrap()
        );
    }

    #[test]
    fn the_flag_is_required_and_unknown_fields_are_refused() {
        let mut json = serde_json::to_value(request(true)).unwrap();
        json.as_object_mut().unwrap().remove("secret");
        assert!(serde_json::from_value::<InputRequest<String>>(json.clone()).is_err());
        json["secret"] = true.into();
        json["echo"] = true.into();
        assert!(serde_json::from_value::<InputRequest<String>>(json).is_err());
    }

    #[test]
    fn structure_is_bounded() {
        let mut bad = request(true);
        bad.token = "has space".into();
        assert_eq!(bad.validate(), Err(InputError::Identity));
        let mut bad = request(true);
        bad.label = "  ".into();
        assert_eq!(bad.validate(), Err(InputError::MissingLabel));
        let mut bad = request(true);
        bad.prompt = "x".repeat(crate::view::MAX_TEXT_BYTES + 1);
        assert_eq!(bad.validate(), Err(InputError::TextLimit));
        for max_bytes in [0, MAX_INPUT_VALUE_BYTES + 1] {
            let mut bad = request(true);
            bad.max_bytes = max_bytes;
            assert_eq!(bad.validate(), Err(InputError::ValueLimit));
        }
    }

    #[test]
    fn answers_name_the_request_and_fit_its_bound() {
        let secret = request(true);
        assert_eq!(secret.accept("surface-input-1", " ab "), Ok(" ab "));
        assert_eq!(
            secret.accept("surface-input-0", "ab"),
            Err(InputError::Stale)
        );
        let long = "k".repeat(129);
        let refused = secret.accept("surface-input-1", &long).unwrap_err();
        assert_eq!(refused, InputError::TooLong);
        // Neither the error nor its message carries the value.
        assert!(!format!("{refused} {refused:?}").contains(&long));
    }
}
