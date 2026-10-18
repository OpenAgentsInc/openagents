mod common;

use archil::{
    AuthorizedUser, CreateApiTokenRequest, CreateBranchOptions, CreateDiskRequest, Delegation,
    DiskUser, Error, GrepOptions, GrepStoppedReason, ListDisksOptions, ListTokensOptions,
    MountConfig, RootAttrs, ShareUrlOptions, TokenUser, UserType,
};
use common::*;
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn get_disk_parses_fields_and_derefs() {
    let server = MockServer::start().await;
    let mut body = disk_json("dsk-1", "data");
    body["activeDataBytes"] = 42.into();
    body["allowedIps"] = json!(["1.2.3.4"]);
    body["rootAttrs"] = json!({"uid": 1000, "gid": 1000, "mode": 493});
    body["mounts"] =
        json!([{"id": "m1", "type": "s3", "accessMode": "rw", "connectionStatus": "connected"}]);
    Mock::given(method("GET"))
        .and(path("/api/disks/dsk-1"))
        .respond_with(ok(body))
        .mount(&server)
        .await;
    let disk = client(&server).disks().get("dsk-1").await.unwrap();
    assert_eq!(disk.name, "data");
    assert_eq!(disk.active_data_bytes, Some(42));
    assert_eq!(
        disk.root_attrs,
        Some(RootAttrs {
            uid: Some(1000),
            gid: Some(1000),
            mode: Some(0o755)
        })
    );
    assert_eq!(disk.mounts.as_ref().unwrap()[0].kind.as_deref(), Some("s3"));
    assert_eq!(disk.to_response().id, "dsk-1");
}

