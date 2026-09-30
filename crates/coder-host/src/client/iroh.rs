//! The device side of iroh: pair with a computer from its connect code, and
//! open the NIP-REACH direct channel over iroh.
//!
//! iroh is transport. It finds a path (the same network, a hole-punched
//! path, or the OpenAgents iroh relay) and proves that the far end holds the
//! `EndpointId` the device dialed; nothing more. Pairing is NIP-HOST's
//! `enroll.redeem` on the `openagents/enroll/1` ALPN, and this device keeps
//! the grant only when it is signed by the host key in the code, names this
//! device, and carries exactly the connect-code rights. The channel is
//! NIP-REACH's handshake on `openagents/reach/1`, which proves both Nostr
//! keys and has the host check the grant, as over TCP.
//!
//! [`Dialer`] is this device's endpoint, bound on first use with the iroh
//! key the platform keeps beside the device key. [`IrohRoute`] is what a
//! device saves about a computer's endpoint: its `EndpointId`, its iroh
//! relay, and its last direct addresses.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use coder_access::client::{finish_redeem, prepare_redeem};
use coder_access::protocol::{HostInvitation, INVITATION_PREFIX};
use coder_access::{Access, RelayPolicy, Rights};
use nostr::domain::Event;
use openagents_connect::code::ConnectCode;
use openagents_connect::endpoint::{ConnectEndpoint, EndpointConfig};
use openagents_connect::enroll::{self, EnrollReply, EnrollRequest};
use openagents_connect::iroh::{EndpointAddr, EndpointId, RelayUrl, SecretKey as IrohSecret};
use openagents_connect::stream::IrohStream;
use openagents_connect::wire::{read_message, write_message};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

use super::{Device, Link};
use crate::{Error, Result, unix_time};

/// The Nostr relay a phone redeems a connect code on when iroh cannot
/// connect, and names in its request when the host does not say otherwise
/// (NIP-HOST, "Connect codes").
pub const DEFAULT_RELAY: &str = "wss://relay.openagents.com/";
/// How long one iroh dial may take before the device moves on.
const DIAL_TIMEOUT: Duration = Duration::from_secs(8);
/// The most direct addresses a saved route keeps.
const MAX_DIRECT: usize = 8;
/// How far the two clocks may differ before the phone names it.
const CLOCK_SKEW: u64 = openagents_connect::CLOCK_SKEW;

/// What a device keeps about a computer's iroh endpoint. Routing only: it
/// never identifies the computer or admits anything.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IrohRoute {
    /// The computer's `EndpointId`: 64 lowercase hex characters.
    pub endpoint: String,
    /// The iroh relay the computer keeps its home connection on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay: Option<String>,
    /// The computer's last known direct addresses, `ip:port`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub direct: Vec<String>,
}

impl IrohRoute {
    /// The route a connect code names.
    #[must_use]
    pub fn of(code: &ConnectCode) -> Self {
        Self {
            endpoint: hex(code.endpoint().as_bytes()),
            relay: code.relay().map(|relay| relay.to_string()),
            direct: code
                .addrs()
                .iter()
                .take(MAX_DIRECT)
                .map(ToString::to_string)
                .collect(),
        }
    }

    /// The computer's `EndpointId`.
    ///
    /// # Errors
    /// Refuses a saved value that is not an Ed25519 key.
    pub fn id(&self) -> Result<EndpointId> {
        let bytes = unhex32(&self.endpoint)
            .ok_or_else(|| Error::Config("the saved iroh endpoint is malformed".into()))?;
        EndpointId::from_bytes(&bytes)
            .map_err(|_| Error::Config("the saved iroh endpoint is not a key".into()))
    }

