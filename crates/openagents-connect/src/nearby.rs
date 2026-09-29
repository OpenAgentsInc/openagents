//! Nearby approval ([NIP-HOST](../../../nips/openagents/NIP-HOST.md#nearby-approval-planned)):
//! a phone on the same network finds a computer over mDNS and pairs after
//! the person compares a six-digit code on both screens and clicks
//! **Connect** on the computer.
//!
//! Discovery only introduces. The host advertises under the service name
//! [`SERVICE_NAME`] (`_openagents._udp`) with its `EndpointId` as the
//! instance name, its IP addresses, and a display label, and nothing else:
//! no relay URL, no Nostr key, no rights ([`advertised`]). Anyone on the
//! network can advertise any label, so nothing here trusts one.
//!
//! The exchange runs on the enroll ALPN ([`ALPN`], `openagents/enroll/1`),
//! one bidirectional stream of length-prefixed strict JSON, with the same
//! framing as enrollment over iroh. The host tells the two apart by the
//! first message's `v`:
//!
//! 1. Device: [`NearbyRequestMessage`] with its Nostr key, a label, and
//!    `SHA-256(nonce_d)` ([`commitment`]).
//! 2. Host: [`NearbyOffer`] with its Nostr key, `nonce_h`, and its clock.
//! 3. Device: [`NearbyReveal`] with `nonce_d`. The host ends the stream on
//!    a reveal that does not open the commitment.
//! 4. Both compute [`sas_code`] over both `EndpointId`s (as each side's
//!    QUIC TLS saw them), both Nostr keys, and both nonces. The device shows
//!    it; the host shows it beside the device's label and waits for the
//!    person's click.
//! 5. On **Connect**, the host sends [`HostAnswer`] with the host-signed
//!    grant envelope, encrypted to the device key. **Don't connect**, a
//!    mismatch, a timeout, a busy host, or its rate limit gets no answer:
//!    the host finishes the stream.
//!
//! The commitment fixes the device's nonce before it sees the host's, and
//! the host fixes its nonce before it sees the device's, so a machine in
//! the middle, which must use its own keys on each side, gets one
//! one-in-a-million guess at making the two screens agree per attempt. The
//! host's gate (one pending request, five per ten minutes) bounds attempts.
//!
//! This module never grants anything: the host's gate decides, and the
//! grant is NIP-HOST's, checked by the device like any other.

use std::fmt;
use std::future::Future;
use std::net::SocketAddr;
use std::time::Duration;

use iroh::address_lookup::{AddressLookup, EndpointData, UserData};
use iroh::{EndpointAddr, EndpointId, TransportAddr};
use iroh_mdns_address_lookup::{DiscoveryEvent, MdnsAddressLookup};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use tokio::sync::oneshot;

use crate::endpoint::ConnectEndpoint;
use crate::stream::IrohStream;
use crate::wire::{read_message, write_message};

/// The ALPN nearby approval shares with enrollment over iroh.
pub const ALPN: &[u8] = crate::ENROLL_ALPN;
/// The mDNS service name; records appear as `<endpoint>._openagents._udp.local`.
pub const SERVICE_NAME: &str = "openagents";
/// The Bonjour service type the iOS app declares in `NSBonjourServices`.
pub const BONJOUR_SERVICE: &str = "_openagents._udp";
/// Domain separation for the confirmation code.
pub const SAS_DOMAIN: &[u8] = b"openagents.connect-sas.v1";
/// The device's first message.
pub const REQUEST: &str = "openagents.connect-nearby-request.v1";
/// The host's offer.
pub const OFFER: &str = "openagents.connect-nearby-offer.v1";
/// The device's reveal.
pub const REVEAL: &str = "openagents.connect-nearby-reveal.v1";
/// The host's answer after a click, as in enrollment over iroh.
pub const ANSWER: &str = "openagents.host-answer.v1";
/// The longest label, in UTF-8 bytes, advertised or sent.
pub const LABEL_MAX: usize = 48;
/// The largest nearby message.
pub const MAX_MESSAGE: usize = 4 * 1024;
/// The largest message on the enroll ALPN, which bounds the answer.
pub const MAX_FRAME: usize = crate::enroll::MAX_MESSAGE_BYTES;
/// How long the host waits for each device message.
pub const STEP_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a request waits for the person at the computer.
pub const DECISION_TIMEOUT: Duration = Duration::from_secs(120);
/// How far apart the two clocks may be before the device stops and says so.
pub const MAX_SKEW: u64 = crate::CLOCK_SKEW;
const DEVICE_SLACK: Duration = Duration::from_secs(10);
const MAX_SAFE: u64 = (1 << 53) - 1;