#[tokio::test]
async fn list_follows_cursors_and_respects_limit() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/disks"))
        .and(query_param_is_missing("cursor"))
        .and(query_param("limit", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true, "data": [disk_json("a", "a"), disk_json("b", "b")], "nextCursor": "c1"
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/disks"))
        .and(query_param("cursor", "c1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true, "data": [disk_json("c", "c")]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let disks = client(&server)
        .disks()
        .list(ListDisksOptions::default())
        .await
        .unwrap();
    assert_eq!(
        disks.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
}

#[tokio::test]
async fn list_limit_caps_total_and_truncates_servers_that_ignore_limit() {
    let server = MockServer::start().await;
    // A server that predates pagination ignores `limit` and returns everything.
    Mock::given(method("GET"))
        .and(path("/api/disks"))
        .and(query_param("limit", "2"))
        .respond_with(ok(json!([
            disk_json("a", "a"),
            disk_json("b", "b"),
            disk_json("c", "c")
        ])))
        .expect(1)
        .mount(&server)
        .await;
    let disks = client(&server)
        .disks()
        .list(ListDisksOptions {
            limit: Some(2),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(disks.len(), 2);
}

#[tokio::test]
async fn list_stops_on_repeated_cursor_and_handles_null_data() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/disks"))
        .and(query_param("cursor", "same"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true, "data": [disk_json("b", "b")], "nextCursor": "same"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let c = client(&server);
    let disks = c
        .disks()
        .list(ListDisksOptions {
            cursor: Some("same".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(disks.len(), 1);

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/disks"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"success": true, "data": null})),
        )
        .mount(&server)
        .await;
    assert!(
        client(&server)
            .disks()
            .list(ListDisksOptions::default())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn list_with_name_returns_one_page_and_passes_filter() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/disks"))
        .and(query_param("name", "x"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true, "data": [disk_json("a", "x")], "nextCursor": "more"
        })))
        .expect(2)
        .mount(&server)
        .await;
    let c = client(&server);
    let opts = ListDisksOptions {
        name: Some("x".into()),
        ..Default::default()
    };
    assert_eq!(c.disks().list(opts.clone()).await.unwrap().len(), 1);
    let page = c.disks().list_page(opts).await.unwrap();
    assert_eq!(page.next_cursor.as_deref(), Some("more"));
}

#[tokio::test]
async fn create_returns_token_and_fetches_disk() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/disks"))
        .and(body_json(json!({
            "name": "my-disk",
            "mounts": [{"type": "s3", "bucketName": "b", "bucketPrefix": "p/"}],
            "allowedIps": ["10.0.0.0/8"],
            "rootAttrs": {"uid": 1000, "gid": 1000, "mode": 493},
        })))
        .respond_with(status(
            201,
            json!({"diskId": "dsk-9", "authorizedUsers": [
                {"type": "token", "nickname": "default", "token": "secret", "identifier": "ident-1"}
            ]}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/disks/dsk-9"))
        .respond_with(ok(disk_json("dsk-9", "my-disk")))
        .expect(1)
        .mount(&server)
        .await;
    let mut req = CreateDiskRequest::new("my-disk");
    req.mounts = Some(vec![MountConfig::S3 {
        bucket_name: "b".into(),
        access_key_id: None,
        secret_access_key: None,
        session_token: None,
        bucket_prefix: Some("p/".into()),
        bucket_region: None,
    }]);
    req.allowed_ips = Some(vec!["10.0.0.0/8".into()]);
    req.root_attrs = Some(RootAttrs {
        uid: Some(1000),
        gid: Some(1000),
        mode: Some(0o755),
    });
    let created = client(&server).disks().create(req).await.unwrap();
    assert_eq!(created.disk.id, "dsk-9");
    assert_eq!(created.token.as_deref(), Some("secret"));
    assert_eq!(created.token_identifier.as_deref(), Some("ident-1"));
    assert_eq!(created.authorized_users.len(), 1);
}

#[tokio::test]
async fn create_without_disk_id_is_an_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/disks"))
        .respond_with(ok(json!({})))
        .mount(&server)
        .await;
    let e = client(&server)
        .disks()
        .create(CreateDiskRequest::new("x"))
        .await
        .unwrap_err();
    assert!(e.to_string().contains("no diskId"), "{e}");
}

#[tokio::test]
async fn mount_config_variants_serialize_with_wire_tags() {
    let m = MountConfig::AzureBlob {
        container_name: "c".into(),
        endpoint: None,
        storage_account_name: Some("acct".into()),
        tenant_id: "t".into(),
        client_id: "i".into(),
        client_secret: "s".into(),
        bucket_prefix: None,
    };
    assert_eq!(
        serde_json::to_value(&m).unwrap(),
        json!({"type": "azure-blob", "containerName": "c", "storageAccountName": "acct",
               "tenantId": "t", "clientId": "i", "clientSecret": "s"})
    );
    let r2: MountConfig = serde_json::from_value(
        json!({"type": "r2", "bucketName": "b", "bucketEndpoint": "e", "accessKeyId": "a", "secretAccessKey": "s"}),
    )
    .unwrap();
    assert!(matches!(r2, MountConfig::R2 { .. }));
}

#[tokio::test]
async fn users_and_tokens() {
    let server = MockServer::start().await;
    let d = disk_json("d1", "n");
    Mock::given(method("GET"))
        .and(path("/api/disks/d1"))
        .respond_with(ok(d))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/disks/d1/users"))
        .and(body_json(
            json!({"type": "token", "nickname": "ci", "ttl": "1h", "readOnly": true}),
        ))
        .respond_with(status(
            201,
            json!({"type": "token", "nickname": "ci", "token": "t", "identifier": "i"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/disks/d1/users"))
        .and(body_json(
            json!({"type": "awssts", "principal": "arn:aws:iam::1:role/r"}),
        ))
        .respond_with(status(
            201,
            json!({"type": "awssts", "identifier": "arn:aws:iam::1:role/r"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/disks/d1/users"))
        .and(body_json(json!({"type": "token", "nickname": "plain"})))
        .respond_with(status(201, json!({"type": "token", "nickname": "plain"})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/disks/d1/users/token"))
        .and(query_param("identifier", "i"))
        .respond_with(ok(json!(null)))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/disks/d1/users/awssts"))
        .and(query_param("identifier", "arn:aws:iam::1:role/r"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let disk = client(&server).disks().get("d1").await.unwrap();
    let user: AuthorizedUser = disk
        .add_user(DiskUser::Token(TokenUser {
            nickname: "ci".into(),
            ttl: Some("1h".into()),
            read_only: Some(true),
            ..Default::default()
        }))
        .await
        .unwrap();
    assert_eq!(user.token.as_deref(), Some("t"));
    disk.add_user(DiskUser::AwsSts(archil::AwsStsUser {
        principal: "arn:aws:iam::1:role/r".into(),
        ..Default::default()
    }))
    .await
    .unwrap();
    disk.remove_user(UserType::Token, "i").await.unwrap();
    disk.remove_token_user("i").await.unwrap();
    disk.remove_user(UserType::AwsSts, "arn:aws:iam::1:role/r")
        .await
        .unwrap();

    // create_token insists on a generated secret.
    let e = disk.create_token("plain").await.unwrap_err();
    assert!(
        e.to_string().contains("did not return a generated token"),
        "{e}"
    );
}

#[tokio::test]
async fn create_token_returns_generated_secret() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/disks/d1"))
        .respond_with(ok(disk_json("d1", "n")))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/disks/d1/users"))
        .and(body_json(json!({"type": "token", "nickname": "agent"})))
        .respond_with(status(
            201,
            json!({"type": "token", "token": "tok", "identifier": "id"}),
        ))
        .mount(&server)
        .await;
    let disk = client(&server).disks().get("d1").await.unwrap();
    let u = disk.create_token("agent").await.unwrap();
    assert_eq!(
        (u.token.as_deref(), u.identifier.as_deref()),
        (Some("tok"), Some("id"))
    );
}

#[tokio::test]
async fn allowed_ips_get_set_add_remove() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/disks/d1"))
        .respond_with(ok(disk_json("d1", "n")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/disks/d1/allowed-ips"))
        .respond_with(ok(json!({"allowedIps": ["1.1.1.1"]})))
        .mount(&server)
        .await;
    let (cap, bodies) = Capture::new(ok(json!({"allowedIps": ["echo"]})));
    Mock::given(method("PUT"))
        .and(path("/api/disks/d1/allowed-ips"))
        .respond_with(cap)
        .mount(&server)
        .await;
    let disk = client(&server).disks().get("d1").await.unwrap();
    assert_eq!(disk.get_allowed_ips().await.unwrap(), ["1.1.1.1"]);
    disk.set_allowed_ips(&[]).await.unwrap();
    // Adding an IP already present performs no write.
    assert_eq!(disk.add_allowed_ip("1.1.1.1").await.unwrap(), ["1.1.1.1"]);
    disk.add_allowed_ip("2.2.2.2").await.unwrap();
    disk.remove_allowed_ip("1.1.1.1").await.unwrap();
    let sent = bodies.lock().unwrap().clone();
    assert_eq!(
        sent,
        [
            json!({"allowedIps": []}),
            json!({"allowedIps": ["1.1.1.1", "2.2.2.2"]}),
            json!({"allowedIps": []}),
        ]
    );
}

#[tokio::test]
async fn delegations_list_and_revoke() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/disks/d1"))
        .respond_with(ok(disk_json("d1", "n")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/disks/d1/delegations"))
        .respond_with(ok(json!({"delegations": [
            {"clientId": "c1", "inodeId": 7, "path": "/a", "isPending": false, "isOrphaned": true}
        ]})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/disks/d1/revoke-delegation"))
        .and(body_json(json!({"clientId": "c1", "inodeId": 7})))
        .respond_with(ok(json!({"message": "ok"})))
        .expect(1)
        .mount(&server)
        .await;
    let disk = client(&server).disks().get("d1").await.unwrap();
    let ds: Vec<Delegation> = disk.list_delegations().await.unwrap();
    assert!(ds[0].is_orphaned);
    for d in ds.iter().filter(|d| d.is_orphaned) {
        disk.revoke_delegation(d).await.unwrap();
    }
}

#[tokio::test]
async fn branches_create_with_idempotency_key_list_get() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/disks/d1"))
        .respond_with(ok(disk_json("d1", "n")))
        .mount(&server)
        .await;
    let branch = json!({
        "root_filesystem_id": "d1", "branch_name": "feat", "filesystem_id": "fs2",
        "from_checkpoint_name": "cp", "from_checkpoint_filesystem_id": "d1", "created_at": "2026-01-01T00:00:00Z"
    });
    Mock::given(method("POST"))
        .and(path("/api/disks/d1/branches"))
        .and(header("idempotency-key", "my-key"))
        .and(body_json(
            json!({"branch_name": "feat", "from_checkpoint_name": "cp", "from_branch": "main"}),
        ))
        .respond_with(status(201, branch.clone()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/disks/d1/branches"))
        .and(body_json(
            json!({"branch_name": "gen", "from_checkpoint_name": "cp"}),
        ))
        .respond_with(status(201, branch.clone()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/disks/d1/branches"))
        .respond_with(ok(json!([branch.clone()])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/disks/d1/branches/feat"))
        .respond_with(ok(branch))
        .mount(&server)
        .await;
    let disk = client(&server).disks().get("d1").await.unwrap();
    let b = disk
        .create_branch(CreateBranchOptions {
            name: "feat".into(),
            from_checkpoint: "cp".into(),
            from_branch: Some("main".into()),
            idempotency_key: Some("my-key".into()),
        })
        .await
        .unwrap();
    assert_eq!(b.filesystem_id, "fs2");
    disk.create_branch(CreateBranchOptions {
        name: "gen".into(),
        from_checkpoint: "cp".into(),
        ..Default::default()
    })
    .await
    .unwrap();
    assert_eq!(disk.list_branches().await.unwrap().len(), 1);
    assert_eq!(disk.get_branch("feat").await.unwrap().branch_name, "feat");
}

#[tokio::test]
async fn branch_create_reuses_idempotency_key_across_retries() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/disks/d1"))
        .respond_with(ok(disk_json("d1", "n")))
        .mount(&server)
        .await;
    let seq = Seq::new(vec![
        err(503, "busy", None),
        status(
            201,
            json!({"root_filesystem_id": "d1", "branch_name": "b", "filesystem_id": "f",
                   "from_checkpoint_name": "c", "from_checkpoint_filesystem_id": "d1", "created_at": "t"}),
        ),
    ]);
    Mock::given(method("POST"))
        .and(path("/api/disks/d1/branches"))
        .respond_with(seq)
        .mount(&server)
        .await;
    let disk = client(&server).disks().get("d1").await.unwrap();
    disk.create_branch(CreateBranchOptions {
        name: "b".into(),
        from_checkpoint: "c".into(),
        ..Default::default()
    })
    .await
    .unwrap();
    let reqs = server.received_requests().await.unwrap();
    let keys: Vec<_> = reqs
        .iter()
        .filter(|r| r.method.as_str() == "POST")
        .map(|r| {
            r.headers
                .get("idempotency-key")
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(keys.len(), 2);
    assert_eq!(keys[0], keys[1], "retry must reuse the key");
    assert_eq!(keys[0].len(), 36, "default key is a UUID");
}

#[tokio::test]
async fn exec_grep_share_and_delete() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/disks/d1"))
        .respond_with(ok(disk_json("d1", "n")))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/disks/d1/exec"))
        .and(body_json(json!({"command": "ls /mnt"})))
        .respond_with(ok(json!({"exitCode": 0, "stdout": "a\n", "stderr": "", "timing": {"totalMs": 10, "queueMs": 4, "executeMs": 6}})))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/disks/d1/grep"))
        .and(body_json(json!({
            "directory": "logs", "pattern": "ERR", "recursive": true,
            "maxDurationSeconds": 30, "concurrency": 50, "maxResults": 5
        })))
        .respond_with(ok(json!({
            "matches": [{"file": "logs/a", "line": 3, "text": "ERR x"}], "stoppedReason": "max_results",
            "filesScanned": 2, "containersDispatched": 1, "computeSecondsUsed": 0.5,
            "durationMs": 100, "listingMs": 20, "grepMs": 70
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/disks/d1/share"))
        .and(body_json(json!({"key": "a/b c.pdf", "expiresIn": 60})))
        .respond_with(ok(json!({"url": "https://x/y", "expiresIn": 60})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/disks/d1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let c = client(&server);
    let disk = c.disks().get("d1").await.unwrap();
    let r = disk.exec("ls /mnt").await.unwrap();
    assert_eq!(
        (r.exit_code, r.stdout.as_str(), r.timing.queue_ms),
        (0, "a\n", 4.0)
    );
    assert_eq!(c.disks().exec("d1", "ls /mnt").await.unwrap().exit_code, 0);
    let g = disk
        .grep(
            GrepOptions::new("logs", "ERR")
                .recursive(true)
                .max_results(5),
        )
        .await
        .unwrap();
    assert_eq!(g.stopped_reason, GrepStoppedReason::MaxResults);
    assert_eq!(g.matches[0].line, 3);
    let s = disk
        .share(
            "a/b c.pdf",
            ShareUrlOptions {
                expires_in: Some(60),
            },
        )
        .await
        .unwrap();
    assert_eq!(s.url, "https://x/y");
    disk.delete().await.unwrap();
}

#[tokio::test]
async fn multi_disk_exec_sends_string_and_object_forms() {
    let server = MockServer::start().await;
    let (cap, bodies) = Capture::new(ok(
        json!({"exitCode": 0, "stdout": "", "stderr": "", "timing": {"totalMs": 1, "queueMs": 0, "executeMs": 1}}),
    ));
    Mock::given(method("POST"))
        .and(path("/api/exec"))
        .respond_with(cap)
        .mount(&server)
        .await;
    let opts = archil::ExecOptions::new("cat /mnt/archil/data/x")
        .disk("data", "dsk-1")
        .disk(
            "logs",
            archil::ExecMountSpec::new("dsk-2")
                .subdirectory("app/logs")
                .read_only(true)
                .conditional(false),
        )
        .disk(
            "work",
            archil::ExecMountSpec::new("dsk-3")
                .queue_ms(500)
                .checkout_paths(["src", "tmp/cache"]),
        );
    client(&server).exec(opts).await.unwrap();
    assert_eq!(
        bodies.lock().unwrap()[0],
        json!({
            "command": "cat /mnt/archil/data/x",
            "disks": {
                "data": "dsk-1",
                "logs": {"disk": "dsk-2", "subdirectory": "app/logs", "readOnly": true, "conditional": false},
                "work": {"disk": "dsk-3", "readOnly": false, "conditional": false, "queueMs": 500, "checkoutPaths": ["src", "tmp/cache"]}
            }
        })
    );
}

#[tokio::test]
async fn api_keys_list_create_delete() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/tokens"))
        .and(query_param("limit", "5"))
        .respond_with(ok(
            json!({"tokens": [{"id": "t1", "name": "ci", "tokenSuffix": "abcd"}]}),
        ))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/tokens"))
        .and(body_json(
            json!({"name": "new", "description": "d", "permissions": ["*"]}),
        ))
        .respond_with(status(
            201,
            json!({"id": "t2", "name": "new", "token": "full-secret"}),
        ))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/tokens/t2"))
        .respond_with(ok(json!(null)))
        .expect(1)
        .mount(&server)
        .await;
    let c = client(&server);
    let list = c
        .tokens()
        .list(ListTokensOptions {
            limit: Some(5),
            cursor: None,
        })
        .await
        .unwrap();
    assert_eq!(list[0].token_suffix.as_deref(), Some("abcd"));
    let mut req = CreateApiTokenRequest::new("new");
    req.description = Some("d".into());
    req.permissions = Some(vec!["*".into()]);
    let created = c.tokens().create(req).await.unwrap();
    assert_eq!(created.token.as_deref(), Some("full-secret"));
    assert_eq!(created.info.id.as_deref(), Some("t2"));
    c.tokens().delete("t2").await.unwrap();
}

#[tokio::test]
async fn api_errors_are_typed_with_status_and_code() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/disks/missing"))
        .respond_with(err(404, "Disk not found", Some("DISK_NOT_FOUND")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/disks/html"))
        .respond_with(ResponseTemplate::new(403).set_body_string("<html>nope</html>"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/disks/unsuccessful"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"success": false})))
        .mount(&server)
        .await;
    let c = client(&server);
    let e = c.disks().get("missing").await.unwrap_err();
    assert!(matches!(e, Error::Api(_)));
    assert_eq!(
        (e.status(), e.code(), e.to_string().as_str()),
        (Some(404), Some("DISK_NOT_FOUND"), "Disk not found")
    );
    let e = c.disks().get("html").await.unwrap_err();
    assert_eq!(
        (e.status(), e.code(), e.to_string().as_str()),
        (Some(403), None, "API request failed with status 403")
    );
    let e = c.disks().get("unsuccessful").await.unwrap_err();
    assert_eq!(e.to_string(), "Unknown API error");
}

#[tokio::test]
async fn path_segments_are_percent_encoded() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/disks/a%2Fb"))
        .respond_with(ok(disk_json("a/b", "n")))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(client(&server).disks().get("a/b").await.unwrap().id, "a/b");
}
