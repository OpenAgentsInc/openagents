//! Enrollment on the `openagents/enroll/1` ALPN.
//!
//! The device opens one stream, sends one [`EnrollRequest`] holding its
//! signed NIP-HOST `enroll.redeem` request (the same original signed private
//! `3188` artifact the relay path carries), and finishes its side. The host
//! answers with one [`EnrollReply`] and closes. The host runs every NIP-HOST
//! redemption check except the relay binding; this module only carries the
//! bytes and never decides admission.
//!
//! Any iroh key may open this ALPN: an unknown device has nothing else to
//! offer. The capability inside the signed request is what the host checks.

use std::time::Duration;

use iroh::protocol::{AcceptError, ProtocolHandler};
use iroh::{EndpointAddr, EndpointId};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

use crate::endpoint::ConnectEndpoint;
use crate::nearby::{NearbyCall, NearbyRequestMessage};
use crate::stream::IrohStream;
use crate::wire::{read_message, write_message};
use crate::{Code, ENROLL_ALPN, Error, Result, fail};

/// Request version string.
pub const REQUEST: &str = "openagents.connect-enroll-request.v1";
/// Reply version string.
pub const REPLY: &str = "openagents.connect-enroll-reply.v1";
/// Largest request or reply, in bytes of JSON.
pub const MAX_MESSAGE_BYTES: usize = 32 * 1024;
/// How long either side waits for the other.
pub const TIMEOUT: Duration = Duration::from_secs(20);

/// What the device sends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrollRequest {
    pub v: String,
    /// The signed `enroll.redeem` request, as its JSON text.
    pub request: String,
}

impl EnrollRequest {
    #[must_use]
    pub fn new(request: String) -> Self {
        Self {
            v: REQUEST.into(),
            request,
        }
    }
}

/// What the host answers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrollReply {
    pub v: String,
    /// The host's clock, in Unix seconds, so the phone can name a clock
    /// problem (see [`crate::clock_warning`]).
    pub now: u64,
    /// The signed NIP-HOST reply (the grant or a signed refusal), as its
    /// JSON text. `None` when the host sends no signed reply, as for an
    /// unknown invitation or a wrong capability.
    pub reply: Option<String>,
}

impl EnrollReply {
    #[must_use]
    pub fn new(now: u64, reply: Option<String>) -> Self {
        Self {
            v: REPLY.into(),
            now,
            reply,
        }
    }
}

/// Dial `addr` on the enroll ALPN, send `request`, and wait for the reply.
///
/// # Errors
/// `unavailable` for a failed dial, a stream that ends early, or a timeout;
/// `malformed` or `unsupported_version` for a bad reply.
pub async fn redeem(
    endpoint: &ConnectEndpoint,
    addr: impl Into<EndpointAddr>,
    request: &EnrollRequest,
) -> Result<EnrollReply> {
    tokio::time::timeout(TIMEOUT, redeem_inner(endpoint, addr.into(), request))
        .await
        .map_err(|_| Error::new(Code::Unavailable, "enrollment timed out"))?
}

async fn redeem_inner(
    endpoint: &ConnectEndpoint,
    addr: EndpointAddr,
    request: &EnrollRequest,
) -> Result<EnrollReply> {
    let connection = endpoint
        .endpoint
        .connect(addr, ENROLL_ALPN)
        .await
        .map_err(|_| Error::new(Code::Unavailable, "iroh connection failed"))?;
    let mut stream = IrohStream::open(connection)
        .await
        .map_err(|_| Error::new(Code::Unavailable, "could not open a stream"))?;
    write_message(&mut stream, request, MAX_MESSAGE_BYTES).await?;
    let reply: EnrollReply = read_message(&mut stream, MAX_MESSAGE_BYTES)
        .await?
        .ok_or_else(|| Error::new(Code::Unavailable, "host closed without a reply"))?;
    if reply.v != REPLY {
        return fail(Code::UnsupportedVersion, "enroll reply version");
    }
    Ok(reply)
}

