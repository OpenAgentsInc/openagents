//! Scratch hosted-chat state for opt-in tests. Never uses an owner identity.
use openagents_chat::{
    basic_chats::BasicChats,
    basic_coder::{RELAY, Relay, WORKER},
    cache::Cache,
};

pub fn chats(runtime: &tokio::runtime::Runtime, root: &std::path::Path) -> BasicChats {
    let secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    let door = Relay::new(RELAY, WORKER, secret).unwrap();
    BasicChats::new(
        Some(runtime.handle().clone()),
        Some(std::sync::Arc::new(door)),
        Some(Cache::open(root, &secret).unwrap()),
    )
}
