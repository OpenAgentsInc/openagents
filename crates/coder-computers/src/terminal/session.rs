//! One terminal on a linked host, driven over the host's current link.
//!
//! [`Session::start`] spawns a task that asks the host to open a shell with
//! NIP-HOST `terminal.open`, then attaches with NIP-TERM in `interact` mode
//! and applies the attachment's frames in sequence order to the model's
//! emulator. Input, resize, and close travel as NIP-TERM requests on the same
//! link. The task never queues input for later: while the link is down,
//! typed bytes are refused on the screen rather than held.
//!
//! When the link drops or the supervisor replaces it, the task attaches
//! again with `after` set to the last frame it applied, so the host replays
//! what the screen missed or reports a gap. A frame that arrives ahead of
//! the next expected one waits in [`Ordered`]; if the missing frame does
//! not arrive soon, the task reattaches to repair it. A host that restarted
//! answers `lost`, which ends the session, except for a terminal the host
//! opened just now: then the link named an older generation (a relay route
//! outlives a host restart), so the task reads the host's generation from
//! fresh presence and attaches again.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use coder_access::protocol::{Operation, Outcome};
use coder_access::{Code, Error as AccessError};
use coder_host::Error as HostError;
use coder_host::client::{Link, Ordered, Route};
use coder_host::mailbox::terminal_generation;
use coder_host::message::TermRequest;
use coder_host::pty::client::{Applied, TerminalState};
use coder_host::pty::wire::{
    Attach, Body, Cause, Close, Detach, Detached, Frame, Input, Mode, Reason, Resize, Size, Status,
    TerminalRef, TerminalResult, Value,
};
use coder_host::reach::new_id;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::time::Instant;

use super::model::{Model, Phase};
use crate::controller::describe;

/// The current link to a host, as the Computers service's supervisor holds
/// it. It answers a transport error while the host is not connected.
pub type Links = Arc<dyn Fn() -> Result<Arc<Link>, AccessError> + Send + Sync>;

/// Output bytes per second asked for over a direct channel.
const DIRECT_RATE: u64 = 64 * 1024;
/// Output bytes per second asked for over the relay.
const RELAY_RATE: u64 = 16 * 1024;
/// The most input bytes one request carries.
const INPUT_CHUNK: usize = 4096;
/// How long a frame may wait for an earlier one before the task reattaches.
const HOLD_LIMIT: Duration = Duration::from_secs(2);
/// How often the task checks whether the supervisor replaced the link.
const LINK_CHECK: Duration = Duration::from_secs(2);
/// How long to wait before trying an unreachable host again.
const RETRY: Duration = Duration::from_secs(1);
/// How many times a session reads presence for a restarted host's new
/// generation before it reports its new terminal lost.
const GENERATION_TRIES: u32 = 10;

enum Command {
    Bytes(Vec<u8>),
    Resize(u16, u16),
    Close,
    Leave,
}

/// A running terminal session. Dropping it detaches.
pub struct Session {
    model: Arc<Mutex<Model>>,
    commands: mpsc::UnboundedSender<Command>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session").finish_non_exhaustive()
    }
}

impl Session {
    /// Open a terminal on the host `model` names, over the links `links`
    /// returns, on `runtime`.
    #[must_use]
    pub fn start(runtime: &Handle, links: Links, model: Model) -> Self {
        let model = Arc::new(Mutex::new(model));
        let (commands, receiver) = mpsc::unbounded_channel();
        runtime.spawn(run(links, model.clone(), receiver));
        Session { model, commands }
    }

