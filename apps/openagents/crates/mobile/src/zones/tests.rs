use super::*;
use aethyrforge_host::{
    BOOTSTRAP, Capability, Engine, MAX_BYTES, Message, NativePeer, Outcome, ResultState, SaveState,
    Status,
};
use rust_native::Activation;

fn house_json() -> serde_json::Value {
    serde_json::from_str(BUNDLED_HOUSE).unwrap()
}

fn launcher() -> Launcher {
    Launcher::new(&bundled_catalog())
}

fn view(json: &str) -> serde_json::Value {
    serde_json::from_str(json).unwrap()
}

fn keys(node: &serde_json::Value, out: &mut Vec<String>) {
    if let Some(k) = node["key"].as_str() {
        out.push(k.into());
    }
    for c in node["element"]["props"]["children"]
        .as_array()
        .into_iter()
        .flatten()
    {
        keys(c, out);
    }
}

#[test]
fn the_bundled_catalog_is_the_websites_pins() {
    let c = bundled_catalog();
    let ids: Vec<_> = c.zones.iter().map(|z| z.id.as_str()).collect();
    assert_eq!(ids, ["haunted-house", "universe"]);
    let house = house_json();
    assert_eq!(c.zones[0].release.engine_url, house["engine_url"]);
    assert_eq!(c.zones[0].release.release, house["release"]);
    let universe: serde_json::Value = serde_json::from_str(BUNDLED_UNIVERSE).unwrap();
    assert_eq!(c.zones[1].release.engine_url, universe["engine_url"]);
}

#[test]
fn manifests_put_the_grid_first_and_carry_release_origin_and_runner() {
    let z = zones(&bundled_catalog());
    let grid = &z[0].manifest;
    assert_eq!(grid.id, GRID_ZONE);
    assert_eq!(grid.runner, ZoneRunnerKind::Builtin);
    assert!(grid.url.is_none() && grid.origin.is_none() && grid.release_id.is_none());
    let house = &z[1].manifest;
    assert_eq!(house.runner, ZoneRunnerKind::Web);
    assert_eq!(
        house.release_id.as_deref(),
        house_json()["release"].as_str()
    );
    let url = house.url.clone().unwrap();
    let origin = house.origin.clone().unwrap();
    assert!(url.starts_with(&origin));
    assert!(!origin.ends_with('/'));
    assert_eq!(
        house.page_url.as_deref(),
        Some("https://openagents.com/new-lander/house")
    );
    assert_eq!(house.detail_floor, ZoneDetailFloor::Any);
}

#[test]
fn a_fetched_catalog_replaces_web_zones_but_never_the_grid() {
    let mut c = bundled_catalog();
    c.zones.truncate(1);
    let text = c.encode().unwrap();
    let parsed = parse_catalog(&text).unwrap();
    let z = zones(&parsed);
    assert_eq!(z.len(), 2);
    assert_eq!(z[0].manifest.id, GRID_ZONE);
    assert!(parse_catalog("{\"schema\":\"openagents.zones.v1\",\"zones\":[],\"x\":1}").is_err());
    assert!(parse_catalog(&text.replace("https://", "http://")).is_err());
    assert!(parse_catalog(&" ".repeat(aethyrforge_host::MAX_ZONE_CATALOG_BYTES + 1)).is_err());
}

#[test]
fn launcher_cards_route_the_grid_and_web_zones() {
    let mut l = launcher();
    let v = view(&l.frame().unwrap());
    let mut k = vec![];
    keys(&v["root"], &mut k);
    for id in ["grid", "haunted-house", "universe"] {
        assert!(k.contains(&format!("open:{id}")), "{k:?}");
        assert!(k.contains(&format!("zone:{id}")));
    }
    let instance = v["instance"].as_str().unwrap().to_owned();
    let revision = v["revision"].as_u64().unwrap();
    let tap = |node: &str| Activation {
        instance: instance.clone(),
        revision,
        node: node.into(),
    };
    assert_eq!(l.activate(&tap("open:grid")).unwrap(), ZoneLaunch::Grid);
    assert_eq!(
        l.activate(&tap("open:haunted-house")).unwrap(),
        ZoneLaunch::Web {
            zone_id: "haunted-house".into()
        }
    );
    // A tap on an older revision, or on a text node, does nothing.
    assert!(l.activate(&tap("title:grid")).is_err());
    l.frame().unwrap();
    assert!(l.activate(&tap("open:grid")).is_err());
}

