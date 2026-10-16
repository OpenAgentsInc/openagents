//! Exercise a deployed broker without printing keys or credentials.
use space_key_broker::wire::{
    intermediate_key_broker_client::IntermediateKeyBrokerClient,
    issue_intermediate_key_response::Outcome as Issued,
    unwrap_intermediate_key_response::Outcome as Opened, *,
};
use std::{env, fs, time::Duration};
use tonic::{Request, transport::ClientTlsConfig};
use zeroize::Zeroizing;

fn signed<T>(body: T, bearer: &str) -> Request<T> {
    let mut request = Request::new(body);
    request.metadata_mut().insert(
        "authorization",
        format!("Bearer {}", bearer.trim())
            .parse()
            .expect("service credential"),
    );
    request
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let endpoint = env::var("SPACE_KEY_BROKER_URL")?;
    if !endpoint.starts_with("https://") {
        return Err("Use the HTTPS service endpoint.".into());
    }
    let bearer = Zeroizing::new(fs::read(env::var("SPACE_KEY_BROKER_TOKEN_FILE")?)?);
    let bearer = std::str::from_utf8(&bearer).map_err(|_| "Invalid service credential.")?;
    let channel = tonic::transport::Endpoint::from_shared(endpoint)?
        .tls_config(ClientTlsConfig::new().with_webpki_roots())?
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(10))
        .connect()
        .await?;
    let mut client = IntermediateKeyBrokerClient::new(channel);
    let tenant = TenantRef {
        namespace: env::var("SPACE_KEY_BROKER_NAMESPACE")?,
        id: env::var("SPACE_KEY_BROKER_TENANT")?,
    };
    let issue = IssueIntermediateKeyRequest {
        tenant: Some(tenant.clone()),
        authorization: None,
    };
    let error = client
        .issue_intermediate_key(issue.clone())
        .await
        .expect_err("unauthenticated request succeeded");
    assert_eq!(error.code(), tonic::Code::Unauthenticated);
    let issued = client
        .issue_intermediate_key(signed(issue.clone(), bearer))
        .await?
        .into_inner();
    let Some(Issued::Key(key)) = issued.outcome else {
        return Err("Key issue failed.".into());
    };
    let open = UnwrapIntermediateKeyRequest {
        tenant: Some(tenant.clone()),
        key_id: key.key_id.clone(),
        wrapped_key: key.wrapped_key.clone(),
        authorization: None,
    };
    let Some(Opened::Key(mut material)) = client
        .unwrap_intermediate_key(signed(open.clone(), bearer))
        .await?
        .into_inner()
        .outcome
    else {
        return Err("Key release failed.".into());
    };
    let plain = Zeroizing::new(std::mem::take(&mut material.plaintext_key));
    assert_eq!(plain.len(), 32);
    assert_eq!(
        material.max_cache_ttl,
        Some(prost_types::Duration {
            seconds: 0,
            nanos: 0
        })
    );
    let Some(Opened::Key(mut again)) = client
        .unwrap_intermediate_key(signed(open.clone(), bearer))
        .await?
        .into_inner()
        .outcome
    else {
        return Err("Repeated key release failed.".into());
    };
    let again_plain = Zeroizing::new(std::mem::take(&mut again.plaintext_key));
    assert!(
        *plain == *again_plain,
        "Repeated key release changed the key."
    );
    let mut corrupt = open.clone();
    corrupt.wrapped_key[0] ^= 1;
    let Some(Opened::Failure(failure)) = client
        .unwrap_intermediate_key(signed(corrupt, bearer))
        .await?
        .into_inner()
        .outcome
    else {
        return Err("A damaged key was accepted.".into());
    };
    assert_eq!(failure.reason, UnwrapFailureReason::InvalidKey as i32);
    let mut wrong_tenant = open;
    let other_tenant = env::var("SPACE_KEY_BROKER_OTHER_TENANT")?;
    assert_ne!(tenant.id, other_tenant);
    wrong_tenant.tenant.as_mut().unwrap().id = other_tenant;
    let Some(Opened::Failure(failure)) = client
        .unwrap_intermediate_key(signed(wrong_tenant, bearer))
        .await?
        .into_inner()
        .outcome
    else {
        return Err("Another tenant opened the key.".into());
    };
    assert_eq!(failure.reason, UnwrapFailureReason::InvalidKey as i32);
    let Some(Issued::Key(second)) = client
        .issue_intermediate_key(signed(issue, bearer))
        .await?
        .into_inner()
        .outcome
    else {
        return Err("Fresh key issue failed.".into());
    };
    assert_ne!(key.key_id, second.key_id);
    assert_ne!(key.wrapped_key, second.wrapped_key);
    println!(
        "PASS: TLS, bearer authentication, KMS wrap/release, zero cache TTL, fresh keys, damage refusal, tenant isolation"
    );
    Ok(())
}
