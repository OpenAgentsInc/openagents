//! Bounded sequential network worker for remote presentation adapters.
use super::{
    client::Client,
    event_cursor::{Cursor, Delivery},
    wire::{Body, Reply, Response},
};
use crate::{Command, Intent, play::Ability};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

pub const INPUT_CAPACITY: usize = 32;
pub const UPDATE_CAPACITY: usize = 8;
/// Leaves request capacity for 30 Hz input refreshes and spell commands.
pub const NATIVE_CADENCE: Duration = Duration::from_millis(50);

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
    Outcome(Response),
}

/// Reuses only recent verified response control; server admission still checks every command.
fn fresh_control(
    input: &Input,
    control: Option<&super::wire::Control>,
    observed: Option<Instant>,
) -> bool {
    let (Input::TrackedCommand { life, epoch, .. }, Some(control), Some(observed)) =
        (input, control, observed)
    else {
        return false;
    };
    control.life == (*life).into()
        && control.epoch == *epoch
        && observed.elapsed() <= Duration::from_millis(50)
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
    mut client: Client,
    mut cursor: Cursor,
    cadence: Duration,
    mut inputs: mpsc::Receiver<Input>,
    updates: mpsc::Sender<Update>,
    stop: oneshot::Receiver<()>,
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
        let mut interval = tokio::time::interval(cadence);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut next_inventory = tokio::time::Instant::now();
        let mut inventory_life = None;
        let mut last_token = 0;
        let mut deferred = None;
        let mut last_response = None;
        loop {
            let mut polling = false;
            let update = tokio::select! {
                _ = interval.tick() => {
                    polling = true;
                    let response = client.request(Body::Snapshot {}).await?;
                    if let Reply::Refused { message, .. } = &response.body {
                        return Err(message.clone());
                    }
                    updates.send(Update::Snapshot(response)).await
                        .map_err(|_| "Chamber update consumer closed")?;
                    let delivery = client.delivered_events(&mut cursor, 64).await?;
                    Update::Events { delivery, checkpoint: cursor.checkpoint()? }
                }
                input = async {
                    match deferred.take() { Some(input) => Some(input), None => inputs.recv().await }
                } => {
                    let Some(input) = input else { return Ok(()); };
                    let (mut input, retired) = coalesce_movement(input, &mut inputs, &mut deferred);
                    for token in retired {
                        updates.send(Update::MovementSuperseded { token, replacement: match &input { Input::TrackedCommand {token,..} => *token, _ => unreachable!() } }).await
                            .map_err(|_| "Chamber update consumer closed")?;
                    }
                    // Retirement delivery can wait on output backpressure; check freshness afterward.
                    if !fresh_control(&input, client.control(), last_response) {
                        let response = client.request(Body::Snapshot {}).await?;
                        if let Reply::Refused { message, .. } = &response.body {
                            return Err(message.clone());
                        }
                        updates.send(Update::Snapshot(response)).await
                            .map_err(|_| "Chamber update consumer closed")?;
                    }
                    // Input may have changed while the refresh was in flight.
                    let (latest, retired) = coalesce_movement(input, &mut inputs, &mut deferred);
                    input = latest;
                    for token in retired {
                        updates.send(Update::MovementSuperseded {token,replacement:match &input {Input::TrackedCommand {token,..}=>*token,_=>unreachable!()}}).await
                            .map_err(|_| "Chamber update consumer closed")?;
                    }
                    let response = match input {
                        Input::TrackedCommand { token, life, epoch, intent } => {
                            let binding = if token == 0 || token <= last_token {
                                Err("Tracked input token must increase".into())
                            } else {
                                last_token = token;
                                match client.control() {
                                    Some(control) if control.life == life.into() && control.epoch == epoch => client.prepare_command(intent),
                                    _ => Err("Tracked input control changed before transmission".into()),
                                }
                            };
                            let command = binding.as_ref().ok().cloned();
                            updates.send(Update::CommandBound { token, binding }).await
                                .map_err(|_| "Chamber update consumer closed")?;
                            let Some(command) = command else { continue; };
                            client.request(Body::Command { command: command.into() }).await?
                        }
                        Input::Command(intent) => client.command(intent).await?,
                        Input::Respawn => client.respawn().await?,
                        Input::EquipGear(slot,item) => {let mut operation=[0;16];getrandom::fill(&mut operation).map_err(|_|"Cannot generate equipment retry identity")?;next_inventory=tokio::time::Instant::now();client.equip_gear(slot,item,operation).await?}
                        Input::EquipOutfit(outfit) => {let mut operation=[0;16];getrandom::fill(&mut operation).map_err(|_|"Cannot generate outfit retry identity")?;next_inventory=tokio::time::Instant::now();client.equip_outfit(outfit,operation).await?}
                        Input::UseItem(item) => {let mut operation=[0;16];getrandom::fill(&mut operation).map_err(|_|"Cannot generate item retry identity")?;next_inventory=tokio::time::Instant::now();client.use_item(item,operation).await?}
                        Input::AcceptQuest(quest,giver) => {next_inventory=tokio::time::Instant::now();client.accept_quest(quest,giver).await?}
                        Input::ClaimQuest(quest) => {next_inventory=tokio::time::Instant::now();client.claim_quest(quest).await?}
                    };
                    Update::Outcome(response)
                }
            };
            last_response = Some(Instant::now());
            updates
                .send(update)
                .await
                .map_err(|_| "Chamber update consumer closed")?;
            let life = client.control().map(|c| c.life);
            if polling
                && life.is_some()
                && (life != inventory_life || tokio::time::Instant::now() >= next_inventory)
            {
                let response = client.request(Body::Inventory {}).await?;
                if let Reply::Refused { message, .. } = &response.body {
                    return Err(message.clone());
                }
                inventory_life = client.control().map(|c| c.life);
                next_inventory = tokio::time::Instant::now() + Duration::from_secs(1);
                updates
                    .send(Update::Inventory(response))
                    .await
                    .map_err(|_| "Chamber update consumer closed")?;
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
                    Update::CommandBound { .. } => panic!("Unexpected binding"),
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
                        Update::MovementSuperseded { .. } | Update::CommandBound { .. } | Update::Outcome(_)=>panic!("No player commands submitted"),
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
                    | Update::CommandBound { .. }
                    | Update::Events { .. }
                    | Update::Inventory(_) => {}
                }
            }
        })
        .await
        .unwrap();
        let input = feeder.await.unwrap();
        assert!(snapshots >= 93);
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
                Update::MovementSuperseded { .. } | Update::CommandBound { .. } => {
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
