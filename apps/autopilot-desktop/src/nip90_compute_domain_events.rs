pub(crate) const TARGET: &str = "autopilot_desktop::compute_domain";

fn optional_str(value: Option<&str>) -> &str {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("none")
}

fn optional_u64(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string())
}

pub(crate) fn emit_buyer_selected_payable_provider(
    request_id: &str,
    provider_pubkey: &str,
    previous_provider_pubkey: Option<&str>,
    result_event_id: Option<&str>,
    feedback_event_id: Option<&str>,
    amount_msats: Option<u64>,
    selection_source: &str,
) {
    tracing::info!(
        target: TARGET,
        domain_event = "buyer.selected_payable_provider",
        flow_role = "buyer",
        request_id,
        provider_pubkey,
        previous_provider_pubkey = optional_str(previous_provider_pubkey),
        result_event_id = optional_str(result_event_id),
        feedback_event_id = optional_str(feedback_event_id),
        amount_msats = optional_u64(amount_msats),
        selection_source,
        "buyer.selected_payable_provider"
    );
}

pub(crate) fn emit_buyer_queued_payment(
    request_id: &str,
    provider_pubkey: Option<&str>,
    feedback_event_id: Option<&str>,
    amount_sats: Option<u64>,
) {
    tracing::info!(
        target: TARGET,
        domain_event = "buyer.queued_payment",
        flow_role = "buyer",
        request_id,
        provider_pubkey = optional_str(provider_pubkey),
        feedback_event_id = optional_str(feedback_event_id),
        amount_sats = optional_u64(amount_sats),
        "buyer.queued_payment"
    );
}

pub(crate) fn emit_buyer_payment_settled(
    request_id: &str,
    provider_pubkey: Option<&str>,
    payment_pointer: &str,
    amount_sats: u64,
    fees_sats: u64,
    total_debit_sats: u64,
) {
    tracing::info!(
        target: TARGET,
        domain_event = "buyer.payment_settled",
        flow_role = "buyer",
        request_id,
        provider_pubkey = optional_str(provider_pubkey),
        payment_pointer,
        amount_sats,
        fees_sats,
        total_debit_sats,
        "buyer.payment_settled"
    );
}

pub(crate) fn emit_provider_result_signed(request_id: &str, event_id: &str, attempt: u32) {
    tracing::info!(
        target: TARGET,
        domain_event = "provider.result_signed",
        flow_role = "provider",
        request_id,
        event_id,
        attempt,
        "provider.result_signed"
    );
}

pub(crate) fn emit_provider_result_published(
    request_id: &str,
    event_id: &str,
    accepted_relays: usize,
    rejected_relays: usize,
) {
    tracing::info!(
        target: TARGET,
        domain_event = "provider.result_published",
        flow_role = "provider",
        request_id,
        event_id,
        accepted_relays,
        rejected_relays,
        "provider.result_published"
    );
}

pub(crate) fn emit_provider_payment_requested(
    request_id: &str,
    feedback_event_id: &str,
    amount_sats: u64,
) {
    tracing::info!(
        target: TARGET,
        domain_event = "provider.payment_requested",
        flow_role = "provider",
        request_id,
        feedback_event_id,
        amount_sats,
        "provider.payment_requested"
    );
}

pub(crate) fn emit_provider_settlement_confirmed(
    request_id: &str,
    payment_id: Option<&str>,
    success_feedback_id: Option<&str>,
    amount_sats: Option<u64>,
    fees_sats: Option<u64>,
) {
    tracing::info!(
        target: TARGET,
        domain_event = "provider.settlement_confirmed",
        flow_role = "provider",
        request_id,
        payment_id = optional_str(payment_id),
        success_feedback_id = optional_str(success_feedback_id),
        amount_sats = optional_u64(amount_sats),
        fees_sats = optional_u64(fees_sats),
        "provider.settlement_confirmed"
    );
}

pub(crate) fn emit_provider_loser_feedback_ignored(
    request_id: &str,
    provider_pubkey: &str,
    winner_provider_pubkey: Option<&str>,
    status: Option<&str>,
    status_extra: Option<&str>,
    ignore_reason: &str,
) {
    tracing::info!(
        target: TARGET,
        domain_event = "provider.loser_feedback_ignored",
        flow_role = "provider",
        request_id,
        provider_pubkey,
        winner_provider_pubkey = optional_str(winner_provider_pubkey),
        status = optional_str(status),
        status_extra = optional_str(status_extra),
        ignore_reason,
        "provider.loser_feedback_ignored"
    );
}