fn runner() -> Runner {
    let z = zones(&bundled_catalog());
    Runner::new(z[1].clone()).unwrap()
}

#[test]
fn the_grid_has_no_web_runner() {
    assert!(Runner::new(grid()).is_err());
}

#[test]
fn the_runner_allows_only_the_exact_release_origin() {
    let r = runner();
    let origin = r.origin().to_owned();
    assert!(r.allows(r.url()));
    assert!(r.allows(&format!("{origin}/assets/world.bin?x=1")));
    for bad in [
        "https://openagents.com/".to_owned(),
        format!("{origin}.evil.example/"),
        origin.replace("https://", "http://"),
        format!("{origin}:8443/"),
        "about:blank".to_owned(),
        "javascript:void(0)".to_owned(),
    ] {
        assert!(!r.allows(&bad), "{bad}");
    }
}

fn peer(origin: &str) -> NativePeer<'_> {
    NativePeer {
        origin,
        main_frame: true,
    }
}

#[test]
fn webgpu_reports_gate_the_zone() {
    let mut r = runner();
    let origin = r.origin().to_owned();
    assert_eq!(r.phase(), ZoneRunnerPhase::Loading);
    r.page_finished();
    assert_eq!(r.phase(), ZoneRunnerPhase::Loading);
    let ok = r#"{"kind":"webgpu","available":true,"adapter":{"vendor":"apple","architecture":"common-3","device":"","description":""},"reason":null,"user_agent":"Mozilla/5.0"}"#;
    // Reports from another origin or a subframe are refused.
    assert!(r.probe(peer("https://evil.example"), ok).is_err());
    assert!(
        r.probe(
            NativePeer {
                origin: &origin,
                main_frame: false
            },
            ok
        )
        .is_err()
    );
    r.probe(peer(&origin), ok).unwrap();
    assert_eq!(r.phase(), ZoneRunnerPhase::Ready);
    assert_eq!(r.gpu().unwrap().vendor, "apple");
    assert_eq!(r.card().unwrap(), None);

    let mut r = runner();
    let missing = r#"{"kind":"webgpu","available":false,"adapter":null,"reason":"no-webgpu","user_agent":null}"#;
    r.probe(peer(&origin), missing).unwrap();
    r.page_finished();
    assert_eq!(r.phase(), ZoneRunnerPhase::Unavailable);
    assert_eq!(r.gpu_reason().as_deref(), Some("no-webgpu"));
    let card = view(&r.card().unwrap().unwrap());
    let mut k = vec![];
    keys(&card["root"], &mut k);
    assert!(k.contains(&"browser".to_owned()));
    let action = r
        .activate(&Activation {
            instance: card["instance"].as_str().unwrap().into(),
            revision: card["revision"].as_u64().unwrap(),
            node: "browser".into(),
        })
        .unwrap();
    assert_eq!(
        action,
        ZoneRunnerAction::OpenInBrowser {
            url: "https://openagents.com/new-lander/house".into()
        }
    );

    // Malformed or oversized reports are refused.
    let mut r = runner();
    assert!(r.probe(peer(&origin), "{}").is_err());
    assert!(
        r.probe(
            peer(&origin),
            &ok.replace("\"kind\":\"webgpu\"", "\"kind\":\"other\"")
        )
        .is_err()
    );
    assert!(r.probe(peer(&origin), &"x".repeat(5000)).is_err());
}

