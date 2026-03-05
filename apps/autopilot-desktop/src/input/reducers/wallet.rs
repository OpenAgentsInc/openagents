use crate::app_state::{EarnFailureClass, RenderState};

pub(super) fn drain_spark_worker_updates(state: &mut RenderState) -> bool {
    let previous_invoice = state.spark_wallet.last_invoice.clone();
    let previous_payment_id = state.spark_wallet.last_payment_id.clone();
    if !state.spark_worker.drain_updates(&mut state.spark_wallet) {
        return false;
    }

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
    true
}