    /// The address to dial: the endpoint with its relay and direct
    /// addresses. A saved entry that does not parse is skipped.
    ///
    /// # Errors
    /// Refuses a saved endpoint that is not a key.
    pub fn addr(&self) -> Result<EndpointAddr> {
        let mut addr = EndpointAddr::new(self.id()?);
        if let Some(relay) = self
            .relay
            .as_deref()
            .and_then(|relay| relay.parse::<RelayUrl>().ok())
        {
            addr = addr.with_relay_url(relay);
        }
        for direct in self.direct.iter().take(MAX_DIRECT) {
            if let Ok(socket) = direct.parse::<SocketAddr>() {
                addr = addr.with_ip_addr(socket);
            }
        }
        Ok(addr)
    }

    /// The address a channel's route reports, `iroh:` and the endpoint.
    #[must_use]
    pub fn address(&self) -> String {
        format!("iroh:{}", self.endpoint)
    }
}

/// This device's iroh endpoint, bound on first use. Its key is the
/// device's iroh secret key, generated apart from every Nostr key and kept
/// in the same this-device-only store as the device key.
pub struct Dialer {
    secret: [u8; 32],
    config: EndpointConfig,
    bound: OnceCell<ConnectEndpoint>,
}

impl std::fmt::Debug for Dialer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The secret key stays out of debug output.
        f.debug_struct("Dialer")
            .field("id", &self.id())
            .finish_non_exhaustive()
    }
}

impl Dialer {
    /// A dialer through the OpenAgents iroh relay.
    #[must_use]
    pub fn new(secret: [u8; 32]) -> Self {
        Self::with_config(secret, EndpointConfig::device())
    }

    /// A dialer with its own endpoint configuration, such as relays
    /// disabled on loopback for a test.
    #[must_use]
    pub fn with_config(secret: [u8; 32], config: EndpointConfig) -> Self {
        Self {
            secret,
            config,
            bound: OnceCell::new(),
        }
    }

    /// A dialer on loopback with relays disabled. Tests only dial
    /// same-machine endpoints with it.
    #[must_use]
    pub fn loopback(secret: [u8; 32]) -> Self {
        Self::with_config(secret, EndpointConfig::loopback(Vec::new()))
    }

    /// This device's `EndpointId`, as hex.
    #[must_use]
    pub fn id(&self) -> String {
        hex(IrohSecret::from_bytes(&self.secret).public().as_bytes())
    }

    /// The bound endpoint.
    ///
    /// # Errors
    /// Reports sockets that cannot be bound.
    pub async fn endpoint(&self) -> Result<&ConnectEndpoint> {
        self.bound
            .get_or_try_init(|| async {
                ConnectEndpoint::bind(IrohSecret::from_bytes(&self.secret), self.config.clone())
                    .await
                    .map_err(|error| Error::Transport(format!("iroh: {error}")))
            })
            .await
    }
}

/// A computer this device just paired with.
#[derive(Debug)]
pub struct Enrolled {
    /// The verified access record, as over the relay.
    pub access: Access,
    /// The computer's iroh endpoint, from the code.
    pub route: IrohRoute,
    /// The computer's name from the code, for display only.
    pub label: String,
    /// How far this phone's clock is from the host's, in seconds, when it
    /// is more than 60.
    pub clock_off: Option<u64>,
    /// A `coder-pair:` invitation to the computer's Coder chats, when its
    /// answer carried one (over iroh only).
    pub chats: Option<String>,
}

/// Why pairing over iroh did not add the computer.
#[derive(Debug)]
pub enum EnrollError {
    /// iroh found no path to the computer, or it closed before it answered.
    /// Redeeming on the Nostr relay may still work.
    Unreachable,
    /// An answer did not come from the computer that showed the code: it
    /// was signed by another host key, or its grant names another computer
    /// or other rights. Nothing is kept.
    Mismatch,
    /// The computer answered; this is its refusal or a check that failed.
    /// `clock_off` is set when the two clocks differ by more than 60
    /// seconds.
    Refused {
        error: Error,
        clock_off: Option<u64>,
    },
}

