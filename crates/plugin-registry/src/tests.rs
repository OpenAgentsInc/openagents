use super::*;
use catalog::{Catalog, Id, artifact, reference};
use nostr::contracts::{digest_bytes, jcs};
use nostr::domain::{Event, RelaySigner};
use publish::{Previous, Publication};
use serde_json::{Value, json};
use source::Source;
use std::path::Path;
use store::{Policy, Store};
const NOW: u64 = 1_790_000_000;
fn signer(n: u8) -> RelaySigner {
    RelaySigner::from_secret_hex(&format!("{n:02x}").repeat(32)).unwrap()
}
fn id(signer: &RelaySigner, slug: &str) -> Id {
    Id::parse(&format!("{}:{slug}", signer.pubkey())).unwrap()
}
fn catalog_for(signer: &RelaySigner, revision: u64, entries: Vec<Value>) -> Catalog {
    Catalog::parse(json!({"v":SCHEMA,"requires":[],"registry":id(signer,"catalog").as_str(),"revision":revision,"as_of":NOW,"valid_until":NOW+3600,"title":"Tools","description":"","entries":entries})).unwrap()
}
fn build(signer: &RelaySigner, revision: u64, entries: Vec<Value>) -> Publication {
    publish::build(
        signer,
        &catalog_for(signer, revision, entries),
        "MIT",
        NOW + revision,
        Previous::default(),
    )
    .unwrap()
}
fn release(p: &Publication) -> Event {
    let body = nostr::ext::parse_record(&p.listing).unwrap();
    p.events[body["release"]["id"].as_str().unwrap()].clone()
}
fn write_event(root: &Path, event: &Event) {
    std::fs::create_dir_all(root.join("events")).unwrap();
    std::fs::write(
        root.join(format!("events/{}.json", event.id)),
        serde_json::to_vec(event).unwrap(),
    )
    .unwrap();
}
fn write_artifact(root: &Path, bytes: &[u8]) {
    let hash = digest_bytes(bytes);
    let path = root.join(format!("artifacts/sha256/{}", &hash[7..]));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}