/// A 32-byte key: an `EndpointId` or a Nostr x-only public key.
pub type Key = [u8; 32];

/// A nearby-pairing failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NearbyError {
    /// The peer sent a frame that is too long, not JSON, or out of order.
    Protocol(&'static str),
    /// The device's reveal does not open its commitment.
    Commitment,
    /// A step took too long.
    Timeout,
    /// The stream failed or closed early.
    Io(String),
    /// mDNS could not start (no IPv4 or IPv6, or no multicast).
    Discovery(String),
    /// No randomness was available for a nonce.
    Random,
}

impl fmt::Display for NearbyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(what) => write!(f, "nearby pairing protocol error: {what}"),
            Self::Commitment => f.write_str("the device's nonce does not match its commitment"),
            Self::Timeout => f.write_str("nearby pairing timed out"),
            Self::Io(error) => write!(f, "nearby pairing stream failed: {error}"),
            Self::Discovery(error) => write!(f, "nearby discovery could not start: {error}"),
            Self::Random => f.write_str("no randomness for a nearby nonce"),
        }
    }
}

impl std::error::Error for NearbyError {}

impl From<std::io::Error> for NearbyError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<crate::Error> for NearbyError {
    fn from(error: crate::Error) -> Self {
        match error.code {
            crate::Code::Bounds => Self::Protocol("frame too long"),
            crate::Code::Malformed => Self::Protocol("malformed frame"),
            _ => Self::Io(error.detail.into_owned()),
        }
    }
}

/// A per-attempt secret nonce.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Nonce(pub [u8; 32]);

impl Nonce {
    /// A fresh nonce from the operating system.
    ///
    /// # Errors
    /// Fails only when the OS has no randomness.
    pub fn random() -> Result<Self, NearbyError> {
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|_| NearbyError::Random)?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for Nonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Nonce(..)")
    }
}

/// `SHA-256(nonce_d)`, sent before the device learns `nonce_h`.
#[must_use]
pub fn commitment(nonce: &Nonce) -> Key {
    Sha256::digest(nonce.0).into()
}

/// Everything the confirmation code covers, with the host's half first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transcript {
    pub host_endpoint: Key,
    pub device_endpoint: Key,
    pub host_nostr: Key,
    pub device_nostr: Key,
    pub host_nonce: Nonce,
    pub device_nonce: Nonce,
}

/// A six-digit confirmation code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Code(u32);

impl Code {
    /// A code from its value, for display fixtures; `None` at one million
    /// or more.
    #[must_use]
    pub fn new(value: u32) -> Option<Self> {
        (value < 1_000_000).then_some(Self(value))
    }
    /// The code as a number below one million.
    #[must_use]
    pub fn value(self) -> u32 {
        self.0
    }
    /// The six digits with no space, for accessibility and tests.
    #[must_use]
    pub fn digits(self) -> String {
        format!("{:06}", self.0)
    }
}

/// Shown as two groups of three digits: `482 913`.
impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:03} {:03}", self.0 / 1000, self.0 % 1000)
    }
}

/// The confirmation code: the first eight bytes of
/// `SHA-256("openagents.connect-sas.v1" || 0x00 || host_endpoint ||
/// device_endpoint || host_nostr || device_nostr || nonce_h || nonce_d)`
/// as a big-endian integer, modulo one million. The modulo bias is below
/// 2^-44.
#[must_use]
pub fn sas_code(transcript: &Transcript) -> Code {
    let mut hash = Sha256::new();
    hash.update(SAS_DOMAIN);
    hash.update([0]);
    hash.update(transcript.host_endpoint);
    hash.update(transcript.device_endpoint);
    hash.update(transcript.host_nostr);
    hash.update(transcript.device_nostr);
    hash.update(transcript.host_nonce.0);
    hash.update(transcript.device_nonce.0);
    let digest = hash.finalize();
    let mut head = [0; 8];
    head.copy_from_slice(&digest[..8]);
    #[allow(clippy::cast_possible_truncation)] // below one million
    Code((u64::from_be_bytes(head) % 1_000_000) as u32)
}