/// Pair with the computer a connect code names, over iroh: dial the code's
/// `EndpointId` on the enroll ALPN, send this device's signed
/// `enroll.redeem` naming `relay`, and keep the grant only when the host key
/// and the `EndpointId` that answered match the code and the rights are the
/// connect-code rights. A connect code names no Nostr relay; a desktop app
/// host's invitations name [`DEFAULT_RELAY`], which a phone passes here.
///
/// # Errors
/// [`EnrollError::Unreachable`] when iroh cannot reach the computer, and
/// [`EnrollError::Refused`] for the computer's refusal or a failed check.
pub async fn enroll(
    dialer: &Dialer,
    code: &str,
    relay: &str,
    secret: &SecretKey,
    policy: RelayPolicy,
) -> std::result::Result<Enrolled, EnrollError> {
    let refused = |error: Error| EnrollError::Refused {
        error,
        clock_off: None,
    };
    let parsed = ConnectCode::parse_shape(code).map_err(|error| {
        refused(Error::Config(format!(
            "the code is not a valid connect code: {error}"
        )))
    })?;
    let local = unix_time().map_err(refused)?;
    let invitation = host_invitation(&parsed, relay, local, policy).map_err(refused)?;
    let pending = prepare_redeem(&invitation, secret, clamp(&parsed, local), policy)
        .map_err(|e| refused(e.into()))?;
    let request = serde_json::to_string(&pending.event)
        .map_err(|_| refused(Error::Config("the request does not encode".into())))?;
    let endpoint = dialer
        .endpoint()
        .await
        .map_err(|_| EnrollError::Unreachable)?;
    let reply = exchange(endpoint, &parsed, &EnrollRequest::new(request))
        .await
        .ok_or(EnrollError::Unreachable)?;
    let local = unix_time().map_err(refused)?;
    let clock_off = openagents_connect::clock_warning(reply.now, local);
    let refused = |error: Error| EnrollError::Refused { error, clock_off };
    let Some(text) = reply.reply else {
        // No signed reply: an unknown or cancelled invitation, or another
        // device redeemed it first.
        return Err(refused(Error::Access(coder_access::Error::new(
            if local >= parsed.expires_at() {
                coder_access::Code::Expired
            } else {
                coder_access::Code::Forbidden
            },
            "the computer did not accept this code",
        ))));
    };
    let event: Event = serde_json::from_str(&text)
        .map_err(|_| refused(Error::Config("the computer's reply is malformed".into())))?;
    // A reply signed by any key but the code's host key is not the
    // computer's answer, whatever it says.
    if event.pubkey != parsed.host() {
        return Err(EnrollError::Mismatch);
    }
    // Every check uses the host's clock, which decides the code's life.
    let access = finish_redeem(&invitation, &pending, &event, secret, reply.now, policy)
        .map_err(|error| refused(error.into()))?;
    accept(&access, &parsed).map_err(|_| EnrollError::Mismatch)?;
    Ok(Enrolled {
        access,
        route: IrohRoute::of(&parsed),
        label: parsed.label().to_owned(),
        clock_off,
        chats: reply
            .chats
            .filter(|text| text.starts_with("coder-pair:") && text.len() <= 4096),
    })
}

/// Dial the code's `EndpointId` on the enroll ALPN, send `request`, and
/// read the answer. `None` when iroh found no path in time, another
/// endpoint answered, or the stream ended before an answer: the relay may
/// still reach the computer.
async fn exchange(
    endpoint: &ConnectEndpoint,
    code: &ConnectCode,
    request: &EnrollRequest,
) -> Option<EnrollReply> {
    let connection = tokio::time::timeout(
        DIAL_TIMEOUT,
        endpoint
            .endpoint
            .connect(code.endpoint_addr(), openagents_connect::ENROLL_ALPN),
    )
    .await
    .ok()?
    .ok()?;
    // iroh's TLS proves the key it dialed; check it rather than assume it.
    if connection.remote_id() != code.endpoint() {
        return None;
    }
    let mut stream = IrohStream::open(connection).await.ok()?;
    tokio::time::timeout(enroll::TIMEOUT, async {
        write_message(&mut stream, request, enroll::MAX_MESSAGE_BYTES)
            .await
            .ok()?;
        let reply: EnrollReply = read_message(&mut stream, enroll::MAX_MESSAGE_BYTES)
            .await
            .ok()??;
        (reply.v == enroll::REPLY).then_some(reply)
    })
    .await
    .ok()?
}

