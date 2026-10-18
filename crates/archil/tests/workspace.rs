mod common;

use archil::{
    Disk, Error, FileSystem, GrepOptions, GrepStoppedReason, ListObjectsOptions, Workspace,
    WorkspaceMount,
};
use bytes::Bytes;
use common::*;
use serde_json::json;
use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn disks(server: &MockServer) -> (Disk, Disk) {
    for (id, name) in [("dsk-data", "data"), ("dsk-logs", "logs")] {
        Mock::given(method("GET"))
            .and(path(format!("/api/disks/{id}")))
            .respond_with(ok(disk_json(id, name)))
            .mount(server)
            .await;
    }
    let c = client(server);
    (
        c.disks().get("dsk-data").await.unwrap(),
        c.disks().get("dsk-logs").await.unwrap(),
    )
}

async fn workspace(server: &MockServer) -> Workspace {
    let (data, logs) = disks(server).await;
    client(server)
        .workspace([
            ("data", WorkspaceMount::new(data)),
            (
                "/logs/",
                WorkspaceMount::new(logs)
                    .read_only(true)
                    .subdirectory("/app/logs/"),
            ),
        ])
        .unwrap()
}

fn listing(keys: &[&str], prefixes: &[&str]) -> ResponseTemplate {
    let contents: String = keys
        .iter()
        .map(|k| format!("<Contents><Key>{k}</Key><Size>1</Size></Contents>"))
        .collect();
    let cps: String = prefixes
        .iter()
        .map(|p| format!("<CommonPrefixes><Prefix>{p}</Prefix></CommonPrefixes>"))
        .collect();
    ResponseTemplate::new(200).set_body_raw(
        format!(
            "<ListBucketResult><IsTruncated>false</IsTruncated>{contents}{cps}</ListBucketResult>"
        ),
        "application/xml",
    )
}

