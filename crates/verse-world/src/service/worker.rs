//! Bounded duplex network worker for remote presentation adapters.
#[cfg(all(test, feature = "service-net"))]
#[path = "worker_delayed.rs"]
mod delayed;
use super::client_runtime;
use super::{
    client::Client,
    event_cursor::{Cursor, Delivery},
    wire::{Body, Control, Reply, Response},
};
use crate::{Command, Intent, play::Ability};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use web_time::Instant;

/// Local measurements contain no principal, command content, or credentials.
#[derive(Clone, Copy)]
pub struct Observation {
    pub kind: &'static str,
    pub turnaround_ms: f64,
    pub verified_at: Instant,
    pub started_at: Instant,
    pub accepted_snapshot: bool,
    pub response_bytes: usize,
    pub pending_requests: usize,
    pub queued_inputs: usize,
    pub queued_updates: usize,
}
/// Frame timing metadata excludes input axes and authentication material.
/// Transport phases retain the enqueue control and queue context; their timestamp
/// marks writer start or successful flush, neither of which proves admission.
#[derive(Clone, Copy)]
pub struct FrameObservation {
    pub at: Instant,
    pub phase: &'static str,
    pub actor: u64,
    pub epoch: u64,
    pub sequence: u64,
    pub start: u64,
    pub end: u64,
    pub authority_tick: u64,
    pub control_epoch: Option<u64>,
    pub credit_step: Option<u64>,
    pub pending_requests: usize,
    pub queued_inputs: usize,
}
#[derive(Default)]
pub struct Observations {
    pub samples: Vec<Observation>,
    pub frames: Vec<FrameObservation>,
    pub omitted_frames: u64,
    pub omitted: u64,
    pub snapshot_verified_at: Option<Instant>,
}
/// Optional read-only telemetry; a full buffer never backpressures authority updates.
#[derive(Clone, Default)]
pub struct Observer(std::sync::Arc<std::sync::Mutex<Observations>>);
impl Observer {
    pub(super) fn frame(&self, value: FrameObservation) {
        let mut state = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        if state.frames.len() < 256 {
            state.frames.push(value);
        } else {
            state.omitted_frames = state.omitted_frames.saturating_add(1);
        }
    }
    fn observe(&self, value: Observation) {
        let mut state = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        if value.accepted_snapshot {
            state.snapshot_verified_at = Some(value.verified_at);
        }
        if state.samples.len() < 256 {
            state.samples.push(value);
        } else {
            state.omitted = state.omitted.saturating_add(1);
        }
    }
    pub fn drain(&self) -> Observations {
        let mut state = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        state.frames.sort_by_key(|frame| frame.at);
        Observations {
            samples: std::mem::take(&mut state.samples),
            frames: std::mem::take(&mut state.frames),
            omitted_frames: std::mem::take(&mut state.omitted_frames),
            omitted: state.omitted,
            snapshot_verified_at: state.snapshot_verified_at,
        }
    }
}

pub const INPUT_CAPACITY: usize = 32;
pub const UPDATE_CAPACITY: usize = 8;
/// Leaves request capacity for 30 Hz input refreshes and spell commands.
pub const NATIVE_CADENCE: Duration = Duration::from_millis(50);

fn periodic_read_room(pending: usize, queued: bool, auxiliary: bool) -> bool {
    // Snapshots supply movement baselines. Auxiliary reads leave slots for a
    // frame and a spell when the producer is waiting for its next credit burst.
    let reserve = if auxiliary { 2 } else { 1 };
    !queued && pending < super::client::PIPELINE_CAPACITY - reserve
}

/// Local input requests contain no principal, controller, or transport handle.
pub enum Input {
    Command(Intent<Ability>),
    /// Tokens increase for this connection; captured control must still be current.
    TrackedCommand {
        token: u64,
        life: verse_engine::core::LifeId,
        epoch: u64,
        intent: Intent<Ability>,
    },
    MovementFrame {
        token: u64,
        frame: crate::movement::frames::Frame,
    },
    BeginMovementFrames {
        life: verse_engine::core::LifeId,
        epoch: u64,
    },
    Respawn,
    QuestCycle {
        quest: u64,
        cycle: u64,
        action: super::progression::Action,
    },
    ClaimQuest(u64),
    AcceptQuest(u64, verse_engine::core::LifeId),
    UseItem(u64),
    EquipOutfit(u64),
    EquipGear(super::equipment::Slot, u64),
}
/// Ordered updates; persist event progress only after consuming its delivery.
pub enum Update {
    Snapshot(Response),
    Inventory(Response),
    /// Verified clock time does not acknowledge a player operation.
    MovementCredit(Response),
    Events {
        delivery: Delivery,
        checkpoint: Vec<u8>,
        control: Option<Control>,
    },
    /// Emitted before transmission. An error consumes the input without an outcome.
    CommandBound {
        token: u64,
        binding: Result<Command<Ability>, String>,
    },
    /// Unsent movement remains local history associated with the replacement.
    MovementSuperseded {
        token: u64,
        replacement: u64,
    },
    FrameBound {
        token: u64,
        binding: Result<crate::movement::frames::Frame, String>,
    },
    Outcome(Response),
}

struct ReadBackoff {
    until: [client_runtime::Instant; 3],
    since: [Option<client_runtime::Instant>; 3],
}
impl ReadBackoff {
    fn new(now: client_runtime::Instant) -> Self {
        Self {
            until: [now; 3],
            since: [None; 3],
        }
    }
    fn ready(&self, class: usize, now: client_runtime::Instant) -> bool {
        now >= self.until[class]
    }
    fn observe(
        &mut self,
        class: usize,
        reply: &Reply,
        now: client_runtime::Instant,
    ) -> Result<bool, String> {
        if let Reply::Refused { code, message, .. } = reply {
            if code != "storage_busy" && code != "rate_limited" {
                return Err(message.clone());
            }
            let started = *self.since[class].get_or_insert(now);
            if now.duration_since(started) >= Duration::from_secs(10) {
                return Err("Chamber read backpressure exceeded ten seconds".into());
            }
            self.until[class] = now + Duration::from_millis(100);
            return Ok(true);
        }
        self.since[class] = None;
        self.until[class] = now;
        Ok(false)
    }
}

// A verified lifecycle change fences every old interval; none can be replayed.
fn interval_control_changed(
    frame: &crate::movement::frames::Frame,
    control: Option<&super::wire::Control>,
) -> bool {
    control.is_some_and(|c| c.life != frame.life.into() || c.epoch != frame.epoch)
}

/// A newer verified generation or epoch permanently retires tracked input.
/// This avoids refreshing the scene for each queued input that cannot be rebound.
fn retired_control(input: &Input, control: Option<&super::wire::Control>) -> bool {
    let (life, epoch) = match input {
        Input::TrackedCommand { life, epoch, .. } => (*life, *epoch),
        Input::MovementFrame { frame, .. } => (frame.life, frame.epoch),
        _ => return false,
    };
    control.is_some_and(|c| {
        c.life.instance == life.instance
            && c.life.actor == life.actor
            && (life.generation < c.life.generation
                || (life.generation == c.life.generation && epoch < c.epoch))
    })
}

/// Reuses only recent verified response control; server admission still checks every command.
fn fresh_control(
    input: &Input,
    control: Option<&super::wire::Control>,
    observed: Option<Instant>,
) -> bool {
    let (life, epoch) = match input {
        Input::TrackedCommand { life, epoch, .. } | Input::BeginMovementFrames { life, epoch } => {
            (*life, *epoch)
        }
        Input::MovementFrame { frame, .. } => (frame.life, frame.epoch),
        Input::Command(Intent::Cast { .. })
        | Input::UseItem(_)
        | Input::EquipGear(_, _)
        | Input::EquipOutfit(_)
        | Input::QuestCycle { .. }
        | Input::ClaimQuest(_)
        | Input::AcceptQuest(_, _) => match control {
            Some(c) => (c.life.into(), c.epoch),
            None => return false,
        },
        _ => return false,
    };
    let (Some(control), Some(observed)) = (control, observed) else {
        return false;
    };
    control.life == life.into()
        && control.epoch == epoch
        && observed.elapsed()
            <= if matches!(input, Input::MovementFrame { .. }) {
                Duration::from_millis(400)
            } else {
                Duration::from_millis(50)
            }
}

/// Coalesces only consecutive, increasing movement tokens in the same control context.
/// A cast, jump, lifecycle action, or different context remains the next queued input.
fn coalesce_movement(
    mut input: Input,
    inputs: &mut mpsc::Receiver<Input>,
    deferred: &mut Option<Input>,
) -> (Input, Vec<u64>) {
    let mut retired = Vec::new();
    if deferred.is_some() {
        return (input, retired);
    }
    for _ in 0..INPUT_CAPACITY {
        let Input::TrackedCommand {
            token,
            life,
            epoch,
            intent: Intent::Move { .. },
        } = &input
        else {
            break;
        };
        let (token, life, epoch) = (*token, *life, *epoch);
        let Ok(next) = inputs.try_recv() else {
            break;
        };
        let compatible = matches!(&next, Input::TrackedCommand {
            token: newer, life: next_life, epoch: next_epoch, intent: Intent::Move { .. }
        } if token > 0 && *newer > token && *next_life == life && *next_epoch == epoch);
        if !compatible {
            *deferred = Some(next);
            break;
        }
        retired.push(token);
        input = next;
    }
    (input, retired)
}