/// Redeem a connect code on the Nostr relay instead, when iroh cannot reach
/// the computer. It succeeds only when the computer's invitations name
/// `relay`.
///
/// # Errors
/// The host's refusal, or a failed check or transport.
pub async fn enroll_on_relay(
    code: &str,
    relay: &str,
    secret: &SecretKey,
    policy: RelayPolicy,
) -> Result<Enrolled> {
    let parsed = ConnectCode::parse_shape(code)
        .map_err(|error| Error::Config(format!("the code is not a valid connect code: {error}")))?;
    // The relay path reads the invitation by this phone's clock, as for a
    // `coder-host:` invitation.
    let access =
        coder_access::client::redeem(&encode_host_invitation(&parsed, relay), secret, policy)
            .await?;
    accept(&access, &parsed)?;
    Ok(Enrolled {
        access,
        route: IrohRoute::of(&parsed),
        label: parsed.label().to_owned(),
        clock_off: None,
        chats: None,
    })
}

/// Open the direct channel to a paired computer over iroh, expecting the
/// host generation from fresh presence.
///
/// # Errors
/// A transport failure, or the handshake's refusal. One whose detail is
/// `coder_reach::channel::UNAUTHENTICATED` came before the host proved its
/// key.
pub async fn open_link(
    dialer: &Dialer,
    device: Arc<Device>,
    route: &IrohRoute,
    generation: u64,
    timeout: Duration,
) -> Result<Link> {
    let endpoint = dialer.endpoint().await?;
    let addr = route.addr()?;
    let expected = addr.id;
    let connection = tokio::time::timeout(
        DIAL_TIMEOUT,
        endpoint
            .endpoint
            .connect(addr, openagents_connect::REACH_ALPN),
    )
    .await
    .map_err(|_| Error::Transport("the iroh connection timed out".into()))?
    .map_err(|_| Error::Transport("the iroh connection failed".into()))?;
    // iroh proves the key it dialed; say so rather than assume it.
    if connection.remote_id() != expected {
        return Err(Error::Transport("another iroh endpoint answered".into()));
    }
    let stream = IrohStream::open(connection)
        .await
        .map_err(|_| Error::Transport("could not open an iroh stream".into()))?;
    Link::direct(device, stream, route.address(), generation, timeout).await
}

/// The connect-code rights: exactly [`Rights::pairing`], every right. Any
/// other set, including the narrower ones earlier codes carried, is not a
/// pairing's.
pub(crate) fn connect_rights(rights: &Rights) -> bool {
    *rights == Rights::pairing()
}

/// Keep a grant only when it names the code's host key and carries the
/// connect-code rights. `finish_redeem` already checked the host's
/// signature, this device's key, and the correlation.
fn accept(access: &Access, code: &ConnectCode) -> Result<()> {
    if access.grant.host != code.host() {
        return Err(Error::Access(coder_access::Error::new(
            coder_access::Code::Forbidden,
            "the grant names another computer",
        )));
    }
    if !connect_rights(&access.grant.rights) {
        return Err(Error::Access(coder_access::Error::new(
            coder_access::Code::Forbidden,
            "the grant's rights are not a connect code's",
        )));
    }
    Ok(())
}

/// A clock for preparing the request: this phone's, held inside the code's
/// window. The host judges the request by its own clock, so a phone whose
/// clock is off still sends a request the host can decide.
fn clamp(code: &ConnectCode, now: u64) -> u64 {
    now.clamp(code.issued_at(), code.expires_at().saturating_sub(1))
}

