//! LM backend implementations.

pub mod fm_bridge;
pub mod mock;
pub mod ollama;
pub mod openai;
pub mod openrouter;
pub mod swarm_sim;

pub use fm_bridge::FmBridgeBackend;
pub use mock::MockBackend;
pub use ollama::OllamaBackend;
pub use openai::OpenAiBackend;
pub use openrouter::OpenRouterBackend;
pub use swarm_sim::SwarmSimulator;