/// A label fit to show: control characters removed, whitespace trimmed,
/// at most [`LABEL_MAX`] bytes on a character boundary.
#[must_use]
pub fn clean_label(label: &str) -> String {
    let mut out = String::new();
    for c in label.trim().chars().filter(|c| !c.is_control()) {
        if out.len() + c.len_utf8() > LABEL_MAX {
            break;
        }
        out.push(c);
    }
    out.trim_end().to_owned()
}

fn valid_label(label: &str) -> bool {
    label.len() <= LABEL_MAX && !label.chars().any(char::is_control)
}

/// `openagents.connect-nearby-request.v1`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NearbyRequestMessage {
    pub v: String,
    pub requires: Vec<String>,
    /// The device's Nostr x-only public key, lowercase hex.
    pub device: String,
    /// 0–48 bytes, no control characters.
    pub label: String,
    /// `SHA-256(nonce_d)`, lowercase hex.
    pub commitment: String,
}

impl NearbyRequestMessage {
    /// Reads a first message the enroll handler already parsed, for
    /// dispatch on its `v`.
    ///
    /// # Errors
    /// Anything but a well-formed request.
    pub fn from_value(value: serde_json::Value) -> Result<Self, NearbyError> {
        serde_json::from_value(value).map_err(|_| NearbyError::Protocol("malformed request"))
    }
}

/// `openagents.connect-nearby-offer.v1`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NearbyOffer {
    pub v: String,
    pub requires: Vec<String>,
    /// The host's Nostr x-only public key, lowercase hex.
    pub host: String,
    /// `nonce_h`, lowercase hex.
    pub nonce: String,
    /// The host's Unix time.
    pub now: u64,
}

/// `openagents.connect-nearby-reveal.v1`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NearbyReveal {
    pub v: String,
    pub requires: Vec<String>,
    /// `nonce_d`, lowercase hex.
    pub nonce: String,
}

/// `openagents.host-answer.v1`: the grant envelope after a click.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostAnswer {
    pub v: String,
    /// The host-signed grant envelope, encrypted to the device key.
    pub event: serde_json::Value,
}

fn schema(v: &str, expected: &str, requires: &[String]) -> Result<(), NearbyError> {
    if v != expected || !requires.is_empty() {
        return Err(NearbyError::Protocol("unsupported version or requirement"));
    }
    Ok(())
}

/// The device's request as the host sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NearbyRequest {
    pub device_endpoint: Key,
    pub device_nostr: Key,
    pub label: String,
}

/// Why the host refused a request before the exchange.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Busy,
    Limited,
}

/// What the person at the computer decided.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// **Connect**: the grant envelope the host signed after the click.
    Connect {
        event: serde_json::Value,
    },
    Decline,
    Expired,
}

/// The host's gate: admission and the person's decision.
pub trait Admission {
    type Ticket: Ticket;
    /// Admits a request as the one pending request, or refuses it.
    ///
    /// # Errors
    /// [`Refusal::Busy`] while another request is pending,
    /// [`Refusal::Limited`] past the rate limit.
    fn admit(&self, request: &NearbyRequest) -> Result<Self::Ticket, Refusal>;
}

/// One admitted request. Dropping it (the device went away) withdraws it.
pub trait Ticket: Send {
    /// Shows `code` to the person and waits for their click. Only a click
    /// on **Connect** may sign the grant in the returned verdict.
    fn decide(self, code: Code) -> impl Future<Output = Verdict> + Send;
}

/// How a host-side exchange ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostOutcome {
    Approved {
        request: NearbyRequest,
        code: Code,
    },
    Declined,
    Expired,
    Refused(Refusal),
    /// The device closed the stream while the request was pending.
    Withdrawn,
}

/// The host keys the exchange covers.
#[derive(Clone, Copy, Debug)]
pub struct HostKeys {
    pub endpoint: Key,
    pub nostr: Key,
}