    /// The screen's state.
    pub fn model(&self) -> MutexGuard<'_, Model> {
        lock(&self.model)
    }

    /// Send typed bytes. Refused on the screen, not queued, unless the
    /// session is attached.
    pub fn send(&self, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }
        {
            let mut model = self.model();
            if model.phase != Phase::Attached {
                model.notice = Some("Not connected. What you typed wasn't sent.".into());
                model.touch();
                return;
            }
        }
        let _ = self.commands.send(Command::Bytes(bytes));
    }

    /// Resize the grid and tell the host.
    pub fn resize(&self, rows: u16, cols: u16) {
        let changed = self.model().resize(rows, cols);
        if let Some((rows, cols)) = changed {
            let _ = self.commands.send(Command::Resize(rows, cols));
        }
    }

    /// End the shell on the host.
    pub fn close(&self) {
        let _ = self.commands.send(Command::Close);
    }

    /// Detach. The shell keeps running on the host.
    pub fn leave(&self) {
        let _ = self.commands.send(Command::Leave);
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.leave();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Why opening or attaching stopped.
enum Stop {
    /// The person left.
    Left,
    /// The session ended in this phase.
    Ended(Phase),
}

fn route(link: &Link) -> String {
    match link.route() {
        Route::Direct(_) => "directly".into(),
        Route::Relay(_) => "through the relay".into(),
    }
}

fn rate(link: &Link) -> u64 {
    match link.route() {
        Route::Direct(_) => DIRECT_RATE,
        Route::Relay(_) => RELAY_RATE,
    }
}

/// The phase a NIP-HOST refusal of `terminal.open` ends in, or `None` to
/// try again.
fn open_refusal(error: &HostError) -> Option<Phase> {
    match error {
        HostError::Access(error) => match error.code {
            Code::Transport => None,
            Code::Unsupported => Some(Phase::Refused(
                "The computer's host serves no workspace, so it can't open a terminal. Add one with `host serve --workspace LABEL=PATH`.".into(),
            )),
            Code::Unavailable => Some(Phase::Refused(
                "The computer can't open a terminal now. Its host's workspace directory is missing or unusable; check the host log.".into(),
            )),
            _ => Some(Phase::Refused(describe(error))),
        },
        HostError::Reach(_) | HostError::Config(_) => Some(Phase::Refused(
            "The computer couldn't open a terminal.".into(),
        )),
        _ => None,
    }
}

/// The phase a refused attach ends in.
fn attach_refusal(reason: Option<Reason>) -> Phase {
    match reason {
        Some(Reason::Lost) => Phase::Lost,
        Some(Reason::Closed | Reason::Unavailable) => Phase::Closed,
        Some(Reason::NotAdmitted | Reason::Revoked) => Phase::Refused(
            "The computer refused: This device doesn't have the \"Open terminals\" right on this computer."
                .into(),
        ),
        _ => Phase::Refused("The computer refused to attach this terminal.".into()),
    }
}

/// Wait for a usable link, or for the person to leave.
async fn wait_link(
    links: &Links,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    first: bool,
) -> Result<Arc<Link>, Stop> {
    let mut delay = !first;
    loop {
        if delay {
            tokio::select! {
                command = commands.recv() => match command {
                    None | Some(Command::Leave) => return Err(Stop::Left),
                    // Nothing is queued while disconnected.
                    Some(_) => continue,
                },
                () = tokio::time::sleep(RETRY) => {}
            }
        }
        delay = true;
        match links() {
            Ok(link) if link.closed().is_none() => return Ok(link),
            Ok(_) => {}
            Err(error) if matches!(error.code, Code::Transport | Code::Unavailable) => {}
            Err(error) => return Err(Stop::Ended(Phase::Refused(describe(&error)))),
        }
    }
}

/// Wait for `work`, or stop when the person leaves. Other commands that
/// arrive meanwhile are dropped: nothing typed is queued while the session
/// is not attached, and the model already holds the latest size.
async fn until_left<T>(
    commands: &mut mpsc::UnboundedReceiver<Command>,
    work: impl Future<Output = T>,
) -> Result<T, Stop> {
    tokio::pin!(work);
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                None | Some(Command::Leave) => return Err(Stop::Left),
                Some(_) => {}
            },
            done = &mut work => return Ok(done),
        }
    }
}

/// Ask the host to open a shell. Returns the link, the terminal, and the
/// size it opened at.
async fn open(
    links: &Links,
    model: &Arc<Mutex<Model>>,
    commands: &mut mpsc::UnboundedReceiver<Command>,
) -> Result<(Arc<Link>, TerminalRef, (u16, u16)), Stop> {
    let mut first = true;
    loop {
        lock(model).set_phase(Phase::Connecting);
        let link = wait_link(links, commands, first).await?;
        first = false;
        let Some(generation) = link.generation() else {
            continue;
        };
        let (rows, cols) = {
            let mut model = lock(model);
            model.set_phase(Phase::Opening);
            model.size()
        };
        let answer =
            until_left(commands, link.call(Operation::OpenTerminal { cols, rows })).await?;
        match answer {
            Ok(Outcome::Dispatched { receipt }) => {
                let reference = TerminalRef {
                    generation: terminal_generation(link.device().host(), generation),
                    terminal: receipt.reference,
                };
                return Ok((link, reference, (rows, cols)));
            }
            Ok(_) => {
                return Err(Stop::Ended(Phase::Refused(
                    "The computer answered with something other than a terminal.".into(),
                )));
            }
            Err(error) => {
                if let Some(phase) = open_refusal(&error) {
                    return Err(Stop::Ended(phase));
                }
            }
        }
    }
}

