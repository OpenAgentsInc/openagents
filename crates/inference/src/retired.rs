//! Models we have retired: never listed at `GET /v1/models`, never
//! offered to the router, and never shown in a model picker, whichever
//! adapter or upstream catalog names them.
//!
//! The gateway's catalog is the adapters' own rows ([`crate::rates::catalog`]
//! over [`crate::run::Gateway::offerings`]), so a retired model leaves the
//! catalog when its row leaves its adapter. This list is the backstop for
//! everything else that can name one: a row a deployment adds with
//! [`crate::upstream::responses::ResponsesUpstream::with_model`], an
//! upstream's own model list, or a client's cache from before the
//! retirement (Coder's `gateway-models.json`).

/// One retired model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Retired {
    /// The public model id, as the gateway listed it.
    pub id: &'static str,
    /// The day it was retired.
    pub retired_on: &'static str,
    /// Why, for whoever reads this list.
    pub why: &'static str,
}

/// Every retired model.
pub const RETIRED: &[Retired] = &[Retired {
    id: "stealth/space-bunny-alpha",
    retired_on: "2026-10-05",
    why: "an anonymous preview model OpenRouter stopped serving (HTTP 404)",
}];

/// Whether `model` (a public model id, any case, surrounding space
/// ignored) is retired.
#[must_use]
pub fn is_retired(model: &str) -> bool {
    let model = model.trim();
    RETIRED
        .iter()
        .any(|retired| retired.id.eq_ignore_ascii_case(model))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_retired_model_is_known_by_its_id_in_any_case() {
        assert!(is_retired("stealth/space-bunny-alpha"));
        assert!(is_retired(" Stealth/Space-Bunny-Alpha "));
        assert!(!is_retired("google/gemini-3.8-flash"));
        assert!(!is_retired("zai/glm-5.3-flash"));
        assert!(!is_retired(""));
    }

    #[test]
    fn every_entry_is_a_full_model_id() {
        for retired in RETIRED {
            assert!(retired.id.contains('/'), "{retired:?}");
            assert_eq!(retired.id, retired.id.trim().to_ascii_lowercase());
            assert!(!retired.why.is_empty() && retired.retired_on.len() == 10);
        }
    }
}
