//! Verse connection acceptance with isolated account, host, and world fixtures.

use super::*;
use ::workbench as contract;

const WORLD_ROUTE: &str = "ws://127.0.0.1:9450/chamber";

async fn get(fixture: &Fixture, cookies: &Cookies, route: &str) -> Answer {
    request(&fixture.site, Method::GET, route, cookies, None, None).await
}

fn host_key(native: &Resident) -> String {
    let document: Value = serde_json::from_slice(&std::fs::read(&native.config).unwrap()).unwrap();
    let access: Value = serde_json::from_slice(
        &std::fs::read(document["bindings"][0]["access_file"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    access["grant"]["host"].as_str().unwrap().into()
}

/// Write an admitted chamber directory and attach it to the binding.
fn world(fixture: &mut Fixture, native: &Resident, host: &str) -> PathBuf {
    let private = native.config.parent().unwrap();
    let directory = private.join("world");
    std::fs::create_dir_all(directory.join("chamber/assets")).unwrap();
    // Every containing directory is private, as for the other protected files.
    for path in ["", "chamber", "chamber/assets"] {
        std::fs::set_permissions(directory.join(path), std::fs::Permissions::from_mode(0o700))
            .unwrap();
    }
    private_file(&directory.join("chamber/pack.json"), b"{\"pack\":1}");
    private_file(&directory.join("chamber/scene.json"), b"{\"scene\":1}");
    private_file(&directory.join("chamber/assets/stone.bin"), b"texture");
    private_file(&directory.join("outside.bin"), b"not admitted");
    let chamber = json!({
        "websocket":WORLD_ROUTE,"host":host,"grant":"a".repeat(64),"epoch":1,
        "generation":7,"instance":3,"content":"b".repeat(64),
        "pack":"chamber/pack.json","scene":"chamber/scene.json","assets":"chamber/assets",
        "mips":false,"bindings":[]
    });
    private_file(
        &directory.join("chamber.json"),
        &serde_json::to_vec(&chamber).unwrap(),
    );
    attach(fixture, native, &directory);
    let renderer = private.join("everglade");
    std::fs::create_dir_all(&renderer).unwrap();
    // Route tests do not execute the renderer; the browser check uses the real build.
    std::fs::write(renderer.join(crate::pages::GLUE), "synthetic").unwrap();
    std::fs::write(renderer.join(crate::pages::WASM), b"synthetic").unwrap();
    fixture.config.everglade = Some(renderer);
    fixture.site = crate::router(fixture.config.clone());
    directory
}

fn attach(fixture: &mut Fixture, native: &Resident, directory: &std::path::Path) {
    let mut document: Value =
        serde_json::from_slice(&std::fs::read(&native.config).unwrap()).unwrap();
    document["bindings"][0]["world"] = json!(directory);
    private_file(&native.config, &serde_json::to_vec(&document).unwrap());
    fixture.config.cloud_hosts = Some(Arc::new(
        super::super::hosts::Hosts::load(&native.config).unwrap(),
    ));
    fixture.site = crate::router(fixture.config.clone());
}

fn no_private_work(answer: &Answer) {
    for value in [
        CANARY,
        "Original private request",
        "synthetic-credential",
        "resident-device.key",
        "sess_",
    ] {
        assert!(
            !answer.body.contains(value),
            "World surface disclosed {value}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connections_stay_visible_and_separate_without_any_host_or_world() {
    let fixture = fixture().await;
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let page = get(&fixture, &cookies, "/cloud/app/verse").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    private(&page);
    for label in ["World", "Computer", "Private work", "Unavailable"] {
        assert!(page.body.contains(label), "{label}");
    }
    assert!(page.body.contains("href=\"/grid\""));
    assert!(
        page.body
            .contains("supply no host, Studio, or private-work connection")
    );
    assert!(!page.body.contains(">Join<"));
    assert!(
        page.body
            .contains("<a aria-current=\"page\" href=\"/cloud/app/verse\">Verse</a>")
    );
    let signed_out = get(&fixture, &Cookies::default(), "/cloud/app/verse").await;
    assert_eq!(signed_out.status, StatusCode::SEE_OTHER);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_world_without_the_native_world_right_offers_no_join() {
    let mut fixture = fixture().await;
    let native = resident(&mut fixture).await;
    let host = host_key(&native);
    world(&mut fixture, &native, &host);
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let page = get(&fixture, &cookies, "/cloud/app/verse").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains("lacks the world right"));
    assert!(page.body.contains("Connected"));
    assert!(!page.body.contains(">Join<"));
    assert_eq!(
        get(&fixture, &cookies, "/cloud/app/hosts/resident/verse")
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    native.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn joining_a_world_uses_a_scoped_ticket_and_grants_no_private_work() {
    let mut fixture = fixture().await;
    let native = resident_with_controls(
        &mut fixture,
        coder_access::Rights::new([coder_access::Right::Observe, coder_access::Right::World])
            .unwrap(),
        false,
    )
    .await;
    let host = host_key(&native);
    let directory = world(&mut fixture, &native, &host);
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let page = get(&fixture, &cookies, "/cloud/app/verse").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    private(&page);
    no_private_work(&page);
    assert!(page.body.contains("Ready to join"));
    assert!(page.body.contains("Chamber instance 3"));
    assert!(
        page.body
            .contains("href=\"/cloud/app/hosts/resident/verse\">Join<")
    );
    assert!(page.body.contains("Open associated work"));
    // The world right is never presented as private work.
    assert!(page.body.contains("Observe through this binding"));
    assert!(!page.body.contains("Operate"));
    assert!(page.body.contains("Joining a world adds none"));

    let join = get(&fixture, &cookies, "/cloud/app/hosts/resident/verse").await;
    assert_eq!(join.status, StatusCode::SEE_OTHER, "{}", join.body);
    let target = join.headers[header::LOCATION].to_str().unwrap().to_owned();
    let base = target.strip_suffix("?zone=chamber").unwrap().to_owned();
    assert!(base.starts_with("/cloud/world/resident/"));
    let chamber = get(&fixture, &cookies, &target).await;
    assert_eq!(chamber.status, StatusCode::OK, "{}", chamber.body);
    no_private_work(&chamber);
    assert_eq!(chamber.headers[header::CACHE_CONTROL], "no-store, private");
    let policy = chamber.headers[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap();
    assert!(policy.contains("connect-src 'self' ws://127.0.0.1:9450"));
    assert!(chamber.body.contains("id=\"everglade-canvas\""));
    assert!(chamber.body.contains("href=\"/cloud/app/verse\">Leave<"));
    // Without the chamber query the renderer would open a public zone instead.
    assert_eq!(
        get(&fixture, &cookies, &base).await.status,
        StatusCode::SEE_OTHER
    );

    // The world client omits credentials; the ticket alone scopes these reads.
    let anonymous = Cookies::default();
    let config = get(&fixture, &anonymous, &format!("{base}chamber.json")).await;
    assert_eq!(config.status, StatusCode::OK, "{}", config.body);
    assert_eq!(
        config.body.as_bytes(),
        std::fs::read(directory.join("chamber.json")).unwrap()
    );
    assert_eq!(config.headers[header::CACHE_CONTROL], "no-store, private");
    let asset = get(
        &fixture,
        &anonymous,
        &format!("{base}chamber/assets/stone.bin"),
    )
    .await;
    assert_eq!(asset.status, StatusCode::OK);
    assert_eq!(asset.body, "texture");
    for path in [
        "outside.bin",
        "chamber/assets/../../outside.bin",
        "chamber/other.json",
    ] {
        let refused = get(&fixture, &anonymous, &format!("{base}{path}")).await;
        assert_ne!(refused.status, StatusCode::OK, "{path}");
        assert!(!refused.body.contains("not admitted"));
    }
    let ticket = base.trim_end_matches('/').rsplit('/').next().unwrap();
    let (front, signature) = ticket.rsplit_once('.').unwrap();
    let altered = if signature.ends_with('A') { 'B' } else { 'A' };
    let forged = format!(
        "/cloud/world/resident/{front}.{}{altered}/chamber.json",
        &signature[..signature.len() - 1]
    );
    assert_eq!(
        get(&fixture, &anonymous, &forged).await.status,
        StatusCode::FORBIDDEN
    );
    // The chamber page itself still needs the current account session.
    assert_eq!(
        get(&fixture, &anonymous, &target).await.status,
        StatusCode::UNAUTHORIZED
    );

    // A changed world configuration fences the ticket and the page.
    let mut changed: Value =
        serde_json::from_slice(&std::fs::read(directory.join("chamber.json")).unwrap()).unwrap();
    changed["instance"] = json!(4);
    private_file(
        &directory.join("chamber.json"),
        &serde_json::to_vec(&changed).unwrap(),
    );
    assert_ne!(
        get(&fixture, &anonymous, &format!("{base}chamber.json"))
            .await
            .status,
        StatusCode::OK
    );
    let page = get(&fixture, &cookies, "/cloud/app/verse").await;
    assert!(!page.body.contains(">Join<"));
    native.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_world_for_another_host_or_generation_is_never_admitted() {
    let mut fixture = fixture().await;
    let native = resident(&mut fixture).await;
    let private = native.config.parent().unwrap();
    let directory = private.join("foreign-world");
    std::fs::create_dir_all(&directory).unwrap();
    for (host, generation) in [("c".repeat(64), 7), (host_key(&native), 8)] {
        private_file(
            &directory.join("chamber.json"),
            &serde_json::to_vec(&json!({
                "websocket":WORLD_ROUTE,"host":host,"grant":"a".repeat(64),"epoch":1,
                "generation":generation,"instance":3,"content":"b".repeat(64),
                "pack":"p.json","scene":"s.json","assets":"a"
            }))
            .unwrap(),
        );
        let mut document: Value =
            serde_json::from_slice(&std::fs::read(&native.config).unwrap()).unwrap();
        document["bindings"][0]["world"] = json!(directory);
        private_file(&native.config, &serde_json::to_vec(&document).unwrap());
        assert!(super::super::hosts::Hosts::load(&native.config).is_err());
    }
    native.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn station_references_resolve_to_the_same_app_resources() {
    let mut fixture = fixture().await;
    let native = resident(&mut fixture).await;
    let host = host_key(&native);
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let encode = |reference: &contract::ResourceRef| {
        URL_SAFE_NO_PAD.encode(serde_json::to_vec(reference).unwrap())
    };
    let open =
        |reference: &str| format!("/cloud/app/verse/open?host=resident&resource={reference}");
    let paired = contract::Host::Paired { key: host.clone() };
    let run = contract::ResourceRef::new(contract::Kind::Run, paired.clone(), native.task.clone());
    let answer = get(&fixture, &cookies, &open(&encode(&run))).await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
    assert_eq!(
        answer.headers[header::LOCATION],
        format!("/cloud/app/hosts/resident/tasks/{}", native.task)
    );
    let terminal = contract::ResourceRef::terminal(
        paired.clone(),
        coder_host::mailbox::terminal_generation(&host, 7),
        "d".repeat(32),
    );
    let encoded = encode(&terminal);
    let answer = get(&fixture, &cookies, &open(&encoded)).await;
    assert_eq!(answer.status, StatusCode::SEE_OTHER, "{}", answer.body);
    assert_eq!(
        answer.headers[header::LOCATION],
        format!("/cloud/app/hosts/resident/workbench?resource={encoded}")
    );
    let studio = contract::ResourceRef::new(contract::Kind::Studio, paired, "goal-1")
        .studio(contract::StudioPart::Goal);
    let answer = get(&fixture, &cookies, &open(&encode(&studio))).await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    assert!(answer.body.contains("needs its own admitted owner viewer"));
    let foreign = contract::ResourceRef::new(
        contract::Kind::Run,
        contract::Host::Paired {
            key: "e".repeat(64),
        },
        native.task.clone(),
    );
    let answer = get(&fixture, &cookies, &open(&encode(&foreign))).await;
    assert_eq!(answer.status, StatusCode::CONFLICT);
    native.stop().await;
}
