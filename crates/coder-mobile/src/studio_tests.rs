//! Everglade's Agent Studio panels through a real phone scene: Interact at
//! a station opens that station's panel as a Rust Native view, the world
//! pauses under it, the view follows the studio's changes under new
//! revisions, only its current close control activates, and leaving
//! Everglade or pausing the surface drops it. The native JSON path the
//! hosts call carries the same view. No relay or Coder host is contacted.
use super::{Config, Request, Scene};
use crate::verse_ffi::VerseHandle;
use coder_access::review::TaskReview;
use coder_access::studio::{
    Activity, Decision, DecisionKind, Goal, GoalStatus, Role, Seat, Snapshot, Station, View,
};
use rust_native::Element;
use verse::controller::InputState;
use verse::zones::everglade::studio::Source;
use verse::zones::everglade_pack::{PACK_DIRECTORY, PACK_EXTENSION, PACK_SHA256, ZonePack};
use verse::zones::{Intent as ZoneIntent, ZoneId};

fn scene() -> Scene {
    Scene::new(Config {
        secret_hex: "11".repeat(32),
        width: 800,
        height: 1200,
        scale: 2.0,
        synthetic: true,
        gym_code: None,
        synthetic_gym: false,
        world_relay: None,
        world_offline: false,
        door_preferences: None,
        zone_cache_directory: None,
        results_base: None,
        results_cache_directory: None,
        computer_hud: true,
        hdr: false,
        bare: false,
        xp_preview: false,
        gym_notes: false,
    })
    .unwrap()
}

/// The committed, pinned Everglade pack, decoded once for every test.
fn pack() -> &'static ZonePack {
    static PACK: std::sync::OnceLock<ZonePack> = std::sync::OnceLock::new();
    PACK.get_or_init(|| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(PACK_DIRECTORY)
            .join(format!("{PACK_SHA256}.{PACK_EXTENSION}"));
        ZonePack::load_local(&path).expect("the committed Everglade pack loads")
    })
}

/// The station's standing point, `id` from Everglade's layout table.
fn station(id: &str) -> [f32; 3] {
    let at = verse::zones::everglade::STATIONS
        .iter()
        .find(|s| s.id == id)
        .unwrap()
        .at;
    [at[0], 0.0, at[1]]
}

/// An active phone scene inside Everglade, standing at `at`.
fn in_everglade(at: [f32; 3]) -> Scene {
    let mut scene = scene();
    scene.activate(true).unwrap();
    scene.world.install_everglade(pack());
    assert_eq!(scene.world.zone, ZoneId::Everglade);
    scene.reset_zone_inputs();
    scene.world.set_spawn(at.into(), 0.0).unwrap();
    scene.update(1.0).unwrap();
    scene.update(1.02).unwrap();
    scene
}

/// A source that hands the studio one snapshot.
struct Once(Option<Snapshot>);

impl Source for Once {
    fn poll(&mut self, _dt: f32) -> Option<Snapshot> {
        self.0.take()
    }

    fn review(&mut self, _task: &str) -> Option<TaskReview> {
        None
    }
}

fn studio() -> Snapshot {
    Snapshot {
        stream: "test".into(),
        sequence: 1,
        view: View {
            goals: vec![Goal {
                goal: "g1".into(),
                text: "Mount the studio on phones".into(),
                workspace: "openagents".into(),
                lead: "ada".into(),
                status: GoalStatus::Decision,
                final_tasks: 0,
                total_tasks: 2,
                submitted_at: 5,
                spend: Default::default(),
            }],
            seats: vec![Seat {
                seat: "ada".into(),
                role: Role::Lead,
                route: "codex:gpt-6".into(),
                look: "default".into(),
                desk: 0,
                activity: Activity::Waiting,
                station: Station::Podium,
                task: None,
                paused: false,
                spend: Default::default(),
            }],
            decisions: vec![Decision {
                decision: "g1".into(),
                goal: "g1".into(),
                task: None,
                seat: Some("ada".into()),
                kind: DecisionKind::Question,
                text: "Which host should run the tests?".into(),
                based_on: 1,
            }],
            ..View::default()
        },
    }
}

