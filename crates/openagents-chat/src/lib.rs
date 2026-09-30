//! Hosted chat shared by the phone and the local desktop host.
//! Credentials and storage stay with the caller; UI adapters receive views.
pub mod basic_chats;
pub mod basic_coder;
pub mod basic_link;
pub mod cache;
pub mod delegation;
pub mod router;
pub mod service;

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
