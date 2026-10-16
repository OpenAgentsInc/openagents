use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};
use tokio::sync::{Mutex, mpsc, oneshot};

type Job = Pin<Box<dyn Future<Output = ()> + Send>>;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum Room {
    Chat(String),
    Registry(String),
    Device(String),
}

/// One bounded mailbox and writer task per room in this process.
#[derive(Clone, Default)]
pub struct Rooms(Arc<Mutex<HashMap<Room, mpsc::Sender<Job>>>>);

#[derive(Debug, thiserror::Error)]
pub enum RoomError {
    #[error("The server is busy. Try again.")]
    Capacity,
    #[error("The connection closed. Try again.")]
    Closed,
}

impl Rooms {
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
