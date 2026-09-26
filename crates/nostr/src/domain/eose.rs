//! NIP-67 EOSE completeness hints.
//!
//! A relay MAY append a third element to the `EOSE` message — an array
//! of hint strings. `"finish"` says every stored event matching the
//! subscription's filters was sent and the client SHOULD NOT paginate;
//! `"more"` says the relay holds matching stored events it did not send
//! and the client SHOULD paginate with `until` at the oldest received
//! `created_at`. The array may carry several hints and unknown values
//! are ignored without error. An absent third element is the legacy
//! two-element `EOSE`: completeness is unknown, not finished.
//!
//! The hint speaks only about stored events; live delivery continues
//! under NIP-01 until `CLOSE` or `CLOSED`.

use serde_json::Value;

use super::DomainError;

/// A completeness hint the pinned table defines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EoseHint {
    /// The relay sent every stored event matching the filters.
    Finish,
    /// The relay holds more matching stored events than it sent.
    More,
    /// Authentication may reveal another authorized view of stored events.
    Auth,
}

/// A parsed `EOSE` message: the subscription it closes and the hints
/// the relay attached, with unknown hint values dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eose {
    /// The subscription identifier this `EOSE` answers.
    pub subscription_id: String,
    /// The recognized hints, in the order the relay sent them.
    pub hints: Vec<EoseHint>,
}

impl Eose {
    /// Whether authentication may reveal more stored events.
    ///
    /// A simultaneous `finish` applies only to the current view. This hint is
    /// not a grant; clients need a prior AUTH challenge and an admitted signer.
    #[must_use]
    pub fn authentication_may_reveal_more(&self) -> bool {
        self.hints.contains(&EoseHint::Auth)
    }

    /// What the relay asserts about its current authorized view of stored events: `Some(true)`
    /// when it sent them all, `Some(false)` when it says more remain,
    /// `None` when it says nothing — an absent or empty hint array, or
    /// only unknown values. Presence is definitive; absence is not.
    #[must_use]
    pub fn complete(&self) -> Option<bool> {
        if self.hints.contains(&EoseHint::Finish) {
            Some(true)
        } else if self.hints.contains(&EoseHint::More) {
            Some(false)
        } else {
            None
        }
    }

    /// Whether the client should issue another `REQ` with
    /// `until = oldest received created_at` — everything except a
    /// definitive `finish`.
    #[must_use]
    pub fn should_paginate(&self) -> bool {
        self.complete() != Some(true)
    }
}

/// Parse one `["EOSE", <subscription_id>, [<hint>, ...]]` message.
///
/// # Errors
///
/// Returns `DomainError::InvalidEvent` when the message is not a
/// two-or-more-element array, is not an `EOSE`, has a non-string
/// subscription id, or has a non-array or non-string hint element.
pub fn open_eose(message: &str) -> Result<Eose, DomainError> {
    let value: Value =
        serde_json::from_str(message).map_err(|_| invalid("an EOSE is a JSON array"))?;
    let array = value
        .as_array()
        .ok_or_else(|| invalid("an EOSE is a JSON array"))?;
    if array.len() < 2 {
        return Err(invalid("an EOSE needs a subscription id"));
    }
    if array[0].as_str() != Some("EOSE") {
        return Err(invalid("the message is not an EOSE"));
    }
    let subscription_id = array[1]
        .as_str()
        .ok_or_else(|| invalid("an EOSE subscription id is a string"))?
        .to_string();
    let hints = match array.get(2) {
        None => Vec::new(),
        Some(value) => {
            let list = value
                .as_array()
                .ok_or_else(|| invalid("EOSE hints are an array"))?;
            let mut hints = Vec::with_capacity(list.len());
            for hint in list {
                match hint.as_str() {
                    Some("finish") => hints.push(EoseHint::Finish),
                    Some("more") => hints.push(EoseHint::More),
                    Some("auth") => hints.push(EoseHint::Auth),
                    Some(_) => {}
                    None => return Err(invalid("an EOSE hint is a string")),
                }
            }
            hints
        }
    };
    Ok(Eose {
        subscription_id,
        hints,
    })
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finish_and_more_answer_completeness() {
        let finish = open_eose(r#"["EOSE", "sub1", ["finish"]]"#).unwrap();
        assert_eq!(finish.subscription_id, "sub1");
        assert_eq!(finish.complete(), Some(true));
        assert!(!finish.should_paginate());

        let more = open_eose(r#"["EOSE", "sub2", ["more"]]"#).unwrap();
        assert_eq!(more.complete(), Some(false));
        assert!(more.should_paginate());

        // Unknown hints are ignored without error; presence of a
        // recognized hint beside one still answers.
        let mixed = open_eose(r#"["EOSE", "s", ["streaming", "finish"]]"#).unwrap();
        assert_eq!(mixed.hints, vec![EoseHint::Finish]);
        assert_eq!(mixed.complete(), Some(true));
        let unknown = open_eose(r#"["EOSE", "s", ["chunked"]]"#).unwrap();
        assert_eq!(unknown.complete(), None);
        assert!(unknown.should_paginate());
    }

    #[test]
    fn authentication_is_separate_from_current_view_completeness() {
        for (hints, complete) in [
            (r#"["auth"]"#, None),
            (r#"["auth", "finish"]"#, Some(true)),
            (r#"["more", "auth", "future"]"#, Some(false)),
        ] {
            let eose = open_eose(&format!(r#"["EOSE", "s", {hints}, {{"future":true}}]"#)).unwrap();
            assert!(eose.authentication_may_reveal_more());
            assert_eq!(eose.complete(), complete);
        }
        assert!(
            !open_eose(r#"["EOSE", "s"]"#)
                .unwrap()
                .authentication_may_reveal_more()
        );
    }

    #[test]
    fn a_legacy_eose_says_nothing() {
        let legacy = open_eose(r#"["EOSE", "sub3"]"#).unwrap();
        assert!(legacy.hints.is_empty());
        assert_eq!(legacy.complete(), None);
        assert!(legacy.should_paginate());

        let empty = open_eose(r#"["EOSE", "sub3", []]"#).unwrap();
        assert_eq!(empty.complete(), None);
    }

    #[test]
    fn malformed_eoses_are_refused() {
        assert!(open_eose("not json").is_err());
        assert!(open_eose(r#"["EVENT", "s", {}]"#).is_err());
        assert!(open_eose(r#"["EOSE"]"#).is_err());
        assert!(open_eose(r#"["EOSE", "s", "finish", "extra"]"#).is_err());
        assert!(open_eose(r#"["EOSE", 7]"#).is_err());
        assert!(open_eose(r#"["EOSE", "s", "finish"]"#).is_err());
        assert!(open_eose(r#"["EOSE", "s", [7]]"#).is_err());
    }
}
