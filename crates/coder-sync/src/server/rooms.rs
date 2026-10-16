use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};
use tokio::sync::{Mutex, mpsc, oneshot};

type Quotas = HashMap<(Room, String), (std::time::Instant, usize, usize)>;

type Job = Pin<Box<dyn Future<Output = ()> + Send>>;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Room {
    Chat(String),
    Registry(String),
    Device(String),
}

/// One bounded mailbox and writer task per room in this process.
#[derive(Clone, Default)]
pub struct Rooms(
    Arc<Mutex<HashMap<Room, mpsc::Sender<Job>>>>,
    Arc<Mutex<HashMap<Room, tokio::sync::broadcast::Sender<(uuid::Uuid, Vec<u8>)>>>>,
    Arc<Mutex<Quotas>>,
    Arc<Mutex<HashMap<Room, HashMap<String, (i64, std::time::Instant)>>>>,
);

#[derive(Debug, thiserror::Error)]
pub enum RoomError {
    #[error("The server is busy. Try again.")]
    Capacity,
    #[error("The connection closed. Try again.")]
    Closed,
}

impl Rooms {
    pub async fn presence(&self, room: Room, beat: Option<(String, i64)>) -> HashMap<String, i64> {
        let now = std::time::Instant::now();
        let mut rooms = self.3.lock().await;
        let presence = rooms.entry(room).or_default();
        presence.retain(|_, (_, seen)| now.duration_since(*seen).as_secs() < 30);
        if let Some((device, at)) = beat {
            if presence.len() < 4096 || presence.contains_key(&device) {
                presence.insert(device, (at, now));
            }
        }
        presence
            .iter()
            .map(|(device, (at, _))| (device.clone(), *at))
            .collect()
    }

    pub async fn admit_push(&self, room: Room, device: &str, bytes: usize) -> bool {
        let mut quotas = self.2.lock().await;
        let now = std::time::Instant::now();
        quotas.retain(|_, (at, _, _)| now.duration_since(*at).as_secs() < 60);
        let key = (room, device.to_owned());
        if quotas.len() >= 65536 && !quotas.contains_key(&key) {
            return false;
        }
        let entry = quotas.entry(key).or_insert((now, 0, 0));
        entry.1 = entry.1.saturating_add(1);
        entry.2 = entry.2.saturating_add(bytes);
        entry.1 <= 300 && entry.2 <= 8 * 1024 * 1024
    }

    pub async fn events(
        &self,
        room: Room,
    ) -> tokio::sync::broadcast::Sender<(uuid::Uuid, Vec<u8>)> {
        self.1
            .lock()
            .await
            .entry(room)
            .or_insert_with(|| tokio::sync::broadcast::channel(64).0)
            .clone()
    }
    pub async fn run<T, F>(&self, room: Room, job: F) -> Result<T, RoomError>
    where
        T: Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        let sender = {
            let mut rooms = self.0.lock().await;
            if let Some(sender) = rooms.get(&room).filter(|s| !s.is_closed()) {
                sender.clone()
            } else {
                // Refuse new rooms before allocating another task or mailbox.
                if rooms.len() >= 4096 && !rooms.contains_key(&room) {
                    return Err(RoomError::Capacity);
                }
                let (sender, mut receiver) = mpsc::channel::<Job>(64);
                tokio::spawn(async move {
                    while let Some(job) = receiver.recv().await {
                        job.await;
                    }
                });
                rooms.insert(room, sender.clone());
                sender
            }
        };
        let (reply, result) = oneshot::channel();
        sender
            .try_send(Box::pin(async move {
                let _ = reply.send(job.await);
            }))
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => RoomError::Capacity,
                mpsc::error::TrySendError::Closed(_) => RoomError::Closed,
            })?;
        result.await.map_err(|_| RoomError::Closed)
    }
}
