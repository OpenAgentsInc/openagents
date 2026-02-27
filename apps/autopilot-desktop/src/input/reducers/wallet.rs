use crate::app_state::RenderState;

pub(super) fn drain_spark_worker_updates(state: &mut RenderState) -> bool {
    let previous_invoice = state.spark_wallet.last_invoice.clone();
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

    super::super::refresh_earnings_scoreboard(state, std::time::Instant::now());
    true
}
