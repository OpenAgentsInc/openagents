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

    let assets =
        match crate::kernel_control::run_kernel_call(client.list_data_assets(None, None, None)) {
            Ok(assets) => assets,
            Err(error) => {
                state.data_market.record_error(error);
                return true;
            }
        };
    let grants = match crate::kernel_control::run_kernel_call(
        client.list_access_grants(None, None, None, None),
    ) {
        Ok(grants) => grants,
        Err(error) => {
            state.data_market.record_error(error);
            return true;
        }
    };
    let deliveries = match crate::kernel_control::run_kernel_call(
        client.list_delivery_bundles(None, None, None, None, None),
    ) {
        Ok(deliveries) => deliveries,
        Err(error) => {
            state.data_market.record_error(error);
            return true;
        }
    };
    let revocations = match crate::kernel_control::run_kernel_call(
        client.list_revocations(None, None, None, None, None),
    ) {
        Ok(revocations) => revocations,
        Err(error) => {
            state.data_market.record_error(error);
            return true;
        }
    };

    let refreshed_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0);
    state
        .data_market
        .apply_snapshot(assets, grants, deliveries, revocations, refreshed_at_ms);
    state.data_buyer.sync_selection(&state.data_market);
    true
}
