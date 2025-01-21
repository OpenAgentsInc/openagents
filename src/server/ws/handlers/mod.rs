use async_trait::async_trait;

pub mod chat;
pub mod solver;

#[async_trait]
pub trait MessageHandler {
    type Message;
    
    async fn handle_message(&self, msg: Self::Message, conn_id: String) -> Result<(), Box<dyn std::error::Error>>;
    async fn broadcast(&self, msg: Self::Message) -> Result<(), Box<dyn std::error::Error>>;
}