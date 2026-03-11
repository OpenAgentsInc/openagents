use crate::app_state::{EarnFailureClass, RenderState};
use qrcode::{render::unicode::Dense1x2, QrCode};

pub(super) fn drain_spark_worker_updates(state: &mut RenderState) -> bool {
    let previous_invoice = state.spark_wallet.last_invoice.clone();
    let previous_payment_id = state.spark_wallet.last_payment_id.clone();
    let previous_error = state.spark_wallet.last_error.clone();
    if !state.spark_worker.drain_updates(&mut state.spark_wallet) {
        return false;
    }
    reconcile_spark_wallet_update(state, previous_invoice, previous_payment_id, previous_error);
    true
}

fn reconcile_spark_wallet_update(
    state: &mut RenderState,
    previous_invoice: Option<String>,
    previous_payment_id: Option<String>,
    previous_error: Option<String>,
) {
    if state.spark_wallet.last_invoice != previous_invoice
        && state
            .spark_wallet
            .last_action
            .as_deref()
            .is_some_and(|action| action.starts_with("Created invoice"))
        && let Some(invoice) = state.spark_wallet.last_invoice.as_deref()
    {
        let invoice = invoice.to_string();
        state.spark_inputs.send_request.set_value(invoice.clone());
        state.pay_invoice_inputs.payment_request.set_value(invoice);
    }

    if state.spark_wallet.last_invoice != previous_invoice
        && state
            .spark_wallet
            .last_action
            .as_deref()
            .is_some_and(|action| action.starts_with("Created Lightning invoice"))
        && let Some(invoice) = state.spark_wallet.last_invoice.as_deref()
    {
        match lightning_invoice_terminal_qr(invoice) {
            Ok(qr) => {
                tracing::info!(
                    target: "autopilot_desktop::spark_wallet",
                    "Lightning invoice QR:\n{qr}"
                );
            }
            Err(error) => {
                tracing::warn!(
                    target: "autopilot_desktop::spark_wallet",
                    "failed to render Lightning invoice QR: {error}"
                );
            }
        }
    }

    reconcile_provider_settlement_invoice_state(
        &mut state.active_job,
        &mut state.provider_runtime,
        &state.spark_wallet,
        previous_invoice.as_deref(),
        previous_error.as_deref(),
    );

    if state.spark_wallet.last_payment_id != previous_payment_id
        && let Some(request_id) = state
            .network_requests
            .pending_auto_payment_request_id
            .clone()
        && let Some(payment_pointer) = state.spark_wallet.last_payment_id.as_deref()
        && state
            .spark_wallet
            .last_action
            .as_deref()
            .is_some_and(|action| action.starts_with("Payment sent"))
    {
        let now_epoch_seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        state.network_requests.mark_auto_payment_sent(
            request_id.as_str(),
            payment_pointer,
            now_epoch_seconds,
        );
        state.provider_runtime.last_result = Some(format!(
            "buyer payment settled request={} pointer={}",
            request_id, payment_pointer
        ));
    }

    if let Some(request_id) = state
        .network_requests
        .pending_auto_payment_request_id
        .clone()
        && let Some(error) = state.spark_wallet.last_error.as_deref()
    {
        let now_epoch_seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        state.network_requests.mark_auto_payment_failed(
            request_id.as_str(),
            error,
            now_epoch_seconds,
        );
    }

    if state.spark_wallet.last_error.is_some() {
        state.provider_runtime.last_authoritative_error_class = Some(EarnFailureClass::Payment);
    } else if state.provider_runtime.last_authoritative_error_class
        == Some(EarnFailureClass::Payment)
    {
        state.provider_runtime.last_authoritative_error_class = None;
    }

    super::super::refresh_earnings_scoreboard(state, std::time::Instant::now());
}

fn lightning_invoice_uri(invoice: &str) -> String {
    let trimmed = invoice.trim();
    if trimmed
        .get(..10)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("lightning:"))
    {
        trimmed.to_string()
    } else {
        format!("lightning:{trimmed}")
    }
}

fn lightning_invoice_terminal_qr(invoice: &str) -> Result<String, String> {
    let invoice_uri = lightning_invoice_uri(invoice);
    let qr = QrCode::new(invoice_uri.as_bytes())
        .map_err(|error| format!("invalid Lightning invoice for QR render: {error}"))?;
    Ok(qr
        .render::<Dense1x2>()
        .quiet_zone(true)
        .module_dimensions(1, 1)
        .build())
}

