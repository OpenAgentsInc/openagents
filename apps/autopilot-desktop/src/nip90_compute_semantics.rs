#[derive(Clone, Copy, Debug)]
pub(crate) struct BuyerProviderObservation<'a> {
    pub provider_pubkey: &'a str,
    pub last_feedback_event_id: Option<&'a str>,
    pub last_feedback_status: Option<&'a str>,
    pub last_feedback_amount_msats: Option<u64>,
    pub last_feedback_bolt11: Option<&'a str>,
    pub last_result_event_id: Option<&'a str>,
    pub last_result_status: Option<&'a str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BuyerWinnerSelection {
    pub provider_pubkey: String,
    pub result_event_id: Option<String>,
    pub feedback_event_id: Option<String>,
    pub amount_msats: Option<u64>,
    pub selection_source: &'static str,
}

pub(crate) fn normalize_pubkey(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub(crate) fn provider_has_non_error_result(observation: &BuyerProviderObservation<'_>) -> bool {
    observation.last_result_event_id.is_some()
        && !matches!(
            observation
                .last_result_status
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("error")
        )
}

pub(crate) fn provider_has_valid_invoice(observation: &BuyerProviderObservation<'_>) -> bool {
    observation
        .last_feedback_bolt11
        .map(str::trim)
        .is_some_and(|bolt11| !bolt11.is_empty())
}

pub(crate) fn provider_has_payable_result(observation: &BuyerProviderObservation<'_>) -> bool {
    provider_has_non_error_result(observation) && provider_has_valid_invoice(observation)
}

pub(crate) fn select_payable_winner(
    current_winner_pubkey: Option<&str>,
    preferred_provider_pubkey: Option<&str>,
    observations: &[BuyerProviderObservation<'_>],
) -> Option<BuyerWinnerSelection> {
    let preferred_provider = preferred_provider_pubkey.map(normalize_pubkey);
    let current_winner = current_winner_pubkey.map(normalize_pubkey);

    let current_candidate = current_winner.as_deref().and_then(|winner| {
        observations.iter().find(|observation| {
            normalize_pubkey(observation.provider_pubkey) == *winner
                && provider_has_payable_result(observation)
        })
    });
    let preferred_candidate = preferred_provider.as_deref().and_then(|preferred| {
        observations.iter().find(|observation| {
            normalize_pubkey(observation.provider_pubkey) == *preferred
                && provider_has_payable_result(observation)
        })
    });
    let first_candidate = observations
        .iter()
        .find(|observation| provider_has_payable_result(observation));

    let (selection_source, observation) = if let Some(observation) = current_candidate {
        ("retained_current_winner", observation)
    } else if let Some(observation) = preferred_candidate {
        ("preferred_provider_became_payable", observation)
    } else if let Some(observation) = first_candidate {
        ("first_payable_provider", observation)
    } else {
        return None;
    };

    Some(BuyerWinnerSelection {
        provider_pubkey: observation.provider_pubkey.to_string(),
        result_event_id: observation.last_result_event_id.map(ToString::to_string),
        feedback_event_id: observation.last_feedback_event_id.map(ToString::to_string),
        amount_msats: observation.last_feedback_amount_msats,
        selection_source,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        BuyerProviderObservation, normalize_pubkey, provider_has_payable_result,
        select_payable_winner,
    };

    #[test]
    fn payable_winner_requires_matching_result_and_invoice() {
        let observations = [BuyerProviderObservation {
            provider_pubkey: "aa",
            last_feedback_event_id: Some("feedback-aa"),
            last_feedback_status: Some("payment-required"),
            last_feedback_amount_msats: Some(2_000),
            last_feedback_bolt11: Some("lnbc20n1winner"),
            last_result_event_id: Some("result-aa"),
            last_result_status: Some("success"),
        }];

        assert!(provider_has_payable_result(&observations[0]));
        let selection = select_payable_winner(None, Some("aa"), &observations)
            .expect("payable winner should be selected");
        assert_eq!(selection.provider_pubkey, "aa");
        assert_eq!(
            selection.selection_source,
            "preferred_provider_became_payable"
        );
    }

    #[test]
    fn payable_winner_retains_current_provider_when_still_payable() {
        let observations = [
            BuyerProviderObservation {
                provider_pubkey: "aa",
                last_feedback_event_id: Some("feedback-aa"),
                last_feedback_status: Some("payment-required"),
                last_feedback_amount_msats: Some(2_000),
                last_feedback_bolt11: Some("lnbc20n1aa"),
                last_result_event_id: Some("result-aa"),
                last_result_status: Some("success"),
            },
            BuyerProviderObservation {
                provider_pubkey: "bb",
                last_feedback_event_id: Some("feedback-bb"),
                last_feedback_status: Some("payment-required"),
                last_feedback_amount_msats: Some(2_000),
                last_feedback_bolt11: Some("lnbc20n1bb"),
                last_result_event_id: Some("result-bb"),
                last_result_status: Some("success"),
            },
        ];

        let selection = select_payable_winner(Some("aa"), Some("bb"), &observations)
            .expect("current winner should remain payable");
        assert_eq!(normalize_pubkey(selection.provider_pubkey.as_str()), "aa");
        assert_eq!(selection.selection_source, "retained_current_winner");
    }
}