/// Runs on the caller's Tokio runtime, independently of the render loop.
/// Bounded output backpressure pauses polling and input; no updates are dropped.
/// Shutdown cancels uncertain IO and closes the owned connection without replay.
pub async fn run(
    client: Client,
    cursor: Cursor,
    cadence: Duration,
    inputs: mpsc::Receiver<Input>,
    updates: mpsc::Sender<Update>,
    stop: oneshot::Receiver<()>,
) -> Result<(), String> {
    run_impl(client, cursor, cadence, inputs, updates, stop, None).await
}
/// Adds bounded local timing observations without changing dispatch or wire messages.
pub async fn run_profiled(
    client: Client,
    cursor: Cursor,
    cadence: Duration,
    inputs: mpsc::Receiver<Input>,
    updates: mpsc::Sender<Update>,
    stop: oneshot::Receiver<()>,
    observer: Observer,
) -> Result<(), String> {
    run_impl(
        client,
        cursor,
        cadence,
        inputs,
        updates,
        stop,
        Some(observer),
    )
    .await
}
async fn run_impl(
    client: Client,
    mut cursor: Cursor,
    cadence: Duration,
    mut inputs: mpsc::Receiver<Input>,
    updates: mpsc::Sender<Update>,
    stop: oneshot::Receiver<()>,
    observer: Option<Observer>,
) -> Result<(), String> {
    if cursor.instance() != client.instance()
        || inputs.max_capacity() > INPUT_CAPACITY
        || updates.max_capacity() > UPDATE_CAPACITY
    {
        return Err("Invalid chamber worker context or queue budget".into());
    }
    if !(Duration::from_millis(33)..=Duration::from_secs(1)).contains(&cadence) {
        return Err("Invalid chamber replication cadence".into());
    }
    let work = async {
        let mut client = client.pipeline_observed(observer.clone())?;
        let mut interval = client_runtime::interval(cadence);
        interval.set_missed_tick_behavior(client_runtime::MissedTickBehavior::Skip);
        let mut next_inventory = client_runtime::Instant::now();
        let mut next_events = client_runtime::Instant::now();
        let mut inventory_life = None;
        let mut last_token = 0;
        let mut deferred = None;
        let mut staged = None;
        let mut last_response = client.verified_at();
        let mut snapshot_pending = false;
        let mut last_snapshot_sent = None;
        let mut snapshot_resume = client_runtime::Instant::now();
        let mut events_pending = false;
        let mut inventory_pending = false;
        let mut refreshed = false;
        let mut barrier = false;
        let mut teleport_barrier = false;
        let mut entry_quiet: Option<((super::wire::Life, u64), client_runtime::Instant)> = None;
        let mut input_closed = false;
        let mut read_backoff = ReadBackoff::new(client_runtime::Instant::now());
        loop {
            if entry_quiet.is_some_and(|(context, deadline)| {
                client
                    .control()
                    .is_none_or(|control| (control.life, control.epoch) != context)
                    || client_runtime::Instant::now() >= deadline
            }) {
                entry_quiet = None;
            }
            if !client.available() && client.pending() == 0 {
                return Err("Chamber pipeline is disconnected".into());
            }
            if input_closed && client.pending() == 0 {
                return Ok(());
            }
            // Frames remain epoch-checked at authority during a teleport reply wait.
            // Other inputs wait so a successful teleport cannot bind later commands.
            if staged.is_some()
                && client.available()
                && (!barrier
                    || (teleport_barrier
                        && matches!(staged.as_ref(), Some(Input::MovementFrame { .. }))))
            {
                let input = staged.take().expect("Staged chamber input");
                let (input, retired) = coalesce_movement(input, &mut inputs, &mut deferred);
                for token in retired {
                    let Input::TrackedCommand {
                        token: replacement, ..
                    } = &input
                    else {
                        unreachable!()
                    };
                    updates
                        .send(Update::MovementSuperseded {
                            token,
                            replacement: *replacement,
                        })
                        .await
                        .map_err(|_| "Chamber update consumer closed")?;
                }
                let obsolete = retired_control(&input, client.control());
                let teleport = matches!(
                    input,
                    Input::Command(Intent::Cast {
                        ability: Ability::MistyStep,
                        ..
                    }) | Input::TrackedCommand {
                        intent: Intent::Cast {
                            ability: Ability::MistyStep,
                            ..
                        },
                        ..
                    }
                );
                // Inventory and quest operations preserve control and stay ordered.
                // Sequenced teleports keep the reply barrier but can send before
                // earlier replies return. Respawn and interval entry drain earlier IO.
                let lifecycle = !obsolete
                    && (matches!(input, Input::Respawn | Input::BeginMovementFrames { .. })
                        || teleport);
                if lifecycle && !teleport && client.pending() > 0 {
                    staged = Some(input);
                } else if !refreshed
                    && !retired_control(&input, client.control())
                    && !fresh_control(&input, client.control(), last_response)
                {
                    // Any owned reply supplies a validated control header. Drain
                    // that prefix before adding a scene request for staged input.
                    if client.pending() == 0
                        && !snapshot_pending
                        && read_backoff.ready(0, client_runtime::Instant::now())
                    {
                        client.send_snapshot()?;
                        last_snapshot_sent = Some(client_runtime::Instant::now());
                        snapshot_pending = true;
                    }
                    staged = Some(input);
                } else {
                    refreshed = false;
                    let body = match input {
                        Input::TrackedCommand {
                            token,
                            life,
                            epoch,
                            intent,
                        } => {
                            let binding = if token == 0 || token <= last_token {
                                Err("Tracked input token must increase".into())
                            } else {
                                last_token = token;
                                match client.control() {
                                    Some(control)
                                        if control.life == life.into()
                                            && control.epoch == epoch =>
                                    {
                                        client.prepare_command(intent)
                                    }
                                    _ => {
                                        Err("Tracked input control changed before transmission"
                                            .into())
                                    }
                                }
                            };
                            let command = binding.as_ref().ok().cloned();
                            updates
                                .send(Update::CommandBound { token, binding })
                                .await
                                .map_err(|_| "Chamber update consumer closed")?;
                            let Some(command) = command else {
                                continue;
                            };
                            Body::Command {
                                command: command.into(),
                            }
                        }
                        Input::MovementFrame { token, frame } => {
                            let binding = if token == 0 || token <= last_token {
                                Err("Movement interval token must increase".into())
                            } else {
                                last_token = token;
                                client.prepare_movement_frame(frame)
                            };
                            let frame = binding.as_ref().ok().cloned();
                            updates
                                .send(Update::FrameBound { token, binding })
                                .await
                                .map_err(|_| "Chamber update consumer closed")?;
                            let Some(frame) = frame else {
                                continue;
                            };
                            Body::MovementFrame { frame }
                        }
                        Input::Command(intent) => Body::Command {
                            command: client.prepare_command(intent)?.into(),
                        },
                        action => {
                            let control = client
                                .control()
                                .ok_or("Client has no admitted adventurer")?
                                .clone();
                            if !matches!(action, Input::BeginMovementFrames { .. }) {
                                next_inventory = client_runtime::Instant::now();
                            }
                            match action {
                                Input::Respawn => Body::Respawn { life: control.life },
                                Input::BeginMovementFrames { life, epoch } => {
                                    Body::BeginMovementFrames {
                                        life: life.into(),
                                        epoch,
                                    }
                                }
                                Input::QuestCycle {
                                    quest,
                                    cycle,
                                    action,
                                } => Body::QuestCycle {
                                    life: control.life,
                                    epoch: control.epoch,
                                    quest,
                                    cycle,
                                    action,
                                },
                                Input::AcceptQuest(quest, giver) => Body::AcceptQuest {
                                    life: control.life,
                                    epoch: control.epoch,
                                    quest,
                                    giver: giver.into(),
                                },
                                Input::ClaimQuest(quest) => Body::ClaimQuest {
                                    life: control.life,
                                    epoch: control.epoch,
                                    quest,
                                },
                                action => {
                                    let mut operation = [0; 16];
                                    getrandom::fill(&mut operation).map_err(
                                        |_| "Cannot generate chamber operation identity",
                                    )?;
                                    match action {
                                        Input::EquipGear(slot, item) => Body::EquipGear {
                                            life: control.life,
                                            epoch: control.epoch,
                                            slot,
                                            item,
                                            operation,
                                        },
                                        Input::EquipOutfit(outfit) => Body::EquipOutfit {
                                            life: control.life,
                                            epoch: control.epoch,
                                            outfit,
                                            operation,
                                        },
                                        Input::UseItem(item) => Body::UseItem {
                                            life: control.life,
                                            epoch: control.epoch,
                                            item,
                                            operation,
                                        },
                                        _ => unreachable!(),
                                    }
                                }
                            }
                        }
                    };
                    let trace = match &body {
                        Body::MovementFrame { frame } if observer.is_some() => {
                            Some(FrameObservation {
                                at: Instant::now(),
                                phase: "enqueued",
                                actor: frame.life.actor,
                                epoch: frame.epoch,
                                sequence: frame.sequence,
                                start: frame.start,
                                end: frame.end()?,
                                authority_tick: client.tick(),
                                control_epoch: client.control().map(|c| c.epoch),
                                credit_step: client.control().map(|c| c.credit_step),
                                pending_requests: client.pending() + 1,
                                queued_inputs: inputs.len(),
                            })
                        }
                        _ => None,
                    };
                    client.send_observed(body, trace)?;
                    if let (Some(observer), Some(trace)) = (&observer, trace) {
                        observer.frame(trace);
                    }
                    if lifecycle {
                        barrier = true;
                        teleport_barrier = teleport;
                    }
                }
            }
            // Schedule from the actual send time so quick replies cannot miss a phased polling tick.
            let snapshot_due = last_snapshot_sent
                .map_or_else(client_runtime::Instant::now, |sent| sent + cadence)
                .max(snapshot_resume);
            tokio::select! {
                _ = client_runtime::sleep_until(snapshot_due), if !input_closed && !barrier
                    && staged.is_none() && entry_quiet.is_none() && client.available() && !snapshot_pending
                    && periodic_read_room(client.pending(), !inputs.is_empty() || deferred.is_some(), false)
                    && last_snapshot_sent.is_some() && read_backoff.ready(0, client_runtime::Instant::now()) => {
                    client.send_snapshot()?;
                    last_snapshot_sent = Some(client_runtime::Instant::now());
                    snapshot_pending = true;
                }
                response = client.receive(), if client.pending() > 0 => {
                    let (body,response) = response?;
                    last_response = Some(Instant::now());
                    let entry = matches!(&body,Body::BeginMovementFrames{..}).then(||response.clone());
                    if entry.as_ref().is_some_and(|response| matches!(response.body, Reply::Snapshot { .. })) {
                        // Entry already delivers the movement baseline. Leave one cadence
                        // for its first input batch before asking for another projection.
                        let now = client_runtime::Instant::now();
                        last_snapshot_sent = Some(now);
                        snapshot_resume = now + cadence;
                        // Let the first interval reach authority before periodic
                        // reads consume the fresh clock's transport prefix. This
                        // affects reads only and expires if no producer sends frames.
                        entry_quiet = response.control.as_ref().map(|control|
                            ((control.life, control.epoch), now + Duration::from_millis(200)));
                        // Entry has completed. A small read can return fresh committed
                        // time before the first movement receipt, without holding inputs.
                        barrier = false;
                        if !input_closed {
                            client.send(Body::MovementCredit {})?;
                        }
                    }
                    if let Some(observer) = &observer {
                        let kind = match &body {
                            Body::Replicate { .. } | Body::Snapshot {} => "snapshot",
                            Body::Inventory {} => "inventory",
                            Body::Events { .. } => "events",
                            _ => "command",
                        };
                        observer.observe(Observation { kind,
                            turnaround_ms: client.last_turnaround().unwrap_or_default().as_secs_f64() * 1000.,
                            verified_at: Instant::now(),
                            started_at: client.last_request_started().unwrap_or_else(Instant::now),
                            accepted_snapshot: matches!(response.body, Reply::Snapshot { .. }),
                            response_bytes: client.last_response_bytes().unwrap_or(0),
                            pending_requests: client.pending(),
                            queued_inputs: inputs.len(), queued_updates: updates.max_capacity() - updates.capacity() });
                    }
                    let update = match body {
                        Body::Snapshot {} | Body::Replicate {..} => {
                            snapshot_pending = false;
                            // A slow read yields one cadence to input before requesting another
                            // projection. Fast routes retain their deadline from the send time.
                            if client.last_turnaround().is_some_and(|elapsed| elapsed > cadence) {
                                snapshot_resume = client_runtime::Instant::now() + cadence;
                            }
                            if read_backoff.observe(0, &response.body, client_runtime::Instant::now())? {
                                refreshed = false;
                                continue;
                            }
                            refreshed = staged.is_some();
                            Update::Snapshot(response)
                        }
                        Body::Events { after,limit } => {
                            events_pending = false;
                            if read_backoff.observe(1, &response.body, client_runtime::Instant::now())? { continue; }
                            let delivery = cursor.admit(&response,after,limit)?;
                            Update::Events { delivery, checkpoint: cursor.checkpoint()?, control: response.control }
                        }
                        Body::Inventory {} => {
                            inventory_pending = false;
                            if read_backoff.observe(2, &response.body, client_runtime::Instant::now())? { continue; }
                            inventory_life = client.control().map(|c| c.life);
                            next_inventory = client_runtime::Instant::now() + Duration::from_secs(1);
                            Update::Inventory(response)
                        }
                        Body::MovementCredit {} => Update::MovementCredit(response),
                        Body::MovementFrame { frame } => {
                            if entry_quiet.is_some_and(|(context, _)| context == (frame.life.into(), frame.epoch)) {
                                entry_quiet = None;
                            }
                            if let Some(observer) = &observer {
                                observer.frame(FrameObservation {
                                    at: Instant::now(), phase: "acknowledged", actor: frame.life.actor,
                                    epoch: frame.epoch, sequence: frame.sequence, start: frame.start,
                                    end: frame.end()?, authority_tick: response.tick,
                                    control_epoch: response.control.as_ref().map(|c| c.epoch),
                                    credit_step: response.control.as_ref().map(|c| c.credit_step),
                                    pending_requests: client.pending(), queued_inputs: inputs.len(),
                                });
                            }
                            if let Reply::Refused { message, .. } = &response.body {
                                if !interval_control_changed(&frame, response.control.as_ref()) {
                                    return Err(format!("Movement interval refused; reconnect before sending another interval: {message}"));
                                }
                                // Report the refusal once. Pipeline control already fences queued
                                // old inputs; periodic replication supplies the new baseline.
                            }
                            Update::Outcome(response)
                        },
                        Body::Command { command } => {
                            if matches!(command.intent, super::wire::Action::Cast { ability: Ability::MistyStep, .. }) {
                                barrier = false;
                                teleport_barrier = false;
                            }
                            Update::Outcome(response)
                        },
                        _ => { barrier = false; teleport_barrier = false; Update::Outcome(response) }
                    };
                    updates.send(update).await.map_err(|_| "Chamber update consumer closed")?;
                    if let Some(entry)=entry { if matches!(entry.body,Reply::Snapshot {..}) { updates.send(Update::Snapshot(entry)).await.map_err(|_| "Chamber update consumer closed")?; } }

                }
                _ = interval.tick() => {
                    // One outstanding request per read class bounds stale work and event cursors.
                    // A staged lifecycle action drains previous IO before changing its context.
                    if input_closed || barrier || staged.is_some() || entry_quiet.is_some() { continue; }
                    if last_snapshot_sent.is_none() && client.available() && !snapshot_pending
                        && read_backoff.ready(0, client_runtime::Instant::now()) {
                        client.send_snapshot()?;
                        last_snapshot_sent = Some(client_runtime::Instant::now());
                        snapshot_pending = true;
                    }
                    if client.available() && periodic_read_room(client.pending(), !inputs.is_empty() || deferred.is_some(), true) && client_runtime::Instant::now() >= next_events
                        && !events_pending && read_backoff.ready(1, client_runtime::Instant::now()) {
                        client.send(Body::Events { after: cursor.after(), limit: 64 })?;
                        events_pending = true;
                        next_events = client_runtime::Instant::now() + cadence.max(Duration::from_millis(200));
                    }
                    let life = client.control().map(|c| c.life);
                    if client.available() && periodic_read_room(client.pending(), !inputs.is_empty() || deferred.is_some(), true)
                        && !inventory_pending && life.is_some()
                        && read_backoff.ready(2, client_runtime::Instant::now())
                        && (life != inventory_life || client_runtime::Instant::now() >= next_inventory) {
                        client.send(Body::Inventory {})?;
                        inventory_pending = true;
                    }
                }
                input = async {
                    match deferred.take() { Some(input) => Some(input), None => inputs.recv().await }
                }, if !input_closed && staged.is_none() && client.available() && (!barrier || teleport_barrier) => {
                    match input {
                        Some(input) => staged = Some(input),
                        None => input_closed = true,
                    }
                }
            }
        }
    };
    tokio::select! {
        result = work => result,
        _ = stop => Ok(()),
    }
}

