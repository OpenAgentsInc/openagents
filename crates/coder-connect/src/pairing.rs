//! Short-lived computer invitations. Only the intended temporary capability is
//! displayed; device and host identity secrets stay in their protected stores.
use crate::{Error, ErrorCode, Result, fail, protocol::*, unix_time};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use nostr::domain::Event;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const PREFIX: &str = "coder-pair:";
pub const MAX_CODE_BYTES: usize = 640;
pub const LIFETIME: u64 = 300;
pub const REQUEST: &str = "openagents.history-observer-pair-request.v1";
pub const REPLY: &str = "openagents.history-observer-pair-reply.v1";

/// This contains a temporary capability: do not log or derive Debug for it.
pub struct Invitation {
    pub host: String,
    pub id: String,
    pub relay: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub(crate) capability: String,
}
impl Invitation {
    pub fn parse(code: &str, now: u64, policy: RelayPolicy) -> Result<Self> {
        if code.len() > MAX_CODE_BYTES {
            return fail(
                ErrorCode::Bounds,
                "pairing invitation exceeds its byte bound",
            );
        }
        let encoded = code
            .strip_prefix(PREFIX)
            .ok_or_else(|| Error::new(ErrorCode::Malformed, "not a computer pairing invitation"))?;
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| Error::new(ErrorCode::Malformed, "invalid pairing invitation encoding"))?;
        if bytes.len() < 115 || bytes[0] != 1 || URL_SAFE_NO_PAD.encode(&bytes) != encoded {
            return fail(
                ErrorCode::Malformed,
                "unsupported pairing invitation layout",
            );
        }
        let relay_len = u16::from_be_bytes([bytes[113], bytes[114]]) as usize;
        if relay_len == 0 || relay_len > 256 || bytes.len() != 115 + relay_len {
            return fail(ErrorCode::Bounds, "pairing relay exceeds its bounds");
        }
        let invitation = Self {
            host: hex(&bytes[1..33]),
            id: hex(&bytes[33..65]),
            capability: hex(&bytes[65..97]),
            issued_at: u64::from_be_bytes(bytes[97..105].try_into().expect("fixed slice")),
            expires_at: u64::from_be_bytes(bytes[105..113].try_into().expect("fixed slice")),
            relay: std::str::from_utf8(&bytes[115..])
                .map_err(|_| Error::new(ErrorCode::Malformed, "pairing relay is not UTF-8"))?
                .into(),
        };
        invitation.validate(now, policy)?;
        Ok(invitation)
    }
    pub(crate) fn validate(&self, now: u64, policy: RelayPolicy) -> Result<()> {
        public(&self.host)?;
        identity(&self.id)?;
        identity(&self.capability)?;
        policy.validate(&self.relay)?;
        window(self.issued_at, self.expires_at, LIFETIME)?;
        if self.expires_at - self.issued_at != LIFETIME || self.relay.len() > 256 {
            return fail(
                ErrorCode::Malformed,
                "pairing invitation lifetime or relay differs",
            );
        }
        fresh(self.issued_at, self.expires_at, now)
    }
    #[cfg(feature = "host")]
    pub(crate) fn encode(&self) -> Result<String> {
        let mut bytes = vec![1];
        bytes.extend(unhex(&self.host)?);
        bytes.extend(unhex(&self.id)?);
        bytes.extend(unhex(&self.capability)?);
        bytes.extend(self.issued_at.to_be_bytes());
        bytes.extend(self.expires_at.to_be_bytes());
        bytes.extend((self.relay.len() as u16).to_be_bytes());
        bytes.extend(self.relay.as_bytes());
        let code = format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes));
        if code.len() > MAX_CODE_BYTES {
            return fail(
                ErrorCode::Bounds,
                "pairing invitation exceeds its byte bound",
            );
        }
        Ok(code)
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
#[cfg(feature = "host")]
fn unhex(text: &str) -> Result<[u8; 32]> {
    identity(text)?;
    let mut result = [0; 32];
    for (index, chunk) in text.as_bytes().chunks_exact(2).enumerate() {
        let digit = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
        result[index] = digit(chunk[0]) * 16 + digit(chunk[1]);
    }
    Ok(result)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub invitation: String,
    pub capability: String,
    pub relay: String,
    pub issued_at: u64,
    pub expires_at: u64,
}
impl Request {
    pub(crate) fn validate(&self, policy: RelayPolicy) -> Result<()> {
        schema(&self.v, REQUEST, &self.requires)?;
        for id in [&self.request, &self.invitation, &self.capability] {
            identity(id)?;
        }
        policy.validate(&self.relay)?;
        window(self.issued_at, self.expires_at, MAX_REQUEST_LIFETIME)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Outcome {
    Ok { connection: Box<ConnectionCode> },
    Refused { code: ErrorCode },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Reply {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub request_event: String,
    pub invitation: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub result: Outcome,
}
pub(crate) struct Pending {
    pub request: Request,
    pub event: Event,
}
pub(crate) fn prepare(
    invitation: &Invitation,
    secret: &SecretKey,
    now: u64,
    policy: RelayPolicy,
) -> Result<Pending> {
    invitation.validate(now, policy)?;
    if pubkey(secret) == invitation.host {
        return fail(
            ErrorCode::Forbidden,
            "phone and computer identities must differ",
        );
    }
    let request = Request {
        v: REQUEST.into(),
        requires: vec![],
        request: random_id(),
        invitation: invitation.id.clone(),
        capability: invitation.capability.clone(),
        relay: invitation.relay.clone(),
        issued_at: now,
        expires_at: (now + MAX_REQUEST_LIFETIME).min(invitation.expires_at),
    };
    request.validate(policy)?;
    let event = seal(
        &request,
        REQUEST,
        secret,
        &invitation.host,
        &request.request,
        now,
        request.expires_at,
    )?;
    Ok(Pending { request, event })
}
pub(crate) fn verify(
    invitation: &Invitation,
    pending: &Pending,
    event: &Event,
    secret: &SecretKey,
    now: u64,
    policy: RelayPolicy,
) -> Result<ConnectionCode> {
    invitation.validate(now, policy)?;
    let reply: Reply = open(event, secret, &invitation.host, &pubkey(secret), REPLY)?;
    schema(&reply.v, REPLY, &reply.requires)?;
    fresh(reply.issued_at, reply.expires_at, now)?;
    if reply.request != pending.request.request
        || reply.request_event != pending.event.id
        || reply.invitation != invitation.id
        || reply.issued_at < pending.request.issued_at
        || reply.expires_at != pending.request.expires_at
        || event.tag_values("h").collect::<Vec<_>>() != [pending.request.request.as_str()]
    {
        return fail(
            ErrorCode::Forbidden,
            "pairing reply does not match this request",
        );
    }
    match reply.result {
        Outcome::Refused { code } => fail(code, "computer refused this pairing invitation"),
        Outcome::Ok { connection } => {
            connection.verify(secret, now, policy)?;
            if connection.host != invitation.host || connection.relay != invitation.relay {
                return fail(
                    ErrorCode::Forbidden,
                    "pairing grant differs from the scanned computer",
                );
            }
            Ok(*connection)
        }
    }
}
/// Redeem a computer invitation with the phone's protected identity. The caller
/// saves the returned connection only after success; errors change no local state.
/// Cancellation drops the finite socket and a retry uses the same device key.
pub async fn redeem(code: &str, secret: &SecretKey, policy: RelayPolicy) -> Result<ConnectionCode> {
    let invitation = Invitation::parse(code, unix_time()?, policy)?;
    let pending = prepare(&invitation, secret, unix_time()?, policy)?;
    let event = tokio::time::timeout(Duration::from_secs(12), async {
        let mut session =
            crate::transport::Session::connect(&invitation.relay, secret, policy).await?;
        session
            .exchange_event(
                &pending.event,
                &pending.request.request,
                (pending.request.issued_at, pending.request.expires_at),
                &invitation.host,
                &pubkey(secret),
            )
            .await
    })
    .await
    .map_err(|_| Error::new(ErrorCode::Transport, "computer pairing timed out"))??;
    verify(&invitation, &pending, &event, secret, unix_time()?, policy)
}

/// Render locally. The code is never sent to a QR-generation service.
#[cfg(feature = "host")]
pub fn qr_svg(code: &str) -> Result<String> {
    let qr = qr(code)?;
    let side = qr.size() + 8;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {side} {side}\" width=\"600\" height=\"600\" shape-rendering=\"crispEdges\"><rect width=\"100%\" height=\"100%\" fill=\"white\"/><path fill=\"black\" d=\""
    );
    for y in 0..qr.size() {
        for x in 0..qr.size() {
            if qr.get_module(x, y) {
                svg.push_str(&format!("M{},{}h1v1h-1z ", x + 4, y + 4));
            }
        }
    }
    svg.push_str("\"/></svg>");
    Ok(svg)
}
#[cfg(feature = "host")]
pub fn terminal_qr(code: &str) -> Result<String> {
    let qr = qr(code)?;
    let side = qr.size() + 8;
    let dark = |x, y| qr.get_module(x - 4, y - 4);
    let mut output = String::new();
    for y in (0..side).step_by(2) {
        output.push_str("\x1b[30;47m");
        for x in 0..side {
            output.push(match (dark(x, y), dark(x, y + 1)) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        output.push_str("\x1b[0m\n");
    }
    Ok(output)
}
#[cfg(feature = "host")]
fn qr(code: &str) -> Result<qrcodegen::QrCode> {
    if !code.starts_with(PREFIX)
        || code.len() > MAX_CODE_BYTES
        || !code.is_ascii()
        || !code[PREFIX.len()..]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return fail(
            ErrorCode::Malformed,
            "invalid invitation for local QR rendering",
        );
    }
    qrcodegen::QrCode::encode_text(code, qrcodegen::QrCodeEcc::Medium).map_err(|_| {
        Error::new(
            ErrorCode::Bounds,
            "pairing invitation cannot fit in a QR code",
        )
    })
}