/// Serves one nearby exchange on the host, reading the request first.
/// `device_endpoint` is the peer's `EndpointId` from the QUIC connection,
/// never from a message.
///
/// # Errors
/// See [`host_session_after`].
pub async fn host_session<R, W, A>(
    mut reader: R,
    writer: W,
    host: HostKeys,
    device_endpoint: Key,
    nonce: Nonce,
    now: u64,
    admission: &A,
) -> Result<HostOutcome, NearbyError>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
    A: Admission,
{
    let request = timed(STEP_TIMEOUT, read_frame(&mut reader, MAX_MESSAGE)).await?;
    host_session_after(
        request,
        reader,
        writer,
        host,
        device_endpoint,
        nonce,
        now,
        admission,
    )
    .await
}

/// Serves a nearby exchange whose request the enroll handler already read.
/// Every refusal and failure finishes the stream with no answer.
///
/// # Errors
/// A malformed or out-of-order message, a reveal that does not open the
/// commitment, a timeout, or a stream failure.
#[allow(clippy::too_many_arguments)]
pub async fn host_session_after<R, W, A>(
    request: NearbyRequestMessage,
    mut reader: R,
    mut writer: W,
    host: HostKeys,
    device_endpoint: Key,
    nonce: Nonce,
    now: u64,
    admission: &A,
) -> Result<HostOutcome, NearbyError>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
    A: Admission,
{
    let outcome = host_steps(
        request,
        &mut reader,
        &mut writer,
        host,
        device_endpoint,
        nonce,
        now,
        admission,
    )
    .await;
    let _ = writer.shutdown().await;
    outcome
}

#[allow(clippy::too_many_arguments)]
async fn host_steps<R, W, A>(
    request: NearbyRequestMessage,
    reader: &mut R,
    writer: &mut W,
    host: HostKeys,
    device_endpoint: Key,
    nonce: Nonce,
    now: u64,
    admission: &A,
) -> Result<HostOutcome, NearbyError>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
    A: Admission,
{
    schema(&request.v, REQUEST, &request.requires)?;
    if !valid_label(&request.label) {
        return Err(NearbyError::Protocol("malformed label"));
    }
    let request_view = NearbyRequest {
        device_endpoint,
        device_nostr: parse_key(&request.device)?,
        label: request.label,
    };
    let committed = parse_key(&request.commitment)?;
    let ticket = match admission.admit(&request_view) {
        Ok(ticket) => ticket,
        Err(refusal) => return Ok(HostOutcome::Refused(refusal)),
    };
    write_frame(
        writer,
        &NearbyOffer {
            v: OFFER.into(),
            requires: vec![],
            host: hex(&host.nostr),
            nonce: hex(&nonce.0),
            now,
        },
        MAX_MESSAGE,
    )
    .await?;
    let reveal: NearbyReveal = timed(STEP_TIMEOUT, read_frame(reader, MAX_MESSAGE)).await?;
    schema(&reveal.v, REVEAL, &reveal.requires)?;
    let device_nonce = Nonce(parse_key(&reveal.nonce)?);
    if commitment(&device_nonce) != committed {
        return Err(NearbyError::Commitment);
    }
    let code = sas_code(&Transcript {
        host_endpoint: host.endpoint,
        device_endpoint,
        host_nostr: host.nostr,
        device_nostr: request_view.device_nostr,
        host_nonce: nonce,
        device_nonce,
    });
    // The device sends nothing more; a read that returns means it left.
    let mut probe = [0u8; 1];
    let verdict = tokio::select! {
        verdict = tokio::time::timeout(DECISION_TIMEOUT, ticket.decide(code)) => {
            verdict.unwrap_or(Verdict::Expired)
        }
        _ = reader.read(&mut probe) => return Ok(HostOutcome::Withdrawn),
    };
    match verdict {
        Verdict::Connect { event } => {
            write_frame(
                writer,
                &HostAnswer {
                    v: ANSWER.into(),
                    event,
                },
                MAX_FRAME,
            )
            .await?;
            Ok(HostOutcome::Approved {
                request: request_view,
                code,
            })
        }
        Verdict::Decline => Ok(HostOutcome::Declined),
        Verdict::Expired => Ok(HostOutcome::Expired),
    }
}

/// The device keys the exchange covers.
#[derive(Clone, Copy, Debug)]
pub struct DeviceKeys {
    pub endpoint: Key,
    pub nostr: Key,
}