/// Creates fixed-capacity channels; callers retain and await the worker task.
pub fn channels() -> (
    mpsc::Sender<Input>,
    mpsc::Receiver<Input>,
    mpsc::Sender<Update>,
    mpsc::Receiver<Update>,
) {
    let (input, inputs) = mpsc::channel(INPUT_CAPACITY);
    let (updates, output) = mpsc::channel(UPDATE_CAPACITY);
    (input, inputs, updates, output)
}

#[cfg(all(test, feature = "service-net"))]
mod tests {
    use super::*;
    use crate::service::{
        net::tests::{key, start},
        replica::Buffer,
    };
    use client_runtime::timeout;
    use rustls::pki_types::ServerName;

    #[tokio::test]
    async fn explicit_read_backpressure_retries_projections_without_replaying_commands() {
        use crate::service::net::{read_frame, tests::gateway, write_frame};
        use crate::service::wire::{MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Request};
        let keys = [key(208), key(209), key(210)];
        let mut gateway = gateway(&keys);
        let (client_socket, mut peer_socket) = tokio::io::duplex(1024 * 1024);
        let (peer_stop, mut peer_stopped) = oneshot::channel();
        let peer = tokio::spawn(async move {
            let (connection, hello) = gateway.open_json(0).unwrap();
            write_frame(&mut peer_socket, &hello, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            let started = Instant::now();
            let mut refused = [false; 3];
            let mut commands = 0;
            loop {
                let bytes = tokio::select! {
                    _ = &mut peer_stopped => break,
                    bytes = read_frame(&mut peer_socket, MAX_REQUEST_BYTES) => match bytes {
                        Ok(bytes) => bytes,
                        Err(_) => break,
                    },
                };
                let request = Request::decode(&bytes).unwrap();
                let class = match request.body {
                    Body::Snapshot {} | Body::Replicate { .. } => Some(0),
                    Body::Events { .. } => Some(1),
                    Body::Inventory {} => Some(2),
                    Body::Command { .. } => {
                        commands += 1;
                        None
                    }
                    _ => None,
                };
                let bytes = gateway
                    .dispatch_json(connection, started.elapsed().as_millis() as u64, &bytes)
                    .unwrap();
                let mut response: Response = serde_json::from_slice(&bytes).unwrap();
                if let Some(class) = class.filter(|class| !refused[*class]) {
                    refused[class] = true;
                    response.body = Reply::Refused {
                        code: if class == 1 {
                            "rate_limited"
                        } else {
                            "storage_busy"
                        }
                        .into(),
                        message: "Transient read backpressure".into(),
                    };
                }
                write_frame(
                    &mut peer_socket,
                    &serde_json::to_vec(&response).unwrap(),
                    MAX_RESPONSE_BYTES,
                )
                .await
                .unwrap();
            }
            (refused, commands)
        });
        let client = Client::connect_stream(Box::new(client_socket), 120, None, &keys[0])
            .await
            .unwrap();
        let (input, inputs, updates, mut output) = channels();
        let (stop, stopping) = oneshot::channel();
        let task = tokio::spawn(run(
            client,
            Cursor::new(120),
            NATIVE_CADENCE,
            inputs,
            updates,
            stopping,
        ));
        input
            .send(Input::Command(Intent::Move {
                axes: [0.1, 0.],
                yaw: 0.,
            }))
            .await
            .unwrap();
        let mut received = [false; 4];
        timeout(Duration::from_secs(3), async {
            while received.contains(&false) {
                match output.recv().await.unwrap() {
                    Update::Snapshot(_) => received[0] = true,
                    Update::Events { control, .. } => {
                        assert!(
                            control.is_some(),
                            "Owned event credit must reach the consumer"
                        );
                        received[1] = true;
                    }
                    Update::Inventory(_) => received[2] = true,
                    Update::Outcome(response) => {
                        assert!(matches!(response.body, Reply::Accepted));
                        received[3] = true;
                    }
                    _ => {}
                }
            }
        })
        .await
        .unwrap();
        stop.send(()).unwrap();
        task.await.unwrap().unwrap();
        let _ = peer_stop.send(());
        let (refused, commands) = peer.await.unwrap();
        assert_eq!(refused, [true; 3]);
        assert_eq!(commands, 1);
    }

    #[test]
    fn read_backoff_separates_classes_and_bounds_explicit_refusal_retries() {
        let now = client_runtime::Instant::now();
        let mut backoff = ReadBackoff::new(now);
        let busy = Reply::Refused {
            code: "storage_busy".into(),
            message: "Busy".into(),
        };
        assert!(backoff.observe(0, &busy, now).unwrap());
        assert!(!backoff.ready(0, now + Duration::from_millis(99)));
        assert!(backoff.ready(1, now));
        assert!(
            backoff
                .observe(0, &busy, now + Duration::from_secs(9))
                .unwrap()
        );
        assert!(
            backoff
                .observe(0, &busy, now + Duration::from_secs(10))
                .is_err()
        );
        assert!(
            !backoff
                .observe(0, &Reply::Accepted, now + Duration::from_secs(11))
                .unwrap()
        );
        assert!(
            backoff
                .observe(0, &busy, now + Duration::from_secs(12))
                .unwrap()
        );
        let semantic = Reply::Refused {
            code: "not_owned".into(),
            message: "Foreign life".into(),
        };
        assert_eq!(
            backoff.observe(0, &semantic, now).unwrap_err(),
            "Foreign life"
        );
    }
    #[test]
    fn optional_observations_remain_bounded_and_refusals_do_not_refresh_snapshots() {
        let observer = Observer::default();
        let at = Instant::now();
        for n in 0..300 {
            observer.observe(Observation {
                kind: "snapshot",
                turnaround_ms: 1.,
                verified_at: at,
                started_at: at,
                accepted_snapshot: n == 0,
                response_bytes: 100,
                pending_requests: 0,
                queued_inputs: 0,
                queued_updates: 0,
            });
        }
        let observations = observer.drain();
        assert_eq!(observations.samples.len(), 256);
        assert_eq!(observations.omitted, 44);
        assert_eq!(observations.snapshot_verified_at, Some(at));
        let later = at + Duration::from_secs(1);
        observer.observe(Observation {
            kind: "snapshot",
            turnaround_ms: 1.,
            verified_at: later,
            started_at: at,
            accepted_snapshot: false,
            response_bytes: 100,
            pending_requests: 0,
            queued_inputs: 0,
            queued_updates: 0,
        });
        assert_eq!(observer.drain().snapshot_verified_at, Some(at));
        observer.observe(Observation {
            kind: "snapshot",
            turnaround_ms: 1.,
            verified_at: later,
            started_at: at,
            accepted_snapshot: true,
            response_bytes: 100,
            pending_requests: 0,
            queued_inputs: 0,
            queued_updates: 0,
        });
        assert_eq!(observer.drain().snapshot_verified_at, Some(later));
    }
    #[test]
    fn interval_recovery_requires_verified_lifecycle_change() {
        use crate::movement::frames::{Frame, Segment};
        let life = verse_engine::core::LifeId {
            instance: 120,
            actor: 14,
            generation: 0,
        };
        let frame = Frame {
            life,
            epoch: 3,
            sequence: 1,
            tick: 1,
            start: 0,
            steps: 4,
            segments: vec![Segment {
                offset: 0,
                axes: [0., 1.],
                yaw: 0.,
                until: 60,
                jump: false,
            }],
        };
        let mut control = super::super::wire::Control {
            credit_step: 0,
            world_step: 0,
            life: life.into(),
            epoch: 3,
            accepted_sequence: 1,
            applied_movement: None,
            dynamic: Vec::new(),
        };
        assert!(!interval_control_changed(&frame, None));
        assert!(!interval_control_changed(&frame, Some(&control)));
        control.accepted_sequence += 1;
        assert!(!interval_control_changed(&frame, Some(&control)));
        control.epoch += 1;
        assert!(interval_control_changed(&frame, Some(&control)));
        control.epoch = frame.epoch;
        control.life.generation += 1;
        assert!(interval_control_changed(&frame, Some(&control)));
    }

    #[tokio::test]
    async fn retired_interval_backlog_rejects_without_refreshing_each_scene() {
        use crate::movement::frames::{Frame, Segment};
        use crate::service::net::{read_frame, tests::gateway, write_frame};
        use crate::service::wire::{MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Request};
        let keys = [key(232), key(233), key(234)];
        let mut gateway = gateway(&keys);
        let (client_socket, mut peer_socket) = tokio::io::duplex(1024 * 1024);
        let (peer_stop, mut peer_stopped) = oneshot::channel();
        let peer = tokio::spawn(async move {
            let (connection, hello) = gateway.open_json(0).unwrap();
            write_frame(&mut peer_socket, &hello, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            let mut snapshots = 0;
            let started = Instant::now();
            loop {
                let bytes = tokio::select! {
                    _=&mut peer_stopped=>break,
                    bytes=read_frame(&mut peer_socket,MAX_REQUEST_BYTES)=>match bytes {Ok(b)=>b,Err(_)=>break},
                };
                let request = Request::decode(&bytes).unwrap();
                assert!(!matches!(request.body, Body::MovementFrame { .. }));
                let response = gateway
                    .dispatch_json(connection, started.elapsed().as_millis() as u64, &bytes)
                    .unwrap();
                if matches!(request.body, Body::Snapshot {} | Body::Replicate { .. }) {
                    snapshots += 1;
                    client_runtime::sleep(Duration::from_millis(100)).await;
                }
                if write_frame(&mut peer_socket, &response, MAX_RESPONSE_BYTES)
                    .await
                    .is_err()
                {
                    break;
                }
            }
            snapshots
        });
        let client = Client::connect_stream(Box::new(client_socket), 120, None, &keys[0])
            .await
            .unwrap();
        let control = client.control().unwrap().clone();
        assert!(control.epoch > 0);
        let (input, inputs, updates, mut output) = channels();
        for token in 1..=16 {
            input
                .try_send(Input::MovementFrame {
                    token,
                    frame: Frame {
                        life: control.life.into(),
                        epoch: control.epoch - 1,
                        sequence: 0,
                        tick: 0,
                        start: 0,
                        steps: 4,
                        segments: vec![Segment {
                            offset: 0,
                            axes: [0.; 2],
                            yaw: 0.,
                            until: 0,
                            jump: false,
                        }],
                    },
                })
                .unwrap();
        }
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(run(
            client,
            Cursor::new(120),
            Duration::from_secs(1),
            inputs,
            updates,
            stopped,
        ));
        timeout(Duration::from_millis(500), async {
            let mut rejected = 0;
            while rejected < 16 {
                if let Update::FrameBound { token, binding } = output.recv().await.unwrap() {
                    assert_eq!(token, rejected + 1);
                    assert!(binding.is_err());
                    rejected += 1;
                }
            }
        })
        .await
        .unwrap();
        stop.send(()).unwrap();
        task.await.unwrap().unwrap();
        let _ = peer_stop.send(());
        assert!(peer.await.unwrap() <= 1);
    }
    #[test]
    fn retirement_requires_an_irreversible_verified_control_transition() {
        let life = verse_engine::core::LifeId {
            instance: 120,
            actor: 14,
            generation: 0,
        };
        let input = Input::TrackedCommand {
            token: 1,
            life,
            epoch: 2,
            intent: Intent::Jump,
        };
        let mut control = super::super::wire::Control {
            life: life.into(),
            epoch: 2,
            accepted_sequence: 0,
            world_step: 0,
            credit_step: 0,
            applied_movement: None,
            dynamic: Vec::new(),
        };
        assert!(!retired_control(&input, None));
        assert!(!retired_control(&input, Some(&control)));
        control.epoch = 3;
        assert!(retired_control(&input, Some(&control)));
        control.epoch = 1;
        assert!(!retired_control(&input, Some(&control)));
        control.life.generation = 1;
        assert!(retired_control(&input, Some(&control)));
        control.life.actor += 1;
        assert!(!retired_control(&input, Some(&control)));
    }

    #[tokio::test]
    async fn teleport_outcome_releases_barrier_for_following_input_and_replication() {
        let keys = [key(217), key(218), key(219)];
        let (address, tls, server_stop, server) = start(&keys).await;
        let client = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            tls.config().clone(),
            120,
            &keys[0],
        )
        .await
        .unwrap();
        let (input, inputs, updates, mut output) = channels();
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(run(
            client,
            Cursor::new(120),
            NATIVE_CADENCE,
            inputs,
            updates,
            stopped,
        ));
        input
            .send(Input::Command(Intent::Cast {
                ability: Ability::MistyStep,
                target: None,
                aim: [1., 0., 0.],
            }))
            .await
            .unwrap();
        input
            .send(Input::Command(Intent::Move {
                axes: [0., 0.],
                yaw: 0.,
            }))
            .await
            .unwrap();
        timeout(Duration::from_secs(3), async {
            let mut outcomes = 0;
            loop {
                match output.recv().await.unwrap() {
                    Update::Outcome(_) => outcomes += 1,
                    Update::Snapshot(_) if outcomes == 2 => break,
                    _ => {}
                }
            }
        })
        .await
        .unwrap();
        stop.send(()).unwrap();
        task.await.unwrap().unwrap();
        server_stop.send(()).unwrap();
        assert!(server.await.unwrap().failure.is_none());
    }

    #[tokio::test]
    async fn refused_teleport_keeps_movement_flowing_before_its_reply() {
        teleport_movement_reply(false).await;
    }
    #[tokio::test]
    async fn successful_teleport_rejects_old_context_movement_before_its_reply() {
        teleport_movement_reply(true).await;
    }
    async fn teleport_movement_reply(accepted: bool) {
        use crate::service::net::{
            read_frame,
            tests::{gateway, tls},
            write_frame,
        };
        use crate::service::wire::{MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Request};
        use tokio::net::TcpListener;
        use tokio_rustls::TlsAcceptor;
        let keys = [key(227), key(228), key(229)];
        let mut gateway = gateway(&keys);
        gateway.tick(0.05).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (server_tls, connector) = tls();
        let (peer_ready, readiness) = oneshot::channel();
        let (peer_stop, peer_stopping) = oneshot::channel();
        let peer = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut socket = TlsAcceptor::from(server_tls).accept(socket).await.unwrap();
            let (id, hello) = gateway.open_json(0).unwrap();
            write_frame(&mut socket, &hello, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            let auth = read_frame(&mut socket, MAX_REQUEST_BYTES).await.unwrap();
            let response = gateway.dispatch_json(id, 0, &auth).unwrap();
            write_frame(&mut socket, &response, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            loop {
                let bytes = read_frame(&mut socket, MAX_REQUEST_BYTES).await.unwrap();
                let request = Request::decode(&bytes).unwrap();
                let response = gateway.dispatch_json(id, 0, &bytes).unwrap();
                if let Body::Command { command } = request.body {
                    assert!(matches!(
                        command.intent,
                        super::super::wire::Action::Cast {
                            ability: Ability::MistyStep,
                            ..
                        }
                    ));
                    let refusal: Response = serde_json::from_slice(&response).unwrap();
                    assert_eq!(matches!(refusal.body, Reply::Accepted), accepted);
                    let movement = timeout(
                        Duration::from_millis(200),
                        read_frame(&mut socket, MAX_REQUEST_BYTES),
                    )
                    .await
                    .expect("Movement stalled behind refused teleport reply")
                    .unwrap();
                    let Request {
                        body: Body::MovementFrame { frame },
                        ..
                    } = Request::decode(&movement).unwrap()
                    else {
                        panic!("Expected movement behind teleport");
                    };
                    assert_eq!(
                        refusal.control.as_ref().unwrap().epoch == frame.epoch,
                        !accepted
                    );
                    let moved = gateway.dispatch_json(id, 0, &movement).unwrap();
                    let reply: Response = serde_json::from_slice(&moved).unwrap();
                    assert_eq!(matches!(reply.body, Reply::Accepted), !accepted);
                    if accepted {
                        assert!(matches!(reply.body, Reply::Refused { .. }));
                    }
                    write_frame(&mut socket, &response, MAX_RESPONSE_BYTES)
                        .await
                        .unwrap();
                    write_frame(&mut socket, &moved, MAX_RESPONSE_BYTES)
                        .await
                        .unwrap();
                    peer_ready.send(()).unwrap();
                    let _ = peer_stopping.await;
                    return;
                }
                write_frame(&mut socket, &response, MAX_RESPONSE_BYTES)
                    .await
                    .unwrap();
            }
        });
        let mut client = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[0],
        )
        .await
        .unwrap();
        let entry = client.begin_movement_frames().await.unwrap();
        let Reply::Snapshot { state } = entry.body else {
            panic!("Interval entry refused");
        };
        let baseline = state.movement.unwrap();
        let (input, inputs, updates, mut output) = channels();
        input
            .send(Input::TrackedCommand {
                token: 1,
                life: baseline.life,
                epoch: baseline.epoch,
                intent: Intent::Cast {
                    ability: Ability::MistyStep,
                    target: None,
                    aim: if accepted { [0., 0., 1.] } else { [0.; 3] },
                },
            })
            .await
            .unwrap();
        input
            .send(Input::MovementFrame {
                token: 2,
                frame: crate::movement::frames::Frame {
                    life: baseline.life,
                    epoch: baseline.epoch,
                    sequence: 0,
                    tick: 0,
                    start: baseline.physics_step,
                    steps: 6,
                    segments: vec![crate::movement::frames::Segment {
                        offset: 0,
                        axes: [0.; 2],
                        yaw: 0.,
                        until: baseline.physics_step + crate::movement::HELD_STEPS,
                        jump: false,
                    }],
                },
            })
            .await
            .unwrap();
        let (stop, stopping) = oneshot::channel();
        let task = tokio::spawn(run(
            client,
            Cursor::new(120),
            NATIVE_CADENCE,
            inputs,
            updates,
            stopping,
        ));
        timeout(Duration::from_secs(3), async {
            readiness.await.unwrap();
            let mut outcomes = 0;
            while outcomes < 2 {
                if let Update::Outcome(response) = output.recv().await.unwrap() {
                    let expected_acceptance = if outcomes == 0 { accepted } else { !accepted };
                    assert_eq!(
                        matches!(response.body, Reply::Accepted),
                        expected_acceptance
                    );
                    if !expected_acceptance {
                        assert!(matches!(response.body, Reply::Refused { .. }));
                    }
                    outcomes += 1;
                }
            }
        })
        .await
        .unwrap();
        let _ = stop.send(());
        task.await.unwrap().unwrap();
        let _ = peer_stop.send(());
        peer.await.unwrap();
    }