fn reconcile_provider_settlement_invoice_state(
    active_job: &mut crate::app_state::ActiveJobState,
    provider_runtime: &mut crate::state::provider_runtime::ProviderRuntimeState,
    spark_wallet: &crate::spark_wallet::SparkPaneState,
    previous_invoice: Option<&str>,
    previous_error: Option<&str>,
) {
    if active_job.payment_required_invoice_requested
        && spark_wallet.last_invoice.as_deref() != previous_invoice
        && spark_wallet
            .last_action
            .as_deref()
            .is_some_and(|action| action.starts_with("Created Lightning invoice"))
        && let Some(invoice) = spark_wallet.last_invoice.as_deref()
    {
        active_job.payment_required_invoice_requested = false;
        active_job.payment_required_failed = false;
        active_job.pending_bolt11 = Some(invoice.to_string());
        active_job.append_event("generated Spark BOLT11 settlement invoice for provider payout");
        provider_runtime.last_result = Some(
            "provider settlement invoice generated; queueing payment-required feedback"
                .to_string(),
        );
    }

    if active_job.payment_required_invoice_requested
        && spark_wallet.last_error.as_deref() != previous_error
        && let Some(error) = spark_wallet.last_error.as_deref()
    {
        active_job.payment_required_invoice_requested = false;
        active_job.payment_required_failed = true;
        let message = format!("provider settlement invoice creation failed: {error}");
        active_job.append_event(message.clone());
        active_job.last_error = Some(message.clone());
        active_job.load_state = crate::app_state::PaneLoadState::Error;
        provider_runtime.last_result = Some(message);
        provider_runtime.last_authoritative_error_class = Some(EarnFailureClass::Payment);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        lightning_invoice_terminal_qr, lightning_invoice_uri,
        reconcile_provider_settlement_invoice_state,
    };
    use crate::app_state::{
        ActiveJobState, EarnFailureClass, JobDemandSource, JobInboxDecision, JobInboxRequest,
        JobInboxValidation, JobLifecycleStage, PaneLoadState,
    };
    use crate::spark_wallet::SparkPaneState;
    use crate::state::provider_runtime::ProviderRuntimeState;

    fn fixture_request(request_id: &str) -> JobInboxRequest {
        JobInboxRequest {
            request_id: request_id.to_string(),
            requester: "buyer".to_string(),
            demand_source: JobDemandSource::OpenNetwork,
            request_kind: 5050,
            capability: "summarize.text".to_string(),
            execution_input: Some(format!("Process request {request_id}")),
            execution_prompt: Some(format!("Prompt for {request_id}")),
            execution_params: Vec::new(),
            requested_model: None,
            requested_output_mime: Some("text/plain".to_string()),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: Some(format!("req-event:{request_id}")),
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats: 2,
            ttl_seconds: 60,
            validation: JobInboxValidation::Valid,
            arrival_seq: 1,
            decision: JobInboxDecision::Accepted {
                reason: "valid + priced".to_string(),
            },
        }
    }

    fn fixture_delivered_active_job(request_id: &str) -> ActiveJobState {
        let mut active_job = ActiveJobState::default();
        active_job.start_from_request(&fixture_request(request_id));
        let job = active_job
            .job
            .as_mut()
            .expect("active job should exist");
        job.stage = JobLifecycleStage::Delivered;
        active_job
    }

    #[test]
    fn provider_settlement_invoice_success_sets_pending_bolt11() {
        let mut active_job = fixture_delivered_active_job("req-provider-invoice-ready");
        let mut provider_runtime = ProviderRuntimeState::default();
        let mut spark_wallet = SparkPaneState::default();
        active_job.payment_required_invoice_requested = true;
        spark_wallet.last_action = Some("Created Lightning invoice for 2 sats".to_string());
        spark_wallet.last_invoice = Some("lnbc20n1providerready".to_string());

        reconcile_provider_settlement_invoice_state(
            &mut active_job,
            &mut provider_runtime,
            &spark_wallet,
            None,
            None,
        );

        assert!(!active_job.payment_required_invoice_requested);
        assert!(!active_job.payment_required_failed);
        assert_eq!(
            active_job.pending_bolt11.as_deref(),
            Some("lnbc20n1providerready")
        );
        assert!(
            active_job.job.as_ref().is_some_and(|job| job.events.iter().any(|event| {
                event
                    .message
                    .contains("generated Spark BOLT11 settlement invoice")
            }))
        );
    }

    #[test]
    fn provider_settlement_invoice_failure_marks_payment_error() {
        let mut active_job = fixture_delivered_active_job("req-provider-invoice-fail");
        let mut provider_runtime = ProviderRuntimeState::default();
        let mut spark_wallet = SparkPaneState::default();
        active_job.payment_required_invoice_requested = true;
        spark_wallet.last_error = Some("spark timeout".to_string());

        reconcile_provider_settlement_invoice_state(
            &mut active_job,
            &mut provider_runtime,
            &spark_wallet,
            None,
            None,
        );

        assert!(!active_job.payment_required_invoice_requested);
        assert!(active_job.payment_required_failed);
        assert_eq!(
            active_job.last_error.as_deref(),
            Some("provider settlement invoice creation failed: spark timeout")
        );
        assert_eq!(active_job.load_state, PaneLoadState::Error);
        assert_eq!(
            provider_runtime.last_authoritative_error_class,
            Some(EarnFailureClass::Payment)
        );
    }

    #[test]
    fn lightning_invoice_terminal_qr_renders_multiline_blocks() {
        let qr = lightning_invoice_terminal_qr("lnbc20n1providerready")
            .expect("qr render should succeed for invoice text");

        assert!(qr.contains('\n'));
        assert!(qr.contains('█') || qr.contains('▀') || qr.contains('▄'));
    }

    #[test]
    fn lightning_invoice_uri_prepends_scheme_once() {
        assert_eq!(
            lightning_invoice_uri("lnbc20n1providerready"),
            "lightning:lnbc20n1providerready"
        );
        assert_eq!(
            lightning_invoice_uri("LIGHTNING:lnbc20n1providerready"),
            "LIGHTNING:lnbc20n1providerready"
        );
    }
}
