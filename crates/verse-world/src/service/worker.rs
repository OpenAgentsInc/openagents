//! Bounded duplex network worker for remote presentation adapters.
#[cfg(test)]
#[path = "worker_delayed.rs"]
mod delayed;
use super::{
    client::Client,
    event_cursor::{Cursor, Delivery},
    wire::{Body, Reply, Response},
};
use crate::{Command, Intent, play::Ability};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

/// Local measurements contain no principal, command content, or credentials.
#[derive(Clone, Copy)]
pub struct Observation {
    pub kind: &'static str,
    pub turnaround_ms: f64,
    pub verified_at: Instant,
    pub started_at: Instant,
    pub accepted_snapshot: bool,
    pub pending_requests: usize,
    pub queued_inputs: usize,
    pub queued_updates: usize,
}
#[derive(Default)]
pub struct Observations {
    pub samples: Vec<Observation>,
    pub omitted: u64,
    pub snapshot_verified_at: Option<Instant>,
}
/// Optional read-only telemetry; a full buffer never backpressures authority updates.
#[derive(Clone, Default)]
pub struct Observer(std::sync::Arc<std::sync::Mutex<Observations>>);
impl Observer {
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
        Observations {
            samples: std::mem::take(&mut state.samples),
            omitted: state.omitted,
            snapshot_verified_at: state.snapshot_verified_at,
        }
    }
}

pub const INPUT_CAPACITY: usize = 32;
pub const UPDATE_CAPACITY: usize = 8;
/// Leaves request capacity for 30 Hz input refreshes and spell commands.
pub const NATIVE_CADENCE: Duration = Duration::from_millis(50);