/// How a device-side exchange ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceOutcome {
    /// The computer approved. The caller accepts `event` only as a grant
    /// envelope signed by `host_nostr` for this device key with the
    /// connect-code rights, judged at the host's clock `host_now`.
    Approved {
        host_nostr: Key,
        host_now: u64,
        code: Code,
        event: serde_json::Value,
    },
    /// The computer finished the stream after showing the code: the person
    /// clicked **Don't connect**, did not answer, or the codes differed.
    NotConnected,
    /// The computer finished the stream before any code: it is busy with
    /// another phone, or too many phones asked lately.
    Refused,
    /// The clocks differ by more than [`MAX_SKEW`] seconds; nothing was
    /// revealed. `host_now` is the computer's clock.
    ClockSkew { host_now: u64 },
}

/// Runs one nearby exchange from the device. `host_endpoint` is the
/// `EndpointId` the QUIC connection authenticated and `now` is this
/// device's clock. `show` gets the code as soon as it is known, before the
/// computer decides.
///
/// # Errors
/// A malformed or out-of-order message, a timeout, or a stream failure.
#[allow(clippy::too_many_arguments)]
pub async fn device_session<R, W>(
    mut reader: R,
    mut writer: W,
    device: DeviceKeys,
    host_endpoint: Key,
    label: &str,
    nonce: Nonce,
    now: u64,
    show: impl FnOnce(Code) + Send,
) -> Result<DeviceOutcome, NearbyError>
where
    R: AsyncRead + Unpin + Send,
    W: AsyncWrite + Unpin + Send,
{
    write_frame(
        &mut writer,
        &NearbyRequestMessage {
            v: REQUEST.into(),
            requires: vec![],
            device: hex(&device.nostr),
            label: clean_label(label),
            commitment: hex(&commitment(&nonce)),
        },
        MAX_MESSAGE,
    )
    .await?;
    let Some(offer) = timed(
        STEP_TIMEOUT,
        read_optional::<_, NearbyOffer>(&mut reader, MAX_MESSAGE),
    )
    .await?
    else {
        return Ok(DeviceOutcome::Refused);
    };
    schema(&offer.v, OFFER, &offer.requires)?;
    if offer.now > MAX_SAFE {
        return Err(NearbyError::Protocol("host time out of range"));
    }
    let (host_nostr, host_nonce) = (parse_key(&offer.host)?, Nonce(parse_key(&offer.nonce)?));
    if crate::clock_warning(offer.now, now).is_some() {
        let _ = writer.shutdown().await;
        return Ok(DeviceOutcome::ClockSkew {
            host_now: offer.now,
        });
    }
    write_frame(
        &mut writer,
        &NearbyReveal {
            v: REVEAL.into(),
            requires: vec![],
            nonce: hex(&nonce.0),
        },
        MAX_MESSAGE,
    )
    .await?;
    let code = sas_code(&Transcript {
        host_endpoint,
        device_endpoint: device.endpoint,
        host_nostr,
        device_nostr: device.nostr,
        host_nonce,
        device_nonce: nonce,
    });
    show(code);
    let answer = timed(
        DECISION_TIMEOUT + DEVICE_SLACK,
        read_optional::<_, HostAnswer>(&mut reader, MAX_FRAME),
    )
    .await?;
    let _ = writer.shutdown().await;
    let Some(answer) = answer else {
        return Ok(DeviceOutcome::NotConnected);
    };
    if answer.v != ANSWER {
        return Err(NearbyError::Protocol("unsupported answer"));
    }
    Ok(DeviceOutcome::Approved {
        host_nostr,
        host_now: offer.now,
        code,
        event: answer.event,
    })
}

/// How long the enroll handler keeps a nearby connection open: the three
/// device steps and the person's decision.
pub const SESSION_LIMIT: Duration = Duration::from_secs(
    STEP_TIMEOUT.as_secs() * 2 + DECISION_TIMEOUT.as_secs() + DEVICE_SLACK.as_secs(),
);

