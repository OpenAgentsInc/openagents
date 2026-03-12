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

pub(crate) fn emit_buyer_result_candidate_observed(
    request_id: &str,
    provider_pubkey: &str,
    result_event_id: &str,
    status: Option<&str>,
) {
    tracing::info!(
        target: TARGET,
        domain_event = "buyer.result_candidate_observed",
        flow_role = "buyer",
        request_id,
        provider_pubkey,
        result_event_id,
        status = optional_str(status),
        "buyer.result_candidate_observed"
    );
}

pub(crate) fn emit_buyer_invoice_candidate_observed(
    request_id: &str,
    provider_pubkey: &str,
    feedback_event_id: &str,
    amount_msats: Option<u64>,
    invoice_amount_sats: Option<u64>,
    approved_budget_sats: u64,
    bolt11_present: bool,
) {
    tracing::info!(
        target: TARGET,
        domain_event = "buyer.invoice_candidate_observed",
        flow_role = "buyer",
        request_id,
        provider_pubkey,
        feedback_event_id,
        amount_msats = optional_u64(amount_msats),
        invoice_amount_sats = optional_u64(invoice_amount_sats),
        approved_budget_sats,
        bolt11_present,
        "buyer.invoice_candidate_observed"
    );
}

pub(crate) fn emit_buyer_invoice_rejected_over_budget(
    request_id: &str,
    provider_pubkey: &str,
    feedback_event_id: &str,
    invoice_amount_sats: u64,
    approved_budget_sats: u64,
    amount_mismatch: bool,
) {
    tracing::info!(
        target: TARGET,
        domain_event = "buyer.invoice_rejected_over_budget",
        flow_role = "buyer",
        request_id,
        provider_pubkey,
        feedback_event_id,
        invoice_amount_sats,
        approved_budget_sats,
        amount_mismatch,
        "buyer.invoice_rejected_over_budget"
    );
}

pub(crate) fn emit_buyer_winner_unresolved(
    request_id: &str,
    result_provider_pubkey: Option<&str>,
    invoice_provider_pubkey: Option<&str>,
    blocker_code: &str,
    blocker_summary: Option<&str>,
) {
    tracing::info!(
        target: TARGET,
        domain_event = "buyer.winner_unresolved",
        flow_role = "buyer",
        request_id,
        result_provider_pubkey = optional_str(result_provider_pubkey),
        invoice_provider_pubkey = optional_str(invoice_provider_pubkey),
        blocker_code,
        blocker_summary = optional_str(blocker_summary),
        "buyer.winner_unresolved"
    );
}

pub(crate) fn emit_buyer_payment_blocked(
    request_id: &str,
    result_provider_pubkey: Option<&str>,
    invoice_provider_pubkey: Option<&str>,
    payable_provider_pubkey: Option<&str>,
    blocker_codes: Option<&str>,
    blocker_summary: Option<&str>,
) {
    tracing::info!(
        target: TARGET,
        domain_event = "buyer.payment_blocked",
        flow_role = "buyer",
        request_id,
        result_provider_pubkey = optional_str(result_provider_pubkey),
        invoice_provider_pubkey = optional_str(invoice_provider_pubkey),
        payable_provider_pubkey = optional_str(payable_provider_pubkey),
        blocker_codes = optional_str(blocker_codes),
        blocker_summary = optional_str(blocker_summary),
        "buyer.payment_blocked"
    );
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

pub(crate) fn emit_buyer_seller_settled_pending_wallet_confirmation(
    request_id: &str,
    provider_pubkey: Option<&str>,
    feedback_event_id: Option<&str>,
    payment_pointer: Option<&str>,
    local_wallet_status: &str,
) {
    tracing::info!(
        target: TARGET,
        domain_event = "buyer.seller_settled_pending_wallet_confirmation",
        flow_role = "buyer",
        request_id,
        provider_pubkey = optional_str(provider_pubkey),
        feedback_event_id = optional_str(feedback_event_id),
        payment_pointer = optional_str(payment_pointer),
        local_wallet_status,
        "buyer.seller_settled_pending_wallet_confirmation"
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

pub(crate) fn emit_provider_delivered_awaiting_settlement(
    request_id: &str,
    pending_bolt11_present: bool,
    payment_required_invoice_requested: bool,
    payment_required_feedback_in_flight: bool,
) {
    tracing::info!(
        target: TARGET,
        domain_event = "provider.delivered_awaiting_settlement",
        flow_role = "provider",
        request_id,
        pending_bolt11_present,
        payment_required_invoice_requested,
        payment_required_feedback_in_flight,
        "provider.delivered_awaiting_settlement"
    );
}

pub(crate) fn emit_provider_delivered_unpaid_timeout(
    request_id: &str,
    timeout_seconds: u64,
    reason: &str,
) {
    tracing::info!(
        target: TARGET,
        domain_event = "provider.delivered_unpaid_timeout",
        flow_role = "provider",
        request_id,
        timeout_seconds,
        blocker_summary = optional_str(Some(reason)),
        "provider.delivered_unpaid_timeout"
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
