use openagents_kernel_core::authority::KernelAuthority;

use crate::app_state::RenderState;

pub(crate) fn refresh_data_market_snapshot(state: &mut RenderState) -> bool {
    state.data_market.begin_refresh();

    let client = match crate::kernel_control::remote_authority_client_for_state(state) {
        Ok(client) => client,
        Err(error) => {
            state.data_market.record_error(error);
            return true;
        }
    };

    let snapshot = match crate::kernel_control::run_kernel_call(client.get_data_market_snapshot()) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            state.data_market.record_error(error);
            return true;
        }
    };
    state.data_market.apply_snapshot(
        snapshot.assets,
        snapshot.grants,
        snapshot.deliveries,
        snapshot.revocations,
        snapshot.refreshed_at_ms,
    );
    state.data_buyer.sync_selection(&state.data_market);
    true
}