async fn run(
    links: Links,
    model: Arc<Mutex<Model>>,
    mut commands: mpsc::UnboundedReceiver<Command>,
) {
    let ended = match drive(&links, &model, &mut commands).await {
        Stop::Left => Phase::Left,
        Stop::Ended(phase) => phase,
    };
    let mut model = lock(&model);
    // An exit the host reported stays on the screen after leaving.
    if !(matches!(model.phase, Phase::Exited { .. }) && ended == Phase::Left) {
        model.set_phase(ended);
    }
}

/// What the attached loop decided.
enum Next {
    /// Attach again after the last applied frame, on this link or a new one.
    Reattach {
        new_link: bool,
    },
    Stop(Stop),
}

async fn drive(
    links: &Links,
    model: &Arc<Mutex<Model>>,
    commands: &mut mpsc::UnboundedReceiver<Command>,
) -> Stop {
    let (mut link, mut reference, mut host_size) = match open(links, model, commands).await {
        Ok(opened) => opened,
        Err(stop) => return stop,
    };
    // The client state tracks sequence numbers and gaps; the emulator in
    // the model draws the output.
    let mut ordered = Ordered::new(TerminalState::new(reference.clone(), 1, 1));
    let mut exited = false;
    let mut first = true;
    // Whether an attach has succeeded, and whether a `lost` refusal of the
    // first one sent the session to fresh presence for the generation.
    let mut attached_once = false;
    let mut rechecked = false;
    loop {
        if !first {
            lock(model).set_phase(Phase::Reconnecting);
        }
        let after = ordered.state().resume_after();
        let attach = TermRequest::Attach(Attach::new(
            new_id(),
            reference.clone(),
            Mode::Interact,
            after,
            rate(&link),
        ));
        let attached = match until_left(commands, link.terminal(attach)).await {
            Ok(attached) => attached,
            Err(stop) => return stop,
        };
        let attachment = match attached {
            Ok(TerminalResult {
                status: Status::Accepted | Status::Duplicate,
                value: Some(Value::Attached { attachment, .. }),
                ..
            }) => attachment,
            Ok(result) if result.status == Status::Refused => {
                // A terminal that already reported its exit stays exited.
                if exited && result.reason == Some(Reason::Closed) {
                    return Stop::Ended(lock(model).phase.clone());
                }
                if !attached_once {
                    if result.reason == Some(Reason::Lost) && !rechecked {
                        // The host opened this terminal just now, on its
                        // current generation, so the link named an older
                        // one: a relay route that outlived a host restart.
                        // Follow fresh presence and attach again.
                        rechecked = true;
                        match until_left(commands, newer_reference(&link, &reference)).await {
                            Err(stop) => return stop,
                            Ok(Some(fresh)) => {
                                reference = fresh;
                                ordered = Ordered::new(TerminalState::new(reference.clone(), 1, 1));
                                continue;
                            }
                            Ok(None) => {}
                        }
                    }
                    // The host restarted again between opening and
                    // attaching: the terminal did not survive it.
                    if rechecked
                        && matches!(result.reason, Some(Reason::Closed | Reason::Unavailable))
                    {
                        return Stop::Ended(Phase::Lost);
                    }
                }
                return Stop::Ended(attach_refusal(result.reason));
            }
            _ => {
                link = match wait_link(links, commands, false).await {
                    Ok(link) => link,
                    Err(stop) => return stop,
                };
                first = false;
                continue;
            }
        };
        attached_once = true;
        {
            let mut model = lock(model);
            model.route = Some(route(&link));
            if !exited {
                model.set_phase(Phase::Attached);
            }
            model.touch();
        }
        // The screen may have changed size while opening or detached.
        let size = lock(model).size();
        if size != host_size && !exited {
            let _ = request(&link, resize(&reference, size.0, size.1)).await;
            host_size = size;
        }
        first = false;
        let next = attached_loop(
            links,
            model,
            commands,
            &link,
            &reference,
            &attachment,
            &mut ordered,
            &mut exited,
            &mut host_size,
        )
        .await;
        match next {
            Next::Stop(stop) => {
                if matches!(stop, Stop::Left) {
                    detach(&link, &reference, &attachment).await;
                }
                return stop;
            }
            Next::Reattach { new_link } => {
                let old = link.clone();
                let old_attachment = attachment.clone();
                let old_reference = reference.clone();
                // End the old attachment if its route still works.
                tokio::spawn(async move {
                    let _ = tokio::time::timeout(
                        Duration::from_secs(3),
                        detach(&old, &old_reference, &old_attachment),
                    )
                    .await;
                });
                if new_link {
                    link = match wait_link(links, commands, false).await {
                        Ok(link) => link,
                        Err(stop) => return stop,
                    };
                }
            }
        }
    }
}