/// A nearby request the enroll handler read and handed to the host
/// ([`crate::enroll::EnrollProtocol::with_nearby`]). The host runs
/// [`host_session_after`] on `stream` with `remote` as the device's
/// `EndpointId`, then drops `done`; the handler holds the connection open
/// until then, at most [`SESSION_LIMIT`].
#[derive(Debug)]
pub struct NearbyCall {
    /// The device's iroh key, proven by QUIC.
    pub remote: EndpointId,
    pub request: NearbyRequestMessage,
    pub stream: IrohStream,
    pub done: oneshot::Sender<()>,
}

/// Dials a nearby computer on the enroll ALPN and runs the exchange as the
/// device whose Nostr key is `device_nostr`. `show` gets the code; the
/// caller checks an approved grant before keeping it.
///
/// # Errors
/// A failed dial, and everything [`device_session`] reports.
pub async fn pair(
    endpoint: &ConnectEndpoint,
    computer: &NearbyComputer,
    device_nostr: Key,
    label: &str,
    now: u64,
    show: impl FnOnce(Code) + Send,
) -> Result<DeviceOutcome, NearbyError> {
    let connection = endpoint
        .endpoint
        .connect(computer.dial_addr(), ALPN)
        .await
        .map_err(|e| NearbyError::Io(e.to_string()))?;
    let host_endpoint = *connection.remote_id().as_bytes();
    let stream = IrohStream::open(connection).await?;
    let (reader, writer) = tokio::io::split(stream);
    device_session(
        reader,
        writer,
        DeviceKeys {
            endpoint: *endpoint.endpoint.id().as_bytes(),
            nostr: device_nostr,
        },
        host_endpoint,
        label,
        Nonce::random()?,
        now,
        show,
    )
    .await
}

/// What the host puts in its mDNS record: its IP addresses and the label
/// as user data. Relay URLs and any other user data are dropped; the
/// instance name is the `EndpointId`.
#[must_use]
pub fn advertised(data: &EndpointData, label: &str) -> EndpointData {
    let addrs = data
        .ip_addrs()
        .map(|addr| TransportAddr::Ip(*addr))
        .collect::<Vec<_>>();
    let mut out = EndpointData::new(addrs);
    out.set_user_data(label_data(label));
    out
}

fn label_data(label: &str) -> Option<UserData> {
    let label = clean_label(label);
    if label.is_empty() {
        return None;
    }
    UserData::try_from(label).ok()
}

/// The host's mDNS advertisement. Add it to the endpoint with
/// `endpoint.address_lookup()?.add(advertiser)`; it publishes
/// [`advertised`] data only and resolves nothing.
#[derive(Debug)]
pub struct NearbyAdvertiser {
    mdns: MdnsAddressLookup,
    label: String,
}

impl NearbyAdvertiser {
    /// Starts advertising `endpoint` with `label` on the local network.
    /// Needs a Tokio runtime.
    ///
    /// # Errors
    /// Fails when the network allows neither IPv4 nor IPv6 multicast.
    pub fn start(endpoint: EndpointId, label: &str) -> Result<Self, NearbyError> {
        let mdns = MdnsAddressLookup::builder()
            .service_name(SERVICE_NAME)
            .advertise(true)
            .build(endpoint)
            .map_err(|e| NearbyError::Discovery(e.to_string()))?;
        Ok(Self {
            mdns,
            label: clean_label(label),
        })
    }
}

/// Advertises a bound host endpoint on the local network with `label`
/// for as long as the endpoint lives.
///
/// # Errors
/// Fails when mDNS cannot start or the endpoint is closed.
pub fn advertise(endpoint: &ConnectEndpoint, label: &str) -> Result<(), NearbyError> {
    let advertiser = NearbyAdvertiser::start(endpoint.endpoint.id(), label)?;
    endpoint
        .endpoint
        .address_lookup()
        .map_err(|e| NearbyError::Discovery(e.to_string()))?
        .add(advertiser);
    Ok(())
}

impl AddressLookup for NearbyAdvertiser {
    fn publish(&self, data: &EndpointData) {
        self.mdns.publish(&advertised(data, &self.label));
    }
}

/// A computer seen on the local network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NearbyComputer {
    pub endpoint: EndpointId,
    /// The advertised label, cleaned; untrusted and for display only.
    pub label: String,
    pub addrs: Vec<SocketAddr>,
}