/// One enrollment request waiting for the host's answer.
#[derive(Debug)]
pub struct EnrollCall {
    /// The device's iroh key, proven by QUIC. The host stores it with the
    /// grant so it can dial or recognize the device later; it grants
    /// nothing.
    pub remote: EndpointId,
    pub request: EnrollRequest,
    /// Send the answer here. Dropping it closes the stream with no reply.
    pub reply: oneshot::Sender<EnrollReply>,
}

/// A router handler for [`ENROLL_ALPN`]: it reads one request per
/// connection, hands it to the host as an [`EnrollCall`], and writes the
/// answer back. With [`Self::with_nearby`], a first message whose `v` is
/// the nearby request goes to the host as a [`NearbyCall`] instead.
pub struct EnrollProtocol {
    calls: mpsc::Sender<EnrollCall>,
    nearby: Option<mpsc::Sender<NearbyCall>>,
}

impl EnrollProtocol {
    #[must_use]
    pub fn new(calls: mpsc::Sender<EnrollCall>) -> Self {
        Self {
            calls,
            nearby: None,
        }
    }

    /// Also serve nearby approval on this ALPN (NIP-HOST).
    #[must_use]
    pub fn with_nearby(mut self, nearby: mpsc::Sender<NearbyCall>) -> Self {
        self.nearby = Some(nearby);
        self
    }
}

impl std::fmt::Debug for EnrollProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnrollProtocol").finish_non_exhaustive()
    }
}

impl ProtocolHandler for EnrollProtocol {
    async fn accept(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> std::result::Result<(), AcceptError> {
        let remote = connection.remote_id();
        let first = tokio::time::timeout(TIMEOUT, async {
            let mut stream = IrohStream::accept(connection)
                .await
                .map_err(|_| Error::new(Code::Unavailable, "no stream arrived"))?;
            let first: serde_json::Value = read_message(&mut stream, MAX_MESSAGE_BYTES)
                .await?
                .ok_or_else(|| Error::new(Code::Malformed, "no request"))?;
            Ok((stream, first))
        })
        .await
        .unwrap_or_else(|_| fail(Code::Unavailable, "enrollment timed out"));
        let (mut stream, first) = first.map_err(AcceptError::from_err)?;
        if let Some(nearby) = &self.nearby
            && first.get("v").and_then(serde_json::Value::as_str) == Some(crate::nearby::REQUEST)
        {
            return serve_nearby(nearby, remote, first, stream)
                .await
                .map_err(AcceptError::from_err);
        }
        let answer = tokio::time::timeout(TIMEOUT, async {
            let request: EnrollRequest = serde_json::from_value(first)
                .map_err(|_| Error::new(Code::Malformed, "message is not the expected JSON"))?;
            if request.v != REQUEST {
                return fail(Code::UnsupportedVersion, "enroll request version");
            }
            let (reply, answer) = oneshot::channel();
            self.calls
                .send(EnrollCall {
                    remote,
                    request,
                    reply,
                })
                .await
                .map_err(|_| Error::new(Code::Unavailable, "host stopped taking requests"))?;
            let reply = answer
                .await
                .map_err(|_| Error::new(Code::Unavailable, "host sent no answer"))?;
            write_message(&mut stream, &reply, MAX_MESSAGE_BYTES).await
        })
        .await
        .unwrap_or_else(|_| fail(Code::Unavailable, "enrollment timed out"));
        answer.map_err(AcceptError::from_err)
    }
}

/// Hands a nearby request to the host and keeps the connection open until
/// the host finishes with it.
async fn serve_nearby(
    nearby: &mpsc::Sender<NearbyCall>,
    remote: EndpointId,
    first: serde_json::Value,
    stream: IrohStream,
) -> Result<()> {
    let request = NearbyRequestMessage::from_value(first)
        .map_err(|_| Error::new(Code::Malformed, "nearby request"))?;
    let (done, finished) = oneshot::channel();
    nearby
        .send(NearbyCall {
            remote,
            request,
            stream,
            done,
        })
        .await
        .map_err(|_| Error::new(Code::Unavailable, "host stopped taking nearby requests"))?;
    let _ = tokio::time::timeout(crate::nearby::SESSION_LIMIT, finished).await;
    Ok(())
}
