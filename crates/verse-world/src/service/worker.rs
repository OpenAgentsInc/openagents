//! Bounded sequential network worker for remote presentation adapters.
use super::{
    client::Client,
    event_cursor::{Cursor, Delivery},
    wire::{Body, Reply, Response},
};
use crate::{Intent, play::Ability};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

pub const INPUT_CAPACITY: usize = 32;
pub const UPDATE_CAPACITY: usize = 8;
/// Leaves request capacity for 30 Hz input refreshes and spell commands.
pub const NATIVE_CADENCE: Duration = Duration::from_millis(50);

/// Local input requests contain no principal, controller, or transport handle.
pub enum Input {
    Command(Intent<Ability>),
    Respawn,
    ClaimQuest(u64),
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
    Outcome(Response),
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
                input = inputs.recv() => {
                    let Some(input) = input else { return Ok(()); };
                    // Refresh admitted tick/control before deriving an owned command.
                    let response = client.request(Body::Snapshot {}).await?;
                    if let Reply::Refused { message, .. } = &response.body {
                        return Err(message.clone());
                    }
                    updates.send(Update::Snapshot(response)).await
                        .map_err(|_| "Chamber update consumer closed")?;
                    let response = match input {
                        Input::Command(intent) => client.command(intent).await?,
                        Input::Respawn => client.respawn().await?,
                        Input::EquipGear(slot,item) => {let mut operation=[0;16];getrandom::fill(&mut operation).map_err(|_|"Cannot generate equipment retry identity")?;next_inventory=tokio::time::Instant::now();client.equip_gear(slot,item,operation).await?}
                        Input::EquipOutfit(outfit) => {let mut operation=[0;16];getrandom::fill(&mut operation).map_err(|_|"Cannot generate outfit retry identity")?;next_inventory=tokio::time::Instant::now();client.equip_outfit(outfit,operation).await?}
                        Input::UseItem(item) => {let mut operation=[0;16];getrandom::fill(&mut operation).map_err(|_|"Cannot generate item retry identity")?;next_inventory=tokio::time::Instant::now();client.use_item(item,operation).await?}
                        Input::ClaimQuest(quest) => {next_inventory=tokio::time::Instant::now();client.claim_quest(quest).await?}
                    };
                    Update::Outcome(response)
                }
            };
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
                        Update::Outcome(_)=>panic!("No player commands submitted"),
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
                    Update::Events { .. } | Update::Inventory(_) => {}
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
