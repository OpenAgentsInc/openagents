//! The disclosure check (router rule 2): a hook or tool that sends part of
//! the session out declares what it sends and to whom, and is registered
//! only when the session's admitted disclosure covers that. A refusal is
//! recorded, never silent.

use serde::{Deserialize, Serialize};

use super::registry::Disclosure;

/// What a session's admission lets leave it, and to whom.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Admission {
    pub disclosures: Vec<Disclosure>,
}

/// A hook or tool left out because the admission doesn't cover what it
/// sends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisclosureRefused {
    /// The hook or tool.
    pub id: String,
    pub disclosure: Disclosure,
    /// Why, in plain words.
    pub message: String,
}

impl Admission {
    /// An admission that lets nothing leave.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Lets `sends` go to `to`.
    #[must_use]
    pub fn admitting(mut self, disclosure: Disclosure) -> Self {
        self.disclosures.push(disclosure);
        self
    }

    /// Whether `disclosure` is admitted: the same thing to the same
    /// recipient. Nothing is widened by a near match.
    #[must_use]
    pub fn covers(&self, disclosure: &Disclosure) -> bool {
        self.disclosures
            .iter()
            .any(|admitted| admitted == disclosure)
    }

    /// The disclosure check for `id`: none to send passes; otherwise the
    /// admission must cover it.
    ///
    /// # Errors
    ///
    /// What was refused, to record.
    pub fn check(
        &self,
        id: &str,
        disclosure: Option<&Disclosure>,
    ) -> Result<(), DisclosureRefused> {
        match disclosure {
            None => Ok(()),
            Some(disclosure) if self.covers(disclosure) => Ok(()),
            Some(disclosure) => Err(DisclosureRefused {
                id: id.to_string(),
                disclosure: disclosure.clone(),
                message: format!(
                    "{id} would send {} to {}, and this session doesn't allow that, so it is off.",
                    disclosure.sends, disclosure.to
                ),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_model() -> Disclosure {
        Disclosure {
            sends: "the command".to_string(),
            to: "a decision model".to_string(),
        }
    }

    #[test]
    fn only_an_admitted_disclosure_passes() {
        let none = Admission::none();
        assert!(none.check("quiet", None).is_ok());
        let refused = none.check("gate", Some(&to_model())).unwrap_err();
        assert_eq!(refused.id, "gate");
        assert!(refused.message.contains("the command"));
        let admitted = Admission::none().admitting(to_model());
        assert!(admitted.check("gate", Some(&to_model())).is_ok());
        let elsewhere = Disclosure {
            to: "a plugin".to_string(),
            ..to_model()
        };
        assert!(admitted.check("gate", Some(&elsewhere)).is_err());
    }
}
