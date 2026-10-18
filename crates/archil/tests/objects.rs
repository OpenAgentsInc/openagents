mod common;

use std::sync::atomic::Ordering;

use archil::{
    AppendObjectOptions, DeleteObjectsOptions, Disk, Error, ListMultipartUploadsOptions,
    ListObjectsOptions, ListPartsOptions, PosixCreateAttrs, PutObjectOptions, UploadPart,
    effective_upload_part_size,
};
use common::*;
use wiremock::matchers::{
    body_string, body_string_contains, header, method, path, query_param, query_param_is_missing,
};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn disk(server: &MockServer) -> Disk {
    Mock::given(method("GET"))
        .and(path("/api/disks/d1"))
        .respond_with(ok(disk_json("d1", "n")))
        .mount(server)
        .await;
    client(server).disks().get("d1").await.unwrap()
}

fn xml(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(body.to_string(), "application/xml")
}

const NO_SUCH_KEY: &str = r#"<?xml version="1.0"?><Error><Code>NoSuchKey</Code><Message>The specified key does not exist.</Message><RequestId>req-42</RequestId></Error>"#;

#[tokio::test]
async fn get_object_returns_bytes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/d1/reports/q1.bin"))
        .and(header("authorization", "key-test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![0u8, 1, 2, 255]))
        .mount(&server)
        .await;
    assert_eq!(
        &disk(&server)
            .await
            .get_object("reports/q1.bin")
            .await
            .unwrap()[..],
        &[0, 1, 2, 255]
    );
    // A leading slash is ignored.
    assert_eq!(
        disk(&server)
            .await
            .get_object("/reports/q1.bin")
            .await
            .unwrap()
            .len(),
        4
    );
}

#[tokio::test]
async fn s3_errors_parse_the_xml_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/d1/nope"))
        .respond_with(ResponseTemplate::new(404).set_body_raw(NO_SUCH_KEY, "application/xml"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/d1/html"))
        .respond_with(ResponseTemplate::new(403).set_body_string("<html>"))
        .mount(&server)
        .await;
    let d = disk(&server).await;
    let e = d.get_object("nope").await.unwrap_err();
    let Error::S3(ref s3) = e else {
        panic!("{e:?}")
    };
    assert_eq!(s3.status, 404);
    assert_eq!(s3.code.as_deref(), Some("NoSuchKey"));
    assert_eq!(s3.request_id.as_deref(), Some("req-42"));
    assert!(s3.raw.contains("<Code>NoSuchKey</Code>"));
    assert_eq!(
        e.to_string(),
        "S3 GetObject failed: 404 NoSuchKey — The specified key does not exist."
    );
    assert_eq!((e.status(), e.code()), (Some(404), Some("NoSuchKey")));

    // A non-XML error body still yields a typed error, falling back to the status text.
    let e = d.get_object("html").await.unwrap_err();
    assert_eq!(e.to_string(), "S3 GetObject failed: 403 — Forbidden");
}

#[tokio::test]
async fn keys_are_percent_encoded_per_segment() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/d1/dir/a%20b%3F%23%25.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        &disk(&server)
            .await
            .get_object("dir/a b?#%.txt")
            .await
            .unwrap()[..],
        b"ok"
    );
}

