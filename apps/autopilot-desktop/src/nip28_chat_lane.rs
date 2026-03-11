use crate::app_state::DefaultNip28ChannelConfig;
use nostr::Event;
use nostr_client::{ConnectionState, PoolConfig, RelayMessage, RelayPool};
use serde_json::json;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

const LANE_POLL: Duration = Duration::from_millis(120);
const RELAY_RECV_TIMEOUT: Duration = Duration::from_millis(4);
const MAX_MESSAGES_PER_RELAY_POLL: usize = 64;
const SUBSCRIPTION_ID: &str = "autopilot-nip28-chat";

#[derive(Debug)]
pub enum Nip28ChatLaneUpdate {
    RelayEvent(Event),
    Eose { relay_url: String },
    ConnectionError { relay_url: String, message: String },
}

pub struct Nip28ChatLaneWorker {
    update_rx: Receiver<Nip28ChatLaneUpdate>,
}

impl Nip28ChatLaneWorker {
    pub fn spawn() -> Self {
        let (update_tx, update_rx) = mpsc::channel::<Nip28ChatLaneUpdate>();
        std::thread::spawn(move || run_nip28_chat_lane_loop(update_tx));
        Self { update_rx }
    }

    pub fn drain_updates(&mut self) -> Vec<Nip28ChatLaneUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = self.update_rx.try_recv() {
            updates.push(update);
        }
        updates
    }
}

fn build_filters(channel_id: &str) -> Vec<serde_json::Value> {
    vec![
        json!({"kinds": [40], "ids": [channel_id]}),
        json!({"kinds": [41, 42], "#e": [channel_id], "limit": 512}),
    ]
}

fn run_nip28_chat_lane_loop(update_tx: Sender<Nip28ChatLaneUpdate>) {
    let config = DefaultNip28ChannelConfig::from_env_or_default();
    if !config.is_valid() {
        return;
    }

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return,
    };

    let mut pool: Option<Arc<RelayPool>> = None;

    loop {
        ensure_connected(&runtime, &config, &mut pool, &update_tx);
        if let Some(p) = pool.as_ref().cloned() {
            runtime.block_on(poll_events(p, &config, &update_tx));
        }
        std::thread::sleep(LANE_POLL);
    }
}

fn ensure_connected(
    runtime: &tokio::runtime::Runtime,
    config: &DefaultNip28ChannelConfig,
    pool: &mut Option<Arc<RelayPool>>,
    update_tx: &Sender<Nip28ChatLaneUpdate>,
) {
    if let Some(existing_pool) = pool.as_ref().cloned() {
        let still_connected = runtime.block_on(async {
            let relays = existing_pool.relays().await;
            for relay in &relays {
                if relay.state().await == ConnectionState::Connected {
                    return true;
                }
            }
            false
        });

        if still_connected {
            return;
        }

        runtime.block_on(existing_pool.disconnect_all());
        *pool = None;
    }

    let new_pool = Arc::new(RelayPool::new(PoolConfig::default()));
    let filters = build_filters(&config.channel_id);

    let connected = runtime.block_on(async {
        if let Err(_) = new_pool.add_relay(config.relay_url.as_str()).await {
            return false;
        }
        match new_pool.connect_relay(&config.relay_url).await {
            Ok(()) => {}
            Err(error) => {
                let _ = update_tx.send(Nip28ChatLaneUpdate::ConnectionError {
                    relay_url: config.relay_url.clone(),
                    message: format!("connect failed: {error}"),
                });
                return false;
            }
        }
        let Some(connection) = new_pool.relay(&config.relay_url).await else {
            let _ = update_tx.send(Nip28ChatLaneUpdate::ConnectionError {
                relay_url: config.relay_url.clone(),
                message: "relay missing from pool after connect".to_string(),
            });
            return false;
        };
        if let Err(error) = connection
            .subscribe_filters(SUBSCRIPTION_ID, filters)
            .await
        {
            let _ = update_tx.send(Nip28ChatLaneUpdate::ConnectionError {
                relay_url: config.relay_url.clone(),
                message: format!("subscribe failed: {error}"),
            });
            return false;
        }
        true
    });

    if connected {
        *pool = Some(new_pool);
    }
}

async fn poll_events(
    pool: Arc<RelayPool>,
    _config: &DefaultNip28ChannelConfig,
    update_tx: &Sender<Nip28ChatLaneUpdate>,
) {
    let relays = pool.relays().await;
    for relay in relays {
        let relay_url = relay.url().to_string();
        for _ in 0..MAX_MESSAGES_PER_RELAY_POLL {
            match tokio::time::timeout(RELAY_RECV_TIMEOUT, relay.recv()).await {
                Ok(Ok(Some(RelayMessage::Event(_, event)))) => {
                    let _ = update_tx.send(Nip28ChatLaneUpdate::RelayEvent(event));
                }
                Ok(Ok(Some(RelayMessage::Eose(_)))) => {
                    let _ = update_tx.send(Nip28ChatLaneUpdate::Eose {
                        relay_url: relay_url.clone(),
                    });
                }
                Ok(Ok(Some(_))) => {
                    continue;
                }
                Ok(Ok(None)) => break,
                Ok(Err(error)) => {
                    let _ = update_tx.send(Nip28ChatLaneUpdate::ConnectionError {
                        relay_url: relay_url.clone(),
                        message: format!("recv error: {error}"),
                    });
                    break;
                }
                Err(_) => break,
            }
        }
    }
}
