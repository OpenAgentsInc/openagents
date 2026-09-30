//! Nearby approval from the phone: pair with a computer found on this
//! network over mDNS, after the person compares a six-digit code on both
//! screens and clicks **Connect** on the computer (NIP-HOST, nearby
//! approval).
//!
//! The listing is untrusted: anyone on the network can advertise any name.
//! What makes the grant this computer's is the code the person compared,
//! which covers both `EndpointId`s and both keys, and the checks here: the
//! grant envelope must be signed by the host key the exchange named, open
//! with this device's key, and carry exactly the connect-code rights.

use coder_access::protocol::OriginKind;
use coder_access::{Access, RelayPolicy};
use nostr::domain::Event;
use openagents_connect::nearby::{self, DeviceOutcome};
pub use openagents_connect::nearby::{Code, NearbyBrowser, NearbyComputer, NearbyEvent};
use secp256k1::SecretKey;

use super::iroh::{Dialer, Enrolled, IrohRoute, clock_off, connect_rights};
use crate::unix_time;

/// Why a nearby pairing did not add the computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NearbyFailure {
    /// The phone could not reach the computer, or the exchange broke off.
    Unreachable,
    /// The computer finished without a grant after showing the code: the
    /// person clicked **Don't connect**, did not answer, or the codes
    /// differed.
    NotConnected,
    /// The computer is answering another phone, or too many asked lately.
    Busy,
    /// The two clocks differ by this many seconds; nothing was sent.
    ClockOff(u64),
    /// The answer was not a grant from that computer for this phone with
    /// the connect-code rights. Nothing is kept.
    Mismatch,
}

/// Pair with `computer`, showing the code through `show` as soon as it is
/// known. `label` is this phone's name for the computer's prompt.
///
/// # Errors
/// A [`NearbyFailure`] that says what the person can do next.
pub async fn pair(
    dialer: &Dialer,
    computer: &NearbyComputer,
    label: &str,
    secret: &SecretKey,
    policy: RelayPolicy,
    show: impl FnOnce(Code) + Send,
) -> Result<Enrolled, NearbyFailure> {
    let endpoint = dialer
        .endpoint()
        .await
        .map_err(|_| NearbyFailure::Unreachable)?;
    let device =
        nearby::parse_key(&coder_reach::pubkey(secret)).map_err(|_| NearbyFailure::Mismatch)?;
    let now = unix_time().map_err(|_| NearbyFailure::Unreachable)?;
    let outcome = nearby::pair(endpoint, computer, device, label, now, show)
        .await
        .map_err(|_| NearbyFailure::Unreachable)?;
    match outcome {
        DeviceOutcome::Approved {
            host_nostr,
            host_now,
            event,
            ..
        } => {
            let access = accept(&nearby::hex(&host_nostr), host_now, event, secret, policy)?;
            Ok(Enrolled {
                access,
                route: IrohRoute {
                    endpoint: nearby::hex(computer.endpoint.as_bytes()),
                    relay: None,
                    direct: computer.addrs.iter().map(ToString::to_string).collect(),
                },
                label: computer.label.clone(),
                clock_off: clock_off(host_now, unix_time().unwrap_or(now)),
                // The nearby exchange carries no chat invitation yet.
                chats: None,
            })
        }
        DeviceOutcome::NotConnected => Err(NearbyFailure::NotConnected),
        DeviceOutcome::Refused => Err(NearbyFailure::Busy),
        DeviceOutcome::ClockSkew { host_now } => {
            Err(NearbyFailure::ClockOff(host_now.abs_diff(now)))
        }
    }
}

/// Keep the grant only when it is the host's nearby approval for this
/// device: signed by `host`, opened with this device's key, judged at the
/// host's clock, with origin `approval` and the connect-code rights.
///
/// # Errors
/// [`NearbyFailure::Mismatch`] for anything else.
pub fn accept(
    host: &str,
    host_now: u64,
    event: serde_json::Value,
    secret: &SecretKey,
    policy: RelayPolicy,
) -> Result<Access, NearbyFailure> {
    let event: Event = serde_json::from_value(event).map_err(|_| NearbyFailure::Mismatch)?;
    if event.pubkey != host {
        return Err(NearbyFailure::Mismatch);
    }
    let access = Access::from_authorization(event, secret, host, host_now, policy)
        .map_err(|_| NearbyFailure::Mismatch)?;
    if access.grant.host != host
        || access.grant.origin.kind != OriginKind::Approval
        || !connect_rights(&access.grant.rights)
    {
        return Err(NearbyFailure::Mismatch);
    }
    Ok(access)
}