fn member(
    root: &Path,
    signer: &RelaySigner,
    slug: &str,
    dependencies: Vec<String>,
) -> (Event, Value) {
    let schema = br#"{"type":"object"}"#;
    write_artifact(root, schema);
    let manifest = json!({"v":"openagents.package.v1","requires":[],"package":id(signer,slug).as_str(),"version":"1.0.0","license":"MIT","provenance":{"source":"local","receipts":[],"unknowns":[]},"components":[{"slug":"settings","kind":"schema","definition":artifact(schema,"application/schema+json",None)}],"files":[{"path":"settings.json","digest":digest_bytes(schema),"size":schema.len(),"media_type":"application/schema+json"}],"dependencies":dependencies});
    let bytes = jcs(&manifest).unwrap();
    write_artifact(root, &bytes);
    let release=publish::sign(signer,3184,None,&json!({"v":1,"requires":[],"type":"release","package":id(signer,slug).as_str(),"version":"1.0.0","manifest":artifact(&bytes,"application/json",Some("openagents.package.v1"))}),NOW).unwrap();
    write_event(root, &release);
    let checkpoint =
        publish::checkpoint(signer, &id(signer, slug), 1, NOW, NOW + 3600, &[]).unwrap();
    publish::write_head(root, &checkpoint).unwrap();
    let entry = json!({"package":id(signer,slug).as_str(),"release":reference(&release),"manifest":artifact(&bytes,"application/json",Some("openagents.package.v1")),"title":"Tool\u{001b}[2J","summary":""});
    (release, entry)
}
fn store(root: &Path, registry: &Id, mirror: &Path) -> Store {
    let s = Store::open(root.join("state")).unwrap();
    s.enroll(
        registry.clone(),
        vec![Source::Directory {
            path: mirror.to_path_buf(),
        }],
        Policy::default(),
    )
    .unwrap();
    s
}
fn revoke(signer: &RelaySigner, package: &Id, event: &Event, at: u64) -> Event {
    publish::sign(signer,3185,None,&json!({"v":1,"requires":[],"type":"revocation","package":package.as_str(),"release":reference(event),"reason":"withdrawn","effective_at":at}),at).unwrap()
}
#[test]
fn intrinsic_catalog_contract_and_exact_package_binding() {
    let s = signer(1);
    let good = catalog_for(&s, 1, vec![]);
    for (field, value) in [
        ("extra", json!(true)),
        ("revision", json!(0)),
        ("requires", json!(["unknown"])),
        ("valid_until", json!(NOW)),
    ] {
        let mut body = good.body().clone();
        body[field] = value;
        assert!(Catalog::parse(body).is_err(), "{field}");
    }
    let temp = tempfile::tempdir().unwrap();
    let (_, entry) = member(temp.path(), &signer(2), "member", vec![]);
    let mut body = good.body().clone();
    body["entries"] = json!([entry.clone(), entry.clone()]);
    assert!(Catalog::parse(body).is_err());
    let mut bad = entry;
    bad["release"]["pubkey"] = json!(s.pubkey());
    let mut body = good.body().clone();
    body["entries"] = json!([bad]);
    assert!(Catalog::parse(body).is_err());
    let p = build(&s, 1, vec![]);
    let release = release(&p);
    let declaration = nostr::ext::parse_record(&release).unwrap();
    let bytes = &p.artifacts[declaration["manifest"]["digest"].as_str().unwrap()];
    assert!(catalog::from_package(&release, bytes, &p.artifacts, 1_000_000).is_ok());
    let mut changed = bytes.clone();
    changed.push(b' ');
    assert!(catalog::from_package(&release, &changed, &p.artifacts, 1_000_000).is_err());
    let mut manifest: Value = serde_json::from_slice(bytes).unwrap();
    manifest["dependencies"] = json!(["a".repeat(64)]);
    let bytes = jcs(&manifest).unwrap();
    let mut body = declaration;
    body["manifest"] = artifact(&bytes, "application/json", Some("openagents.package.v1"));
    let event = publish::sign(&s, 3184, None, &body, NOW).unwrap();
    assert!(catalog::package_shape(&event, &bytes).is_err());
}
#[tokio::test]
async fn browse_is_inert_selection_verifies_independent_publishers_and_closure() {
    let temp = tempfile::tempdir().unwrap();
    let mirror = temp.path().join("mirror");
    let curator = signer(1);
    let publisher = signer(2);
    let (dependency, _) = member(&mirror, &signer(3), "dep", vec![]);
    let (root, mut entry) = member(&mirror, &publisher, "tool", vec![dependency.id.clone()]);
    entry["configuration"] = artifact(br#"{"type":"object"}"#, "application/schema+json", None);
    let p = build(&curator, 1, vec![entry.clone()]);
    publish::mirror(&mirror, &p).unwrap();
    let registry = id(&curator, "catalog");
    let s = store(temp.path(), &registry, &mirror);
    s.refresh(&registry, NOW + 10).await.unwrap();
    let browse = s.browse(&registry, NOW + 10, false).unwrap();
    assert_eq!(browse["entries"][0], entry);
    assert!(
        !temp
            .path()
            .join(format!("state/events/{}.json", root.id))
            .exists(),
        "browsing fetched a member"
    );
    let selected = s
        .select(&registry, &id(&publisher, "tool"), NOW + 10)
        .await
        .unwrap();
    assert_eq!(selected.release.pubkey, publisher.pubkey());
    assert_eq!(selected.dependencies.len(), 2);
    assert_eq!(selected.release.id, root.id);
    assert!(selected.usage["requests"].as_u64().unwrap() > 0);
    assert_eq!(
        s.prior_selection(&registry, &id(&publisher, "tool"), NOW + 10)
            .unwrap()
            .release
            .id,
        root.id
    );
    assert!(!cli::display(&browse).contains('\u{001b}'));
    assert!(!temp.path().join("installed").exists());
    // A later catalog may withdraw a recommendation, but cannot revoke the pin.
    let p2 = build(&curator, 2, vec![]);
    publish::mirror(&mirror, &p2).unwrap();
    s.refresh(&registry, NOW + 10).await.unwrap();
    assert!(
        s.prior_selection(&registry, &id(&publisher, "tool"), NOW + 10)
            .is_err()
    );
    let mut policy = Policy::default();
    policy.allow_pinned_use = true;
    policy.require_current_membership = false;
    s.enroll(
        registry.clone(),
        vec![Source::Directory {
            path: mirror.clone(),
        }],
        policy,
    )
    .unwrap();
    s.remove(&registry).unwrap();
    assert_eq!(
        s.prior_selection(&registry, &id(&publisher, "tool"), NOW + 10)
            .unwrap()
            .release
            .id,
        root.id
    );
}
#[tokio::test]
async fn hidden_and_unavailable_new_heads_survive_source_changes_and_restart() {
    let temp = tempfile::tempdir().unwrap();
    let mirror = temp.path().join("mirror");
    let curator = signer(1);
    let registry = id(&curator, "catalog");
    let p = build(&curator, 1, vec![]);
    publish::mirror(&mirror, &p).unwrap();
    let s = store(temp.path(), &registry, &mirror);
    s.refresh(&registry, NOW + 10).await.unwrap();
    let hidden = publish::hidden(&curator, &registry, NOW + 20).unwrap();
    publish::write_head(&mirror, &hidden).unwrap();
    assert!(s.refresh(&registry, NOW + 21).await.is_err());
    assert_eq!(s.record(&registry).unwrap().observed.unwrap().id, hidden.id);
    assert_eq!(s.browse(&registry, NOW + 21, true).unwrap()["fresh"], false);
    // Old discovery aliases cannot recover current status after a withdrawal.
    std::fs::remove_file(mirror.join(format!("heads/30184/{}/catalog.json", curator.pubkey())))
        .unwrap();
    s.remove(&registry).unwrap();
    s.enroll(
        registry.clone(),
        vec![Source::Directory {
            path: mirror.clone(),
        }],
        Policy::default(),
    )
    .unwrap();
    let restarted = Store::open(temp.path().join("state")).unwrap();
    assert!(restarted.refresh(&registry, NOW + 21).await.is_err());
    assert_eq!(
        restarted
            .record(&registry)
            .unwrap()
            .snapshot
            .unwrap()
            .revision,
        1
    );
    publish::write_head(&mirror, &hidden).unwrap();
    let newer = build(&curator, 2, vec![]);
    write_event(&mirror, &release(&newer));
    publish::write_head(&mirror, &newer.listing).unwrap_err(); // timestamp is older than the hidden head
    let mut body = nostr::ext::parse_record(&newer.listing).unwrap();
    body["release"] = reference(&release(&newer));
    let new_head = publish::sign(&curator, 30184, Some("catalog"), &body, NOW + 30).unwrap();
    publish::write_head(&mirror, &new_head).unwrap();
    assert!(restarted.refresh(&registry, NOW + 31).await.is_err());
    assert_eq!(
        restarted.record(&registry).unwrap().observed.unwrap().id,
        new_head.id
    );
    assert_eq!(
        restarted
            .record(&registry)
            .unwrap()
            .snapshot
            .unwrap()
            .revision,
        1
    );
}
#[tokio::test]
async fn revisions_conflicts_tampering_and_corrupt_cache_refuse() {
    let temp = tempfile::tempdir().unwrap();
    let mirror = temp.path().join("mirror");
    let curator = signer(1);
    let registry = id(&curator, "catalog");
    let p = build(&curator, 2, vec![]);
    publish::mirror(&mirror, &p).unwrap();
    let s = store(temp.path(), &registry, &mirror);
    s.refresh(&registry, NOW + 10).await.unwrap();
    let mut altered = catalog_for(&curator, 2, vec![]).body().clone();
    altered["description"] = json!("different");
    let conflict = publish::build(
        &curator,
        &Catalog::parse(altered).unwrap(),
        "MIT",
        NOW + 20,
        Previous::default(),
    )
    .unwrap();
    assert!(publish::mirror(&mirror, &conflict).is_err());
    // A remote mirror can serve conflicting signed records; acceptance still refuses.
    for event in conflict.events.values() {
        write_event(&mirror, event);
    }
    for bytes in conflict.artifacts.values() {
        write_artifact(&mirror, bytes);
    }
    publish::write_head(&mirror, &conflict.listing).unwrap();
    assert!(
        s.refresh(&registry, NOW + 21)
            .await
            .unwrap_err()
            .contains("conflicting")
    );
    assert_eq!(
        s.record(&registry).unwrap().snapshot.unwrap().release.id,
        release(&p).id
    );
    let low = build(&curator, 1, vec![]);
    let mut body = nostr::ext::parse_record(&low.listing).unwrap();
    body["release"] = reference(&release(&low));
    let head = publish::sign(&curator, 30184, Some("catalog"), &body, NOW + 30).unwrap();
    for event in low.events.values() {
        write_event(&mirror, event);
    }
    for bytes in low.artifacts.values() {
        write_artifact(&mirror, bytes);
    }
    publish::write_head(&mirror, &head).unwrap();
    assert!(
        s.refresh(&registry, NOW + 31)
            .await
            .unwrap_err()
            .contains("rolled back")
    );
    let mut record = s.record(&registry).unwrap();
    record.snapshot.as_mut().unwrap().catalog_digest = format!("sha256:{}", "0".repeat(64));
    let state = temp.path().join(format!(
        "state/registry-{}.json",
        &digest_bytes(registry.as_str().as_bytes())[7..]
    ));
    std::fs::write(state, serde_json::to_vec(&record).unwrap()).unwrap();
    assert!(s.browse(&registry, NOW + 31, true).is_err());
}
#[tokio::test]
async fn revocations_are_independent_monotone_and_retained_on_partial_failure() {
    let temp = tempfile::tempdir().unwrap();
    let mirror = temp.path().join("mirror");
    let curator = signer(1);
    let publisher = signer(2);
    let package = id(&publisher, "tool");
    let (root, entry) = member(&mirror, &publisher, "tool", vec![]);
    let p = build(&curator, 1, vec![entry]);
    publish::mirror(&mirror, &p).unwrap();
    let registry = id(&curator, "catalog");
    let s = store(temp.path(), &registry, &mirror);
    s.select(&registry, &package, NOW + 10).await.unwrap();
    let r1 = revoke(&publisher, &package, &root, NOW + 11);
    let other = publish::sign(
        &publisher,
        3184,
        None,
        &nostr::ext::parse_record(&root).unwrap(),
        NOW + 1,
    )
    .unwrap();
    let r2 = revoke(&publisher, &package, &other, NOW + 12);
    let mut revocations = vec![r1, r2];
    revocations.sort_by(|a, b| a.id.cmp(&b.id));
    write_event(&mirror, &revocations[0]);
    let checkpoint =
        publish::checkpoint(&publisher, &package, 2, NOW + 15, NOW + 3600, &revocations).unwrap();
    publish::write_head(&mirror, &checkpoint).unwrap();
    assert!(s.select(&registry, &package, NOW + 20).await.is_err());
    let record = s.record(&registry).unwrap();
    assert!(
        record.revocations[package.as_str()]
            .events
            .contains_key(&revocations[0].id)
    );
    assert_ne!(
        record.revocations[package.as_str()].observed,
        record.revocations[package.as_str()].accepted
    );
    let omission = publish::checkpoint(&publisher, &package, 3, NOW + 21, NOW + 3600, &[]).unwrap(); // A malicious mirror may serve an omission despite publisher safeguards.
    let path = mirror.join(format!("heads/30185/{}/tool.json", publisher.pubkey()));
    std::fs::write(path, serde_json::to_vec(&omission).unwrap()).unwrap();
    assert!(
        s.select(&registry, &package, NOW + 22)
            .await
            .unwrap_err()
            .contains("omitted")
    );
    let complete =
        publish::checkpoint(&publisher, &package, 4, NOW + 23, NOW + 3600, &revocations).unwrap();
    write_event(&mirror, &revocations[1]);
    publish::write_head(&mirror, &complete).unwrap();
    assert!(
        s.select(&registry, &package, NOW + 24)
            .await
            .unwrap_err()
            .contains("revoked")
    );
    s.remove(&registry).unwrap();
    let mut policy = Policy::default();
    policy.allow_pinned_use = true;
    policy.require_current_membership = false;
    s.enroll(
        registry.clone(),
        vec![Source::Directory { path: mirror }],
        policy,
    )
    .unwrap();
    assert!(s.prior_selection(&registry, &package, NOW + 24).is_err());
}
#[tokio::test]
async fn mismatched_member_manifest_unavailable_closure_and_stale_checkpoint_refuse() {
    let temp = tempfile::tempdir().unwrap();
    let mirror = temp.path().join("mirror");
    let curator = signer(1);
    let publisher = signer(2);
    let package = id(&publisher, "tool");
    let (_root, mut entry) = member(&mirror, &publisher, "tool", vec!["a".repeat(64)]);
    let p = build(&curator, 1, vec![entry.clone()]);
    publish::mirror(&mirror, &p).unwrap();
    let registry = id(&curator, "catalog");
    let s = store(temp.path(), &registry, &mirror);
    s.refresh(&registry, NOW + 10).await.unwrap();
    assert!(s.select(&registry, &package, NOW + 10).await.is_err());
    entry["manifest"]["digest"] = json!(format!("sha256:{}", "b".repeat(64)));
    let p = build(&curator, 2, vec![entry]);
    publish::mirror(&mirror, &p).unwrap();
    assert!(
        s.select(&registry, &package, NOW + 10)
            .await
            .unwrap_err()
            .contains("catalog pin")
    );
    assert!(s.refresh(&registry, NOW + 7200).await.is_err());
}
#[tokio::test]
async fn frozen_snapshot_requires_explicit_policy_and_still_checks_publishers() {
    let temp = tempfile::tempdir().unwrap();
    let mirror = temp.path().join("mirror");
    let curator = signer(1);
    let publisher = signer(2);
    let (_, entry) = member(&mirror, &publisher, "tool", vec![]);
    let p = build(&curator, 1, vec![entry]);
    publish::mirror(&mirror, &p).unwrap();
    let registry = id(&curator, "catalog");
    let s = Store::open(temp.path().join("state")).unwrap();
    let sources = vec![Source::Directory { path: mirror }];
    s.enroll_with_pin(
        registry.clone(),
        sources.clone(),
        Policy::default(),
        Some(release(&p).id.clone()),
    )
    .unwrap();
    s.refresh(&registry, NOW + 10).await.unwrap();
    assert_eq!(s.browse(&registry, NOW + 10, true).unwrap()["frozen"], true);
    assert!(
        s.select(&registry, &id(&publisher, "tool"), NOW + 10)
            .await
            .is_err()
    );
    let mut policy = Policy::default();
    policy.allow_pinned_use = true;
    policy.require_current_membership = false;
    s.enroll_with_pin(registry.clone(), sources, policy, Some(release(&p).id))
        .unwrap();
    assert!(
        s.select(&registry, &id(&publisher, "tool"), NOW + 10)
            .await
            .is_ok()
    );
}
#[tokio::test]
async fn multiple_curators_preserve_origins_and_conflicting_pins() {
    let temp = tempfile::tempdir().unwrap();
    let mirror = temp.path().join("mirror");
    let publisher = signer(3);
    let (_, entry) = member(&mirror, &publisher, "tool", vec![]);
    let a = signer(1);
    let b = signer(2);
    let pa = build(&a, 1, vec![entry.clone()]);
    let pb = build(&b, 1, vec![entry.clone()]);
    publish::mirror(&mirror, &pa).unwrap();
    publish::mirror(&mirror, &pb).unwrap();
    let s = store(temp.path(), &id(&a, "catalog"), &mirror);
    s.enroll(
        id(&b, "catalog"),
        vec![Source::Directory {
            path: mirror.clone(),
        }],
        Policy::default(),
    )
    .unwrap();
    s.refresh(&id(&a, "catalog"), NOW + 10).await.unwrap();
    s.refresh(&id(&b, "catalog"), NOW + 10).await.unwrap();
    let compared = s.compare(NOW + 10).unwrap();
    assert_eq!(
        compared["packages"][0]["choices"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        compared["packages"][0]["choices"][0]["curators"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let mut other = entry;
    other["release"]["id"] = json!("f".repeat(64));
    let p2 = build(&b, 2, vec![other]);
    publish::mirror(&mirror, &p2).unwrap();
    s.refresh(&id(&b, "catalog"), NOW + 10).await.unwrap();
    assert_eq!(
        s.compare(NOW + 10).unwrap()["packages"][0]["conflict"],
        true
    );
}
#[test]
fn publication_retries_immutable_versions_and_paths() {
    let temp = tempfile::tempdir().unwrap();
    let s = signer(1);
    let c = catalog_for(&s, 1, vec![]);
    let p = publish::build(&s, &c, "MIT", NOW + 1, Previous::default()).unwrap();
    publish::mirror(temp.path(), &p).unwrap();
    publish::mirror(temp.path(), &p).unwrap();
    let retry = publish::build(
        &s,
        &c,
        "MIT",
        NOW + 2,
        Previous {
            release: Some(release(&p)),
            listing: Some(p.listing.clone()),
            checkpoint: Some(p.checkpoint.clone()),
            revocations: vec![],
        },
    )
    .unwrap();
    assert_eq!(release(&p).id, release(&retry).id);
    assert_eq!(p.listing.id, retry.listing.id);
    publish::mirror(temp.path(), &retry).unwrap();
    let mut body = c.body().clone();
    body["title"] = json!("Other");
    assert!(
        publish::build(
            &s,
            &Catalog::parse(body).unwrap(),
            "MIT",
            NOW + 3,
            Previous {
                release: Some(release(&p)),
                ..Default::default()
            }
        )
        .is_err()
    );
    for path in ["../a", "/etc/passwd", "a/../../b", "a\\b"] {
        assert!(source::relative(path).is_err());
    }
    let github = Source::Github {
        owner: "owner".into(),
        repository: "repo".into(),
        reference: "branch/tag".into(),
        directory: "catalogs".into(),
    };
    assert_eq!(
        github.url("registry.json").unwrap().as_str(),
        "https://api.github.com/repos/owner/repo/contents/catalogs/registry.json?ref=branch%2Ftag"
    );
    assert!(
        Source::Https {
            base: "http://example.com".into()
        }
        .validate()
        .is_err()
    );
    assert!(
        Source::Https {
            base: "https://a:b@example.com".into()
        }
        .validate()
        .is_err()
    );
}
#[test]
fn pointers_preserve_identity_and_ignore_unadmitted_hints() {
    let s = signer(1);
    let key: [u8; 32] = hex_bytes(s.pubkey()).try_into().unwrap();
    let address = nostr::nip19::encode_naddr(&nostr::nip19::Naddr {
        identifier: "catalog".into(),
        pubkey: key,
        kind: 30184,
        relays: vec!["wss://untrusted.example".into()],
    })
    .unwrap();
    assert_eq!(cli::registry_id(&address).unwrap(), id(&s, "catalog"));
    let mut bytes = vec![0, 32];
    bytes.extend_from_slice(&[5; 32]);
    bytes.extend_from_slice(&[3, 4]);
    bytes.extend_from_slice(&3184u32.to_be_bytes());
    let pointer = nostr::nip19::encode("nevent", &bytes).unwrap();
    assert_eq!(cli::release_id(&pointer).unwrap(), "05".repeat(32));
}
fn hex_bytes(text: &str) -> Vec<u8> {
    text.as_bytes()
        .chunks_exact(2)
        .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
        .collect()
}

/// A real HTTPS proxy fixture with a test-only CA. Requests still use the
/// production GitHub/HTTPS URL and header construction and byte reader.
async fn https_fixture(
    root: std::path::PathBuf,
) -> (
    reqwest::Client,
    std::sync::Arc<tokio::sync::Mutex<Vec<String>>>,
) {
    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let certificate = rcgen::generate_simple_self_signed(vec![
        "api.github.com".into(),
        "mirror.example".into(),
        "blobs.example".into(),
    ])
    .unwrap();
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(
        certificate.signing_key.serialize_der(),
    ));
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(vec![certificate.cert.der().clone()], key)
    .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .proxy(reqwest::Proxy::https(format!("http://{address}")).unwrap())
        .add_root_certificate(reqwest::Certificate::from_der(certificate.cert.der()).unwrap())
        .build()
        .unwrap();
    let requests = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let seen = requests.clone();
    async fn headers<S: tokio::io::AsyncRead + Unpin>(stream: &mut S) -> std::io::Result<String> {
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            if bytes.len() > 8192 {
                return Err(std::io::Error::other("header bound"));
            }
            bytes.push(stream.read_u8().await?);
        }
        String::from_utf8(bytes).map_err(std::io::Error::other)
    }
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let acceptor = acceptor.clone();
            let root = root.clone();
            let seen = seen.clone();
            tokio::spawn(async move {
                let request = headers(&mut stream).await.unwrap();
                assert!(request.starts_with("CONNECT "));
                stream
                    .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                    .await
                    .unwrap();
                let mut stream = acceptor.accept(stream).await.unwrap();
                let request = headers(&mut stream).await.unwrap();
                seen.lock().await.push(request.clone());
                let path = request
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .split('?')
                    .next()
                    .unwrap();
                let path = path
                    .strip_prefix("/repos/owner/repo/contents/mirror/")
                    .or_else(|| path.strip_prefix("/mirror/"))
                    .map(str::to_string)
                    .or_else(|| {
                        path.strip_prefix("/blobs/")
                            .map(|hash| format!("artifacts/sha256/{hash}"))
                    })
                    .unwrap();
                source::relative(&path).unwrap();
                let bytes = std::fs::read(root.join(path));
                let (status, bytes) = match bytes {
                    Ok(bytes) => ("200 OK", bytes),
                    Err(_) => ("404 Not Found", Vec::new()),
                };
                stream
                    .write_all(
                        format!(
                            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            bytes.len()
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
                stream.write_all(&bytes).await.unwrap();
                let _ = stream.shutdown().await;
            });
        }
    });
    (client, requests)
}
#[tokio::test]
async fn equivalent_signed_catalog_and_selection_over_directory_github_https_and_nostr() {
    let temp = tempfile::tempdir().unwrap();
    let mirror = temp.path().join("mirror");
    let curator = signer(1);
    let publisher = signer(2);
    let (member, entry) = member(&mirror, &publisher, "tool", vec![]);
    let publication = build(&curator, 1, vec![entry]);
    publish::mirror(&mirror, &publication).unwrap();
    let registry = id(&curator, "catalog");
    let (client, requests) = https_fixture(mirror.clone()).await;
    let (relay, hub) = pylon::fixture::relay().await.unwrap();
    for event in publication.events.values() {
        hub.lock().await.stored.push(event.clone());
    }
    hub.lock().await.stored.push(member.clone());
    let checkpoint_path = mirror.join(format!("heads/30185/{}/tool.json", publisher.pubkey()));
    hub.lock()
        .await
        .stored
        .push(serde_json::from_slice(&std::fs::read(checkpoint_path).unwrap()).unwrap());
    let sources = [
        Source::Directory { path: mirror },
        Source::Https {
            base: "https://mirror.example/mirror".into(),
        },
        Source::Github {
            owner: "owner".into(),
            repository: "repo".into(),
            reference: "main".into(),
            directory: "mirror".into(),
        },
        Source::Nostr {
            relay,
            blobs: vec!["https://blobs.example/blobs".into()],
        },
    ];
    let mut pins = Vec::new();
    for (index, source) in sources.into_iter().enumerate() {
        let mut s = Store::open(temp.path().join(format!("state-{index}"))).unwrap();
        s.client = Some(client.clone());
        s.enroll(registry.clone(), vec![source], Policy::default())
            .unwrap();
        s.refresh(&registry, NOW + 10).await.unwrap();
        let selected = s
            .select(&registry, &id(&publisher, "tool"), NOW + 10)
            .await
            .unwrap();
        assert_eq!(selected.release.id, member.id);
        pins.push((
            selected.catalog_release,
            selected.catalog_digest,
            selected.release,
        ));
    }
    assert!(pins.windows(2).all(|p| p[0] == p[1]));
    let requests = requests.lock().await;
    assert!(requests.iter().any(|r| {
        r.starts_with("GET /repos/")
            && r.to_ascii_lowercase()
                .contains("accept: application/vnd.github.raw+json")
            && r.contains("2026-03-10")
    }));
}
#[tokio::test]
async fn source_fallback_hash_checks_and_fetch_budgets_are_shared() {
    let temp = tempfile::tempdir().unwrap();
    let good = temp.path().join("good");
    let bad = temp.path().join("bad");
    let curator = signer(1);
    let p = build(&curator, 1, vec![]);
    publish::mirror(&good, &p).unwrap();
    publish::mirror(&bad, &p).unwrap();
    let registry = id(&curator, "catalog");
    let digest = digest_bytes(SCHEMA_BYTES);
    std::fs::write(
        bad.join(format!("artifacts/sha256/{}", &digest[7..])),
        b"corrupt",
    )
    .unwrap();
    let s = Store::open(temp.path().join("state")).unwrap();
    let sources = vec![
        Source::Directory { path: bad },
        Source::Directory { path: good },
    ];
    s.enroll(registry.clone(), sources.clone(), Policy::default())
        .unwrap();
    s.refresh(&registry, NOW + 10).await.unwrap();
    let mut policy = Policy::default();
    policy.limits.requests = 2;
    s.enroll(registry.clone(), sources, policy).unwrap();
    assert!(s.refresh(&registry, NOW + 10).await.is_err());
    assert_eq!(s.record(&registry).unwrap().snapshot.unwrap().revision, 1);
}