#[tokio::test]
async fn keys_route_to_the_named_disk_and_apply_the_subdirectory() {
    let server = MockServer::start().await;
    let ws = workspace(&server).await;
    assert_eq!(
        ws.disk_names(),
        ["data", "logs"],
        "slashes in mount names are trimmed"
    );
    Mock::given(method("GET"))
        .and(path("/dsk-data/a/b.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_string("data!"))
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/dsk-logs/app/logs/x.log"))
        .respond_with(ResponseTemplate::new(200).set_body_string("log!"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/dsk-data/new.txt"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/dsk-data/old.txt"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    assert_eq!(&ws.get_object("data/a/b.txt").await.unwrap()[..], b"data!");
    // Paths are normalized like a shell would: "." and ".." resolve.
    assert_eq!(
        &ws.get_object("/data/./a/../a/b.txt").await.unwrap()[..],
        b"data!"
    );
    assert_eq!(&ws.get_object("logs/x.log").await.unwrap()[..], b"log!");
    ws.put_object("data/new.txt", "x").await.unwrap();
    ws.delete_object("data/old.txt").await.unwrap();
}

#[tokio::test]
async fn read_only_mounts_reject_writes_without_a_request() {
    let server = MockServer::start().await;
    let ws = workspace(&server).await;
    let e = ws.put_object("logs/x.log", "nope").await.unwrap_err();
    assert!(matches!(e, Error::InvalidArgument(_)));
    assert_eq!(
        e.to_string(),
        "logs/x.log is on a read-only disk and cannot be written."
    );
    let e = ws.delete_object("logs/x.log").await.unwrap_err();
    assert_eq!(
        e.to_string(),
        "logs/x.log is on a read-only disk and cannot be deleted."
    );
    assert!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|r| r.url.path().starts_with("/api/disks/")),
        "no S3 traffic"
    );
}

#[tokio::test]
async fn bad_keys_explain_how_to_address_objects() {
    let server = MockServer::start().await;
    let ws = workspace(&server).await;
    let e = ws.get_object("nodisk/file").await.unwrap_err();
    assert_eq!(
        e.to_string(),
        "No disk named 'nodisk'. Address objects by disk: data/…, logs/…"
    );
    let e = ws.get_object("/../..").await.unwrap_err();
    assert!(e.to_string().contains("workspace root"), "{e}");
}

#[tokio::test]
async fn root_listing_names_the_disks_without_any_request() {
    let server = MockServer::start().await;
    let ws = workspace(&server).await;
    let before = server.received_requests().await.unwrap().len();
    let r = ws
        .list_objects(None, ListObjectsOptions::default())
        .await
        .unwrap();
    assert_eq!(r.common_prefixes, ["data/", "logs/"]);
    assert!(r.objects.is_empty() && !r.is_truncated);
    assert_eq!(server.received_requests().await.unwrap().len(), before);
}

#[tokio::test]
async fn recursive_listing_fans_out_remaps_keys_and_sorts() {
    let server = MockServer::start().await;
    let ws = workspace(&server).await;
    Mock::given(method("GET"))
        .and(path("/dsk-data"))
        .respond_with(listing(&["b.txt", "a.txt"], &[]))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/dsk-logs"))
        .and(query_param("prefix", "app/logs/"))
        .respond_with(listing(&["app/logs/x.log"], &[]))
        .expect(1)
        .mount(&server)
        .await;
    let r = ws
        .list_objects(
            None,
            ListObjectsOptions {
                recursive: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        r.objects.iter().map(|o| o.key.as_str()).collect::<Vec<_>>(),
        ["data/a.txt", "data/b.txt", "logs/x.log"]
    );
    assert_eq!(r.key_count, 3);
    assert!(!r.is_truncated);
}

#[tokio::test]
async fn prefix_listing_targets_one_disk_and_maps_prefixes_back() {
    let server = MockServer::start().await;
    let ws = workspace(&server).await;
    Mock::given(method("GET"))
        .and(path("/dsk-logs"))
        .and(query_param("prefix", "app/logs/2026/"))
        .and(query_param("delimiter", "/"))
        .respond_with(listing(&["app/logs/2026/a.log"], &["app/logs/2026/jan/"]))
        .expect(1)
        .mount(&server)
        .await;
    let r = ws
        .list_objects(Some("logs/2026"), ListObjectsOptions::default())
        .await
        .unwrap();
    assert_eq!(r.objects[0].key, "logs/2026/a.log");
    assert_eq!(r.common_prefixes, ["logs/2026/jan/"]);
}

#[tokio::test]
async fn a_failing_disk_marks_the_listing_truncated_instead_of_failing() {
    let server = MockServer::start().await;
    let ws = workspace(&server).await;
    Mock::given(method("GET"))
        .and(path("/dsk-data"))
        .respond_with(listing(&["a.txt"], &[]))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/dsk-logs"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    let r = ws
        .list_objects(
            None,
            ListObjectsOptions {
                recursive: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(r.objects.len(), 1);
    assert!(r.is_truncated);
}

fn grep_result(matches: serde_json::Value, reason: &str) -> ResponseTemplate {
    ok(
        json!({"matches": matches, "stoppedReason": reason, "filesScanned": 2, "containersDispatched": 1,
              "computeSecondsUsed": 1.5, "durationMs": 100, "listingMs": 10, "grepMs": 80}),
    )
}

#[tokio::test]
async fn grep_fans_out_merges_and_reports_the_most_serious_reason() {
    let server = MockServer::start().await;
    let ws = workspace(&server).await;
    Mock::given(method("POST"))
        .and(path("/api/disks/dsk-data/grep"))
        .and(body_json(json!({"directory": "", "pattern": "ERR", "recursive": false, "maxDurationSeconds": 30, "concurrency": 50, "maxResults": 1000})))
        .respond_with(grep_result(json!([{"file": "a.txt", "line": 1, "text": "ERR a"}]), "completed"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/disks/dsk-logs/grep"))
        .and(body_json(json!({"directory": "app/logs", "pattern": "ERR", "recursive": false, "maxDurationSeconds": 30, "concurrency": 50, "maxResults": 1000})))
        .respond_with(grep_result(json!([{"file": "app/logs/x.log", "line": 7, "text": "ERR x"}]), "deadline"))
        .expect(1)
        .mount(&server)
        .await;
    let r = ws.grep(GrepOptions::new("", "ERR")).await.unwrap();
    assert_eq!(
        r.matches
            .iter()
            .map(|m| (m.file.as_str(), m.line))
            .collect::<Vec<_>>(),
        [("data/a.txt", 1), ("logs/x.log", 7)]
    );
    assert_eq!(r.stopped_reason, GrepStoppedReason::Deadline);
    assert_eq!((r.files_scanned, r.containers_dispatched), (4, 2));
    assert_eq!(r.compute_seconds_used, 3.0);
    assert_eq!(r.duration_ms, 100.0, "durations take the max across disks");
}

#[tokio::test]
async fn grep_failure_on_one_disk_becomes_list_failed_and_the_cap_is_enforced() {
    let server = MockServer::start().await;
    let ws = workspace(&server).await;
    Mock::given(method("POST"))
        .and(path("/api/disks/dsk-data/grep"))
        .respond_with(grep_result(
            json!([
                {"file": "a", "line": 1, "text": "x"}, {"file": "b", "line": 2, "text": "y"}
            ]),
            "completed",
        ))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/disks/dsk-logs/grep"))
        .respond_with(err(400, "bad regex", None))
        .mount(&server)
        .await;
    let r = ws
        .grep(GrepOptions::new("", "(").max_results(1))
        .await
        .unwrap();
    assert_eq!(
        r.stopped_reason,
        GrepStoppedReason::ListFailed,
        "failure outranks truncation"
    );
    assert_eq!(r.matches.len(), 1, "capped to max_results");

    // Cap exceeded by merging while every disk completed => max_results.
    let server = MockServer::start().await;
    let ws = workspace(&server).await;
    for (id, file) in [("dsk-data", "a"), ("dsk-logs", "app/logs/b")] {
        Mock::given(method("POST"))
            .and(path(format!("/api/disks/{id}/grep")))
            .respond_with(grep_result(
                json!([{"file": file, "line": 1, "text": "t"}]),
                "completed",
            ))
            .mount(&server)
            .await;
    }
    let r = ws
        .grep(GrepOptions::new("/", "t").max_results(1))
        .await
        .unwrap();
    assert_eq!(r.stopped_reason, GrepStoppedReason::MaxResults);
    assert_eq!(r.matches.len(), 1);
}

#[tokio::test]
async fn grep_with_a_directory_targets_one_disk() {
    let server = MockServer::start().await;
    let ws = workspace(&server).await;
    Mock::given(method("POST"))
        .and(path("/api/disks/dsk-logs/grep"))
        .and(body_json(json!({"directory": "app/logs/2026", "pattern": "p", "recursive": true, "maxDurationSeconds": 30, "concurrency": 50, "maxResults": 1000})))
        .respond_with(grep_result(json!([]), "completed"))
        .expect(1)
        .mount(&server)
        .await;
    let r = ws
        .grep(GrepOptions::new("logs/2026", "p").recursive(true))
        .await
        .unwrap();
    assert_eq!(r.stopped_reason, GrepStoppedReason::Completed);
}

#[tokio::test]
async fn exec_mounts_every_disk_with_its_options() {
    let server = MockServer::start().await;
    let ws = workspace(&server).await;
    Mock::given(method("POST"))
        .and(path("/api/exec"))
        .and(body_json(json!({
            "command": "ls /mnt/archil",
            "disks": {
                "data": "dsk-data",
                "logs": {"disk": "dsk-logs", "subdirectory": "/app/logs/", "readOnly": true, "conditional": false}
            }
        })))
        .respond_with(ok(json!({"exitCode": 0, "stdout": "data logs\n", "stderr": "", "timing": {"totalMs": 1, "queueMs": 0, "executeMs": 1}})))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        ws.exec("ls /mnt/archil").await.unwrap().stdout,
        "data logs\n"
    );
}

#[tokio::test]
async fn mount_management_validates_names_and_keeps_one_disk() {
    let server = MockServer::start().await;
    let (data, logs) = disks(&server).await;
    let c = client(&server);
    assert!(c.workspace(Vec::<(String, WorkspaceMount)>::new()).is_err());
    for bad in ["", "/", "a/b", ".", ".."] {
        let e = c.workspace([(bad, data.clone())]).unwrap_err();
        assert!(matches!(e, Error::InvalidArgument(_)), "{bad:?}");
    }
    let mut ws = c.workspace([("one", data.clone())]).unwrap();
    ws.add_disk("two", logs.clone())
        .unwrap()
        .add_disk("one", logs.clone())
        .unwrap();
    assert_eq!(
        ws.disk_names(),
        ["one", "two"],
        "re-adding replaces in place"
    );
    assert!(!ws.remove_disk("missing").unwrap());
    assert!(ws.remove_disk("/two/").unwrap());
    let e = ws.remove_disk("one").unwrap_err();
    assert!(e.to_string().contains("cannot remove the last disk"), "{e}");
}

async fn read_both(fs: &impl FileSystem, key: &str) -> Bytes {
    fs.get_object(key).await.unwrap()
}

#[tokio::test]
async fn disk_and_workspace_share_the_filesystem_trait() {
    let server = MockServer::start().await;
    let ws = workspace(&server).await;
    let (data, _) = disks(&server).await;
    Mock::given(method("GET"))
        .and(path("/dsk-data/f"))
        .respond_with(ResponseTemplate::new(200).set_body_string("same"))
        .mount(&server)
        .await;
    assert_eq!(read_both(&data, "f").await, "same");
    assert_eq!(read_both(&ws, "data/f").await, "same");
}
