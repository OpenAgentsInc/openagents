//! Bounded live provider check on a disposable host with a dedicated identity.
//! Arguments contain a registered numeric version and a hash, never a secret.
use computer_credentials::{
    SecretProvider,
    secret_manager::{GoogleSecretManager, Version},
};
use sha2::{Digest, Sha256};

async fn run() -> Result<(), ()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 || !matches!(args[0].as_str(), "read" | "denied") {
        return Err(());
    }
    let version = Version::parse(&args[1]).map_err(|_| ())?;
    let provider = GoogleSecretManager::from_metadata().map_err(|_| ())?;
    let result = provider.access(&version).await;
    if args[0] == "denied" {
        if result.is_ok() {
            return Err(());
        }
        println!("{{\"access_denied\":true}}");
        return Ok(());
    }
    if !computer_credentials::digest(&args[2]) {
        return Err(());
    }
    let secret = result.map_err(|_| ())?;
    let hash = format!("{:x}", Sha256::digest(secret.bytes()));
    if hash != args[2] || secret.bytes().len() < 32 {
        return Err(());
    }
    println!("{{\"numeric_version_read\":true,\"crc32c_verified\":true,\"hash_matched\":true}}");
    Ok(())
}

#[tokio::main]
async fn main() {
    if run().await.is_err() {
        eprintln!("The live credential provider check failed.");
        std::process::exit(1);
    }
}
