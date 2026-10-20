//! Wallet configuration shared by native and browser clients.
use breez_sdk_spark::{MaxFee, Network, default_config};
/// OpenAgents' Breez API key. The owner confirmed with the Breez team that it
/// is a basic validation key, which every shipped app carries and none can
/// hide, so it is committed here plainly (2026-09-28). It is a Base64 X.509
/// certificate that Breez's services check; it is not a spending credential.
pub const BREEZ_API_KEY: &str = "MIIBfjCCATCgAwIBAgIHPYzgGw0A+zAFBgMrZXAwEDEOMAwGA1UEAxMFQnJlZXowHhcNMjQxMTI0MjIxOTMzWhcNMzQxMTIyMjIxOTMzWjA3MRkwFwYDVQQKExBPcGVuQWdlbnRzLCBJbmMuMRowGAYDVQQDExFDaHJpc3RvcGhlciBEYXZpZDAqMAUGAytlcAMhANCD9cvfIDwcoiDKKYdT9BunHLS2/OuKzV8NS0SzqV13o4GBMH8wDgYDVR0PAQH/BAQDAgWgMAwGA1UdEwEB/wQCMAAwHQYDVR0OBBYEFNo5o+5ea0sNMlW/75VgGJCv2AcJMB8GA1UdIwQYMBaAFN6q1pJW843ndJIW/Ey2ILJrKJhrMB8GA1UdEQQYMBaBFGNocmlzQG9wZW5hZ2VudHMuY29tMAUGAytlcANBABvQIfNsop0kGIk0bgO/2kPum5B5lv6pYaSBXz73G1RV+eZj/wuW88lNQoGwVER+rA9+kWWTaR/dpdi8AFwjxw0=";

/// The SDK configuration every device uses: the given network, OpenAgents'
/// API key, and no real-time sync server.
pub fn sdk_config(network: Network) -> breez_sdk_spark::Config {
    let mut config = default_config(network);
    config.api_key = Some(BREEZ_API_KEY.to_owned());
    // Encrypted sync to Breez's server is for several devices on one
    // wallet, which the app does not offer.
    config.real_time_sync_server_url = None;
    config.lnurl_domain = Some("openagents.com".into());
    // Claim on-chain deposits (a MoonPay purchase, or bitcoin sent to the
    // deposit address) at maturity for up to the network's fastest
    // recommended rate plus 1 sat/vB, so ordinary deposits credit without a
    // tap. The default, 1 sat/vB, leaves them waiting whenever fees rise;
    // the screen offers a quoted claim for any deposit still waiting.
    config.max_deposit_claim_fee = Some(MaxFee::NetworkRecommended {
        leeway_sat_per_vbyte: 1,
    });
    config
}

/// Select the same Lightning or direct Spark route for the quote and the send.
pub(crate) fn lightning_route(lightning: u64, spark: Option<u64>) -> (u64, bool) {
    match spark {
        Some(fee) if fee <= lightning => (fee, true),
        _ => (lightning, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quote_and_payment_choose_the_lower_available_bitcoin_route() {
        assert_eq!(lightning_route(12, Some(3)), (3, true));
        assert_eq!(lightning_route(3, Some(12)), (3, false));
        assert_eq!(lightning_route(3, None), (3, false));
    }
}
