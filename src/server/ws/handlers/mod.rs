use async_trait::async_trait;
use std::error::Error;

pub mod chat;

#[async_trait]
pub trait MessageHandler {
    type Message;

    async fn handle_message(
        &self,
        msg: Self::Message,
        conn_id: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn broadcast(&self, msg: Self::Message) -> Result<(), Box<dyn Error + Send + Sync>>;
}