    #[tokio::test]
    async fn tracked_teleport_waits_for_its_reply_before_binding_old_epoch_commands() {
        use crate::service::net::{
            read_frame,
            tests::{gateway, tls},
            write_frame,
        };
        use crate::service::wire::{MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Request};
        use tokio::net::TcpListener;
        use tokio_rustls::TlsAcceptor;
        let keys = [key(227), key(228), key(229)];
        let mut gateway = gateway(&keys);
        gateway.tick(0.05).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (server_tls, connector) = tls();
        let (peer_stop, peer_stopping) = oneshot::channel();
        let peer = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut socket = TlsAcceptor::from(server_tls).accept(socket).await.unwrap();
            let (id, hello) = gateway.open_json(0).unwrap();
            write_frame(&mut socket, &hello, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            let auth = read_frame(&mut socket, MAX_REQUEST_BYTES).await.unwrap();
            let response = gateway.dispatch_json(id, 0, &auth).unwrap();
            write_frame(&mut socket, &response, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            tokio::pin!(peer_stopping);
            let mut teleported = false;
            let mut withheld = Vec::new();
            loop {
                let bytes = tokio::select! {
                    _ = &mut peer_stopping => break,
                    bytes = read_frame(&mut socket, MAX_REQUEST_BYTES) => { let Ok(bytes) = bytes else { break }; bytes }
                };
                let request = Request::decode(&bytes).unwrap();
                let teleport = matches!(&request.body, Body::Command { command }
                    if matches!(command.intent, super::super::wire::Action::Cast { ability: Ability::MistyStep, .. }));
                assert!(
                    !teleported || !matches!(request.body, Body::Command { .. }),
                    "Old-epoch command reached the authority"
                );
                let response = gateway.dispatch_json(id, 0, &bytes).unwrap();
                if matches!(request.body, Body::MovementFrame { .. }) {
                    assert!(
                        !teleported && withheld.is_empty(),
                        "Unexpected movement reached authority"
                    );
                    let reply: Response = serde_json::from_slice(&response).unwrap();
                    assert!(matches!(reply.body, Reply::Accepted));
                    withheld.push(response);
                    continue;
                }
                if teleport {
                    assert!(
                        !withheld.is_empty(),
                        "Teleport must follow a pending movement reply"
                    );
                    let reply: Response = serde_json::from_slice(&response).unwrap();
                    assert!(
                        matches!(reply.body, Reply::Accepted),
                        "Teleport refused: {:?}",
                        reply.body
                    );
                    assert!(
                        timeout(
                            Duration::from_millis(100),
                            read_frame(&mut socket, MAX_REQUEST_BYTES)
                        )
                        .await
                        .is_err(),
                        "Input crossed the pending teleport barrier"
                    );
                }
                assert!(
                    !matches!(request.body, Body::MovementFrame { .. }),
                    "Old-epoch movement reached the authority"
                );
                // Withhold all later reads: obsolete inputs must retire without a reply.
                if teleported {
                    continue;
                }
                if !teleport && !withheld.is_empty() {
                    withheld.push(response);
                    continue;
                }
                for earlier in withheld.drain(..) {
                    write_frame(&mut socket, &earlier, MAX_RESPONSE_BYTES)
                        .await
                        .unwrap();
                }
                write_frame(&mut socket, &response, MAX_RESPONSE_BYTES)
                    .await
                    .unwrap();
                teleported = teleport;
            }
        });
        let mut client = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[0],
        )
        .await
        .unwrap();
        let entry = client.begin_movement_frames().await.unwrap();
        let Reply::Snapshot { state } = entry.body else {
            panic!("Interval entry refused")
        };
        let baseline = state.movement.unwrap();
        assert_eq!(baseline.profile, crate::movement::Profile::Frames);
        let control = client.control().unwrap().clone();
        let (input, inputs, updates, mut output) = channels();
        input
            .send(Input::MovementFrame {
                token: 1,
                frame: crate::movement::frames::Frame {
                    life: baseline.life,
                    epoch: baseline.epoch,
                    sequence: 0,
                    tick: 0,
                    start: baseline.physics_step,
                    steps: 6,
                    segments: vec![crate::movement::frames::Segment {
                        offset: 0,
                        axes: [0., 0.],
                        yaw: 0.,
                        until: baseline.physics_step + crate::movement::HELD_STEPS,
                        jump: false,
                    }],
                },
            })
            .await
            .unwrap();
        input
            .send(Input::TrackedCommand {
                token: 2,
                life: control.life.into(),
                epoch: control.epoch,
                intent: Intent::Cast {
                    ability: Ability::MistyStep,
                    target: None,
                    aim: [0., 0., 1.],
                },
            })
            .await
            .unwrap();
        for (token, ability) in [(4, Ability::MistyStep), (5, Ability::Bow)] {
            input
                .send(Input::TrackedCommand {
                    token,
                    life: control.life.into(),
                    epoch: control.epoch,
                    intent: Intent::Cast {
                        ability,
                        target: None,
                        aim: [0., 0., 1.],
                    },
                })
                .await
                .unwrap();
        }
        let (stop, stopping) = oneshot::channel();
        let task = tokio::spawn(run(
            client,
            Cursor::new(120),
            NATIVE_CADENCE,
            inputs,
            updates,
            stopping,
        ));
        timeout(Duration::from_secs(3), async {
            let mut accepted = false;
            let mut retired = Vec::new();
            loop {
                match output.recv().await.unwrap() {
                    Update::FrameBound { token: 1, binding } => assert!(binding.is_ok()),
                    Update::CommandBound { token: 2, binding } => assert!(binding.is_ok()),
                    Update::Outcome(response) => {
                        assert!(matches!(response.body, Reply::Accepted));
                        let epoch = response.control.unwrap().epoch;
                        if epoch > control.epoch {
                            accepted = true;
                        } else {
                            assert_eq!(epoch, control.epoch);
                        }
                    }
                    Update::CommandBound {
                        token: token @ (4 | 5),
                        binding,
                    } => {
                        assert!(accepted);
                        assert!(binding.is_err());
                        retired.push(token);
                    }
                    _ => {}
                }
                if retired.len() == 2 {
                    assert_eq!(retired, vec![4, 5]);
                    break;
                }
            }
        })
        .await
        .unwrap();
        let _ = stop.send(());
        task.await.unwrap().unwrap();
        let _ = peer_stop.send(());
        peer.await.unwrap();
    }

    #[tokio::test]
    async fn worker_binds_complete_intervals_and_snapshots_confirm_only_completed_time() {
        use crate::movement::Profile;
        use crate::movement::frames::{Frame, Segment};
        let keys = [key(214), key(215), key(216)];
        let (address, tls, host_stop, host) = start(&keys).await;
        let mut client = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            tls.config().clone(),
            120,
            &keys[0],
        )
        .await
        .unwrap();
        loop {
            let state = client.snapshot().await.unwrap();
            if state
                .movement
                .is_some_and(|b| b.character.support.is_some())
            {
                break;
            }
            client_runtime::sleep(Duration::from_millis(10)).await;
        }
        assert!(matches!(
            client.begin_movement_frames().await.unwrap().body,
            Reply::Snapshot { .. }
        ));
        let state = client.snapshot().await.unwrap();
        let b = state.movement.unwrap();
        assert_eq!(b.profile, Profile::Frames);
        let (input, inputs, updates, mut output) = channels();
        let (stop, stopping) = oneshot::channel();
        let task = tokio::spawn(run(
            client,
            Cursor::new(120),
            NATIVE_CADENCE,
            inputs,
            updates,
            stopping,
        ));
        for index in 0..3u64 {
            input
                .send(Input::MovementFrame {
                    token: index + 1,
                    frame: Frame {
                        life: b.life,
                        epoch: b.epoch,
                        sequence: 0,
                        tick: 0,
                        start: b.physics_step + index * 4,
                        steps: 4,
                        segments: vec![Segment {
                            offset: 0,
                            axes: [1., 0.],
                            yaw: 0.,
                            until: b.physics_step + 60,
                            jump: false,
                        }],
                    },
                })
                .await
                .unwrap();
        }
        let mut bound = 0;
        let mut accepted = 0;
        let mut confirmed = false;
        for _ in 0..30 {
            match timeout(Duration::from_secs(3), output.recv())
                .await
                .unwrap()
                .unwrap()
            {
                Update::FrameBound { token, binding } => {
                    let f = binding.unwrap();
                    bound += 1;
                    assert_eq!(token, bound);
                    assert_eq!(f.sequence, bound);
                    assert_eq!(f.start, b.physics_step + (bound - 1) * 4);
                }
                Update::Outcome(r) => {
                    assert!(matches!(r.body, Reply::Accepted));
                    accepted += 1;
                }
                Update::Snapshot(r) => {
                    if let Reply::Snapshot { state } = r.body {
                        if state.movement.is_some_and(|next| {
                            next.epoch == b.epoch
                                && next.applied_sequence == 3
                                && next.physics_step == b.physics_step + 12
                        }) {
                            confirmed = true;
                        }
                    }
                }
                Update::Events { .. } | Update::Inventory(_) => {}
                _ => panic!("Intervals cannot supersede movement history"),
            }
            if bound == 3 && accepted == 3 && confirmed {
                break;
            }
        }
        assert_eq!((bound, accepted, confirmed), (3, 3, true));
        let _ = stop.send(());
        task.await.unwrap().unwrap();
        let _ = host_stop.send(());
        host.await.unwrap();
    }
    #[tokio::test]
    async fn interval_entry_reads_fresh_credit_without_duplicate_scene_work() {
        use crate::service::net::{
            read_frame,
            tests::{gateway, tls},
            write_frame,
        };
        use crate::service::wire::{MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Request};
        use tokio::net::TcpListener;
        use tokio_rustls::TlsAcceptor;
        let keys = [key(231), key(232), key(233)];
        let mut gateway = gateway(&keys);
        gateway.tick(0.05).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (server_tls, connector) = tls();
        let (reads, mut observed) = mpsc::channel(16);
        let (peer_stop, peer_stopping) = oneshot::channel();
        let peer = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut socket = TlsAcceptor::from(server_tls).accept(socket).await.unwrap();
            let (id, hello) = gateway.open_json(0).unwrap();
            write_frame(&mut socket, &hello, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            let mut entered = false;
            tokio::pin!(peer_stopping);
            loop {
                tokio::select! {
                    _ = &mut peer_stopping => break,
                    bytes = read_frame(&mut socket, MAX_REQUEST_BYTES) => {
                        let Ok(bytes) = bytes else { break };
                        let request = Request::decode(&bytes).unwrap();
                        if entered && matches!(request.body, Body::Snapshot {} | Body::Replicate { .. } | Body::Events { .. } | Body::Inventory {}) {
                            reads.send(()).await.unwrap();
                        }
                        if matches!(request.body, Body::BeginMovementFrames { .. }) {
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                        if matches!(request.body, Body::MovementCredit {}) {
                            gateway.tick(1. / 120.).unwrap();
                        }
                        entered |= matches!(request.body, Body::BeginMovementFrames { .. });
                        let response = gateway.dispatch_json(id, 0, &bytes).unwrap();
                        write_frame(&mut socket, &response, MAX_RESPONSE_BYTES).await.unwrap();
                    }
                }
            }
        });
        let client = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[0],
        )
        .await
        .unwrap();
        let control = client.control().unwrap().clone();
        let (input, inputs, updates, mut output) = channels();
        input
            .send(Input::BeginMovementFrames {
                life: control.life.into(),
                epoch: control.epoch,
            })
            .await
            .unwrap();
        let (stop, stopping) = oneshot::channel();
        let task = tokio::spawn(run(
            client,
            Cursor::new(120),
            NATIVE_CADENCE,
            inputs,
            updates,
            stopping,
        ));
        let entry = async {
            while let Some(update) = output.recv().await {
                if let Update::Snapshot(response) = update {
                    if matches!(response.body, Reply::Snapshot { ref state }
                        if state.movement.is_some_and(|b| b.profile == crate::movement::Profile::Frames))
                    {
                        let Reply::Snapshot { state } = response.body else {
                            unreachable!()
                        };
                        return state.movement.unwrap();
                    }
                }
            }
            panic!("Worker stopped before interval entry");
        };
        let baseline = timeout(Duration::from_secs(3), entry).await.unwrap();
        // Keep consuming other read classes while checking the scene deadline.
        let mut credit_reads = 0;
        let check = async {
            loop {
                tokio::select! {
                    read = observed.recv() => { assert!(read.is_none(), "Entry triggered a duplicate scene read"); return; }
                    update = output.recv() => {
                        let update = update.expect("Worker stopped during entry");
                        if let Update::MovementCredit(response) = update {
                            credit_reads += 1;
                            let control = response.control.unwrap();
                            assert_eq!((control.life.into(), control.epoch), (baseline.life, baseline.epoch));
                            assert!(control.credit_step > baseline.world_step);
                        }
                    }
                }
            }
        };
        assert!(timeout(Duration::from_millis(100), check).await.is_err());
        assert_eq!(
            credit_reads, 1,
            "Entry must supply one small fresh clock read"
        );
        input
            .send(Input::MovementFrame {
                token: 1,
                frame: crate::movement::frames::Frame {
                    life: baseline.life,
                    epoch: baseline.epoch,
                    sequence: 0,
                    tick: 0,
                    start: baseline.physics_step,
                    steps: 4,
                    segments: vec![crate::movement::frames::Segment {
                        offset: 0,
                        axes: [0.; 2],
                        yaw: 0.,
                        until: baseline.physics_step + crate::movement::HELD_STEPS,
                        jump: false,
                    }],
                },
            })
            .await
            .unwrap();
        assert!(
            timeout(Duration::from_millis(50), observed.recv())
                .await
                .unwrap()
                .is_some(),
            "The first frame reply must release periodic reads before the startup timeout"
        );
        let _ = stop.send(());
        task.await.unwrap().unwrap();
        let _ = peer_stop.send(());
        peer.await.unwrap();
    }
    #[tokio::test]
    async fn periodic_reads_leave_a_gameplay_slot_when_replies_are_withheld() {
        withheld_reply_slots(false).await;
    }
    #[tokio::test]
    async fn frame_credit_wait_reserves_the_next_gameplay_burst() {
        withheld_reply_slots(true).await;
    }
    async fn withheld_reply_slots(frames: bool) {
        use crate::service::net::{
            read_frame,
            tests::{gateway, tls},
            write_frame,
        };
        use crate::service::wire::{MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Request};
        use tokio::net::TcpListener;
        use tokio_rustls::TlsAcceptor;
        let keys = [key(224), key(225), key(226)];
        let mut gateway = gateway(&keys);
        gateway.tick(0.05).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (server_tls, connector) = tls();
        let (admitted, mut observed) = mpsc::channel(16);
        let (peer_stop, peer_stopping) = oneshot::channel();
        let peer = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut socket = TlsAcceptor::from(server_tls).accept(socket).await.unwrap();
            let (id, hello) = gateway.open_json(0).unwrap();
            write_frame(&mut socket, &hello, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            let auth = read_frame(&mut socket, MAX_REQUEST_BYTES).await.unwrap();
            let response = gateway.dispatch_json(id, 0, &auth).unwrap();
            write_frame(&mut socket, &response, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            if frames {
                let bytes = read_frame(&mut socket, MAX_REQUEST_BYTES).await.unwrap();
                assert!(matches!(
                    Request::decode(&bytes).unwrap().body,
                    Body::BeginMovementFrames { .. }
                ));
                let response = gateway.dispatch_json(id, 0, &bytes).unwrap();
                write_frame(&mut socket, &response, MAX_RESPONSE_BYTES)
                    .await
                    .unwrap();
            }
            tokio::pin!(peer_stopping);
            loop {
                tokio::select! {
                    _ = &mut peer_stopping => break,
                    bytes = read_frame(&mut socket, MAX_REQUEST_BYTES) => {
                        let Ok(bytes) = bytes else { break };
                        let request = Request::decode(&bytes).unwrap();
                        admitted.send(matches!(request.body, Body::MovementFrame { .. })).await.unwrap();
                    }
                }
            }
        });
        let mut client = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[0],
        )
        .await
        .unwrap();
        if frames {
            let entry = client.begin_movement_frames().await.unwrap();
            assert!(matches!(entry.body, Reply::Snapshot { .. }));
        }
        let control = client.control().unwrap().clone();
        let frame_input = |token| Input::MovementFrame {
            token,
            frame: crate::movement::frames::Frame {
                life: control.life.into(),
                epoch: control.epoch,
                sequence: 0,
                tick: 0,
                start: (token - 1) * 6,
                steps: 6,
                segments: vec![crate::movement::frames::Segment {
                    offset: 0,
                    axes: [0., 0.],
                    yaw: 0.,
                    until: (token - 1) * 6 + crate::movement::HELD_STEPS,
                    jump: false,
                }],
            },
        };
        let (input, inputs, updates, mut output) = channels();
        for token in 1..=5 {
            input.send(frame_input(token)).await.unwrap();
        }
        let (stop, stopping) = oneshot::channel();
        let task = tokio::spawn(run(
            client,
            Cursor::new(120),
            NATIVE_CADENCE,
            inputs,
            updates,
            stopping,
        ));
        let mut commands = 0;
        timeout(Duration::from_secs(2), async {
            while commands < 5 {
                if observed.recv().await.unwrap() {
                    commands += 1;
                }
            }
        })
        .await
        .unwrap();
        // Polling has time to use the remaining observation slots while replies stay withheld.
        client_runtime::sleep(Duration::from_millis(150)).await;
        let total = if frames { 7 } else { 6 };
        for token in 6..=total {
            input.send(frame_input(token)).await.unwrap();
        }
        timeout(Duration::from_millis(300), async {
            while commands < total {
                if observed.recv().await.unwrap() {
                    commands += 1;
                }
            }
        })
        .await
        .expect("Periodic reads occupied the last gameplay slot");
        let mut bound = 0;
        while let Ok(update) = output.try_recv() {
            let Update::FrameBound { binding, .. } = update else {
                panic!("A withheld response was delivered")
            };
            assert!(binding.is_ok());
            bound += 1;
        }
        assert_eq!(bound, total);
        let _ = stop.send(());
        task.await.unwrap().unwrap();
        let _ = peer_stop.send(());
        peer.await.unwrap();
    }

    #[tokio::test]
    async fn worker_pipelines_ordered_actions_before_command_acknowledgments() {
        use crate::service::net::{
            read_frame,
            tests::{gateway, tls},
            write_frame,
        };
        use crate::service::wire::{MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Request};
        use tokio::net::TcpListener;
        use tokio_rustls::TlsAcceptor;
        let keys = [key(204), key(205), key(206)];
        let mut gateway = gateway(&keys)
            .with_items(super::super::items::Catalog {
                version: 1,
                items: vec![super::super::items::Item {
                    id: 1,
                    name: "Pipeline recovery fixture".into(),
                    health: 0,
                    mana: 1,
                }],
            })
            .unwrap();
        gateway
            .grant_reward(super::super::rewards::Transaction {
                acceptance: None,
                instance: 120,
                actor: gateway.game().player_life().actor,
                source: [204; 32],
                experience: 0,
                items: vec![super::super::rewards::Entry { id: 1, count: 1 }],
                quests: vec![],
                spent: vec![],
                outfit: None,
                equipment: None,
            })
            .unwrap();
        let actor = gateway.game().player_life().actor;
        let source = gateway.game().player_source(actor).unwrap();
        gateway
            .chamber
            .game
            .simulation
            .spend_mana_for(source, 1)
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (server_tls, connector) = tls();
        let (peer_stop, peer_stopped) = oneshot::channel();
        let peer = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let mut socket = TlsAcceptor::from(server_tls).accept(socket).await.unwrap();
            let (id, hello) = gateway.open_json(0).unwrap();
            write_frame(&mut socket, &hello, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            let auth = read_frame(&mut socket, MAX_REQUEST_BYTES).await.unwrap();
            let response = gateway.dispatch_json(id, 0, &auth).unwrap();
            write_frame(&mut socket, &response, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            let mut commands = Vec::new();
            let mut held = Vec::new();
            let mut actions = Vec::new();
            while actions.len() < 4 {
                let bytes = timeout(
                    Duration::from_secs(3),
                    read_frame(&mut socket, MAX_REQUEST_BYTES),
                )
                .await
                .unwrap()
                .unwrap();
                let request = Request::decode(&bytes).unwrap();
                match request.body {
                    Body::Command { command } => {
                        actions.push("command");
                        commands.push(command);
                    }
                    Body::UseItem { item, .. } => {
                        assert_eq!(item, 1);
                        actions.push("item");
                    }
                    _ => {}
                }
                let response = gateway.dispatch_json(id, 0, &bytes).unwrap();
                if actions.last() == Some(&"item") {
                    let response =
                        serde_json::from_slice::<super::super::wire::Response>(&response).unwrap();
                    assert!(
                        matches!(response.body, Reply::ItemUsed { .. }),
                        "{response:?}"
                    );
                }
                if commands.is_empty() {
                    write_frame(&mut socket, &response, MAX_RESPONSE_BYTES)
                        .await
                        .unwrap();
                } else {
                    held.push(response);
                }
            }
            assert_eq!(actions, vec!["command", "command", "item", "command"]);
            assert_eq!(
                gateway
                    .character_rewards(gateway.game().player_life().actor)
                    .unwrap()
                    .items
                    .get(&1),
                None
            );
            assert_eq!(
                commands.iter().map(|c| c.sequence).collect::<Vec<_>>(),
                vec![1, 2, 3]
            );
            assert!(matches!(
                commands[0].intent,
                crate::service::wire::Action::Move { .. }
            ));
            assert!(matches!(
                commands[1].intent,
                crate::service::wire::Action::Jump {}
            ));
            assert!(matches!(
                commands[2].intent,
                crate::service::wire::Action::Move { .. }
            ));
            for response in held {
                write_frame(&mut socket, &response, MAX_RESPONSE_BYTES)
                    .await
                    .unwrap();
            }
            let _ = peer_stopped.await;
        });
        let client = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[0],
        )
        .await
        .unwrap();
        let control = client.control().unwrap().clone();
        let (input, inputs, updates, mut output) = channels();
        for (index, intent) in [
            Intent::Move {
                axes: [0., 0.],
                yaw: 0.,
            },
            Intent::Jump,
            Intent::Move {
                axes: [0., 0.],
                yaw: 0.,
            },
        ]
        .into_iter()
        .enumerate()
        {
            if index == 2 {
                input.send(Input::UseItem(1)).await.unwrap();
            }
            input
                .send(Input::TrackedCommand {
                    token: index as u64 + 1,
                    life: control.life.into(),
                    epoch: control.epoch,
                    intent,
                })
                .await
                .unwrap();
        }
        drop(input);
        let (_stop, stopped) = oneshot::channel();
        let observer = Observer::default();
        let worker = tokio::spawn(run_profiled(
            client,
            Cursor::new(120),
            NATIVE_CADENCE,
            inputs,
            updates,
            stopped,
            observer.clone(),
        ));
        let mut bound = 0;
        let mut outcomes = 0;
        timeout(Duration::from_secs(4), async {
            while outcomes < 4 {
                match output.recv().await.unwrap() {
                    Update::CommandBound { token, binding } => {
                        bound += 1;
                        assert_eq!(token, bound);
                        assert_eq!(binding.unwrap().sequence, bound);
                    }
                    Update::Outcome(response) => {
                        assert_eq!(bound, 3);
                        assert!(matches!(
                            response.body,
                            Reply::Accepted | Reply::ItemUsed { .. }
                        ));
                        outcomes += 1;
                    }
                    Update::MovementSuperseded { .. } => panic!("Movement crossed a jump barrier"),
                    _ => {}
                }
            }
        })
        .await
        .unwrap();
        timeout(Duration::from_secs(3), worker)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let observations = observer.drain();
        assert!(observations.samples.len() >= 3);
        assert!(
            observations
                .samples
                .iter()
                .all(|sample| sample.turnaround_ms.is_finite()
                    && sample.turnaround_ms >= 0.
                    && sample.started_at <= sample.verified_at)
        );
        peer_stop.send(()).unwrap();
        peer.await.unwrap();
    }

    #[test]
    fn verified_control_reuse_requires_recent_exact_life_and_epoch() {
        let life = verse_engine::core::LifeId {
            instance: 120,
            actor: 14,
            generation: 0,
        };
        let input = Input::TrackedCommand {
            token: 1,
            life,
            epoch: 1,
            intent: Intent::Jump,
        };
        let mut control = super::super::wire::Control {
            credit_step: 0,
            world_step: 0,
            life: life.into(),
            epoch: 1,
            accepted_sequence: 0,
            applied_movement: None,
            dynamic: Vec::new(),
        };
        assert!(fresh_control(&input, Some(&control), Some(Instant::now())));
        assert!(!fresh_control(&input, Some(&control), None));
        assert!(!fresh_control(&input, None, Some(Instant::now())));
        assert!(!fresh_control(
            &input,
            Some(&control),
            Some(Instant::now() - Duration::from_millis(75))
        ));
        control.epoch += 1;
        assert!(!fresh_control(&input, Some(&control), Some(Instant::now())));
        control.epoch = 1;
        control.life.generation += 1;
        assert!(!fresh_control(&input, Some(&control), Some(Instant::now())));
        assert!(!fresh_control(
            &Input::Respawn,
            Some(&control),
            Some(Instant::now())
        ));
        control.life.generation = life.generation;
        let begin = Input::BeginMovementFrames { life, epoch: 1 };
        assert!(fresh_control(&begin, Some(&control), Some(Instant::now())));
        control.epoch += 1;
        assert!(!fresh_control(&begin, Some(&control), Some(Instant::now())));
        control.epoch = 1;
        control.life.generation += 1;
        assert!(!fresh_control(&begin, Some(&control), Some(Instant::now())));
        control.life.generation = life.generation;
        for input in [
            begin,
            Input::UseItem(1),
            Input::EquipGear(super::super::equipment::Slot::Head, 1),
            Input::EquipOutfit(1),
            Input::ClaimQuest(1),
            Input::AcceptQuest(1, life),
        ] {
            assert!(fresh_control(&input, Some(&control), Some(Instant::now())));
            assert!(!fresh_control(&input, None, Some(Instant::now())));
            assert!(!fresh_control(&input, Some(&control), None));
            assert!(!fresh_control(
                &input,
                Some(&control),
                Some(Instant::now() - Duration::from_millis(75))
            ));
        }
    }
    #[test]
    fn unsent_movement_coalesces_without_crossing_actions_or_control_fences() {
        let life = verse_engine::core::LifeId {
            instance: 120,
            actor: 14,
            generation: 0,
        };
        let make = |token, epoch, intent| Input::TrackedCommand {
            token,
            life,
            epoch,
            intent,
        };
        let movement = || Intent::Move {
            axes: [1., 0.],
            yaw: 0.,
        };
        let (send, mut inputs) = mpsc::channel(INPUT_CAPACITY);
        send.try_send(make(2, 1, movement())).unwrap();
        send.try_send(make(3, 1, movement())).unwrap();
        send.try_send(make(4, 1, Intent::Jump)).unwrap();
        send.try_send(make(5, 1, movement())).unwrap();
        let mut deferred = None;
        let (last, retired) = coalesce_movement(make(1, 1, movement()), &mut inputs, &mut deferred);
        assert_eq!(retired, vec![1, 2]);
        assert!(matches!(last, Input::TrackedCommand { token: 3, .. }));
        assert!(matches!(
            deferred.take(),
            Some(Input::TrackedCommand {
                token: 4,
                intent: Intent::Jump,
                ..
            })
        ));
        assert!(matches!(
            inputs.try_recv().unwrap(),
            Input::TrackedCommand { token: 5, .. }
        ));
        for (token, epoch) in [(6, 2), (6, 1), (5, 1)] {
            send.try_send(make(token, epoch, movement())).unwrap();
            let (_, retired) =
                coalesce_movement(make(6, 1, movement()), &mut inputs, &mut deferred);
            assert!(retired.is_empty());
            assert!(deferred.take().is_some());
        }
        send.try_send(Input::Command(Intent::Cast {
            ability: Ability::Shield,
            target: None,
            aim: [0., 0., 1.],
        }))
        .unwrap();
        let (_, retired) = coalesce_movement(make(7, 1, movement()), &mut inputs, &mut deferred);
        assert!(retired.is_empty());
        assert!(matches!(
            deferred,
            Some(Input::Command(Intent::Cast { .. }))
        ));
    }
    #[tokio::test]
    async fn pending_ack_refreshes_staged_cast_without_an_extra_scene_request() {
        use crate::service::net::{read_frame, tests::gateway, write_frame};
        use crate::service::wire::{MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Request};
        let keys = [key(241), key(242), key(243)];
        let mut gateway = gateway(&keys);
        let (socket, mut peer_socket) = tokio::io::duplex(1024 * 1024);
        let (first_admitted, admitted) = oneshot::channel();
        let (cast_staged, staged) = oneshot::channel();
        let peer = tokio::spawn(async move {
            let (id, hello) = gateway.open_json(0).unwrap();
            write_frame(&mut peer_socket, &hello, MAX_RESPONSE_BYTES)
                .await
                .unwrap();
            let mut first_admitted = Some(first_admitted);
            let mut staged = Some(staged);
            let mut commands = 0;
            loop {
                let bytes = read_frame(&mut peer_socket, MAX_REQUEST_BYTES)
                    .await
                    .unwrap();
                let request = Request::decode(&bytes).unwrap();
                let reply = gateway.dispatch_json(id, 0, &bytes).unwrap();
                if let Body::Command { command } = &request.body {
                    commands += 1;
                    assert_eq!(command.sequence, commands);
                    if commands == 1 {
                        first_admitted.take().unwrap().send(()).unwrap();
                        staged.take().unwrap().await.unwrap();
                        assert!(
                            timeout(
                                Duration::from_millis(100),
                                read_frame(&mut peer_socket, MAX_REQUEST_BYTES)
                            )
                            .await
                            .is_err(),
                            "Staged cast requested a scene despite an outstanding control acknowledgment"
                        );
                    } else {
                        assert!(matches!(
                            command.intent,
                            super::super::wire::Action::Cast {
                                ability: Ability::Shield,
                                ..
                            }
                        ));
                    }
                    let response: Response = serde_json::from_slice(&reply).unwrap();
                    assert!(matches!(response.body, Reply::Accepted));
                }
                write_frame(&mut peer_socket, &reply, MAX_RESPONSE_BYTES)
                    .await
                    .unwrap();
                if commands == 2 {
                    break;
                }
            }
        });
        let client = Client::connect_stream(Box::new(socket), 120, None, &keys[0])
            .await
            .unwrap();
        let control = client.control().unwrap().clone();
        let (input, inputs, updates, mut output) = channels();
        let (stop, stopping) = oneshot::channel();
        let task = tokio::spawn(run(
            client,
            Cursor::new(120),
            Duration::from_secs(1),
            inputs,
            updates,
            stopping,
        ));
        timeout(Duration::from_secs(3), async {
            while !matches!(output.recv().await.unwrap(), Update::Inventory(_)) {}
        })
        .await
        .unwrap();
        input
            .send(Input::TrackedCommand {
                token: 1,
                life: control.life.into(),
                epoch: control.epoch,
                intent: Intent::Move {
                    axes: [0., 0.],
                    yaw: 0.,
                },
            })
            .await
            .unwrap();
        timeout(Duration::from_secs(3), admitted)
            .await
            .unwrap()
            .unwrap();
        tokio::time::sleep(Duration::from_millis(75)).await;
        input
            .send(Input::TrackedCommand {
                token: 2,
                life: control.life.into(),
                epoch: control.epoch,
                intent: Intent::Cast {
                    ability: Ability::Shield,
                    target: None,
                    aim: [0., 0., 1.],
                },
            })
            .await
            .unwrap();
        cast_staged.send(()).unwrap();
        let mut outcomes = 0;
        let mut tokens = Vec::new();
        timeout(Duration::from_secs(3), async {
            while outcomes < 2 {
                match output.recv().await.unwrap() {
                    Update::CommandBound { token, binding } => {
                        assert!(binding.is_ok());
                        tokens.push(token);
                    }
                    Update::Outcome(response) => {
                        assert!(matches!(response.body, Reply::Accepted));
                        outcomes += 1;
                    }
                    _ => {}
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(tokens, vec![1, 2]);
        peer.await.unwrap();
        let _ = stop.send(());
        task.await.unwrap().unwrap();
    }
    #[tokio::test]
    async fn movement_arriving_during_refresh_replaces_the_unsent_command() {
        let keys = [key(71), key(72), key(73)];
        let (address, connector, server_stop, server) = start(&keys).await;
        let mut client = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[0],
        )
        .await
        .unwrap();
        client.snapshot().await.unwrap();
        let control = client.control().unwrap().clone();
        let (input, inputs) = mpsc::channel(INPUT_CAPACITY);
        let (updates, mut output) = mpsc::channel(1);
        let gate = updates.clone();
        let (stop, stopped) = oneshot::channel();
        let worker = tokio::spawn(run(
            client,
            Cursor::new(120),
            Duration::from_secs(1),
            inputs,
            updates,
            stopped,
        ));
        timeout(Duration::from_secs(3), async {
            while !matches!(output.recv().await.unwrap(), Update::Inventory(_)) {}
        })
        .await
        .unwrap();
        client_runtime::sleep(Duration::from_millis(75)).await;
        gate.send(Update::CommandBound {
            token: 0,
            binding: Err("Test delivery barrier".into()),
        })
        .await
        .unwrap();
        let movement = |token| Input::TrackedCommand {
            token,
            life: control.life.into(),
            epoch: control.epoch,
            intent: Intent::Move {
                axes: if token == 1 { [1., 0.] } else { [0., 0.] },
                yaw: 0.,
            },
        };
        input.send(movement(1)).await.unwrap();
        client_runtime::sleep(Duration::from_millis(20)).await;
        input.send(movement(2)).await.unwrap();
        assert!(matches!(
            output.recv().await.unwrap(),
            Update::CommandBound { token: 0, .. }
        ));
        let mut retired = false;
        let mut bound = false;
        timeout(Duration::from_secs(3), async {
            loop {
                match output.recv().await.unwrap() {
                    Update::MovementSuperseded {
                        token: 1,
                        replacement: 2,
                    } => {
                        retired = true;
                    }
                    Update::CommandBound { token: 2, binding } => {
                        assert!(retired);
                        let command = binding.unwrap();
                        assert!(matches!(
                            command.intent,
                            Intent::Move { axes: [0., 0.], .. }
                        ));
                        bound = true;
                    }
                    Update::Outcome(response) => {
                        assert!(retired && bound);
                        assert!(matches!(response.body, Reply::Accepted));
                        break;
                    }
                    Update::FrameBound { .. } | Update::CommandBound { .. } => {
                        panic!("Unexpected binding")
                    }
                    _ => {}
                }
            }
        })
        .await
        .unwrap();
        stop.send(()).unwrap();
        worker.await.unwrap().unwrap();
        server_stop.send(()).unwrap();
        assert!(server.await.unwrap().failure.is_none());
    }
    #[tokio::test]
    async fn coalesced_tls_inputs_bind_only_latest_moves_and_preserve_jump_order() {
        let keys = [key(81), key(82), key(83)];
        let (address, connector, server_stop, server) = start(&keys).await;
        let mut client = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[0],
        )
        .await
        .unwrap();
        client.snapshot().await.unwrap();
        let control = client.control().unwrap().clone();
        let (input, inputs, updates, mut output) = channels();
        for token in 1..=5 {
            input
                .try_send(Input::TrackedCommand {
                    token,
                    life: control.life.into(),
                    epoch: control.epoch,
                    intent: if token == 4 {
                        Intent::Jump
                    } else {
                        Intent::Move {
                            axes: [0., 0.],
                            yaw: 0.,
                        }
                    },
                })
                .unwrap();
        }
        let (stop, stopped) = oneshot::channel();
        let worker = tokio::spawn(run(
            client,
            Cursor::new(120),
            NATIVE_CADENCE,
            inputs,
            updates,
            stopped,
        ));
        let mut tokens = vec![];
        let mut outcomes = 0;
        let mut sequences = vec![];
        timeout(Duration::from_secs(5), async {
            while outcomes < 3 {
                match output.recv().await.unwrap() {
                    Update::MovementSuperseded { token, replacement } => {
                        assert!(token < 3);
                        assert_eq!(replacement, 3);
                        tokens.push(token);
                    }
                    Update::CommandBound { token, binding } => {
                        tokens.push(token);
                        let command = binding.unwrap();
                        sequences.push(command.sequence);
                        assert_eq!(matches!(command.intent, Intent::Jump), token == 4);
                    }
                    Update::Outcome(response) => {
                        assert!(matches!(response.body, Reply::Accepted));
                        outcomes += 1;
                    }
                    _ => {}
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(tokens, vec![1, 2, 3, 4, 5]);
        assert_eq!(
            sequences,
            vec![
                control.accepted_sequence + 1,
                control.accepted_sequence + 2,
                control.accepted_sequence + 3
            ]
        );
        stop.send(()).unwrap();
        worker.await.unwrap().unwrap();
        server_stop.send(()).unwrap();
        assert!(server.await.unwrap().failure.is_none());
    }
    #[tokio::test]
    async fn tracked_inputs_bind_before_outcomes_and_reject_stale_control() {
        let keys = [key(91), key(92), key(93)];
        let (address, connector, server_stop, server) = start(&keys).await;
        let mut client = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[0],
        )
        .await
        .unwrap();
        client.snapshot().await.unwrap();
        let control = client.control().unwrap().clone();
        let (input, inputs, updates, mut output) = channels();
        let (stop, stopped) = oneshot::channel();
        let worker = tokio::spawn(run(
            client,
            Cursor::new(120),
            NATIVE_CADENCE,
            inputs,
            updates,
            stopped,
        ));
        let mut sequence = control.accepted_sequence;
        for (token, epoch, valid) in [
            (1, control.epoch, true),
            (1, control.epoch, false),
            (2, control.epoch + 1, false),
            (3, control.epoch, true),
        ] {
            input
                .send(Input::TrackedCommand {
                    token,
                    life: control.life.into(),
                    epoch,
                    intent: Intent::Move {
                        axes: [0., 0.],
                        yaw: 0.,
                    },
                })
                .await
                .unwrap();
            timeout(Duration::from_secs(3), async {
                let mut bound = false;
                loop {
                    match output.recv().await.unwrap() {
                        Update::CommandBound {
                            token: received,
                            binding,
                        } => {
                            assert_eq!(received, token);
                            assert_eq!(binding.is_ok(), valid);
                            if let Ok(command) = binding {
                                assert_eq!(command.epoch, control.epoch);
                                assert_eq!(command.sequence, sequence + 1);
                                bound = true;
                            } else {
                                break;
                            }
                        }
                        Update::Outcome(response) => {
                            assert!(valid && bound);
                            assert!(matches!(response.body, Reply::Accepted));
                            sequence += 1;
                            break;
                        }
                        _ => {}
                    }
                }
            })
            .await
            .unwrap();
        }
        stop.send(()).unwrap();
        worker.await.unwrap().unwrap();
        server_stop.send(()).unwrap();
        assert!(server.await.unwrap().failure.is_none());
    }

    #[tokio::test]
    async fn native_inventory_polling_is_bounded_and_excludes_spectators() {
        for player in [true, false] {
            let keys = [key(81), key(82), key(83)];
            let (address, connector, server_stop, server) = start(&keys).await;
            let client = Client::connect(
                address,
                ServerName::try_from("localhost").unwrap(),
                connector.config().clone(),
                120,
                &keys[if player { 0 } else { 2 }],
            )
            .await
            .unwrap();
            let (_input, inputs, updates, mut output) = channels();
            let (stop, stopped) = oneshot::channel();
            let task = tokio::spawn(run(
                client,
                Cursor::new(120),
                NATIVE_CADENCE,
                inputs,
                updates,
                stopped,
            ));
            let mut inventories = 0;
            let mut snapshots = 0;
            let deadline = client_runtime::Instant::now() + Duration::from_millis(1250);
            loop {
                tokio::select! {
                    _=client_runtime::sleep_until(deadline)=>break,
                    update=output.recv()=>match update.unwrap() {
                        Update::Snapshot(_)=>snapshots+=1,
                        Update::Inventory(response)=> {assert!(player);let Reply::Inventory{inventory}=response.body else {panic!("Missing inventory");};assert_eq!(inventory.experience,0);inventories+=1;},
                        Update::Events{..}=>{},
                        Update::FrameBound { .. } | Update::MovementSuperseded { .. }
                    | Update::CommandBound { .. } | Update::Outcome(_) | Update::MovementCredit(_)=>panic!("No player commands submitted"),
                    }
                }
            }
            assert!(snapshots > 0);
            assert!(if player {
                (1..=2).contains(&inventories)
            } else {
                inventories == 0
            });
            stop.send(()).unwrap();
            assert!(task.await.unwrap().is_ok());
            server_stop.send(()).unwrap();
            assert!(server.await.unwrap().failure.is_none());
        }
    }
    #[tokio::test]
    async fn native_polling_and_sustained_movement_fit_the_tls_request_budget() {
        let keys = [key(81), key(82), key(83)];
        let (address, connector, server_stop, server) = start(&keys).await;
        let client = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[0],
        )
        .await
        .unwrap();
        let (input, inputs, updates, mut output) = channels();
        let (stop, stopped) = oneshot::channel();
        let worker = tokio::spawn(run(
            client,
            Cursor::new(120),
            NATIVE_CADENCE,
            inputs,
            updates,
            stopped,
        ));
        let feeder = tokio::spawn(async move {
            let mut clock = client_runtime::interval(Duration::from_millis(33));
            clock.set_missed_tick_behavior(client_runtime::MissedTickBehavior::Skip);
            for index in 0..90 {
                clock.tick().await;
                input
                    .send(Input::Command(Intent::Move {
                        axes: [0., 0.],
                        yaw: 0.,
                    }))
                    .await
                    .unwrap();
                if index % 30 == 0 {
                    input
                        .send(Input::Command(Intent::Cast {
                            ability: Ability::Shield,
                            target: None,
                            aim: [0., 0., 1.],
                        }))
                        .await
                        .unwrap();
                }
            }
            input
        });
        let mut outcomes = 0;
        let mut accepted = 0;
        let mut snapshots = 0;
        let mut event_pages = Vec::new();
        timeout(Duration::from_secs(8), async {
            while outcomes < 93 {
                match output
                    .recv()
                    .await
                    .expect("Worker disconnected during sustained input")
                {
                    Update::Snapshot(_) => snapshots += 1,
                    Update::Outcome(response) => {
                        outcomes += 1;
                        if matches!(response.body, Reply::Accepted) {
                            accepted += 1;
                        }
                    }
                    Update::Events { .. } => event_pages.push(Instant::now()),
                    Update::MovementCredit(_) => panic!("No interval entry submitted"),
                    Update::MovementSuperseded { .. }
                    | Update::FrameBound { .. }
                    | Update::CommandBound { .. }
                    | Update::Inventory(_) => {}
                }
            }
        })
        .await
        .unwrap();
        let input = feeder.await.unwrap();
        // Raw moves refresh control; casts can reuse a recent verified acknowledgment.
        assert!(snapshots >= 90);
        assert!(accepted >= 90);
        assert!(event_pages.len() >= 10);
        assert!(
            event_pages
                .windows(2)
                .all(|pages| pages[1].duration_since(pages[0]) >= Duration::from_millis(180))
        );
        stop.send(()).unwrap();
        assert!(worker.await.unwrap().is_ok());
        drop(input);
        server_stop.send(()).unwrap();
        assert!(server.await.unwrap().failure.is_none());
    }

    #[tokio::test]
    async fn tls_worker_polls_admits_commands_and_stops_under_backpressure() {
        let keys = [key(71), key(72), key(73)];
        let (address, connector, server_stop, server) = start(&keys).await;
        let client = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[0],
        )
        .await
        .unwrap();
        let (input, inputs, updates, mut output) = channels();
        let (stop, stopped) = oneshot::channel();
        let worker = tokio::spawn(run(
            client,
            Cursor::new(120),
            Duration::from_millis(33),
            inputs,
            updates,
            stopped,
        ));
        input
            .send(Input::Command(Intent::Cast {
                ability: Ability::Shield,
                target: None,
                aim: [0., 0., 1.],
            }))
            .await
            .unwrap();
        let mut replica = Buffer::new(120, 5.).unwrap();
        let mut accepted = false;
        let mut shield = false;
        let mut events = false;
        for _ in 0..16 {
            match timeout(Duration::from_secs(2), output.recv())
                .await
                .unwrap()
                .unwrap()
            {
                Update::Snapshot(r) => {
                    replica.push(&r).unwrap();
                    if let Reply::Snapshot { state } = r.body {
                        shield |= state.presentation.effects.iter().any(|e| e.shield > 0);
                    }
                }
                Update::Events { checkpoint, .. } => {
                    Cursor::restore(&checkpoint, 120).unwrap();
                    events = true;
                }
                Update::Inventory(r) => {
                    assert!(matches!(r.body, Reply::Inventory { .. }));
                }
                Update::FrameBound { .. }
                | Update::MovementSuperseded { .. }
                | Update::CommandBound { .. } => {
                    panic!("No tracked commands submitted")
                }
                Update::MovementCredit(_) => panic!("No interval entry submitted"),
                Update::Outcome(r) => {
                    assert!(matches!(r.body, Reply::Accepted));
                    accepted = true;
                }
            }
            if accepted && shield && events {
                break;
            }
        }
        assert!(accepted && shield && events);
        // Let the bounded output queue fill, then stop without draining it.
        client_runtime::sleep(Duration::from_millis(200)).await;
        stop.send(()).unwrap();
        timeout(Duration::from_secs(1), worker)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        server_stop.send(()).unwrap();
        assert!(server.await.unwrap().failure.is_none());
    }
    #[tokio::test]
    async fn spectator_worker_checks_context_budgets_and_consumer_lifetime() {
        let keys = [key(74), key(75), key(76)];
        let (address, connector, server_stop, server) = start(&keys).await;
        for case in 0..4 {
            let client = Client::connect(
                address,
                ServerName::try_from("localhost").unwrap(),
                connector.config().clone(),
                120,
                &keys[2],
            )
            .await
            .unwrap();
            let (_, inputs) = mpsc::channel(if case == 2 {
                INPUT_CAPACITY + 1
            } else {
                INPUT_CAPACITY
            });
            let (updates, _) = mpsc::channel(if case == 3 {
                UPDATE_CAPACITY + 1
            } else {
                UPDATE_CAPACITY
            });
            let (_stop, stopped) = oneshot::channel();
            assert!(
                run(
                    client,
                    Cursor::new(if case == 1 { 121 } else { 120 }),
                    if case == 0 {
                        Duration::ZERO
                    } else {
                        Duration::from_millis(33)
                    },
                    inputs,
                    updates,
                    stopped
                )
                .await
                .is_err()
            );
        }
        let client = Client::connect(
            address,
            ServerName::try_from("localhost").unwrap(),
            connector.config().clone(),
            120,
            &keys[2],
        )
        .await
        .unwrap();
        let (_input, inputs, updates, mut output) = channels();
        let (_stop, stopped) = oneshot::channel();
        let worker = tokio::spawn(run(
            client,
            Cursor::new(120),
            Duration::from_millis(33),
            inputs,
            updates,
            stopped,
        ));
        let Update::Snapshot(response) = timeout(Duration::from_secs(2), output.recv())
            .await
            .unwrap()
            .unwrap()
        else {
            panic!("Expected spectator snapshot")
        };
        assert!(response.control.is_none());
        drop(output);
        assert!(
            timeout(Duration::from_secs(1), worker)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        server_stop.send(()).unwrap();
        assert!(server.await.unwrap().failure.is_none());
    }
}
