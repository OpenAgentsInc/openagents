//! NIP-32 reputation mapping helpers for NIP-AC outcomes.

use super::{default_notice::CreditDefaultNotice, settlement::CreditSettlementReceipt};
use crate::nip32::{Label, LabelEvent, LabelTarget};

/// Namespace used for AC reputation labels.
pub const CREDIT_REPUTATION_NAMESPACE: &str = "agent/credit";

/// Build a NIP-32 success label from a settlement receipt.
pub fn settlement_to_reputation_label(
    settlement_event_id: &str,
    receipt: &CreditSettlementReceipt,
) -> LabelEvent {
    LabelEvent::new(
        vec![Label::new("success", CREDIT_REPUTATION_NAMESPACE)],
        vec![
            LabelTarget::pubkey(receipt.agent_pubkey.clone(), None::<String>),
            LabelTarget::event(settlement_event_id.to_string(), None::<String>),
        ],
    )
}

/// Build a NIP-32 default label from a default notice.
pub fn default_to_reputation_label(
    default_event_id: &str,
    notice: &CreditDefaultNotice,
) -> LabelEvent {
    LabelEvent::new(
        vec![Label::new("default", CREDIT_REPUTATION_NAMESPACE)],
        vec![
            LabelTarget::pubkey(notice.agent_pubkey.clone(), None::<String>),
            LabelTarget::event(default_event_id.to_string(), None::<String>),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nip_ac::{CreditDefaultContent, ScopeReference, ScopeType};

    #[test]
    fn test_settlement_to_reputation_label() {
        let receipt = CreditSettlementReceipt::new(
            "envelope-1",
            "agent-pubkey",
            "issuer-pubkey",
            ScopeReference::new(ScopeType::Nip90, "job-hash"),
            crate::nip_ac::CreditSettlementContent::new(31_200, 600, "success"),
        );
        let label_event = settlement_to_reputation_label("settlement-event-id", &receipt);
        let tags = label_event.to_tags();

        assert!(tags.iter().any(|tag| tag[0] == "l" && tag[1] == "success"));
        assert!(
            tags.iter()
                .any(|tag| tag[0] == "p" && tag[1] == "agent-pubkey")
        );
        assert!(
            tags.iter()
                .any(|tag| tag[0] == "e" && tag[1] == "settlement-event-id")
        );
    }

    #[test]
    fn test_default_to_reputation_label() {
        let notice = CreditDefaultNotice::new(
            "envelope-1",
            "agent-pubkey",
            ScopeReference::new(ScopeType::Nip90, "job-hash"),
            CreditDefaultContent::new("verification failed", 30_000),
        );
        let label_event = default_to_reputation_label("default-event-id", &notice);
        let tags = label_event.to_tags();

        assert!(tags.iter().any(|tag| tag[0] == "l" && tag[1] == "default"));
        assert!(
            tags.iter()
                .any(|tag| tag[0] == "p" && tag[1] == "agent-pubkey")
        );
        assert!(
            tags.iter()
                .any(|tag| tag[0] == "e" && tag[1] == "default-event-id")
        );
    }
}
