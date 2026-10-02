// Published response examples exercise the generated wire types.
use boat::models::*;
#[test]
fn me_user() {
    let _: MeResponse = serde_json::from_str(r##"{"ok": true, "type": "user.info", "user": {"login": "octocat", "email": "octocat@example.com", "zeroDataRetention": false, "zeroDataRetentionEnabledAt": null}}"##).expect("published response");
}
#[test]
fn list_organizations_orgs() {
    let _: OrgListResponse = serde_json::from_str(r##"{"ok": true, "type": "org.list", "activeWalletChosen": true, "orgs": [{"id": "4f1c2e0a-7b3d-4a6e-9c8b-2d5e1f0a3b7c", "name": "Personal", "type": "personal", "role": "owner", "memberCount": 1, "active": false}, {"id": "team_583fd89e-6d29-4007-b48d-52477656cb0f", "name": "acme", "type": "org", "role": "owner", "memberCount": 3, "subscriptionStatus": "active", "active": true}]}"##).expect("published response");
}
#[test]
fn set_active_organization_org() {
    let _: ActiveOrgResponse = serde_json::from_str(r##"{"ok": true, "type": "org.active_updated", "active": {"id": "team_583fd89e-6d29-4007-b48d-52477656cb0f", "name": "acme", "type": "org"}}"##).expect("published response");
}
#[test]
fn limits_ready() {
    let _: LimitsResponse = serde_json::from_str(r##"{"ok": true, "type": "limits.info", "canStart": true, "activeSandboxes": 1, "activeStates": ["provisioned", "cloning", "ready", "idle", "running"], "sandboxPlanKey": "box_20", "sandboxPlanDollars": 20, "maxActiveSandboxes": 100, "maxCreationRequestsPerMinute": 10, "maxCreationRequestsPerDay": null, "startLimits": {"perMinute": 10, "perHour": 50, "perDay": 150}, "starts": {"unlimited": false, "minute": {"limit": 10, "used": 3, "remaining": 7}, "hour": {"limit": 50, "used": 12, "remaining": 38}, "day": {"limit": 150, "used": 47, "remaining": 103}}, "billingStatus": "active", "creditBalanceSeconds": 7200, "creditBalanceHours": 2, "packBalanceSeconds": 500000, "packBalanceHours": 138.89, "packBalanceDollars": 5}"##).expect("published response");
}
#[test]
fn repos_repos() {
    let _: ReposResponse = serde_json::from_str(r##"{"ok": true, "type": "repos.list", "environmentId": "env_123", "installations": [{"type": "Organization", "accountLogin": "acme", "accountAvatarUrl": "https://github.com/acme.png", "repositories": [{"id": 123456, "databaseId": "repo_org_123456", "name": "web", "fullName": "acme/web", "description": "Marketing site", "url": "https://github.com/acme/web", "private": true, "permissions": "admin", "pushedAt": "2026-05-31T12:00:00Z"}]}], "selectedRepositories": [{"id": 123456, "databaseId": "repo_org_123456", "name": "web", "fullName": "acme/web", "private": true, "permissions": "admin", "pushedAt": "2026-05-31T12:00:00Z", "baseBranch": "dev", "setupRoutineId": null, "setupScript": "", "setupBlocking": false}]}"##).expect("published response");
}
#[test]
fn select_repo_selected() {
    let _: RepoSelectionResponse = serde_json::from_str(r##"{"ok": true, "type": "repos.updated", "success": true, "environmentId": "env_123", "selectedRepositories": [{"databaseId": "repo_org_123456", "name": "web", "fullName": "acme/web", "baseBranch": "dev", "setupRoutineId": null, "setupScript": "", "setupBlocking": false}]}"##).expect("published response");
}
#[test]
fn api_keys_keys() {
    let _: ApiKeysResponse = serde_json::from_str(r##"{"ok": true, "type": "api_key.list", "apiKeys": [{"id": "sak_123", "name": "Production worker", "credentialLane": "scoped-v1", "keyPrefix": "boat_live", "keyLastFour": "9abc", "sandboxId": null, "createdAt": "2026-05-31T12:00:00Z", "lastUsedAt": null, "usage": {"requests": 1842, "windowDays": 30}, "resources": {"total": 3, "sandboxes": 2, "agents": 1}, "expiresAt": "2026-11-21T12:00:00Z", "expired": false, "expiringSoon": false, "scope": {"actions": ["*"], "sandboxes": "*", "environments": "*", "grandfathered": false}}], "catalog": {"actions": ["sandbox.read", "exec", "*"], "presets": {"read-only": ["sandbox.read", "file.read", "snapshot.read", "environment.read", "account.read"]}, "defaultTtl": "90d", "maxTtl": "365d", "scopedCreationEnabled": true}}"##).expect("published response");
}
#[test]
fn api_key_usage_usage() {
    let _: ApiKeyUsageResponse = serde_json::from_str(r##"{"ok": true, "type": "api_key.usage", "id": "sak_123", "name": "Production worker", "keyPrefix": "boat_live", "keyLastFour": "9abc", "sandboxId": null, "createdAt": "2026-05-31T12:00:00Z", "lastUsedAt": "2026-08-25T09:30:00Z", "usage": {"requests": 1842, "windowDays": 30}, "resources": {"total": 2, "sandboxes": 1, "agents": 1}, "createdResources": [{"kind": "sandbox", "id": "bx_123", "name": "CI build", "state": "ready", "createdAt": "2026-08-24T14:00:00Z"}, {"kind": "agent", "id": "agent_456", "name": "Review", "state": "idle", "createdAt": "2026-08-23T12:00:00Z"}]}"##).expect("published response");
}
#[test]
fn create_webhook_created() {
    let _: WebhookSecretResponse = serde_json::from_str(r##"{"ok": true, "type": "webhook.created", "webhook": {"id": "wh_0123456789abcdef01234567", "name": "Production automation", "url": "https://example.com/hooks/sandbox", "events": ["sandbox.ready", "sandbox.error"], "createdAt": "2026-08-11T12:00:00Z", "updatedAt": "2026-08-11T12:00:00Z"}, "secret": "whsec_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}"##).expect("published response");
}
#[test]
fn secrets_secrets() {
    let _: SecretsResponse = serde_json::from_str(r##"{"ok": true, "type": "secrets.info", "environmentId": "env_123", "envContents": "OPENAI_API_KEY=sk-...\n", "secretFiles": [{"path": ".config/service-account.json", "contents": "{\"type\":\"service_account\"}"}]}"##).expect("published response");
}
#[test]
fn update_secrets_updated() {
    let _: SecretsResponse = serde_json::from_str(r##"{"ok": true, "type": "secrets.updated", "success": true, "environmentId": "env_123", "envContents": "OPENAI_API_KEY=sk-...\n", "secretFiles": [], "pushed": {"updated": 2, "failed": 0}}"##).expect("published response");
}
#[test]
fn environments_environments() {
    let _: SandboxEnvironmentListResponse = serde_json::from_str(r##"{"environments": [{"id": "8f1c2d3e-0000-4000-8000-000000000001", "name": "default", "isDefault": true, "latestVersionId": "8f1c2d3e-0000-4000-8000-0000000000a1", "safeForThirdParties": false, "passGithub": true, "passSecrets": true, "passSandboxCredentials": true, "passAgentsCredentials": true, "envContents": "OPENAI_API_KEY=sk-...\n", "secretFiles": [], "versions": [{"id": "8f1c2d3e-0000-4000-8000-0000000000a1", "versionNumber": 1, "sandboxCount": 3, "createdAt": "2026-06-01T12:00:00Z"}]}]}"##).expect("published response");
}
#[test]
fn upgrade_environment_upgraded() {
    let _: UpgradeSandboxEnvironmentResponse =
        serde_json::from_str(r##"{"success": true, "upgraded": 2, "failed": 0}"##)
            .expect("published response");
}
#[test]
fn sandboxes_sandboxes() {
    let _: SandboxListResponse = serde_json::from_str(r##"{"ok": true, "type": "sandbox.list", "sandboxes": [{"id": "bx_23456789", "name": "Sandbox 2026-05-31 12:00", "state": "idle", "url": "https://machine.on.boat.dev", "ip": "203.0.113.10", "createdAt": "2026-05-31T12:00:00Z", "updatedAt": "2026-05-31T12:05:00Z", "archiveAfter": "2026-05-31T13:00:00Z", "desktopAvailable": true, "desktopUrl": "https://desktop.example/stream.html?token=redacted", "snapshotAvailable": false, "snapshotCompletedAt": null}]}"##).expect("published response");
}
#[test]
fn create_provisioning() {
    let _: CreateSandboxResponse = serde_json::from_str(r##"{"ok": true, "type": "sandbox.created", "status": "provisioning", "ttlSeconds": 3600, "sandbox": {"id": "bx_23456789", "name": "Sandbox 2026-05-31 12:00", "state": "provisioning", "url": null, "ip": null, "createdAt": "2026-05-31T12:00:00Z", "updatedAt": "2026-05-31T12:00:00Z", "archiveAfter": "2026-05-31T13:00:00Z", "desktopAvailable": false, "desktopUrl": null, "snapshotAvailable": false, "snapshotCompletedAt": null}}"##).expect("published response");
}
#[test]
fn get_sandbox() {
    let _: SandboxInfoResponse = serde_json::from_str(r##"{"ok": true, "type": "sandbox.info", "sandbox": {"id": "bx_23456789", "name": "Sandbox 2026-05-31 12:00", "state": "idle", "url": "https://machine.on.boat.dev", "ip": "203.0.113.10", "createdAt": "2026-05-31T12:00:00Z", "updatedAt": "2026-05-31T12:05:00Z", "archiveAfter": "2026-05-31T13:00:00Z", "desktopAvailable": true, "desktopUrl": "https://desktop.example/stream.html?token=redacted", "snapshotAvailable": false, "snapshotCompletedAt": null}}"##).expect("published response");
}
#[test]
fn stop_archiving() {
    let _: SandboxActionResponse = serde_json::from_str(r##"{"ok": true, "type": "sandbox.stopping", "id": "bx_23456789", "status": "archiving", "sandbox": {"id": "bx_23456789", "name": "Sandbox 2026-05-31 12:00", "state": "archiving", "desktopAvailable": true, "snapshotAvailable": false}}"##).expect("published response");
}
#[test]
fn resume_resuming() {
    let _: SandboxActionResponse = serde_json::from_str(r##"{"ok": true, "type": "sandbox.resuming", "id": "bx_23456789", "status": "resuming", "sandbox": {"id": "bx_23456789", "name": "Sandbox 2026-05-31 12:00", "state": "provisioning", "desktopAvailable": false, "snapshotAvailable": true}}"##).expect("published response");
}
#[test]
fn fork_forking() {
    let _: SandboxActionResponse = serde_json::from_str(r##"{"ok": true, "type": "sandbox.forking", "id": "bx_abcdef23", "status": "forking", "sandbox": {"id": "bx_abcdef23", "name": "Sandbox fork", "state": "provisioning", "desktopAvailable": false, "snapshotAvailable": false}}"##).expect("published response");
}
#[test]
fn prompt_queued() {
    let _: PromptResponse = serde_json::from_str(r##"{"ok": true, "type": "prompt.queued", "id": "bx_23456789", "promptId": "prompt_123", "conversationId": "8f1c2b7a-3d4e-4f5a-9b0c-1d2e3f4a5b6c", "promptRun": {"id": "prompt_123", "promptId": "prompt_123", "sandboxId": "bx_23456789", "status": "queued", "done": false, "conversationId": "8f1c2b7a-3d4e-4f5a-9b0c-1d2e3f4a5b6c"}, "status": "queued", "provider": "codex", "model": "gpt-5.4", "reasoningEffort": "medium", "fast": false}"##).expect("published response");
}
#[test]
fn events_progress() {
    let _: EventsResponse = serde_json::from_str(r##"{"ok": true, "type": "events.list", "id": "bx_23456789", "events": [{"id": "prompt_123", "type": "prompt", "timestamp": 1780347055370, "taskId": "prompt_123", "data": {"prompt": "Run tests and summarize failures.", "status": "running", "is_reverted": false}}, {"id": "response_123-tools", "type": "response", "timestamp": 1780347063427, "taskId": "prompt_123", "data": {"content": "", "model": "gpt-5.4", "tools": [{"use": {"id": "call_123", "type": "tool_use", "name": "Bash", "input": {"command": "npm test"}}, "result": {"type": "tool_result", "tool_use_id": "call_123", "is_error": false, "content": "{\"exitCode\":0}"}}], "is_reverted": false}}, {"id": "response_123", "type": "response", "timestamp": 1780347065650, "taskId": "prompt_123", "data": {"content": "Tests passed.", "model": "gpt-5.4", "is_reverted": false}}], "pageInfo": {"nextCursor": null, "hasMore": false, "limit": 100}}"##).expect("published response");
}
#[test]
fn conversations_two_conversations() {
    let _: ConversationsResponse = serde_json::from_str(r##"{"ok": true, "type": "conversation.list", "id": "bx_23456789", "conversations": [{"id": "8f1c2b7a-3d4e-4f5a-9b0c-1d2e3f4a5b6c", "createdAt": "2026-09-08T10:05:00.000Z", "lastPromptAt": "2026-09-08T10:05:00.000Z", "prompts": 1, "running": true, "lastHarness": "pi", "lastModel": "claude-sonnet-5", "lastPromptPreview": "Investigate the flaky CI job", "current": true}, {"id": "2c0d9e4b-7a1f-4c3e-8b5d-6e7f8a9b0c1d", "createdAt": "2026-09-08T09:40:00.000Z", "lastPromptAt": "2026-09-08T10:03:00.000Z", "prompts": 2, "running": false, "lastHarness": "claude", "lastModel": "claude-opus-4-8", "lastPromptPreview": "Now add tests", "current": false}]}"##).expect("published response");
}
#[test]
fn steer_steered() {
    let _: SteerResponse = serde_json::from_str(r##"{"ok": true, "type": "prompt.steered", "id": "bx_23456789", "conversationId": "8f1c2b7a-3d4e-4f5a-9b0c-1d2e3f4a5b6c", "promptId": "prompt_456", "native": true, "mode": "native", "status": "steered"}"##).expect("published response");
}
#[test]
fn interrupt_interrupted() {
    let _: SandboxActionResponse = serde_json::from_str(r##"{"ok": true, "type": "sandbox.interrupted", "id": "bx_23456789", "status": "interrupted"}"##).expect("published response");
}
#[test]
fn desktop_ready() {
    let _: DesktopResponse = serde_json::from_str(r##"{"ok": true, "type": "desktop.url", "success": true, "desktopUrl": "https://sandbox-preview.example/vnc.html?_token=redacted", "ip": "203.0.113.10", "mode": "vnc"}"##).expect("published response");
}
#[test]
fn desktop_provisioning() {
    let _: DesktopResponse = serde_json::from_str(r##"{"ok": true, "type": "desktop.provisioning", "provisioning": true, "message": "Preparing VNC desktop\u2026"}"##).expect("published response");
}
#[test]
fn host_port_hosted() {
    let _: HostPortResponse = serde_json::from_str(r##"{"ok": true, "type": "port.hosted", "success": true, "port": 3000, "url": "https://swift-otter-9021-3000.on.boat.dev?_token=redacted", "isProtected": true, "access": "private"}"##).expect("published response");
}
#[test]
fn ssh_key_configured() {
    let _: SshKeyResponse = serde_json::from_str(r##"{"ok": true, "type": "ssh_key.configured", "success": true, "machineIp": "203.0.113.10", "sshUser": "user"}"##).expect("published response");
}
#[test]
fn list_snapshots_snapshots() {
    let _: SnapshotListResponse = serde_json::from_str(r##"{"ok": true, "type": "snapshot.list", "snapshots": [{"id": "7417be09-d419-4ae0-b3fc-7f04a5a71ef1", "sandboxId": "bx_23456789", "status": "completed", "kind": "incremental", "generation": 3, "chainId": "4ced5b04-d2cb-4ec3-b127-3b3ed836cab5", "createdAt": "2026-06-24T06:24:00Z", "completedAt": "2026-06-24T06:24:50Z", "sizeBytes": 18874368, "fileCount": 6781}], "pageInfo": {"nextCursor": null, "hasMore": false, "limit": 50}}"##).expect("published response");
}
#[test]
fn get_latest_sandbox_snapshot_latest() {
    let _: SnapshotLatestResponse = serde_json::from_str(r##"{"ok": true, "type": "snapshot.latest", "snapshot": {"id": "7417be09-d419-4ae0-b3fc-7f04a5a71ef1", "sandboxId": "bx_23456789", "status": "completed", "kind": "incremental", "generation": 3, "chainId": "4ced5b04-d2cb-4ec3-b127-3b3ed836cab5", "createdAt": "2026-06-24T06:24:00Z", "completedAt": "2026-06-24T06:24:50Z", "sizeBytes": 18874368, "fileCount": 6781}}"##).expect("published response");
}
#[test]
fn get_latest_sandbox_snapshot_none() {
    let _: SnapshotLatestResponse =
        serde_json::from_str(r##"{"ok": true, "type": "snapshot.latest", "snapshot": null}"##)
            .expect("published response");
}
#[test]
fn usage_usage() {
    let _: SandboxUsageResponse = serde_json::from_str(r##"{"ok": true, "type": "sandbox.usage", "sandboxId": "bx_23456789", "sandboxType": "default", "billingMultiplier": 1, "since": "2026-09-01T00:00:00.000Z", "until": "2026-09-14T09:30:00.000Z", "seconds": 4980, "dollars": 0.0498, "secondsPerDollar": 100000, "running": false}"##).expect("published response");
}
#[test]
fn get_snapshot_tree_tree() {
    let _: SnapshotTreeResponse = serde_json::from_str(r##"{"ok": true, "type": "snapshot.tree", "snapshotId": "7417be09-d419-4ae0-b3fc-7f04a5a71ef1", "sandboxId": "bx_23456789", "generation": 3, "treeAvailable": true, "truncated": false, "fileCount": 6781, "totalSizeBytes": 458291, "entries": [{"path": "src", "kind": "dir"}, {"path": "src/main.ts", "kind": "file", "size": 1024}]}"##).expect("published response");
}
#[test]
fn get_snapshot_tree_legacy() {
    let _: SnapshotTreeResponse = serde_json::from_str(r##"{"ok": true, "type": "snapshot.tree", "snapshotId": "7417be09-d419-4ae0-b3fc-7f04a5a71ef1", "sandboxId": "bx_23456789", "generation": 0, "treeAvailable": false, "truncated": false, "fileCount": 0, "totalSizeBytes": 0, "entries": [], "reason": "legacy_snapshot"}"##).expect("published response");
}
#[test]
fn get_snapshot_download_download() {
    let _: SnapshotDownloadResponse = serde_json::from_str(r##"{"ok": true, "type": "snapshot.download", "snapshotId": "7417be09-d419-4ae0-b3fc-7f04a5a71ef1", "sandboxId": "bx_23456789", "kind": "incremental", "generation": 3, "expiresInSeconds": 3600, "reconstruct": "Download every chunk. For each snapshot in ascending generation, concat its chunks by chunkIndex and pipe through `zstd -d | tar -x`.", "inventory": {"r2Key": "chains/4ced5b04/inventory-3.json.zst", "signedUrl": "https://r2.example/chains/4ced5b04/inventory-3.json.zst?sig=redacted"}, "chunks": [{"snapshotId": "2db50582-716c-424c-817e-9495484f88dd", "generation": 0, "chunkIndex": 0, "r2Key": "chains/4ced5b04/snapshots/2db50582/chunks/00.tar.zst", "sizeBytes": 209715200, "sha256": "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08", "signedUrl": "https://r2.example/chains/4ced5b04/snapshots/2db50582/chunks/00.tar.zst?sig=redacted"}]}"##).expect("published response");
}