fn periodic_read_room(pending: usize, queued: bool) -> bool {
    !queued && pending < super::client::PIPELINE_CAPACITY - 1
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
    Events {
        delivery: Delivery,
        checkpoint: Vec<u8>,
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
    until: [tokio::time::Instant; 3],
    since: [Option<tokio::time::Instant>; 3],
}
impl ReadBackoff {
    fn new(now: tokio::time::Instant) -> Self {
        Self {
            until: [now; 3],
            since: [None; 3],
        }
    }
    fn ready(&self, class: usize, now: tokio::time::Instant) -> bool {
        now >= self.until[class]
    }
    fn observe(
        &mut self,
        class: usize,
        reply: &Reply,
        now: tokio::time::Instant,
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

/// Reuses only recent verified response control; server admission still checks every command.
fn fresh_control(
    input: &Input,
    control: Option<&super::wire::Control>,
    observed: Option<Instant>,
) -> bool {
    let (life, epoch) = match input {
        Input::TrackedCommand { life, epoch, .. } => (*life, *epoch),
        Input::MovementFrame { frame, .. } => (frame.life, frame.epoch),
        Input::Command(Intent::Cast { .. }) => match control {
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
        let mut client = client.pipeline()?;
        let mut interval = tokio::time::interval(cadence);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut next_inventory = tokio::time::Instant::now();
        let mut inventory_life = None;
        let mut last_token = 0;
        let mut deferred = None;
        let mut staged = None;
        let mut last_response = client.verified_at();
        let mut snapshot_pending = false;
        let mut last_snapshot_sent = None;
        let mut events_pending = false;
        let mut inventory_pending = false;
        let mut refreshed = false;
        let mut barrier = false;
        let mut input_closed = false;
        let mut read_backoff = ReadBackoff::new(tokio::time::Instant::now());
        loop {
            if !client.available() && client.pending() == 0 {
                return Err("Chamber pipeline is disconnected".into());
            }
            if input_closed && client.pending() == 0 {
                return Ok(());
            }
            if staged.is_some() && client.available() && !barrier {
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
                let lifecycle = !matches!(
                    input,
                    Input::Command(_) | Input::TrackedCommand { .. } | Input::MovementFrame { .. }
                ) || matches!(
                    input,
                    Input::Command(Intent::Cast {
                        ability: Ability::MistyStep,
                        ..
                    })
                );
                if lifecycle && client.pending() > 0 {
                    staged = Some(input);
                } else if !refreshed && !fresh_control(&input, client.control(), last_response) {
                    if !snapshot_pending && read_backoff.ready(0, tokio::time::Instant::now()) {
                        client.send_snapshot()?;
                        last_snapshot_sent = Some(tokio::time::Instant::now());
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
                            next_inventory = tokio::time::Instant::now();
                            match action {
                                Input::Respawn => Body::Respawn { life: control.life },
                                Input::BeginMovementFrames { life, epoch } => {
                                    Body::BeginMovementFrames {
                                        life: life.into(),
                                        epoch,
                                    }
                                }
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
                    client.send(body)?;
                    barrier = lifecycle;
                }
            }
            // Schedule from the actual send time so quick replies cannot miss a phased polling tick.
            let snapshot_due =
                last_snapshot_sent.map_or_else(tokio::time::Instant::now, |sent| sent + cadence);
            tokio::select! {
                _ = tokio::time::sleep_until(snapshot_due), if !input_closed && !barrier
                    && staged.is_none() && client.available() && !snapshot_pending
                    && periodic_read_room(client.pending(), !inputs.is_empty() || deferred.is_some())
                    && last_snapshot_sent.is_some() && read_backoff.ready(0, tokio::time::Instant::now()) => {
                    client.send_snapshot()?;
                    last_snapshot_sent = Some(tokio::time::Instant::now());
                    snapshot_pending = true;
                }
                response = client.receive(), if client.pending() > 0 => {
                    let (body,response) = response?;
                    last_response = Some(Instant::now());
                    let entry = matches!(&body,Body::BeginMovementFrames{..}).then(||response.clone());
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
                            pending_requests: client.pending(),
                            queued_inputs: inputs.len(), queued_updates: updates.max_capacity() - updates.capacity() });
                    }
                    let update = match body {
                        Body::Snapshot {} | Body::Replicate {..} => {
                            snapshot_pending = false;
                            if read_backoff.observe(0, &response.body, tokio::time::Instant::now())? {
                                refreshed = false;
                                continue;
                            }
                            refreshed = staged.is_some();
                            Update::Snapshot(response)
                        }
                        Body::Events { after,limit } => {
                            events_pending = false;
                            if read_backoff.observe(1, &response.body, tokio::time::Instant::now())? { continue; }
                            let delivery = cursor.admit(&response,after,limit)?;
                            Update::Events { delivery, checkpoint: cursor.checkpoint()? }
                        }
                        Body::Inventory {} => {
                            inventory_pending = false;
                            if read_backoff.observe(2, &response.body, tokio::time::Instant::now())? { continue; }
                            inventory_life = client.control().map(|c| c.life);
                            next_inventory = tokio::time::Instant::now() + Duration::from_secs(1);
                            Update::Inventory(response)
                        }
                        Body::MovementFrame { frame } => {
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
                            }
                            Update::Outcome(response)
                        },
                        _ => { barrier = false; Update::Outcome(response) }
                    };
                    updates.send(update).await.map_err(|_| "Chamber update consumer closed")?;
                    if let Some(entry)=entry { if matches!(entry.body,Reply::Snapshot {..}) { updates.send(Update::Snapshot(entry)).await.map_err(|_| "Chamber update consumer closed")?; } }

                }
                _ = interval.tick() => {
                    // One outstanding request per read class bounds stale work and event cursors.
                    // A staged lifecycle action drains previous IO before changing its context.
                    if input_closed || barrier || staged.is_some() { continue; }
                    if last_snapshot_sent.is_none() && client.available() && !snapshot_pending
                        && read_backoff.ready(0, tokio::time::Instant::now()) {
                        client.send_snapshot()?;
                        last_snapshot_sent = Some(tokio::time::Instant::now());
                        snapshot_pending = true;
                    }
                    if client.available() && periodic_read_room(client.pending(), !inputs.is_empty() || deferred.is_some())
                        && !events_pending && read_backoff.ready(1, tokio::time::Instant::now()) {
                        client.send(Body::Events { after: cursor.after(), limit: 64 })?;
                        events_pending = true;
                    }
                    let life = client.control().map(|c| c.life);
                    if client.available() && periodic_read_room(client.pending(), !inputs.is_empty() || deferred.is_some())
                        && !inventory_pending && life.is_some()
                        && read_backoff.ready(2, tokio::time::Instant::now())
                        && (life != inventory_life || tokio::time::Instant::now() >= next_inventory) {
                        client.send(Body::Inventory {})?;
                        inventory_pending = true;
                    }
                }
                input = async {
                    match deferred.take() { Some(input) => Some(input), None => inputs.recv().await }
                }, if !input_closed && staged.is_none() && client.available() && !barrier => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::{
        net::tests::{key, start},
        replica::Buffer,
    };
    use rustls::pki_types::ServerName;
    use tokio::time::timeout;

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
                    Update::Events { .. } => received[1] = true,
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
        let now = tokio::time::Instant::now();
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
            world_step: 0,
            life: life.into(),
            epoch: 3,
            accepted_sequence: 1,
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
            tokio::time::sleep(Duration::from_millis(10)).await;
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
    async fn periodic_reads_leave_a_gameplay_slot_when_replies_are_withheld() {
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
        tokio::time::sleep(Duration::from_millis(150)).await;
        input.send(frame_input(6)).await.unwrap();
        timeout(Duration::from_millis(300), async {
            while commands < 6 {
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
        assert_eq!(bound, 6);
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
        let mut gateway = gateway(&keys);
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
            while commands.len() < 3 {
                let bytes = timeout(
                    Duration::from_secs(3),
                    read_frame(&mut socket, MAX_REQUEST_BYTES),
                )
                .await
                .unwrap()
                .unwrap();
                let request = Request::decode(&bytes).unwrap();
                if let Body::Command { command } = request.body {
                    commands.push(command);
                }
                let response = gateway.dispatch_json(id, 0, &bytes).unwrap();
                if commands.is_empty() {
                    write_frame(&mut socket, &response, MAX_RESPONSE_BYTES)
                        .await
                        .unwrap();
                } else {
                    held.push(response);
                }
            }
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
            while outcomes < 3 {
                match output.recv().await.unwrap() {
                    Update::CommandBound { token, binding } => {
                        bound += 1;
                        assert_eq!(token, bound);
                        assert_eq!(binding.unwrap().sequence, bound);
                    }
                    Update::Outcome(response) => {
                        assert_eq!(bound, 3);
                        assert!(matches!(response.body, Reply::Accepted));
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
            world_step: 0,
            life: life.into(),
            epoch: 1,
            accepted_sequence: 0,
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
        tokio::time::sleep(Duration::from_millis(75)).await;
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
        tokio::time::sleep(Duration::from_millis(20)).await;
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
            let deadline = tokio::time::Instant::now() + Duration::from_millis(1250);
            loop {
                tokio::select! {
                    _=tokio::time::sleep_until(deadline)=>break,
                    update=output.recv()=>match update.unwrap() {
                        Update::Snapshot(_)=>snapshots+=1,
                        Update::Inventory(response)=> {assert!(player);let Reply::Inventory{inventory}=response.body else {panic!("Missing inventory");};assert_eq!(inventory.experience,0);inventories+=1;},
                        Update::Events{..}=>{},
                        Update::FrameBound { .. } | Update::MovementSuperseded { .. }
                    | Update::CommandBound { .. } | Update::Outcome(_)=>panic!("No player commands submitted"),
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
            let mut clock = tokio::time::interval(Duration::from_millis(33));
            clock.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
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
                    Update::MovementSuperseded { .. }
                    | Update::FrameBound { .. }
                    | Update::CommandBound { .. }
                    | Update::Events { .. }
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
        tokio::time::sleep(Duration::from_millis(200)).await;
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