#[test]
fn failures_offer_retry_and_close() {
    let mut r = runner();
    r.page_failed(true);
    let card = view(&r.card().unwrap().unwrap());
    let mut k = vec![];
    keys(&card["root"], &mut k);
    assert!(k.contains(&"retry".to_owned()) && k.contains(&"close".to_owned()));
    let text = card.to_string();
    assert!(text.contains("offline"));
    let act = |r: &mut Runner, node: &str, card: &serde_json::Value| {
        r.activate(&Activation {
            instance: card["instance"].as_str().unwrap().into(),
            revision: card["revision"].as_u64().unwrap(),
            node: node.into(),
        })
    };
    assert_eq!(
        act(&mut r, "retry", &card).unwrap(),
        ZoneRunnerAction::Retry
    );
    assert_eq!(r.phase(), ZoneRunnerPhase::Loading);
    let card = view(&r.card().unwrap().unwrap());
    assert_eq!(
        act(&mut r, "close", &card).unwrap(),
        ZoneRunnerAction::Close
    );
}

#[test]
fn the_bridge_bootstraps_and_opens_as_a_guest_within_bounds() {
    let mut r = runner();
    let origin = r.origin().to_owned();
    let release = r.zone.release.clone().unwrap();
    // Wrong origin, subframes, and oversized frames are refused.
    assert!(r.bridge(peer("https://evil.example"), BOOTSTRAP, 0).refused);
    assert!(
        r.bridge(
            NativePeer {
                origin: &origin,
                main_frame: false
            },
            BOOTSTRAP,
            0
        )
        .refused
    );
    assert!(
        r.bridge(peer(&origin), &"x".repeat(MAX_BYTES + 1), 0)
            .refused
    );
    let hello = r.bridge(peer(&origin), BOOTSTRAP, 0);
    assert!(!hello.refused);
    let (mut engine, ready) = Engine::accept(
        &hello.deliver[0],
        release.release.clone(),
        release.engine_abi.clone(),
        release.project_format,
        vec![Capability::Inspect, Capability::Observe],
    )
    .unwrap();
    let open = r.bridge(peer(&origin), &ready.encode().unwrap(), 1);
    assert_eq!(open.deliver.len(), 1);
    let Message::Request { request_id, .. } = engine.receive(&open.deliver[0]).unwrap() else {
        panic!("open request")
    };
    let reply = engine
        .reply(
            &request_id,
            Outcome {
                state: ResultState::Applied,
                message: "Opened.".into(),
                status: Some(Status {
                    project: release.project.clone(),
                    generation: "g1".into(),
                    frame: 1,
                    save: SaveState::Unsaved,
                    checkpoint: None,
                    selection: None,
                    authoring: None,
                    view: None,
                }),
                detail: None,
            },
        )
        .unwrap()
        .encode()
        .unwrap();
    assert!(!r.bridge(peer(&origin), &reply, 2).refused);
    assert_eq!(r.bridge_phase(), aethyrforge_host::Phase::Open);
    // A replayed envelope is refused; a new document starts a new session.
    assert!(r.bridge(peer(&origin), &reply, 3).refused);
    r.page_started();
    assert!(!r.bridge(peer(&origin), BOOTSTRAP, 4).refused);
}

#[test]
fn scripts_name_the_channels_and_never_splice_frames_into_source() {
    let ios = probe_script(ZoneWebPlatform::Ios);
    assert!(ios.contains("window.webkit.messageHandlers.aethyrShell.postMessage"));
    assert!(ios.contains("requestAdapter"));
    assert!(ios.contains("\"gpu\" in navigator"));
    let android = probe_script(ZoneWebPlatform::Android);
    assert!(android.contains("window.aethyrShell.postMessage"));
    assert!(ios_channel_script().contains("messageHandlers.aethyrHost"));
    assert!(ios_channel_script().contains("window.top !== window"));
    // The delivered frame is the `text` argument, not part of the script.
    assert!(ios_deliver_script().contains("data: text"));
}