/// The reference to `reference`'s terminal on the host generation fresh
/// presence names, once it differs from the one `reference` names; `None`
/// when it does not change soon, or the link is a direct channel, whose
/// handshake proved its generation.
async fn newer_reference(link: &Link, reference: &TerminalRef) -> Option<TerminalRef> {
    if !matches!(link.route(), Route::Relay(_)) {
        return None;
    }
    for attempt in 0..GENERATION_TRIES {
        if attempt > 0 {
            tokio::time::sleep(RETRY).await;
        }
        if let Ok(Some(generation)) = link.refresh_generation().await {
            let generation = terminal_generation(link.device().host(), generation);
            if generation != reference.generation {
                return Some(TerminalRef {
                    generation,
                    terminal: reference.terminal.clone(),
                });
            }
        }
    }
    None
}

fn resize(reference: &TerminalRef, rows: u16, cols: u16) -> TermRequest {
    TermRequest::Resize(Resize::new(
        new_id(),
        reference.clone(),
        Size::new(rows, cols),
    ))
}

async fn request(link: &Link, request: TermRequest) -> Result<TerminalResult, HostError> {
    link.terminal(request).await
}

async fn detach(link: &Link, reference: &TerminalRef, attachment: &str) {
    let _ = tokio::time::timeout(
        Duration::from_secs(3),
        link.terminal(TermRequest::Detach(Detach::new(
            new_id(),
            reference.clone(),
            attachment,
        ))),
    )
    .await;
}

#[allow(clippy::too_many_arguments)]
async fn attached_loop(
    links: &Links,
    model: &Arc<Mutex<Model>>,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    link: &Arc<Link>,
    reference: &TerminalRef,
    attachment: &str,
    ordered: &mut Ordered,
    exited: &mut bool,
    host_size: &mut (u16, u16),
) -> Next {
    let mut held_since: Option<Instant> = None;
    let mut checked = Instant::now();
    loop {
        tokio::select! {
            command = commands.recv() => {
                let command = command.unwrap_or(Command::Leave);
                if let Some(next) = handle(command, model, link, reference, *exited, host_size).await {
                    return next;
                }
            }
            frame = link.next_frame(Duration::from_millis(250)) => {
                if let Some(frame) = frame
                    && frame.terminal == *reference
                    && frame.attachment == attachment
                {
                    let (replies, next) = apply(model, ordered, frame, exited);
                    if !replies.is_empty() && !*exited {
                        send_input(link, reference, model, replies).await;
                    }
                    if let Some(next) = next {
                        return next;
                    }
                }
            }
        }
        if link.closed().is_some() {
            return Next::Reattach { new_link: true };
        }
        // A frame waits for an earlier one that did not arrive: repair.
        if ordered.held() > 0 {
            let since = *held_since.get_or_insert_with(Instant::now);
            if since.elapsed() > HOLD_LIMIT {
                return Next::Reattach { new_link: false };
            }
        } else {
            held_since = None;
        }
        // The supervisor may have replaced the route, for example with a
        // direct channel after the relay.
        if checked.elapsed() > LINK_CHECK {
            checked = Instant::now();
            if let Ok(current) = links()
                && !Arc::ptr_eq(&current, link)
                && current.closed().is_none()
                && !*exited
            {
                return Next::Reattach { new_link: true };
            }
        }
    }
}

