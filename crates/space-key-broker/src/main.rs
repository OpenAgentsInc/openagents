use space_key_broker::{
    Broker, Policy, kms::GoogleKms,
    wire::intermediate_key_broker_server::IntermediateKeyBrokerServer,
};
use std::{env, fs, sync::Arc};
use zeroize::Zeroizing;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let policy_path = env::var("SPACE_KEY_BROKER_CONFIG")?;
    let policy_bytes = fs::read(policy_path)?;
    if policy_bytes.len() > 65536 {
        return Err("Check the key broker settings.".into());
    }
    let policy: Policy = serde_json::from_slice(&policy_bytes)?;
    policy.validate()?;
    let token = Zeroizing::new(fs::read(env::var("SPACE_KEY_BROKER_TOKEN_FILE")?)?);
    if token.len() > 1024 {
        return Err("Choose a valid service credential.".into());
    }
    let token_text =
        std::str::from_utf8(&token).map_err(|_| "Choose a valid service credential.")?;
    let kms =
        GoogleKms::new(policy.key_routes.clone()).map_err(|_| "Key protection is unavailable.")?;
    let broker = Broker::new(policy, Arc::new(kms), token_text.trim().as_bytes())?;
    let address = format!(
        "0.0.0.0:{}",
        env::var("PORT").unwrap_or_else(|_| "8080".into())
    )
    .parse()?;
    tonic::transport::Server::builder()
        .concurrency_limit_per_connection(8)
        .add_service(
            IntermediateKeyBrokerServer::new(broker)
                .max_decoding_message_size(16384)
                .max_encoding_message_size(16384),
        )
        .serve_with_shutdown(address, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