/// The same invitation in NIP-HOST's `coder-host:` layout, naming `relay`,
/// for redeeming it on that relay. The text holds the capability: never
/// log it.
fn encode_host_invitation(code: &ConnectCode, relay: &str) -> String {
    use base64::Engine as _;
    let mut bytes = vec![1u8];
    bytes.extend(unhex32(&code.host()).unwrap_or_default());
    bytes.extend(unhex32(&code.invitation()).unwrap_or_default());
    bytes.extend(unhex32(&code.capability()).unwrap_or_default());
    bytes.extend(code.issued_at().to_be_bytes());
    bytes.extend(code.expires_at().to_be_bytes());
    bytes.extend(u16::try_from(relay.len()).unwrap_or(u16::MAX).to_be_bytes());
    bytes.extend(relay.as_bytes());
    format!(
        "{INVITATION_PREFIX}{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    )
}

/// The code's invitation as NIP-HOST's parsed host invitation, read at a
/// time inside its window.
fn host_invitation(
    code: &ConnectCode,
    relay: &str,
    now: u64,
    policy: RelayPolicy,
) -> Result<HostInvitation> {
    HostInvitation::from_parts(
        &code.host(),
        &code.invitation(),
        &code.capability(),
        relay,
        code.issued_at(),
        code.expires_at(),
        clamp(code, now),
        policy,
    )
    .map_err(Error::from)
}

/// How far the phone's clock is off, when more than the skew.
#[must_use]
pub fn clock_off(host_now: u64, device_now: u64) -> Option<u64> {
    let off = host_now.abs_diff(device_now);
    (off > CLOCK_SKEW).then_some(off)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn unhex32(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let digit = |c: u8| match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            _ => None,
        };
        bytes[index] = digit(pair[0])? << 4 | digit(pair[1])?;
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_access::Right;

    #[test]
    fn a_route_round_trips_and_names_its_endpoint() {
        let secret = IrohSecret::from_bytes(&[7; 32]);
        let route = IrohRoute {
            endpoint: hex(secret.public().as_bytes()),
            relay: Some("https://iroh.openagents.com".into()),
            direct: vec!["192.168.1.20:4000".into(), "not an address".into()],
        };
        let addr = route.addr().unwrap();
        assert_eq!(addr.id, secret.public());
        assert_eq!(addr.ip_addrs().count(), 1);
        assert_eq!(addr.relay_urls().count(), 1);
        assert!(route.address().starts_with("iroh:"));
        let text = serde_json::to_string(&route).unwrap();
        assert_eq!(serde_json::from_str::<IrohRoute>(&text).unwrap(), route);
        let bad = IrohRoute {
            endpoint: "AB".repeat(32),
            relay: None,
            direct: vec![],
        };
        assert!(bad.addr().is_err());
    }

    #[test]
    fn only_the_connect_code_rights_are_kept() {
        let rights = |list: &[Right]| Rights::new(list.iter().copied()).unwrap();
        assert!(connect_rights(&Rights::pairing()));
        assert!(!connect_rights(&Rights::standard()));
        assert!(!connect_rights(&rights(&[Right::Observe, Right::Operate])));
        assert!(!connect_rights(&rights(&[
            Right::Observe,
            Right::Operate,
            Right::Terminal
        ])));
        assert!(!connect_rights(&rights(&[Right::Observe])));
        assert!(!connect_rights(&rights(&[
            Right::Observe,
            Right::Operate,
            Right::Review
        ])));
        assert!(!connect_rights(&rights(&[
            Right::Observe,
            Right::Operate,
            Right::AccessAdmin
        ])));
    }

    #[test]
    fn the_clock_is_named_only_past_a_minute() {
        assert_eq!(clock_off(1_000, 1_000), None);
        assert_eq!(clock_off(1_000, 940), None);
        assert_eq!(clock_off(1_000, 939), Some(61));
        assert_eq!(clock_off(1_000, 1_300), Some(300));
    }
}

/// Pairing against a fake computer on loopback iroh with relays disabled:
/// a real NIP-HOST access store answering the enroll ALPN.
#[cfg(all(test, feature = "host"))]
mod pairing {
    use super::*;
    use base64::Engine as _;
    use coder_access::host::{Host, Unconnected};
    use openagents_connect::ENROLL_ALPN;
    use openagents_connect::code::CodeParts;
    use openagents_connect::enroll::{EnrollProtocol, EnrollReply};
    use openagents_connect::iroh::protocol::Router;
    use tokio::sync::mpsc;

    /// How the fake computer answers.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Answer {
        /// Its own store answers, as a real computer does.
        Honest,
        /// Another host key signs the answer.
        OtherKey,
    }

    struct Computer {
        _dir: tempfile::TempDir,
        _other: tempfile::TempDir,
        host: Arc<Host>,
        endpoint: ConnectEndpoint,
        _router: Router,
    }

    fn now() -> u64 {
        unix_time().unwrap()
    }

    fn device() -> SecretKey {
        SecretKey::new(&mut secp256k1::rand::rng())
    }

    async fn computer(answer: Answer, phone: SecretKey) -> Computer {
        let owner = coder_reach::pubkey(&device());
        let dir = tempfile::tempdir().unwrap();
        let host = Arc::new(Host::new(
            dir.path().join("access"),
            RelayPolicy::Production,
        ));
        host.init(&owner).unwrap();
        let other_dir = tempfile::tempdir().unwrap();
        let other = Host::new(other_dir.path().join("access"), RelayPolicy::Production);
        other.init(&owner).unwrap();
        let endpoint = ConnectEndpoint::bind(
            IrohSecret::generate(),
            EndpointConfig::loopback(vec![ENROLL_ALPN.to_vec()]),
        )
        .await
        .unwrap();
        let (calls, mut queue) = mpsc::channel(4);
        let router = Router::builder(endpoint.endpoint.clone())
            .accept(ENROLL_ALPN, EnrollProtocol::new(calls))
            .spawn();
        let answering = host.clone();
        tokio::spawn(async move {
            while let Some(call) = queue.recv().await {
                let reply = match answer {
                    Answer::Honest => serde_json::from_str::<Event>(&call.request.request)
                        .ok()
                        .and_then(|event| {
                            answering
                                .handle(&event, DEFAULT_RELAY, now(), &mut Unconnected)
                                .ok()
                        }),
                    // A well-formed answer, signed by another host's key:
                    // its own grant for the same phone.
                    Answer::OtherKey => {
                        let issued = other
                            .invite(DEFAULT_RELAY, Rights::pairing(), now(), now() + 86_400)
                            .unwrap();
                        let invitation =
                            HostInvitation::parse(&issued.code, now(), RelayPolicy::Production)
                                .unwrap();
                        let pending =
                            prepare_redeem(&invitation, &phone, now(), RelayPolicy::Production)
                                .unwrap();
                        other
                            .handle(&pending.event, DEFAULT_RELAY, now(), &mut Unconnected)
                            .ok()
                    }
                };
                let reply = reply.map(|event| serde_json::to_string(&event).unwrap());
                let _ = call.reply.send(EnrollReply::new(now(), reply));
            }
        });
        Computer {
            _dir: dir,
            _other: other_dir,
            host,
            endpoint,
            _router: router,
        }
    }

    impl Computer {
        /// A connect code for a new invitation issued at `issued_at`.
        fn code(&self, issued_at: u64) -> String {
            let issued = self
                .host
                .invite(
                    DEFAULT_RELAY,
                    Rights::pairing(),
                    issued_at,
                    issued_at + 86_400,
                )
                .unwrap();
            let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(issued.code.strip_prefix(INVITATION_PREFIX).unwrap())
                .unwrap();
            let capability = hex(&bytes[65..97]);
            let local = self.endpoint.local_addr();
            ConnectCode::from_invitation(
                CodeParts {
                    host: self.host.public_key().unwrap(),
                    endpoint: self.endpoint.endpoint.id(),
                    issued_at,
                    relay: None,
                    addrs: local.ip_addrs().copied().collect(),
                    label: "Studio Mac".into(),
                },
                &issued.id,
                &capability,
            )
            .unwrap()
            .encode()
        }
    }

    fn phone_dialer() -> Dialer {
        Dialer::loopback(rand_bytes())
    }

    fn rand_bytes() -> [u8; 32] {
        let secret = device();
        secret.secret_bytes()
    }

    #[tokio::test]
    async fn a_phone_pairs_over_loopback_iroh_and_keeps_the_grant_and_route() {
        let phone = device();
        let computer = computer(Answer::Honest, phone).await;
        let code = computer.code(now());
        let dialer = phone_dialer();
        let enrolled = enroll(
            &dialer,
            &code,
            DEFAULT_RELAY,
            &phone,
            RelayPolicy::Production,
        )
        .await
        .unwrap_or_else(|error| panic!("{error:?}"));
        assert_eq!(
            enrolled.access.grant.host,
            computer.host.public_key().unwrap()
        );
        assert_eq!(enrolled.access.grant.device, coder_reach::pubkey(&phone));
        assert_eq!(enrolled.access.grant.rights, Rights::pairing());
        assert_eq!(enrolled.label, "Studio Mac");
        assert_eq!(enrolled.clock_off, None);
        assert_eq!(
            enrolled.route.id().unwrap(),
            computer.endpoint.endpoint.id()
        );
        assert!(!enrolled.route.direct.is_empty());
        // The code is single-use: another phone gets the computer's refusal.
        let second = device();
        match enroll(
            &phone_dialer(),
            &code,
            DEFAULT_RELAY,
            &second,
            RelayPolicy::Production,
        )
        .await
        {
            Err(EnrollError::Refused {
                error: Error::Access(error),
                ..
            }) => assert_eq!(error.code, coder_access::Code::Forbidden),
            other => panic!("a second redemption must be refused: {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_answer_signed_by_another_host_key_is_refused() {
        let phone = device();
        let computer = computer(Answer::OtherKey, phone).await;
        let code = computer.code(now());
        let outcome = enroll(
            &phone_dialer(),
            &code,
            DEFAULT_RELAY,
            &phone,
            RelayPolicy::Production,
        )
        .await;
        assert!(matches!(outcome, Err(EnrollError::Mismatch)), "{outcome:?}");
    }

    #[tokio::test]
    async fn an_expired_code_is_refused_as_expired() {
        let phone = device();
        let computer = computer(Answer::Honest, phone).await;
        let code = computer.code(now() - 400);
        match enroll(
            &phone_dialer(),
            &code,
            DEFAULT_RELAY,
            &phone,
            RelayPolicy::Production,
        )
        .await
        {
            Err(EnrollError::Refused {
                error: Error::Access(error),
                clock_off,
            }) => {
                assert_eq!(error.code, coder_access::Code::Expired);
                assert_eq!(clock_off, None);
            }
            other => panic!("an expired code must be refused: {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_computer_iroh_cannot_reach_is_unreachable_so_the_relay_is_tried() {
        let phone = device();
        let computer = computer(Answer::Honest, phone).await;
        let code = computer.code(now());
        computer.endpoint.close().await;
        let outcome = enroll(
            &phone_dialer(),
            &code,
            DEFAULT_RELAY,
            &phone,
            RelayPolicy::Production,
        )
        .await;
        assert!(
            matches!(outcome, Err(EnrollError::Unreachable)),
            "{outcome:?}"
        );
    }
}