/// Apply one frame and every held frame it releases. Returns reply bytes
/// the emulator queued and what to do next, if anything.
fn apply(
    model: &Arc<Mutex<Model>>,
    ordered: &mut Ordered,
    frame: Frame,
    exited: &mut bool,
) -> (Vec<u8>, Option<Next>) {
    let mut model = lock(model);
    let mut next = None;
    for (applied, frame) in ordered.push_frames(frame) {
        match applied {
            Applied::Output { .. } => {
                if let Body::Output { data, .. } = &frame.body {
                    model.output(data);
                }
            }
            Applied::Gap { bytes, .. } => model.gap(bytes),
            Applied::Exit(exit) => {
                *exited = true;
                model.set_phase(Phase::Exited {
                    code: exit.code,
                    signal: exit.signal,
                    cause: match exit.cause {
                        Cause::Exited => "exited",
                        Cause::Closed => "closed",
                        Cause::IdleExpired => "idle",
                        Cause::HostShutdown => "shutdown",
                    },
                });
            }
            Applied::Detached(Detached::Revoked) => {
                next = Some(Next::Stop(Stop::Ended(Phase::Refused(
                    "This computer revoked this device's terminal access.".into(),
                ))));
            }
            Applied::Detached(Detached::Transport) | Applied::Behind { .. } => {
                next.get_or_insert(Next::Reattach { new_link: false });
            }
            Applied::Detached(Detached::Requested) | Applied::Duplicate | Applied::Refused(_) => {}
        }
    }
    let replies = model.vt.take_replies();
    (replies, next)
}

/// Run one command. Returns what to do next when the command ends the
/// attachment.
async fn handle(
    command: Command,
    model: &Arc<Mutex<Model>>,
    link: &Link,
    reference: &TerminalRef,
    exited: bool,
    host_size: &mut (u16, u16),
) -> Option<Next> {
    match command {
        Command::Leave => Some(Next::Stop(Stop::Left)),
        Command::Bytes(bytes) => {
            if exited {
                return None;
            }
            send_input(link, reference, model, bytes).await
        }
        Command::Resize(rows, cols) => {
            if !exited && *host_size != (rows, cols) {
                let _ = request(link, resize(reference, rows, cols)).await;
                *host_size = (rows, cols);
            }
            None
        }
        Command::Close => {
            if exited {
                return None;
            }
            let closed = request(
                link,
                TermRequest::Close(Close::new(new_id(), reference.clone())),
            )
            .await;
            if !matches!(closed, Ok(ref result) if result.status != Status::Refused) {
                let mut model = lock(model);
                model.notice = Some("The computer didn't close the terminal. Try again.".into());
                model.touch();
            }
            // The exit frame follows on the attachment.
            None
        }
    }
}