/// Every text the open panel's view shows, in tree order.
fn texts(scene: &Scene) -> Vec<String> {
    let view = scene.studio_view().expect("an open studio panel");
    let mut out = Vec::new();
    let mut pending = vec![&view.root];
    while let Some(node) = pending.pop() {
        match &node.element {
            Element::Stack { children, .. } | Element::List { children, .. } => {
                pending.extend(children.iter().rev());
            }
            Element::Text { value, .. } => out.push(value.clone()),
            _ => {}
        }
    }
    out
}

fn interact(scene: &mut Scene) -> Result<(), String> {
    scene.action(Request::Zone {
        intent: ZoneIntent::Interact,
    })
}

#[test]
fn interact_opens_the_station_panel_and_pauses_the_world() {
    let mut scene = in_everglade(station("podium"));
    // Interact is the zone's own control, drawn and offered to the host.
    let hud = scene.zone_hud_snapshot();
    assert!(
        hud.buttons
            .iter()
            .any(|b| b.action == ZoneIntent::Interact && b.enabled && b.label == "Decisions"),
        "{:?}",
        hud.buttons
    );
    interact(&mut scene).unwrap();
    let open = scene.studio.as_ref().unwrap();
    assert_eq!(
        open.kind,
        verse::zones::everglade::studio::PanelKind::Decisions
    );
    let shown = texts(&scene);
    assert_eq!(shown[0], "Decisions");
    assert!(
        shown
            .iter()
            .any(|t| t.starts_with("The studio has not loaded"))
    );
    let packet = serde_json::to_value(scene.packet()).unwrap();
    assert_eq!(packet["studio_open"], true);
    assert_eq!(packet["studio_revision"], 1);
    // A frame packet omits the view; the host asks for it by revision.
    assert!(packet.get("studio_view").is_none());
    // The world stands still under the panel, and its controls step aside.
    let before = scene.world.player.pos;
    for n in 1..=20 {
        scene
            .update_with_input(
                1.02 + f64::from(n) / 60.0,
                Some(InputState {
                    forward: true,
                    ..InputState::default()
                }),
            )
            .unwrap();
    }
    assert_eq!(scene.world.player.pos, before);
    assert!(!scene.zone_hud_snapshot().visible);
    assert!(interact(&mut scene).is_err());
}

#[test]
fn each_station_opens_its_own_panel_and_away_from_one_nothing_opens() {
    use verse::zones::everglade::studio::PanelKind;
    for (id, kind, title) in [
        ("task_wall", PanelKind::Console, "Console"),
        ("podium", PanelKind::Decisions, "Decisions"),
        ("merge", PanelKind::Review, "Diff review"),
        ("desks", PanelKind::Desk(0), "Desk 1"),
    ] {
        let at = if id == "desks" {
            let seat = verse::zones::everglade::layout::DESKS[0].seat;
            [seat[0], 0.0, seat[1]]
        } else {
            station(id)
        };
        let mut scene = in_everglade(at);
        interact(&mut scene).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(scene.studio.as_ref().unwrap().kind, kind, "{id}");
        assert_eq!(texts(&scene)[0], title, "{id}");
    }
    let mut scene = in_everglade(station("approach"));
    assert!(interact(&mut scene).is_err());
    assert!(scene.studio.is_none());
}

