// Generated contract cases from the pinned specification.
mod support;
use boat::{Error, models::*};
#[tokio::test]
async fn me_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    assert!(matches!(client.me().await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/me");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn list_organizations_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    assert!(matches!(
        client.list_organizations().await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/orgs");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn set_active_organization_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: SetActiveOrganizationParams =
        serde_json::from_str(r#"{"body": {"org": "sample"}}"#).expect("parameters");
    assert!(matches!(
        client.set_active_organization(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.target, "/api/v1/orgs/active");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"org": "sample"})
    );
}
#[tokio::test]
async fn get_data_retention_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    assert!(matches!(
        client.get_data_retention().await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/account/data-retention");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn update_data_retention_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: UpdateDataRetentionParams = serde_json::from_str(r#"{"body": {"enabled": true, "snapshotsOff": true, "confirmation": "delete archived sandbox data"}}"#).expect("parameters");
    assert!(matches!(
        client.update_data_retention(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.target, "/api/v1/account/data-retention");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"enabled": true, "snapshotsOff": true, "confirmation": "delete archived sandbox data"})
    );
}
#[tokio::test]
async fn get_deletion_operation_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: GetDeletionOperationParams =
        serde_json::from_str(r#"{"operationId": "sample"}"#).expect("parameters");
    assert!(matches!(
        client.get_deletion_operation(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/deletion-operations/sample");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn limits_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: LimitsParams =
        serde_json::from_str(r#"{"org": "sample", "X-Boat-Org": "sample", "teamId": "sample"}"#)
            .expect("parameters");
    assert!(matches!(client.limits(&params).await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/limits?org=sample&teamId=sample");
    assert_eq!(request.headers["x-boat-org"], "sample");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn repos_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: ReposParams = serde_json::from_str(r#"{"sync": true, "limit": 1, "cursor": "sample", "sort": "asc", "q": "sample", "selected": true}"#).expect("parameters");
    assert!(matches!(client.repos(&params).await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.target,
        "/api/v1/repos?sync=true&limit=1&cursor=sample&sort=asc&q=sample&selected=true"
    );
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn select_repo_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: SelectRepoParams =
        serde_json::from_str(r#"{"body": {"repositoryId": "sample", "baseBranch": "sample"}}"#)
            .expect("parameters");
    assert!(matches!(
        client.select_repo(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/repos");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"repositoryId": "sample", "baseBranch": "sample"})
    );
}
#[tokio::test]
async fn api_keys_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    assert!(matches!(client.api_keys().await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/api-keys");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn create_scoped_api_key_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: CreateScopedApiKeyParams = serde_json::from_str(r#"{"body": {"name": "sample", "ttl": "sample", "preset": "read-only", "actions": [], "sandboxIds": [], "environmentIds": []}}"#).expect("parameters");
    assert!(matches!(
        client.create_scoped_api_key(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/api-keys/scoped");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"name": "sample", "ttl": "sample", "preset": "read-only", "actions": [], "sandboxIds": [], "environmentIds": []})
    );
}
#[tokio::test]
async fn revoke_api_key_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: RevokeApiKeyParams =
        serde_json::from_str(r#"{"apiKeyId": "sample"}"#).expect("parameters");
    assert!(matches!(
        client.revoke_api_key(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.target, "/api/v1/api-keys/sample");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn rotate_api_key_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: RotateApiKeyParams =
        serde_json::from_str(r#"{"apiKeyId": "sample"}"#).expect("parameters");
    assert!(matches!(
        client.rotate_api_key(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/api-keys/sample/rotate");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn api_key_usage_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: ApiKeyUsageParams =
        serde_json::from_str(r#"{"apiKeyId": "sample"}"#).expect("parameters");
    assert!(matches!(
        client.api_key_usage(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/api-keys/sample/usage");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn list_webhooks_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    assert!(matches!(client.list_webhooks().await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/webhooks");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn create_webhook_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: CreateWebhookParams =
        serde_json::from_str(r#"{"body": {"name": "sample", "url": "sample", "events": []}}"#)
            .expect("parameters");
    assert!(matches!(
        client.create_webhook(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/webhooks");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"name": "sample", "url": "sample", "events": []})
    );
}
#[tokio::test]
async fn get_webhook_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: GetWebhookParams =
        serde_json::from_str(r#"{"webhookId": "sample"}"#).expect("parameters");
    assert!(matches!(
        client.get_webhook(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/webhooks/sample");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn update_webhook_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: UpdateWebhookParams = serde_json::from_str(
        r#"{"webhookId": "sample", "body": {"name": "sample", "url": "sample", "events": []}}"#,
    )
    .expect("parameters");
    assert!(matches!(
        client.update_webhook(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.target, "/api/v1/webhooks/sample");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"name": "sample", "url": "sample", "events": []})
    );
}
#[tokio::test]
async fn delete_webhook_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: DeleteWebhookParams =
        serde_json::from_str(r#"{"webhookId": "sample"}"#).expect("parameters");
    assert!(matches!(
        client.delete_webhook(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.target, "/api/v1/webhooks/sample");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn rotate_webhook_signing_secret_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: RotateWebhookSigningSecretParams =
        serde_json::from_str(r#"{"webhookId": "sample"}"#).expect("parameters");
    assert!(matches!(
        client.rotate_webhook_signing_secret(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/webhooks/sample/rotate");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn secrets_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    assert!(matches!(client.secrets().await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/secrets");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn update_secrets_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: UpdateSecretsParams =
        serde_json::from_str(r#"{"body": {"envContents": "sample", "secretFiles": []}}"#)
            .expect("parameters");
    assert!(matches!(
        client.update_secrets(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/secrets");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"envContents": "sample", "secretFiles": []})
    );
}
#[tokio::test]
async fn environments_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    assert!(matches!(client.environments().await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/environments");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn create_environment_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: CreateEnvironmentParams =
        serde_json::from_str(r#"{"body": {"name": "sample"}}"#).expect("parameters");
    assert!(matches!(
        client.create_environment(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/environments");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"name": "sample"})
    );
}
#[tokio::test]
async fn update_environment_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: UpdateEnvironmentParams = serde_json::from_str(r#"{"environmentId": "sample", "body": {"name": "sample", "isDefault": true, "safeForThirdParties": true, "passGithub": true, "passSecrets": true, "passSandboxCredentials": true, "passAgentsCredentials": true, "envContents": "sample", "secretFiles": [], "repositories": []}}"#).expect("parameters");
    assert!(matches!(
        client.update_environment(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "PUT");
    assert_eq!(request.target, "/api/v1/environments/sample");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"name": "sample", "isDefault": true, "safeForThirdParties": true, "passGithub": true, "passSecrets": true, "passSandboxCredentials": true, "passAgentsCredentials": true, "envContents": "sample", "secretFiles": [], "repositories": []})
    );
}
#[tokio::test]
async fn delete_environment_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: DeleteEnvironmentParams =
        serde_json::from_str(r#"{"environmentId": "sample"}"#).expect("parameters");
    assert!(matches!(
        client.delete_environment(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.target, "/api/v1/environments/sample");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn upgrade_environment_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: UpgradeEnvironmentParams =
        serde_json::from_str(r#"{"environmentId": "sample", "body": {"agentIds": []}}"#)
            .expect("parameters");
    assert!(matches!(
        client.upgrade_environment(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/environments/sample/upgrade");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"agentIds": []})
    );
}
#[tokio::test]
async fn set_environment_var_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: SetEnvironmentVarParams = serde_json::from_str(
        r#"{"environmentId": "sample", "key": "sample", "body": {"value": "sample"}}"#,
    )
    .expect("parameters");
    assert!(matches!(
        client.set_environment_var(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "PUT");
    assert_eq!(request.target, "/api/v1/environments/sample/vars/sample");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"value": "sample"})
    );
}
#[tokio::test]
async fn delete_environment_var_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: DeleteEnvironmentVarParams =
        serde_json::from_str(r#"{"environmentId": "sample", "key": "sample"}"#)
            .expect("parameters");
    assert!(matches!(
        client.delete_environment_var(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.target, "/api/v1/environments/sample/vars/sample");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn set_environment_secret_file_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: SetEnvironmentSecretFileParams = serde_json::from_str(
        r#"{"environmentId": "sample", "body": {"path": "sample", "contents": "sample"}}"#,
    )
    .expect("parameters");
    assert!(matches!(
        client.set_environment_secret_file(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "PUT");
    assert_eq!(request.target, "/api/v1/environments/sample/secret-files");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"path": "sample", "contents": "sample"})
    );
}
#[tokio::test]
async fn delete_environment_secret_file_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: DeleteEnvironmentSecretFileParams =
        serde_json::from_str(r#"{"environmentId": "sample", "path": "sample"}"#)
            .expect("parameters");
    assert!(matches!(
        client.delete_environment_secret_file(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "DELETE");
    assert_eq!(
        request.target,
        "/api/v1/environments/sample/secret-files?path=sample"
    );
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn add_environment_repo_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: AddEnvironmentRepoParams = serde_json::from_str(r#"{"environmentId": "sample", "body": {"repositoryId": "sample", "baseBranch": "sample"}}"#).expect("parameters");
    assert!(matches!(
        client.add_environment_repo(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/environments/sample/repos");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"repositoryId": "sample", "baseBranch": "sample"})
    );
}
#[tokio::test]
async fn delete_environment_repo_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: DeleteEnvironmentRepoParams =
        serde_json::from_str(r#"{"environmentId": "sample", "repositoryId": "sample"}"#)
            .expect("parameters");
    assert!(matches!(
        client.delete_environment_repo(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.target, "/api/v1/environments/sample/repos/sample");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn sandboxes_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: SandboxesParams = serde_json::from_str(r#"{"org": "sample", "X-Boat-Org": "sample", "limit": 1, "cursor": "sample", "sort": "asc", "state": "sample"}"#).expect("parameters");
    assert!(matches!(
        client.sandboxes(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.target,
        "/api/v1/sandboxes?org=sample&limit=1&cursor=sample&sort=asc&state=sample"
    );
    assert_eq!(request.headers["x-boat-org"], "sample");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn create_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: CreateParams = serde_json::from_str(r#"{"Idempotency-Key": "sample", "org": "sample", "X-Boat-Org": "sample", "body": {"type": "small", "ttlSeconds": 1, "env": {}, "environment": "sample", "noEnv": true, "snapshots": true, "failFast": true, "setupScript": "sample", "org": "sample", "teamId": "sample", "from": "sample"}}"#).expect("parameters");
    assert!(matches!(client.create(&params).await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/sandboxes?org=sample");
    assert_eq!(request.headers["idempotency-key"], "sample");
    assert_eq!(request.headers["x-boat-org"], "sample");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"type": "small", "ttlSeconds": 1, "env": {}, "environment": "sample", "noEnv": true, "snapshots": true, "failFast": true, "setupScript": "sample", "org": "sample", "teamId": "sample", "from": "sample"})
    );
}
#[tokio::test]
async fn get_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: GetParams = serde_json::from_str(r#"{"sandboxId": "sample"}"#).expect("parameters");
    assert!(matches!(client.get(&params).await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/sandboxes/sample");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn update_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: UpdateParams = serde_json::from_str(r#"{"sandboxId": "sample", "body": {"name": "sample", "ttlSeconds": 1, "subdomain": "sample"}}"#).expect("parameters");
    assert!(matches!(client.update(&params).await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.target, "/api/v1/sandboxes/sample");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"name": "sample", "ttlSeconds": 1, "subdomain": "sample"})
    );
}
#[tokio::test]
async fn delete_sandbox_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: DeleteSandboxParams =
        serde_json::from_str(r#"{"sandboxId": "sample", "X-Ascii-Confirm-Delete": "sample"}"#)
            .expect("parameters");
    assert!(matches!(
        client.delete_sandbox(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.target, "/api/v1/sandboxes/sample");
    assert_eq!(request.headers["x-ascii-confirm-delete"], "sample");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn stop_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: StopParams =
        serde_json::from_str(r#"{"sandboxId": "sample", "body": {"force": true}}"#)
            .expect("parameters");
    assert!(matches!(client.stop(&params).await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/sandboxes/sample/stop");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"force": true})
    );
}
#[tokio::test]
async fn share_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: ShareParams =
        serde_json::from_str(r#"{"sandboxId": "sample"}"#).expect("parameters");
    assert!(matches!(client.share(&params).await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/sandboxes/sample/share");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn resume_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: ResumeParams = serde_json::from_str(r#"{"sandboxId": "sample", "body": {"failFast": true, "type": "small", "env": {}, "environment": "sample", "noEnv": true, "ttlSeconds": 1}}"#).expect("parameters");
    assert!(matches!(client.resume(&params).await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/sandboxes/sample/resume");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"failFast": true, "type": "small", "env": {}, "environment": "sample", "noEnv": true, "ttlSeconds": 1})
    );
}
#[tokio::test]
async fn fork_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: ForkParams = serde_json::from_str(r#"{"sandboxId": "sample", "Idempotency-Key": "sample", "body": {"failFast": true, "env": {}, "environment": "sample", "noEnv": true, "type": "small", "ttlSeconds": 1}}"#).expect("parameters");
    assert!(matches!(client.fork(&params).await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/sandboxes/sample/fork");
    assert_eq!(request.headers["idempotency-key"], "sample");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"failFast": true, "env": {}, "environment": "sample", "noEnv": true, "type": "small", "ttlSeconds": 1})
    );
}
#[tokio::test]
async fn prompt_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: PromptParams = serde_json::from_str(r#"{"sandboxId": "sample", "body": {"provider": "codex", "model": "sample", "reasoningEffort": "sample", "fast": true, "new": true, "conversationId": "sample", "prompt": "sample"}}"#).expect("parameters");
    assert!(matches!(client.prompt(&params).await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/sandboxes/sample/prompt");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"provider": "codex", "model": "sample", "reasoningEffort": "sample", "fast": true, "new": true, "conversationId": "sample", "prompt": "sample"})
    );
}
#[tokio::test]
async fn events_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: EventsParams = serde_json::from_str(r#"{"sandboxId": "sample", "limit": 1, "cursor": "sample", "sort": "asc", "type": "sample", "conversation": "sample"}"#).expect("parameters");
    assert!(matches!(client.events(&params).await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.target,
        "/api/v1/sandboxes/sample/events?limit=1&cursor=sample&sort=asc&type=sample&conversation=sample"
    );
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn conversations_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: ConversationsParams =
        serde_json::from_str(r#"{"sandboxId": "sample"}"#).expect("parameters");
    assert!(matches!(
        client.conversations(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/sandboxes/sample/conversations");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn prompt_run_status_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: PromptRunStatusParams =
        serde_json::from_str(r#"{"sandboxId": "sample", "promptId": "sample"}"#)
            .expect("parameters");
    assert!(matches!(
        client.prompt_run_status(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/sandboxes/sample/prompts/sample");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn read_file_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: ReadFileParams =
        serde_json::from_str(r#"{"sandboxId": "sample", "path": "sample", "encoding": "utf8"}"#)
            .expect("parameters");
    assert!(matches!(
        client.read_file(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.target,
        "/api/v1/sandboxes/sample/files?path=sample&encoding=utf8"
    );
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn write_file_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: WriteFileParams = serde_json::from_str(r#"{"sandboxId": "sample", "body": {"path": "sample", "content": "sample", "encoding": "utf8"}}"#).expect("parameters");
    assert!(matches!(
        client.write_file(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "PUT");
    assert_eq!(request.target, "/api/v1/sandboxes/sample/files");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"path": "sample", "content": "sample", "encoding": "utf8"})
    );
}
#[tokio::test]
async fn command_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: CommandParams = serde_json::from_str(r#"{"sandboxId": "sample", "body": {"command": "sample", "cwd": "sample", "timeoutSeconds": 1, "detached": true, "stream": true}}"#).expect("parameters");
    assert!(matches!(client.command(&params).await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/sandboxes/sample/commands");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"command": "sample", "cwd": "sample", "timeoutSeconds": 1, "detached": true, "stream": true})
    );
}
#[tokio::test]
async fn command_status_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: CommandStatusParams =
        serde_json::from_str(r#"{"sandboxId": "sample", "processId": 1, "tailBytes": 1}"#)
            .expect("parameters");
    assert!(matches!(
        client.command_status(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.target,
        "/api/v1/sandboxes/sample/commands/1?tailBytes=1"
    );
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn artifact_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: ArtifactParams =
        serde_json::from_str(r#"{"sandboxId": "sample", "path": "sample"}"#).expect("parameters");
    assert!(matches!(client.artifact(&params).await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.target,
        "/api/v1/sandboxes/sample/artifacts?path=sample"
    );
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn steer_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: SteerParams = serde_json::from_str(r#"{"sandboxId": "sample", "conversation": "sample", "body": {"message": "sample", "conversation": "sample"}}"#).expect("parameters");
    assert!(matches!(client.steer(&params).await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.target,
        "/api/v1/sandboxes/sample/steer?conversation=sample"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"message": "sample", "conversation": "sample"})
    );
}
#[tokio::test]
async fn interrupt_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: InterruptParams =
        serde_json::from_str(r#"{"sandboxId": "sample", "conversation": "sample"}"#)
            .expect("parameters");
    assert!(matches!(
        client.interrupt(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.target,
        "/api/v1/sandboxes/sample/interrupt?conversation=sample"
    );
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn desktop_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: DesktopParams = serde_json::from_str(
        r#"{"sandboxId": "sample", "vnc": 1, "theme": "light", "body": {"publicAccess": true}}"#,
    )
    .expect("parameters");
    assert!(matches!(client.desktop(&params).await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.target,
        "/api/v1/sandboxes/sample/desktop?vnc=1&theme=light"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"publicAccess": true})
    );
}
#[tokio::test]
async fn host_port_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: HostPortParams = serde_json::from_str(
        r#"{"sandboxId": "sample", "body": {"port": 1, "public": true, "title": "sample"}}"#,
    )
    .expect("parameters");
    assert!(matches!(
        client.host_port(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/sandboxes/sample/host");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"port": 1, "public": true, "title": "sample"})
    );
}
#[tokio::test]
async fn ssh_key_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: SshKeyParams =
        serde_json::from_str(r#"{"sandboxId": "sample", "body": {"key": "sample"}}"#)
            .expect("parameters");
    assert!(matches!(client.ssh_key(&params).await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/sandboxes/sample/sshkey");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"key": "sample"})
    );
}
#[tokio::test]
async fn list_named_snapshots_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    assert!(matches!(
        client.list_named_snapshots().await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/named-snapshots");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn save_named_snapshot_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: SaveNamedSnapshotParams =
        serde_json::from_str(r#"{"body": {"sandboxId": "sample", "name": "sample"}}"#)
            .expect("parameters");
    assert!(matches!(
        client.save_named_snapshot(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "POST");
    assert_eq!(request.target, "/api/v1/named-snapshots");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"),
        serde_json::json!({"sandboxId": "sample", "name": "sample"})
    );
}
#[tokio::test]
async fn get_named_snapshot_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: GetNamedSnapshotParams =
        serde_json::from_str(r#"{"name": "sample"}"#).expect("parameters");
    assert!(matches!(
        client.get_named_snapshot(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/named-snapshots/sample");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn delete_named_snapshot_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: DeleteNamedSnapshotParams =
        serde_json::from_str(r#"{"name": "sample"}"#).expect("parameters");
    assert!(matches!(
        client.delete_named_snapshot(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.target, "/api/v1/named-snapshots/sample");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn list_snapshots_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: ListSnapshotsParams =
        serde_json::from_str(r#"{"limit": 1, "cursor": "sample", "sort": "asc"}"#)
            .expect("parameters");
    assert!(matches!(
        client.list_snapshots(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.target,
        "/api/v1/snapshots?limit=1&cursor=sample&sort=asc"
    );
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn list_sandbox_snapshots_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: ListSandboxSnapshotsParams = serde_json::from_str(
        r#"{"sandboxId": "sample", "limit": 1, "cursor": "sample", "sort": "asc"}"#,
    )
    .expect("parameters");
    assert!(matches!(
        client.list_sandbox_snapshots(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.target,
        "/api/v1/sandboxes/sample/snapshots?limit=1&cursor=sample&sort=asc"
    );
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn delete_sandbox_snapshots_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: DeleteSandboxSnapshotsParams =
        serde_json::from_str(r#"{"sandboxId": "sample", "X-Ascii-Confirm-Delete": "sample"}"#)
            .expect("parameters");
    assert!(matches!(
        client.delete_sandbox_snapshots(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.target, "/api/v1/sandboxes/sample/snapshots");
    assert_eq!(request.headers["x-ascii-confirm-delete"], "sample");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn get_latest_sandbox_snapshot_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: GetLatestSandboxSnapshotParams =
        serde_json::from_str(r#"{"sandboxId": "sample"}"#).expect("parameters");
    assert!(matches!(
        client.get_latest_sandbox_snapshot(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/sandboxes/sample/snapshots/latest");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn usage_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: UsageParams =
        serde_json::from_str(r#"{"sandboxId": "sample", "since": "sample", "until": "sample"}"#)
            .expect("parameters");
    assert!(matches!(client.usage(&params).await, Err(Error::Api(_))));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.target,
        "/api/v1/sandboxes/sample/usage?since=sample&until=sample"
    );
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn delete_snapshot_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: DeleteSnapshotParams =
        serde_json::from_str(r#"{"snapshotId": "sample", "X-Ascii-Confirm-Delete": "sample"}"#)
            .expect("parameters");
    assert!(matches!(
        client.delete_snapshot(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.target, "/api/v1/snapshots/sample");
    assert_eq!(request.headers["x-ascii-confirm-delete"], "sample");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn get_snapshot_tree_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: GetSnapshotTreeParams =
        serde_json::from_str(r#"{"snapshotId": "sample"}"#).expect("parameters");
    assert!(matches!(
        client.get_snapshot_tree(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/snapshots/sample/tree");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn get_snapshot_file_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: GetSnapshotFileParams =
        serde_json::from_str(r#"{"snapshotId": "sample", "path": "sample"}"#).expect("parameters");
    assert!(matches!(
        client.get_snapshot_file(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/snapshots/sample/files?path=sample");
    assert!(request.body.is_empty());
}
#[tokio::test]
async fn get_snapshot_download_contract() {
    let (client, job) = support::serve(418, &[], b"refused").await;
    let params: GetSnapshotDownloadParams =
        serde_json::from_str(r#"{"snapshotId": "sample"}"#).expect("parameters");
    assert!(matches!(
        client.get_snapshot_download(&params).await,
        Err(Error::Api(_))
    ));
    let request = job.await.expect("request");
    assert_eq!(request.method, "GET");
    assert_eq!(request.target, "/api/v1/snapshots/sample/download");
    assert!(request.body.is_empty());
}