#[tokio::test]
async fn head_object_metadata_missing_and_exists() {
    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .and(path("/d1/there"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("etag", "\"abc\"")
                .insert_header("content-type", "text/plain")
                .insert_header("last-modified", "Wed, 21 Oct 2015 07:28:00 GMT")
                .set_body_bytes(vec![0u8; 7]),
        )
        .mount(&server)
        .await;
    Mock::given(method("HEAD"))
        .and(path("/d1/gone"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("HEAD"))
        .and(path("/d1/denied"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    let d = disk(&server).await;
    let m = d.head_object("there").await.unwrap().unwrap();
    assert_eq!(m.size, 7);
    assert_eq!(m.etag.as_deref(), Some("\"abc\""));
    assert_eq!(m.content_type.as_deref(), Some("text/plain"));
    assert_eq!(
        m.last_modified.unwrap().to_rfc3339(),
        "2015-10-21T07:28:00+00:00"
    );
    assert!(d.head_object("gone").await.unwrap().is_none());
    assert!(d.object_exists("there").await.unwrap());
    assert!(!d.object_exists("gone").await.unwrap());
    assert_eq!(
        d.head_object("denied").await.unwrap_err().status(),
        Some(403)
    );
}

#[tokio::test]
async fn put_object_sends_content_type_and_posix_headers() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/d1/a/b.json"))
        .and(header("content-type", "application/json"))
        .and(header("x-archil-mode", "640"))
        .and(header("x-archil-uid", "1000"))
        .and(header("x-archil-gid", "1001"))
        .and(body_string("{}"))
        .respond_with(ResponseTemplate::new(200).insert_header("etag", "\"e1\""))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/d1/plain"))
        .and(header("content-type", "application/octet-stream"))
        .respond_with(ResponseTemplate::new(200).insert_header("etag", "\"e2\""))
        .expect(1)
        .mount(&server)
        .await;
    let d = disk(&server).await;
    let r = d
        .put_object_with(
            "a/b.json",
            "{}",
            PutObjectOptions {
                content_type: Some("application/json".into()),
                attrs: PosixCreateAttrs {
                    mode: Some(0o640),
                    uid: Some(1000),
                    gid: Some(1001),
                },
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(r.etag.as_deref(), Some("\"e1\""));
    assert_eq!(
        d.put_object("plain", "x").await.unwrap().etag.as_deref(),
        Some("\"e2\"")
    );
    // A content-type string converts into options.
    Mock::given(method("PUT"))
        .and(path("/d1/ct"))
        .and(header("content-type", "text/csv"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    d.put_object_with("ct", "a,b", "text/csv").await.unwrap();
}

#[tokio::test]
async fn s3_requests_retry_transient_failures_but_append_and_complete_do_not() {
    let server = MockServer::start().await;
    let get = Seq::new(vec![
        ResponseTemplate::new(503),
        ResponseTemplate::new(429),
        ResponseTemplate::new(200).set_body_string("fine"),
    ]);
    let get_hits = get.hits.clone();
    Mock::given(method("GET"))
        .and(path("/d1/flaky"))
        .respond_with(get)
        .mount(&server)
        .await;
    let put = Seq::new(vec![ResponseTemplate::new(500), ResponseTemplate::new(200)]);
    let put_hits = put.hits.clone();
    Mock::given(method("PUT"))
        .and(path("/d1/flaky-put"))
        .respond_with(put)
        .mount(&server)
        .await;
    let app = Seq::new(vec![ResponseTemplate::new(503), ResponseTemplate::new(200)]);
    let app_hits = app.hits.clone();
    Mock::given(method("PUT"))
        .and(path("/d1/log"))
        .and(query_param("append", "true"))
        .respond_with(app)
        .mount(&server)
        .await;
    let comp = Seq::new(vec![ResponseTemplate::new(503), ResponseTemplate::new(200)]);
    let comp_hits = comp.hits.clone();
    Mock::given(method("POST"))
        .and(path("/d1/big"))
        .and(query_param("uploadId", "u1"))
        .respond_with(comp)
        .mount(&server)
        .await;

    let d = disk(&server).await;
    assert_eq!(&d.get_object("flaky").await.unwrap()[..], b"fine");
    assert_eq!(get_hits.load(Ordering::SeqCst), 3);
    d.put_object("flaky-put", "x").await.unwrap();
    assert_eq!(put_hits.load(Ordering::SeqCst), 2);

    // Append is not idempotent: a retry could duplicate the bytes.
    assert_eq!(
        d.append_object("log", "line\n").await.unwrap_err().status(),
        Some(503)
    );
    assert_eq!(app_hits.load(Ordering::SeqCst), 1);
    // Neither is completing a multipart upload.
    let e = d
        .multipart()
        .complete(
            "big",
            "u1",
            &[UploadPart {
                part_number: 1,
                etag: "\"x\"".into(),
            }],
        )
        .await
        .unwrap_err();
    assert_eq!(e.status(), Some(503));
    assert_eq!(comp_hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn append_object_uses_append_query_and_headers() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/d1/logs/app.log"))
        .and(query_param("append", "true"))
        .and(header("content-type", "text/plain"))
        .and(header("x-archil-mode", "600"))
        .and(body_string("hello\n"))
        .respond_with(ResponseTemplate::new(200).insert_header("etag", "\"after\""))
        .expect(1)
        .mount(&server)
        .await;
    let r = disk(&server)
        .await
        .append_object_with(
            "logs/app.log",
            "hello\n",
            AppendObjectOptions {
                content_type: Some("text/plain".into()),
                attrs: PosixCreateAttrs {
                    mode: Some(0o600),
                    ..Default::default()
                },
            },
        )
        .await
        .unwrap();
    assert_eq!(r.etag.as_deref(), Some("\"after\""));
}

#[tokio::test]
async fn delete_object_is_idempotent() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/d1/a"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/d1/absent"))
        .respond_with(ResponseTemplate::new(404).set_body_raw(NO_SUCH_KEY, "application/xml"))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/d1/locked"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    let d = disk(&server).await;
    d.delete_object("a").await.unwrap();
    d.delete_object("absent").await.unwrap();
    assert_eq!(
        d.delete_object("locked").await.unwrap_err().status(),
        Some(403)
    );
}

fn page(keys: &[&str], prefixes: &[&str], token: Option<&str>) -> ResponseTemplate {
    let contents: String = keys
        .iter()
        .map(|k| format!("<Contents><Key>{k}</Key><Size>{}</Size><ETag>&quot;e&quot;</ETag><LastModified>2026-01-02T03:04:05.000Z</LastModified></Contents>", k.len()))
        .collect();
    let cps: String = prefixes
        .iter()
        .map(|p| format!("<CommonPrefixes><Prefix>{p}</Prefix></CommonPrefixes>"))
        .collect();
    let next = token
        .map(|t| format!("<NextContinuationToken>{t}</NextContinuationToken>"))
        .unwrap_or_default();
    xml(&format!(
        "<?xml version=\"1.0\"?><ListBucketResult><Prefix>p/</Prefix><KeyCount>{}</KeyCount><IsTruncated>{}</IsTruncated>{next}{contents}{cps}</ListBucketResult>",
        keys.len(),
        token.is_some()
    ))
}

#[tokio::test]
async fn list_objects_auto_paginates_and_defaults_to_one_level() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/d1"))
        .and(query_param("list-type", "2"))
        .and(query_param("prefix", "p/"))
        .and(query_param("delimiter", "/"))
        .and(query_param_is_missing("continuation-token"))
        .respond_with(page(&["p/a", "p/b"], &["p/sub/"], Some("t1")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/d1"))
        .and(query_param("continuation-token", "t1"))
        .respond_with(page(&["p/c"], &["p/sub/", "p/other/"], None))
        .expect(1)
        .mount(&server)
        .await;
    let r = disk(&server)
        .await
        .list_objects(Some("p/"), ListObjectsOptions::default())
        .await
        .unwrap();
    assert_eq!(
        r.objects.iter().map(|o| o.key.as_str()).collect::<Vec<_>>(),
        ["p/a", "p/b", "p/c"]
    );
    assert_eq!(
        r.common_prefixes,
        ["p/sub/", "p/other/"],
        "duplicate prefixes across pages are merged"
    );
    assert_eq!(r.key_count, 3);
    assert!(!r.is_truncated);
    assert_eq!(r.prefix.as_deref(), Some("p/"));
    assert_eq!(r.objects[0].etag.as_deref(), Some("\"e\""));
    assert_eq!(
        r.objects[0].last_modified.unwrap().to_rfc3339(),
        "2026-01-02T03:04:05+00:00"
    );
}

#[tokio::test]
async fn list_objects_recursive_limit_single_page_and_start_after() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/d1"))
        .and(query_param_is_missing("delimiter"))
        .and(query_param_is_missing("prefix"))
        .and(query_param_is_missing("continuation-token"))
        .respond_with(page(&["a", "b", "c"], &[], Some("t1")))
        .mount(&server)
        .await;
    let d = disk(&server).await;
    // limit cuts the listing short and flags it truncated; no second page is fetched.
    let r = d
        .list_objects(
            None,
            ListObjectsOptions {
                recursive: true,
                limit: Some(2),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(r.objects.len(), 2);
    assert!(r.is_truncated);
    // single_page returns the page verbatim with its token.
    let r = d
        .list_objects(
            None,
            ListObjectsOptions {
                recursive: true,
                single_page: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(r.is_truncated);
    assert_eq!(r.next_continuation_token.as_deref(), Some("t1"));
    assert_eq!(r.objects.len(), 3);

    Mock::given(method("GET"))
        .and(path("/d1"))
        .and(query_param("start-after", "m"))
        .and(query_param("continuation-token", "resume"))
        .respond_with(page(&["z"], &[], None))
        .expect(1)
        .mount(&server)
        .await;
    let r = d
        .list_objects(
            None,
            ListObjectsOptions {
                recursive: true,
                single_page: true,
                start_after: Some("m".into()),
                continuation_token: Some("resume".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(r.objects[0].key, "z");
}

#[tokio::test]
async fn list_objects_pages_streams_and_stops_on_repeated_token() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/d1"))
        .and(query_param_is_missing("continuation-token"))
        .respond_with(page(&["a"], &[], Some("t1")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/d1"))
        .and(query_param("continuation-token", "t1"))
        .respond_with(page(&["b"], &[], Some("t1")))
        .mount(&server)
        .await;
    let mut pages = disk(&server)
        .await
        .list_objects_pages(None, ListObjectsOptions::default());
    let mut keys = vec![];
    while let Some(p) = pages.next_page().await.unwrap() {
        keys.extend(p.objects.into_iter().map(|o| o.key));
    }
    // The server keeps handing back "t1": we must not loop forever.
    assert_eq!(keys, ["a", "b"]);
    assert!(pages.next_page().await.unwrap().is_none());
}

#[tokio::test]
async fn list_objects_decodes_xml_entities_in_keys() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/d1"))
        .respond_with(page(&["a&amp;b&lt;c&gt;.txt", "caf&#233;"], &[], None))
        .mount(&server)
        .await;
    let r = disk(&server)
        .await
        .list_objects(None, ListObjectsOptions::default())
        .await
        .unwrap();
    assert_eq!(r.objects[0].key, "a&b<c>.txt");
    assert_eq!(r.objects[1].key, "café");
}

#[tokio::test]
async fn delete_objects_batches_of_1000_and_reports_per_key_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/d1"))
        .and(query_param("delete", ""))
        .and(header("content-type", "application/xml"))
        .and(body_string_contains("<Key>k0</Key>"))
        .respond_with(xml("<DeleteResult><Deleted><Key>k0</Key></Deleted><Deleted><Key>k1</Key></Deleted><Error><Key>k2</Key><Code>AccessDenied</Code><Message>no</Message></Error></DeleteResult>"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/d1"))
        .and(query_param("delete", ""))
        .and(body_string_contains("<Key>k1000</Key>"))
        .respond_with(xml(
            "<DeleteResult><Deleted><Key>k1000</Key></Deleted></DeleteResult>",
        ))
        .expect(1)
        .mount(&server)
        .await;
    let keys: Vec<String> = (0..1001).map(|i| format!("k{i}")).collect();
    let r = disk(&server)
        .await
        .delete_objects(&keys, DeleteObjectsOptions::default())
        .await
        .unwrap();
    assert_eq!(r.deleted, ["k0", "k1", "k1000"]);
    assert_eq!(r.errors.len(), 1);
    assert_eq!(
        (
            r.errors[0].key.as_str(),
            r.errors[0].code.as_deref(),
            r.errors[0].message.as_deref()
        ),
        ("k2", Some("AccessDenied"), Some("no"))
    );
}

#[tokio::test]
async fn delete_objects_escapes_keys_and_supports_quiet() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/d1"))
        .and(body_string(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Delete><Object><Key>a&amp;b&lt;c&gt;</Key></Object><Quiet>true</Quiet></Delete>",
        ))
        .respond_with(xml("<DeleteResult></DeleteResult>"))
        .expect(1)
        .mount(&server)
        .await;
    let r = disk(&server)
        .await
        .delete_objects(
            &["a&b<c>".to_string()],
            DeleteObjectsOptions { quiet: true },
        )
        .await
        .unwrap();
    assert!(r.deleted.is_empty() && r.errors.is_empty());
}

#[tokio::test]
async fn manual_multipart_lifecycle() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/d1/big.bin"))
        .and(query_param("uploads", ""))
        .and(header("content-type", "video/mp4"))
        .and(header("x-archil-uid", "7"))
        .respond_with(xml("<InitiateMultipartUploadResult><Bucket>d1</Bucket><Key>big.bin</Key><UploadId>u-1</UploadId></InitiateMultipartUploadResult>"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/d1/big.bin"))
        .and(query_param("uploadId", "u-1"))
        .and(query_param("partNumber", "2"))
        .and(body_string("part-two"))
        .respond_with(ResponseTemplate::new(200).insert_header("etag", "\"p2\""))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/d1/big.bin"))
        .and(query_param("uploadId", "u-1"))
        .and(body_string(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><CompleteMultipartUpload><Part><PartNumber>1</PartNumber><ETag>&quot;p1&quot;</ETag></Part><Part><PartNumber>2</PartNumber><ETag>\"p2\"</ETag></Part></CompleteMultipartUpload>"
                .replace("&quot;", "\""),
        ))
        .respond_with(xml("<CompleteMultipartUploadResult><Location>/d1/big.bin</Location><Bucket>d1</Bucket><Key>big.bin</Key><ETag>\"final-2\"</ETag></CompleteMultipartUploadResult>"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/d1/big.bin"))
        .and(query_param("uploadId", "u-1"))
        .and(query_param("max-parts", "10"))
        .and(query_param("part-number-marker", "0"))
        .respond_with(xml("<ListPartsResult><Bucket>d1</Bucket><Key>big.bin</Key><UploadId>u-1</UploadId><PartNumberMarker>0</PartNumberMarker><NextPartNumberMarker>2</NextPartNumberMarker><MaxParts>10</MaxParts><IsTruncated>true</IsTruncated><Part><PartNumber>1</PartNumber><ETag>\"p1\"</ETag><Size>5242880</Size><LastModified>2026-01-01T00:00:00Z</LastModified></Part><Part><PartNumber>2</PartNumber><ETag>\"p2\"</ETag><Size>8</Size></Part></ListPartsResult>"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/d1"))
        .and(query_param("uploads", ""))
        .and(query_param("prefix", "b"))
        .and(query_param("max-uploads", "5"))
        .respond_with(xml("<ListMultipartUploadsResult><Bucket>d1</Bucket><Prefix>b</Prefix><MaxUploads>5</MaxUploads><IsTruncated>true</IsTruncated><NextKeyMarker>big.bin</NextKeyMarker><NextUploadIdMarker>u-1</NextUploadIdMarker><Upload><Key>big.bin</Key><UploadId>u-1</UploadId><Initiated>2026-01-01T00:00:00Z</Initiated></Upload><CommonPrefixes><Prefix>b/</Prefix></CommonPrefixes></ListMultipartUploadsResult>"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/d1/big.bin"))
        .and(query_param("uploadId", "u-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/d1/big.bin"))
        .and(query_param("uploadId", "gone"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;

    let mp = disk(&server).await.multipart();
    let up = mp
        .create_with(
            "big.bin",
            Some("video/mp4"),
            PosixCreateAttrs {
                uid: Some(7),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        (up.upload_id.as_str(), up.key.as_str(), up.bucket.as_str()),
        ("u-1", "big.bin", "d1")
    );
    let p2 = mp
        .upload_part("big.bin", "u-1", 2, "part-two")
        .await
        .unwrap();
    assert_eq!((p2.part_number, p2.etag.as_str()), (2, "\"p2\""));
    // Parts are sorted before submission.
    let done = mp
        .complete(
            "big.bin",
            "u-1",
            &[
                p2,
                UploadPart {
                    part_number: 1,
                    etag: "\"p1\"".into(),
                },
            ],
        )
        .await
        .unwrap();
    assert_eq!(done.etag.as_deref(), Some("\"final-2\""));
    assert_eq!(done.location.as_deref(), Some("/d1/big.bin"));

    let parts = mp
        .list_parts(
            "big.bin",
            "u-1",
            ListPartsOptions {
                max_parts: Some(10),
                part_number_marker: Some(0),
            },
        )
        .await
        .unwrap();
    assert!(parts.is_truncated);
    assert_eq!(
        (parts.next_part_number_marker, parts.max_parts),
        (Some(2), 10)
    );
    assert_eq!(parts.parts.len(), 2);
    assert_eq!(parts.parts[0].size, 5_242_880);
    assert!(parts.parts[0].last_modified.is_some() && parts.parts[1].last_modified.is_none());

    let ups = mp
        .list_uploads(ListMultipartUploadsOptions {
            prefix: Some("b".into()),
            max_uploads: Some(5),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(ups.uploads[0].upload_id, "u-1");
    assert_eq!(ups.common_prefixes, ["b/"]);
    assert_eq!(
        (
            ups.next_key_marker.as_deref(),
            ups.next_upload_id_marker.as_deref()
        ),
        (Some("big.bin"), Some("u-1"))
    );

    mp.abort("big.bin", "u-1").await.unwrap();
    mp.abort("big.bin", "gone").await.unwrap(); // already gone is fine
}

#[tokio::test]
async fn create_multipart_without_upload_id_is_an_s3_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/d1/k"))
        .respond_with(xml("<InitiateMultipartUploadResult/>"))
        .mount(&server)
        .await;
    let e = disk(&server)
        .await
        .multipart()
        .create("k")
        .await
        .unwrap_err();
    assert!(matches!(e, Error::S3(_)));
    assert!(e.to_string().contains("did not contain an UploadId"), "{e}");
}

#[tokio::test]
async fn put_object_goes_multipart_above_the_threshold() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/d1/big"))
        .and(query_param("uploads", ""))
        .respond_with(xml("<InitiateMultipartUploadResult><UploadId>u9</UploadId></InitiateMultipartUploadResult>"))
        .expect(1)
        .mount(&server)
        .await;
    for n in 1..=3 {
        Mock::given(method("PUT"))
            .and(path("/d1/big"))
            .and(query_param("uploadId", "u9"))
            .and(query_param("partNumber", n.to_string()))
            .respond_with(ResponseTemplate::new(200).insert_header("etag", format!("\"p{n}\"")))
            .expect(1)
            .mount(&server)
            .await;
    }
    Mock::given(method("POST"))
        .and(path("/d1/big"))
        .and(query_param("uploadId", "u9"))
        .and(body_string_contains("<Part><PartNumber>3</PartNumber><ETag>\"p3\"</ETag></Part>"))
        .respond_with(xml("<CompleteMultipartUploadResult><ETag>\"multi-3\"</ETag></CompleteMultipartUploadResult>"))
        .expect(1)
        .mount(&server)
        .await;
    // 11 MiB at the 5 MiB minimum part size => parts of 5, 5 and 1 MiB.
    let body = vec![7u8; 11 * 1024 * 1024];
    let r = disk(&server)
        .await
        .put_object_with(
            "big",
            body,
            PutObjectOptions {
                part_size: Some(1),
                concurrency: Some(2),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(r.etag.as_deref(), Some("\"multi-3\""));
}

#[tokio::test]
async fn small_bodies_stay_single_put_and_threshold_can_be_forced() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/d1/s"))
        .respond_with(ResponseTemplate::new(200))
        .expect(2)
        .mount(&server)
        .await;
    let d = disk(&server).await;
    d.put_object("s", vec![0u8; 1024]).await.unwrap();
    // u64::MAX forces a single PutObject however large the body.
    d.put_object_with(
        "s",
        vec![0u8; 6 * 1024 * 1024],
        PutObjectOptions {
            multipart_threshold: Some(u64::MAX),
            ..Default::default()
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn failed_part_aborts_the_multipart_upload() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/d1/big"))
        .and(query_param("uploads", ""))
        .respond_with(xml("<InitiateMultipartUploadResult><UploadId>u5</UploadId></InitiateMultipartUploadResult>"))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/d1/big"))
        .and(query_param("partNumber", "1"))
        .respond_with(ResponseTemplate::new(200).insert_header("etag", "\"p1\""))
        .mount(&server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/d1/big"))
        .and(query_param("partNumber", "2"))
        .respond_with(ResponseTemplate::new(400).set_body_raw(
            "<Error><Code>InvalidPart</Code><Message>bad</Message></Error>",
            "application/xml",
        ))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/d1/big"))
        .and(query_param("uploadId", "u5"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    let e = disk(&server)
        .await
        .put_object_with(
            "big",
            vec![0u8; 6 * 1024 * 1024],
            PutObjectOptions {
                part_size: Some(5 * 1024 * 1024),
                concurrency: Some(1),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert_eq!(e.code(), Some("InvalidPart"));
    // wiremock verifies the abort (`expect(1)`) when the server drops.
}

#[test]
fn part_size_grows_to_respect_the_10000_part_cap() {
    const MIB: u64 = 1024 * 1024;
    assert_eq!(effective_upload_part_size(100 * MIB, 16 * MIB), 16 * MIB);
    assert_eq!(
        effective_upload_part_size(10_000 * 16 * MIB, 16 * MIB),
        16 * MIB
    );
    let total = 10_000 * 16 * MIB + 1;
    let grown = effective_upload_part_size(total, 16 * MIB);
    assert_eq!(grown % MIB, 0);
    assert!(grown > 16 * MIB && total.div_ceil(grown) <= 10_000);
}