#[test]
fn the_open_panel_follows_the_studio_under_new_revisions() {
    let mut scene = in_everglade(station("podium"));
    scene
        .world
        .set_studio_source(Box::new(Once(Some(studio()))));
    interact(&mut scene).unwrap();
    let first = scene.studio_view().unwrap();
    // The next frame starts the source, takes its snapshot, and rebuilds.
    scene.update(1.1).unwrap();
    let second = scene.studio_view().unwrap();
    assert_eq!(second.instance, first.instance);
    assert!(second.revision > first.revision);
    let shown = texts(&scene);
    assert!(
        shown
            .iter()
            .any(|t| t.starts_with("**ada** asks") && t.contains("Which host")),
        "{shown:?}"
    );
    // Nothing changed: the view and its revision stay.
    scene.update(1.2).unwrap();
    assert_eq!(scene.studio_view().unwrap().revision, second.revision);
    // Only the current view's close control activates.
    let stale = Request::StudioActivate {
        instance: first.instance.clone(),
        revision: first.revision,
        node: "studio-close".into(),
    };
    assert!(scene.action(stale).is_err());
    assert!(scene.studio.is_some());
    let not_a_control = Request::StudioActivate {
        instance: second.instance.clone(),
        revision: second.revision,
        node: "studio-title".into(),
    };
    assert!(scene.action(not_a_control).is_err());
    scene
        .action(Request::StudioActivate {
            instance: second.instance,
            revision: second.revision,
            node: "studio-close".into(),
        })
        .unwrap();
    assert!(scene.studio.is_none());
    // Back in the world, the player walks again, away from the lectern.
    scene
        .world
        .set_spawn(station("podium").into(), std::f32::consts::PI)
        .unwrap();
    let before = scene.world.player.pos;
    for n in 1..=20 {
        scene
            .update_with_input(
                1.2 + f64::from(n) / 60.0,
                Some(InputState {
                    forward: true,
                    ..InputState::default()
                }),
            )
            .unwrap();
    }
    assert!(scene.world.player.pos.distance(before) > 0.1);
}

#[test]
fn pausing_or_leaving_everglade_drops_the_panel() {
    let mut scene = in_everglade(station("podium"));
    interact(&mut scene).unwrap();
    scene.activate(false).unwrap();
    assert!(scene.studio.is_none());
    assert!(!scene.world.studio().active());
    scene.activate(true).unwrap();
    scene.update(2.0).unwrap();
    interact(&mut scene).unwrap();
    // The portal is refused under the panel; the runtime leaving the zone
    // still closes it on the next frame.
    assert!(
        scene
            .action(Request::Zone {
                intent: ZoneIntent::Return
            })
            .is_err()
    );
    scene.world.zone_intent(ZoneIntent::Return).unwrap();
    scene.update(2.1).unwrap();
    assert!(scene.world.is_plaza());
    assert!(scene.studio.is_none());
    assert!(scene.action(Request::CloseStudio).is_ok());
}

#[test]
fn the_native_json_path_carries_the_studio_view() {
    let mut handle = VerseHandle {
        scene: in_everglade(station("task_wall")),
        renderer: None,
        rendered_zone_revision: 0,
    };
    let call = |handle: &mut VerseHandle, request: &str| -> serde_json::Value {
        serde_json::from_slice(&handle.call_bytes(request.as_bytes()).unwrap()).unwrap()
    };
    let opened = call(&mut handle, r#"{"action":"zone","intent":"interact"}"#);
    assert_eq!(opened["studio_open"], true, "{}", opened["error"]);
    let view = &opened["studio_view"];
    assert_eq!(view["schema"], "rust-native.view.v2");
    assert_eq!(view["revision"], opened["studio_revision"]);
    assert_eq!(view["root"]["key"], "studio");
    let frame = call(&mut handle, r#"{"action":"snapshot"}"#);
    assert!(frame.get("studio_view").is_none());
    let again = call(&mut handle, r#"{"action":"studio_view"}"#);
    assert_eq!(again["studio_view"], *view);
    let close = serde_json::json!({
        "action": "studio_activate",
        "instance": view["instance"],
        "revision": view["revision"],
        "node": "studio-close",
    });
    let closed = call(&mut handle, &close.to_string());
    assert_eq!(closed["studio_open"], false, "{}", closed["error"]);
    assert!(closed.get("studio_view").is_none());
    assert_eq!(closed["studio_revision"], 0);
}
