//! Nearby pairing on the host: the gate that holds at most one nearby
//! request, admits at most five in ten minutes, shows the confirmation
//! code to the person at the computer (`DSK-04`), and mints an invitation
//! only when they click **Connect**.
//!
//! The exchange itself is `openagents_connect::nearby`; this module is its
//! [`Admission`]. Only a click reaches [`Mint`], which signs the NIP-HOST
//! nearby-approval grant (`coder_access::host::Host::approve_nearby`) for
//! the device key the exchange named, with the connect-code rights
//! (`observe,operate`, plus `terminal` when the checkbox was set).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use coder_access::Rights;
use openagents_connect::nearby::{
    self, Admission, Code, HostKeys, HostOutcome, NearbyError, NearbyRequest, NearbyRequestMessage,
    Nonce, Refusal, Ticket, Verdict,
};
use tokio::sync::{oneshot, watch};
use tokio::time::Instant;

/// Nearby requests admitted per [`WINDOW`].
pub const MAX_PER_WINDOW: usize = 5;
/// The rate limit's sliding window.
pub const WINDOW: Duration = Duration::from_secs(10 * 60);

/// Signs the grant for an approved nearby device (its Nostr key, lowercase
/// hex) with these rights and returns the grant envelope. Called only
/// after a click on **Connect**.
pub type Mint = Arc<dyn Fn(&str, Rights) -> Result<serde_json::Value, String> + Send + Sync>;

/// The rights a nearby pairing carries: those of a desktop QR code.
///
/// # Panics
/// Never: the lists are constant.
#[must_use]
pub fn nearby_rights(terminal: bool) -> Rights {
    let list = if terminal {
        "observe,operate,terminal"
    } else {
        "observe,operate"
    };
    Rights::parse_list(list).expect("constant rights parse")
}

/// What the computer shows while a nearby request waits for a click.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    /// Identifies this request in [`NearbyGate::decide`].
    pub id: u64,
    /// The phone's label, cleaned; the phone chose it, so it is a name to
    /// show, never an identity.
    pub label: String,
    /// The code both screens must show.
    pub code: Code,
}

/// The person's answer on `DSK-04`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// **Connect**, with the terminal checkbox's state.
    Connect { terminal: bool },
    /// **Don't connect**.
    Decline,
}

/// Why [`NearbyGate::decide`] did nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecideError {
    /// No request with that ID is waiting for a click: it was answered,
    /// withdrawn, or expired, or its code was never shown.
    NotPending,
}

struct Slot {
    id: u64,
    device: String,
    label: String,
    code: Option<Code>,
    answer: Option<oneshot::Sender<Choice>>,
}

struct State {
    next: u64,
    slot: Option<Slot>,
    admitted: VecDeque<Instant>,
}

struct Inner {
    state: Mutex<State>,
    shown: watch::Sender<Option<Pending>>,
    mint: Mint,
}

/// The host's nearby gate. Clones share one gate.
#[derive(Clone)]
pub struct NearbyGate {
    inner: Arc<Inner>,
}

