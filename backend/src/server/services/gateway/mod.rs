pub mod types;

use anyhow::Result;
use std::pin::Pin;
use tokio_stream::Stream;

use self::types::GatewayMetadata;

#[async_trait::async_trait]
pub trait Gateway {
    fn metadata(&self) -> GatewayMetadata;

    async fn chat(&self, prompt: String, use_reasoner: bool) -> Result<(String, Option<String>)>;

    async fn chat_stream(
        &self,
        prompt: String,
        use_reasoner: bool,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>>;
}
