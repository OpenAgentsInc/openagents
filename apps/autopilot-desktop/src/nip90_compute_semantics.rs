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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct BuyerInvoiceAmountAnalysis {
    pub metadata_amount_msats: Option<u64>,
    pub bolt11_amount_msats: Option<u64>,
    pub effective_amount_msats: Option<u64>,
    pub amount_mismatch: bool,
}

pub(crate) fn normalize_pubkey(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn trim_lightning_prefix(value: &str) -> &str {
    let trimmed = value.trim();
    trimmed
        .strip_prefix("lightning://")
        .or_else(|| trimmed.strip_prefix("LIGHTNING://"))
        .or_else(|| trimmed.strip_prefix("lightning:"))
        .or_else(|| trimmed.strip_prefix("LIGHTNING:"))
        .unwrap_or(trimmed)
}

pub(crate) fn decode_bolt11_amount_msats(bolt11: &str) -> Option<u64> {
    let normalized = trim_lightning_prefix(bolt11).to_ascii_lowercase();
    let separator_index = normalized.rfind('1')?;
    let hrp = &normalized[..separator_index];

    for prefix in ["lnbcrt", "lnbc", "lntb", "lntbs", "lnsb"] {
        let Some(amount_part) = hrp.strip_prefix(prefix) else {
            continue;
        };
        if amount_part.is_empty() {
            return None;
        }

        let (digits, unit) = match amount_part.chars().last() {
            Some(last) if last.is_ascii_alphabetic() => {
                let digits = &amount_part[..amount_part.len().saturating_sub(last.len_utf8())];
                (digits, Some(last))
            }
            _ => (amount_part, None),
        };
        if digits.is_empty() || !digits.chars().all(|ch| ch.is_ascii_digit()) {
            return None;
        }
        let amount = digits.parse::<u64>().ok()?;
        let amount_msats = match unit {
            None => amount.checked_mul(100_000_000_000),
            Some('m') => amount.checked_mul(100_000_000),
            Some('u') => amount.checked_mul(100_000),
            Some('n') => amount.checked_mul(100),
            Some('p') => Some(amount.saturating_add(9) / 10),
            Some(_) => None,
        }?;
        return Some(amount_msats);
    }

    None
}

pub(crate) fn analyze_invoice_amount_msats(
    metadata_amount_msats: Option<u64>,
    bolt11: Option<&str>,
) -> BuyerInvoiceAmountAnalysis {
    let metadata_amount_msats = metadata_amount_msats.filter(|amount| *amount > 0);
    let bolt11_amount_msats = bolt11.and_then(decode_bolt11_amount_msats);
    let effective_amount_msats = match (metadata_amount_msats, bolt11_amount_msats) {
        (Some(metadata), Some(decoded)) => Some(metadata.max(decoded)),
        (Some(metadata), None) => Some(metadata),
        (None, Some(decoded)) => Some(decoded),
        (None, None) => None,
    };

    BuyerInvoiceAmountAnalysis {
        metadata_amount_msats,
        bolt11_amount_msats,
        effective_amount_msats,
        amount_mismatch: metadata_amount_msats
            .zip(bolt11_amount_msats)
            .is_some_and(|(metadata, decoded)| metadata != decoded),
    }
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

pub(crate) fn provider_has_budget_approved_payable_result(
    observation: &BuyerProviderObservation<'_>,
    budget_sats: u64,
) -> bool {
    provider_has_payable_result(observation)
        && analyze_invoice_amount_msats(
            observation.last_feedback_amount_msats,
            observation.last_feedback_bolt11,
        )
        .effective_amount_msats
        .is_none_or(|amount_msats| amount_msats <= budget_sats.saturating_mul(1_000))
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

pub(crate) fn select_budget_approved_payable_winner(
    current_winner_pubkey: Option<&str>,
    preferred_provider_pubkey: Option<&str>,
    observations: &[BuyerProviderObservation<'_>],
    budget_sats: u64,
) -> Option<BuyerWinnerSelection> {
    let preferred_provider = preferred_provider_pubkey.map(normalize_pubkey);
    let current_winner = current_winner_pubkey.map(normalize_pubkey);

    let current_candidate = current_winner.as_deref().and_then(|winner| {
        observations.iter().find(|observation| {
            normalize_pubkey(observation.provider_pubkey) == *winner
                && provider_has_budget_approved_payable_result(observation, budget_sats)
        })
    });
    let preferred_candidate = preferred_provider.as_deref().and_then(|preferred| {
        observations.iter().find(|observation| {
            normalize_pubkey(observation.provider_pubkey) == *preferred
                && provider_has_budget_approved_payable_result(observation, budget_sats)
        })
    });
    let first_candidate = observations
        .iter()
        .find(|observation| provider_has_budget_approved_payable_result(observation, budget_sats));

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
        BuyerProviderObservation, analyze_invoice_amount_msats, decode_bolt11_amount_msats,
        normalize_pubkey, provider_has_budget_approved_payable_result, provider_has_payable_result,
        select_budget_approved_payable_winner, select_payable_winner,
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

    #[test]
    fn decode_bolt11_amount_msats_supports_common_units() {
        assert_eq!(decode_bolt11_amount_msats("lnbc250n1example"), Some(25_000));
        assert_eq!(
            decode_bolt11_amount_msats("lnbc20u1example"),
            Some(2_000_000)
        );
        assert_eq!(
            decode_bolt11_amount_msats("lightning:lnbc1zeroamount"),
            None
        );
    }

    #[test]
    fn invoice_amount_analysis_prefers_larger_of_metadata_and_bolt11() {
        let analysis = analyze_invoice_amount_msats(Some(2_000), Some("lnbc250n1example"));
        assert_eq!(analysis.metadata_amount_msats, Some(2_000));
        assert_eq!(analysis.bolt11_amount_msats, Some(25_000));
        assert_eq!(analysis.effective_amount_msats, Some(25_000));
        assert!(analysis.amount_mismatch);
    }

    #[test]
    fn budget_approved_payable_winner_rejects_over_budget_invoice() {
        let observations = [
            BuyerProviderObservation {
                provider_pubkey: "aa",
                last_feedback_event_id: Some("feedback-aa"),
                last_feedback_status: Some("payment-required"),
                last_feedback_amount_msats: Some(1_000),
                last_feedback_bolt11: Some("lnbc250n1overbudget"),
                last_result_event_id: Some("result-aa"),
                last_result_status: Some("success"),
            },
            BuyerProviderObservation {
                provider_pubkey: "bb",
                last_feedback_event_id: Some("feedback-bb"),
                last_feedback_status: Some("payment-required"),
                last_feedback_amount_msats: None,
                last_feedback_bolt11: Some("lnbc20n1withinbudget"),
                last_result_event_id: Some("result-bb"),
                last_result_status: Some("success"),
            },
        ];

        assert!(!provider_has_budget_approved_payable_result(
            &observations[0],
            2
        ));
        assert!(provider_has_budget_approved_payable_result(
            &observations[1],
            2
        ));

        let selection = select_budget_approved_payable_winner(None, Some("aa"), &observations, 2)
            .expect("under-budget provider should still win");
        assert_eq!(selection.provider_pubkey, "bb");
        assert_eq!(selection.selection_source, "first_payable_provider");
    }
}