impl NearbyGate {
    /// A gate whose clicks mint invitations with `mint`.
    #[must_use]
    pub fn new(mint: Mint) -> Self {
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(State {
                    next: 1,
                    slot: None,
                    admitted: VecDeque::new(),
                }),
                shown: watch::channel(None).0,
                mint,
            }),
        }
    }

    /// The request waiting for a click, if any.
    #[must_use]
    pub fn pending(&self) -> Option<Pending> {
        self.inner.shown.borrow().clone()
    }

    /// Follows [`Self::pending`].
    #[cfg(test)]
    #[must_use]
    pub fn watch(&self) -> watch::Receiver<Option<Pending>> {
        self.inner.shown.subscribe()
    }

    /// Answers the pending request `id`. Only the local owner surface (the
    /// desktop app over the control socket) calls this; a device cannot.
    ///
    /// # Errors
    /// [`DecideError::NotPending`] when `id` is not waiting for a click.
    pub fn decide(&self, id: u64, choice: Choice) -> Result<(), DecideError> {
        let answer = {
            let mut state = self.lock();
            let slot = state
                .slot
                .as_mut()
                .filter(|slot| slot.id == id && slot.code.is_some())
                .ok_or(DecideError::NotPending)?;
            slot.answer.take().ok_or(DecideError::NotPending)?
        };
        // The prompt leaves the screen with the click, before the exchange
        // finishes.
        self.inner.shown.send_replace(None);
        answer.send(choice).map_err(|_| DecideError::NotPending)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn admit_at(&self, request: &NearbyRequest, now: Instant) -> Result<GateTicket, Refusal> {
        let mut state = self.lock();
        while state
            .admitted
            .front()
            .is_some_and(|at| now.duration_since(*at) >= WINDOW)
        {
            state.admitted.pop_front();
        }
        if state.slot.is_some() {
            return Err(Refusal::Busy);
        }
        if state.admitted.len() >= MAX_PER_WINDOW {
            return Err(Refusal::Limited);
        }
        state.admitted.push_back(now);
        let id = state.next;
        state.next += 1;
        state.slot = Some(Slot {
            id,
            device: nearby::hex(&request.device_nostr),
            label: request.label.clone(),
            code: None,
            answer: None,
        });
        Ok(GateTicket {
            gate: self.clone(),
            id,
        })
    }

    fn clear(&self, id: u64) {
        let mut state = self.lock();
        if state.slot.as_ref().is_some_and(|slot| slot.id == id) {
            state.slot = None;
            drop(state);
            self.inner.shown.send_replace(None);
        }
    }
}

impl Admission for NearbyGate {
    type Ticket = GateTicket;
    fn admit(&self, request: &NearbyRequest) -> Result<GateTicket, Refusal> {
        self.admit_at(request, Instant::now())
    }
}

/// The one admitted request. Dropping it frees the gate.
pub struct GateTicket {
    gate: NearbyGate,
    id: u64,
}

impl Drop for GateTicket {
    fn drop(&mut self) {
        self.gate.clear(self.id);
    }
}

impl Ticket for GateTicket {
    async fn decide(self, code: Code) -> Verdict {
        let (answer, choice) = oneshot::channel();
        let (shown, device) = {
            let mut state = self.gate.lock();
            let Some(slot) = state.slot.as_mut().filter(|slot| slot.id == self.id) else {
                return Verdict::Expired;
            };
            slot.code = Some(code);
            slot.answer = Some(answer);
            let shown = Pending {
                id: self.id,
                label: slot.label.clone(),
                code,
            };
            (shown, slot.device.clone())
        };
        self.gate.inner.shown.send_replace(Some(shown));
        match choice.await {
            Ok(Choice::Connect { terminal }) => {
                match (self.gate.inner.mint)(&device, nearby_rights(terminal)) {
                    Ok(event) => Verdict::Connect { event },
                    Err(_) => Verdict::Decline,
                }
            }
            Ok(Choice::Decline) => Verdict::Decline,
            Err(_) => Verdict::Expired,
        }
    }
}

/// Serves a nearby exchange on the enroll ALPN after the enroll handler
/// read its first message and found `v` to be the nearby request. The
/// device's `EndpointId` comes from the connection, never from a message.
///
/// # Errors
/// See [`nearby::host_session_after`].
pub async fn serve<R, W>(
    request: NearbyRequestMessage,
    reader: R,
    writer: W,
    host: HostKeys,
    device_endpoint: nearby::Key,
    gate: &NearbyGate,
) -> Result<HostOutcome, NearbyError>
where
    R: tokio::io::AsyncRead + Unpin + Send,
    W: tokio::io::AsyncWrite + Unpin + Send,
{
    let now = crate::unix_time().map_err(|_| NearbyError::Protocol("no clock"))?;
    nearby::host_session_after(
        request,
        reader,
        writer,
        host,
        device_endpoint,
        Nonce::random()?,
        now,
        gate,
    )
    .await
}

#[cfg(test)]
#[path = "nearby_tests.rs"]
mod tests;