async fn send_input(
    link: &Link,
    reference: &TerminalRef,
    model: &Arc<Mutex<Model>>,
    bytes: Vec<u8>,
) -> Option<Next> {
    for chunk in bytes.chunks(INPUT_CHUNK) {
        let sent = request(
            link,
            TermRequest::Input(Input::new(new_id(), reference.clone(), chunk)),
        )
        .await;
        match sent {
            Ok(result) if result.status == Status::Refused => {
                let mut model = lock(model);
                model.notice = Some(match result.reason {
                    Some(Reason::NotAdmitted | Reason::Revoked) => {
                        "The computer refused your input: this device lacks the terminal right."
                            .into()
                    }
                    Some(Reason::Closed) => "The shell has ended; input wasn't sent.".into(),
                    _ => "The computer refused your input.".into(),
                });
                model.touch();
                return None;
            }
            Ok(_) => {}
            Err(_) => {
                let mut model = lock(model);
                model.notice =
                    Some("Couldn't reach the computer. Your input may not have arrived.".into());
                model.touch();
                return Some(Next::Reattach { new_link: true });
            }
        }
    }
    let mut model = lock(model);
    if model.notice.take().is_some() {
        model.touch();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_host::pty::wire::Exit;

    fn reference() -> TerminalRef {
        TerminalRef {
            generation: "a".repeat(64),
            terminal: "b".repeat(64),
        }
    }

    fn frame(body: Body) -> Frame {
        Frame::new(reference(), "c".repeat(64), body)
    }

    fn output(seq: u64, text: &str) -> Frame {
        frame(Body::Output {
            seq,
            data: text.as_bytes().to_vec(),
        })
    }

    fn setup() -> (Arc<Mutex<Model>>, Ordered, bool) {
        let mut model = Model::new("h", "Mac", 6, 50);
        model.phase = Phase::Attached;
        (
            Arc::new(Mutex::new(model)),
            Ordered::new(TerminalState::new(reference(), 1, 1)),
            false,
        )
    }

    #[test]
    fn output_gaps_and_exit_reach_the_model_in_order() {
        let (model, mut ordered, mut exited) = setup();
        let (_, next) = apply(&model, &mut ordered, output(1, "$ make\r\n"), &mut exited);
        assert!(next.is_none());
        // A frame ahead of the next one waits; nothing is drawn yet.
        apply(&model, &mut ordered, output(5, "tail\r\n"), &mut exited);
        assert_eq!(ordered.held(), 1);
        assert!(!lock(&model).vt.text().contains("tail"));
        // The host reports 2 through 4 discarded: the gap shows, then the
        // held frame applies.
        let gap = frame(Body::Gap {
            from: 2,
            to: 4,
            bytes: Some(640),
        });
        apply(&model, &mut ordered, gap, &mut exited);
        let text = lock(&model).vt.text();
        assert!(
            text.contains("$ make\n[output lost: 640 bytes discarded by the host]\ntail"),
            "{text}"
        );
        assert_eq!(lock(&model).gaps, 1);
        // A duplicate changes nothing.
        let before = lock(&model).revision;
        apply(&model, &mut ordered, output(5, "tail\r\n"), &mut exited);
        assert_eq!(lock(&model).revision, before);
        let exit = Exit {
            cause: Cause::Exited,
            code: Some(2),
            signal: None,
        };
        apply(
            &model,
            &mut ordered,
            frame(Body::Exit { seq: 6, exit }),
            &mut exited,
        );
        assert!(exited);
        assert_eq!(
            lock(&model).phase.describe(),
            "The shell exited with code 2."
        );
    }

    #[test]
    fn replies_go_to_the_host_and_revocation_ends_the_session() {
        let (model, mut ordered, mut exited) = setup();
        let (replies, _) = apply(&model, &mut ordered, output(1, "\x1b[6n"), &mut exited);
        assert_eq!(replies, b"\x1b[1;1R");
        let (_, next) = apply(
            &model,
            &mut ordered,
            frame(Body::Detached {
                reason: Detached::Revoked,
            }),
            &mut exited,
        );
        assert!(matches!(
            next,
            Some(Next::Stop(Stop::Ended(Phase::Refused(ref reason)))) if reason.contains("revoked")
        ));
        let (_, next) = apply(
            &model,
            &mut ordered,
            frame(Body::Detached {
                reason: Detached::Transport,
            }),
            &mut exited,
        );
        assert!(matches!(next, Some(Next::Reattach { new_link: false })));
    }

    #[test]
    fn refusals_map_to_clear_phases() {
        assert_eq!(attach_refusal(Some(Reason::Lost)), Phase::Lost);
        assert_eq!(attach_refusal(Some(Reason::Closed)), Phase::Closed);
        let Phase::Refused(text) = attach_refusal(Some(Reason::NotAdmitted)) else {
            panic!("a missing right is a refusal")
        };
        assert!(text.contains("\"Open terminals\" right"));
        let mut error = AccessError::new(Code::MissingRight, "missing");
        error.missing = Some(coder_access::Right::Terminal);
        let Some(Phase::Refused(text)) = open_refusal(&HostError::Access(error)) else {
            panic!("a missing right is a refusal")
        };
        assert!(text.contains("\"Open terminals\" right"), "{text}");
        let none = HostError::Access(AccessError::new(Code::Unsupported, "none"));
        let Some(Phase::Refused(text)) = open_refusal(&none) else {
            panic!("no workspace is a refusal")
        };
        assert!(text.contains("serves no workspace"), "{text}");
        let unusable = HostError::Access(AccessError::new(Code::Unavailable, "gone"));
        let Some(Phase::Refused(text)) = open_refusal(&unusable) else {
            panic!("a missing root is a refusal")
        };
        assert!(text.contains("workspace directory is missing"), "{text}");
        let transport = HostError::Access(AccessError::new(Code::Transport, "down"));
        assert!(open_refusal(&transport).is_none());
        assert!(open_refusal(&HostError::Closed(None)).is_none());
    }
}
