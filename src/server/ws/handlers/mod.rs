use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait MessageHandler {
    type Message;

    async fn handle_message(&self, msg: Self::Message, conn_id: String) -> Result<()>;
    async fn broadcast(&self, msg: Self::Message) -> Result<()>;
}