impl NearbyComputer {
    /// The address to dial: its advertised IP addresses only.
    #[must_use]
    pub fn dial_addr(&self) -> EndpointAddr {
        EndpointAddr::from_parts(
            self.endpoint,
            self.addrs.iter().map(|addr| TransportAddr::Ip(*addr)),
        )
    }
}

/// A change in the nearby list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NearbyEvent {
    Found(NearbyComputer),
    Lost(EndpointId),
}

/// Maps an mDNS event to a nearby event.
#[must_use]
pub fn nearby_event(event: DiscoveryEvent) -> Option<NearbyEvent> {
    match event {
        DiscoveryEvent::Discovered { endpoint_info, .. } => {
            let addrs = endpoint_info.data.ip_addrs().copied().collect::<Vec<_>>();
            if addrs.is_empty() {
                return None;
            }
            Some(NearbyEvent::Found(NearbyComputer {
                endpoint: endpoint_info.endpoint_id,
                label: endpoint_info
                    .data
                    .user_data()
                    .map(|data| clean_label(data.as_ref()))
                    .unwrap_or_default(),
                addrs,
            }))
        }
        DiscoveryEvent::Expired { endpoint_id } => Some(NearbyEvent::Lost(endpoint_id)),
        _ => None,
    }
}

/// The phone's mDNS listener. It never advertises.
#[derive(Debug, Clone)]
pub struct NearbyBrowser {
    mdns: MdnsAddressLookup,
}

impl NearbyBrowser {
    /// Starts listening for computers. `own` is this device's `EndpointId`,
    /// used by mDNS only as its own instance name. Needs a Tokio runtime.
    ///
    /// # Errors
    /// Fails when the network allows neither IPv4 nor IPv6 multicast.
    pub fn start(own: EndpointId) -> Result<Self, NearbyError> {
        let mdns = MdnsAddressLookup::builder()
            .service_name(SERVICE_NAME)
            .advertise(false)
            .build(own)
            .map_err(|e| NearbyError::Discovery(e.to_string()))?;
        Ok(Self { mdns })
    }

    /// The stream of changes; dropping it stops them.
    pub async fn events(&self) -> impl futures_util::Stream<Item = NearbyEvent> + Unpin + use<> {
        use futures_util::StreamExt;
        self.mdns
            .subscribe()
            .await
            .filter_map(|event| std::future::ready(nearby_event(event)))
    }
}

async fn timed<T>(
    limit: Duration,
    step: impl Future<Output = Result<T, NearbyError>>,
) -> Result<T, NearbyError> {
    tokio::time::timeout(limit, step)
        .await
        .map_err(|_| NearbyError::Timeout)?
}

async fn write_frame<W: AsyncWrite + Unpin, T: Serialize>(
    writer: &mut W,
    message: &T,
    max: usize,
) -> Result<(), NearbyError> {
    Ok(write_message(writer, message, max).await?)
}

async fn read_frame<R: AsyncRead + Unpin, T: DeserializeOwned>(
    reader: &mut R,
    max: usize,
) -> Result<T, NearbyError> {
    read_optional(reader, max)
        .await?
        .ok_or_else(|| NearbyError::Io("the stream finished".into()))
}

/// `None` when the peer finished the stream before a message: how a host
/// says no.
async fn read_optional<R: AsyncRead + Unpin, T: DeserializeOwned>(
    reader: &mut R,
    max: usize,
) -> Result<Option<T>, NearbyError> {
    Ok(read_message(reader, max).await?)
}

/// Lowercase hex of a key.
#[must_use]
pub fn hex(bytes: &Key) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::with_capacity(64), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// Parses 64 lowercase hex digits.
///
/// # Errors
/// Anything else.
pub fn parse_key(text: &str) -> Result<Key, NearbyError> {
    let bad = NearbyError::Protocol("malformed key");
    let bytes = text.as_bytes();
    if bytes.len() != 64 {
        return Err(bad);
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    let mut out = [0; 32];
    for (i, pair) in bytes.chunks_exact(2).enumerate() {
        out[i] = (digit(pair[0]).ok_or(bad.clone())? << 4) | digit(pair[1]).ok_or(bad.clone())?;
    }
    Ok(out)
}

#[cfg(test)]
#[path = "nearby_tests.rs"]
mod tests;
