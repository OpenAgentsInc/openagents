//! Hosted chat shared by the phone and the local desktop host.
//! Credentials and storage stay with the caller; UI adapters receive views.
pub mod api;
pub mod basic_chats;
pub mod basic_coder;
pub mod basic_link;
pub mod cache;
pub mod capability;
pub mod client;
pub mod coder_events;
pub mod delegation;
pub mod home_cards;
pub mod migrate;
pub mod pane;
pub mod plan;
pub mod plugin_flow;
pub mod plugin_workbench;
pub mod remote;
pub mod route;
pub mod router;
pub mod service;
pub mod studio;
pub mod suggestions;
pub mod thread;
pub mod tool_groups;

fn public(secret: &secp256k1::SecretKey) -> String {
    let key = secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), secret)
        .x_only_public_key()
        .0;
    key.serialize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// An injected notification; the caller chooses how to wake its UI.
pub type Wake = std::sync::Arc<dyn Fn() + Send + Sync>;
