//! Everglade's Agent Studio panels through a real phone scene: Interact at
//! a station opens that station's panel as a Rust Native view, the world
//! pauses under it, the view follows the studio's changes under new
//! revisions, only its current close control activates, and leaving
//! Everglade or pausing the surface drops it. The native JSON path the
//! hosts call carries the same view. These tests contact no relay or Coder
//! host.
//!
//! The `scratch_host` module drives the panels' intents against a scratch
//! Coder host over a local relay, as a paired phone does: it answers a
//! decision at the podium and merges a task at the merge station.
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
                approval: None,
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

/// The phone's studio panels acting on a scratch Coder host (#10570): a
/// host with its access store, host root, control socket, and task store
/// under a temporary directory, its studio over a scratch repository, and
/// a local authenticated relay. The phone enrolls with a grant of
/// `observe`, `operate`, and `review`, and its studio reaches the host
/// over that grant's NIP-HOST link, as a paired phone's does. Nothing here
/// reaches the person's own host, home directory, or relays.
///
/// No engine runs, so the test stands in for what an engine would do: it
/// submits the goal and cancels the lead's task over the same link, runs
/// the coordinator's pass, commits a change in a released task's
/// worktree, and ends that task's turn with a bounded command through the
/// task owner. Every task the test creates is archived when it ends, pass
/// or fail.
#[cfg(unix)]
mod scratch_host {
    use super::{Request, Scene, in_everglade, interact, station};
    use crate::connection_tests::relay;
    use coder::task::studio::{self as coordinator, Role, Seat, Studio as Coordinator};
    use coder::task::{Status, Store, studio_sim};
    use coder_access::review::PublishState;
    use coder_access::studio::{DecisionKind, GoalStatus, TaskStatus, Verdict, View};
    use coder_access::{Code, Operation, Outcome, RelayPolicy, Right, Rights};
    use coder_host::client::{Device, Link};
    use coder_host::config::{Config, Control, Iroh};
    use rust_native::Element;
    use secp256k1::SecretKey;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use verse::zones::everglade::studio::intents;
    use verse::zones::everglade::studio::live::{ControlSocket, Transport};

    const POLICY: RelayPolicy = RelayPolicy::LoopbackTest;
    /// How long the phone may take to show what a step did.
    const WAIT: Duration = Duration::from_secs(60);
    /// The goal the test submits.
    const GOAL: &str = "Greet with Hello, studio and document the greeting.";
    /// The change the greeting task's seat makes.
    const GREETING: &str = "Hello, studio\n";

    fn step(name: &str) {
        println!("step: {name}");
    }

    /// A scratch host and everything it keeps.
    struct Host {
        temp: tempfile::TempDir,
        runtime: tokio::runtime::Runtime,
        running: Option<coder_host::Running>,
        _relay: tokio::task::JoinHandle<()>,
        relay: String,
        access: PathBuf,
        socket: PathBuf,
        store: PathBuf,
        checkout: PathBuf,
        origin: PathBuf,
    }

    /// Seats a lead and two workers in the task store at `store`.
    fn seat(store: &Path) {
        let mut studio = Coordinator::open(store).expect("the coordinator opens");
        let route = coordinator::parse_route("codex:gpt-6-luna").expect("a route");
        for (desk, (name, role)) in [
            ("lead", Role::Lead),
            ("ada", Role::Worker),
            ("grace", Role::Worker),
        ]
        .into_iter()
        .enumerate()
        {
            studio
                .set_seat(Seat {
                    name: name.into(),
                    role,
                    route: route.clone(),
                    look: "default".into(),
                    desk: desk as u32,
                })
                .expect("a seat");
        }
    }

    /// Runs `git` in `dir` with no user or system configuration and a
    /// fixed identity, and returns its output.
    fn git(dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=seat",
                "-c",
                "user.email=seat@studio.invalid",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    fn host() -> Host {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .expect("a runtime");
        let temp = tempfile::tempdir().expect("a scratch directory");
        let fixture =
            studio_sim::Fixture::create(&temp.path().join("fixture")).expect("a repository");
        let checkout = std::fs::canonicalize(&fixture.checkout).expect("the checkout");
        // The merge commit is made as the checkout's own person: the
        // checkout's configuration names one and signs nothing.
        git(&checkout, &["config", "user.name", "Scratch Person"]);
        git(
            &checkout,
            &["config", "user.email", "person@studio.invalid"],
        );
        git(&checkout, &["config", "commit.gpgsign", "false"]);
        let store = temp.path().join("tasks");
        seat(&store);
        // Seats' turns end through the scripted engine (#10572), admitted only
        // in this scratch store.
        coder::task::owner::allow_scripted(&store, "the phone's studio test")
            .expect("scripted turns in the scratch store");
        let socket = temp.path().join("c/control.sock");
        let access = temp.path().join("access");
        let workspaces = BTreeMap::from([("scratch".to_owned(), checkout.clone())]);
        let (running, relay, relay_task) = runtime.block_on(async {
            let (relay, task, _) = relay::start().await;
            let owner = SecretKey::new(&mut secp256k1::rand::rng());
            coder_host::access::host::Host::new(&access, POLICY)
                .init(&coder_host::reach::pubkey(&owner))
                .expect("the access store");
            let mut config = Config::new(access.clone(), vec![relay.clone()], 3);
            config.policy = POLICY;
            config.iroh = Some(Iroh::loopback());
            config.control = Some(Control {
                path: socket.clone(),
                root: temp.path().join("host"),
                autostart: None,
                tasks: store.clone(),
                uid: coder_host::control::own_uid(),
            });
            config.workspaces = workspaces.clone();
            let tasks = Arc::new(coder::task::remote::Inbox::new(store.clone(), workspaces))
                as Arc<dyn coder_host::Tasks>;
            let running = coder_host::start(config, tasks).await.expect("the host");
            (running, relay, task)
        });
        Host {
            origin: fixture.origin.clone(),
            temp,
            runtime,
            running: Some(running),
            _relay: relay_task,
            relay,
            access,
            socket,
            store,
            checkout,
        }
    }

    impl Host {
        /// Enrolls a phone with `rights` over the relay, as a pasted
        /// `coder-host:` code does, and returns its link to the host.
        fn enroll(&self, rights: Rights) -> Arc<Link> {
            let now = coder_host::unix_time().expect("the time");
            let started = Instant::now();
            // The running host holds the access store's lock between its
            // own writes; the local command waits for it.
            let issued = loop {
                match coder_host::access::host::Host::new(&self.access, POLICY).invite(
                    &self.relay,
                    rights.clone(),
                    now,
                    now + 3600,
                ) {
                    Err(error) if error.code == Code::Conflict && started.elapsed() < WAIT => {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    other => break other.expect("an invitation"),
                }
            };
            let phone = SecretKey::new(&mut secp256k1::rand::rng());
            let access = self
                .runtime
                .block_on(coder_host::access::client::redeem(
                    &issued.code,
                    &phone,
                    POLICY,
                ))
                .expect("the phone redeems the code");
            let device = Arc::new(Device::new(access, phone, POLICY).expect("a device"));
            Arc::new(Link::relay(device, self.relay.clone()))
        }

        /// Cancels every task still open and archives every task in the
        /// store, through the host's control socket, as `task.cancel` and
        /// `task.archive` do.
        fn archive(&self) -> Result<usize, String> {
            let mut control = ControlSocket::new(self.socket.clone());
            let tasks = Store::open(&self.store)
                .and_then(|store| store.list())
                .map_err(|error| format!("{error:?}"))?;
            for task in &tasks {
                if matches!(
                    task.status,
                    Status::Queued | Status::Running | Status::CancelRequested
                ) {
                    control
                        .call(
                            &intents::mint(),
                            &Operation::CancelTask {
                                task: task.task_id.clone(),
                                revision: task.revision,
                                reason: "The studio test ended.".into(),
                            },
                        )
                        .map_err(|error| format!("cancel {}: {error:?}", task.task_id))?;
                }
                control
                    .call(
                        &intents::mint(),
                        &Operation::ArchiveTask {
                            task: task.task_id.clone(),
                        },
                    )
                    .map_err(|error| format!("archive {}: {error:?}", task.task_id))?;
            }
            Ok(tasks.len())
        }

        fn shutdown(&mut self) {
            if let Some(running) = self.running.take() {
                self.runtime.block_on(running.shutdown());
            }
        }

        /// The host's coordinator pass, which its auto-start sweep runs.
        fn reconcile(&self) {
            let mut tasks = Store::open(&self.store).expect("the store");
            let mut studio = Coordinator::open(&self.store).expect("the coordinator");
            studio
                .reconcile(&mut tasks, intents::now(), &|_: &str| None)
                .expect("the coordinator's pass");
        }

        /// Ends queued task `task`'s turn through the scripted engine, as an
        /// engine's finished turn records it.
        fn finish(&self, task: &str) {
            coder::task::owner::scripted(&self.store, task, |_, _| {
                Ok(coder::task::owner::Scripted {
                    ending: "model_finished".into(),
                    reply: "Changed the greeting.".into(),
                })
            })
            .expect("the turn ends");
        }
    }

    /// Advances the phone's scene a frame at a time, as the native host's
    /// display link does, until `test` holds, or fails naming `what`.
    fn drive(scene: &mut Scene, clock: &mut f64, what: &str, mut test: impl FnMut(&Scene) -> bool) {
        let deadline = Instant::now() + WAIT;
        loop {
            *clock += 0.05;
            scene.update(*clock).expect("a frame");
            if test(&*scene) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the phone never showed {what}; the computer last said {:?}",
                scene.world.studio().status()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn view(scene: &Scene) -> Option<&View> {
        scene.world.studio().view()
    }

    /// The host's answer to the phone's last `operation`, once it came.
    fn answered(scene: &Scene, operation: &str) -> Option<Result<Outcome, coder_access::Error>> {
        scene
            .world
            .studio()
            .status()
            .filter(|answer| answer.operation == operation)
            .map(|answer| answer.result.clone())
    }

    /// Whether the open panel's view holds the enabled control `key`.
    fn offers(scene: &Scene, key: &str) -> bool {
        let Some(view) = scene.studio_view() else {
            return false;
        };
        let mut pending = vec![&view.root];
        while let Some(node) = pending.pop() {
            if node.key == key {
                return matches!(node.element, Element::Button { enabled: true, .. });
            }
            if let Element::Stack { children, .. } | Element::List { children, .. } = &node.element
            {
                pending.extend(children);
            }
        }
        false
    }

    /// Activates the open panel's control `key`, as the host's tap does.
    fn tap(scene: &mut Scene, key: &str) {
        let view = scene.studio_view().expect("an open studio panel");
        scene
            .action(Request::StudioActivate {
                instance: view.instance,
                revision: view.revision,
                node: key.into(),
            })
            .unwrap_or_else(|error| panic!("{key}: {error}"));
    }

    fn run(host: &Host) {
        step("the phone enrolls and connects the studio over its grant");
        let link =
            host.enroll(Rights::new([Right::Observe, Right::Operate, Right::Review]).unwrap());
        let links: crate::studio_panel::Links = {
            let link = link.clone();
            Arc::new(move || -> Result<Arc<Link>, coder_access::Error> { Ok(link.clone()) })
        };
        let mut scene = in_everglade(station("podium"));
        let rights = scene
            .connect_studio(links, host.runtime.handle().clone())
            .expect("the studio connects");
        assert_eq!(rights, [Right::Observe, Right::Operate, Right::Review]);
        let mut clock = 2.0;
        drive(&mut scene, &mut clock, "the seats", |scene| {
            view(scene).is_some_and(|view| view.seats.len() == 3)
        });
        assert_eq!(
            scene.world.studio().rights(),
            [Right::Observe, Right::Operate, Right::Review]
        );

        step("a goal's lead fails, which opens a decision at the podium");
        let call = |operation: Operation| {
            host.runtime
                .block_on(link.call(operation))
                .expect("the computer takes it")
        };
        call(Operation::SubmitGoal {
            text: GOAL.into(),
            workspace: "scratch".into(),
            lead: None,
        });
        drive(&mut scene, &mut clock, "the goal's lead task", |scene| {
            view(scene).is_some_and(|view| {
                view.goals.iter().any(|goal| goal.text == GOAL)
                    && view.tasks.iter().any(|task| task.entry == "lead")
            })
        });
        let (goal, lead) = {
            let view = view(&scene).expect("the studio");
            let goal = view
                .goals
                .iter()
                .find(|goal| goal.text == GOAL)
                .map(|goal| goal.goal.clone())
                .expect("the goal");
            let lead = view
                .tasks
                .iter()
                .find(|task| task.goal == goal && task.entry == "lead")
                .map(|task| task.task.clone())
                .expect("the lead's task");
            (goal, lead)
        };
        call(Operation::CancelStudioTask { task: lead.clone() });
        drive(
            &mut scene,
            &mut clock,
            "the lead's task cancelled",
            |scene| {
                view(scene).is_some_and(|view| {
                    view.tasks
                        .iter()
                        .any(|task| task.task == lead && task.status == TaskStatus::Cancelled)
                })
            },
        );
        host.reconcile();
        drive(&mut scene, &mut clock, "the goal's decision", |scene| {
            view(scene).is_some_and(|view| {
                view.decisions
                    .iter()
                    .any(|open| open.goal == goal && open.kind == DecisionKind::LeadFailed)
            })
        });

        step("the podium answers the decision with a plan");
        interact(&mut scene).expect("the podium opens");
        let shown = super::texts(&scene);
        assert!(
            shown.iter().any(|text| text.contains("The lead failed")),
            "{shown:?}"
        );
        assert!(
            shown
                .iter()
                .any(|text| text.starts_with("Text you send from this panel answers")),
            "{shown:?}"
        );
        scene
            .action(Request::StudioText {
                text: studio_sim::plan(),
            })
            .expect("the answer is sent");
        drive(&mut scene, &mut clock, "the answer's receipt", |scene| {
            answered(scene, "studio.decision.answer").is_some()
        });
        let receipt = answered(&scene, "studio.decision.answer")
            .expect("an answer")
            .expect("the computer takes the answer");
        assert!(matches!(receipt, Outcome::Dispatched { .. }), "{receipt:?}");
        // The frame that took the answer rebuilt the panel with it as the
        // first row.
        let shown = super::texts(&scene);
        assert!(
            shown
                .iter()
                .any(|text| text.starts_with("**Sent** · `studio.decision.answer`")),
            "{shown:?}"
        );
        drive(&mut scene, &mut clock, "the goal resumed", |scene| {
            view(scene).is_some_and(|view| {
                let running = view
                    .goals
                    .iter()
                    .any(|g| g.goal == goal && g.status == GoalStatus::Running);
                let released = ["greet", "docs"].iter().all(|entry| {
                    view.tasks.iter().any(|task| {
                        task.goal == goal
                            && task.entry == *entry
                            && task.status == TaskStatus::Queued
                    })
                });
                running && released && view.decisions.iter().all(|open| open.goal != goal)
            })
        });
        let greet = view(&scene)
            .and_then(|view| {
                view.tasks
                    .iter()
                    .find(|task| task.goal == goal && task.entry == "greet")
            })
            .map(|task| task.task.clone())
            .expect("the greeting task");

        step("the greeting task's seat commits its change and ends its turn");
        let worktree = coder::task::studio::git::prepare(
            &host.temp.path().join("worktrees"),
            &host.store,
            &host.checkout,
            "ada",
            &greet,
            "Change the greeting",
            None,
        )
        .expect("the task's worktree");
        std::fs::write(worktree.join("greeting.txt"), GREETING).expect("a change");
        git(
            &worktree,
            &["commit", "-q", "-am", "Greet with Hello, studio"],
        );
        host.finish(&greet);
        let landed_before = git(&host.checkout, &["rev-parse", "HEAD"]);
        let origin_before = git(&host.origin, &["rev-parse", studio_sim::BRANCH]);

        step("the merge station merges the task");
        scene
            .action(Request::CloseStudio)
            .expect("the podium closes");
        scene
            .world
            .set_spawn(station("merge").into(), 0.0)
            .expect("the merge station");
        drive(&mut scene, &mut clock, "the greeting task done", |scene| {
            view(scene).is_some_and(|view| {
                view.tasks
                    .iter()
                    .any(|task| task.task == greet && task.status == TaskStatus::Done)
            })
        });
        interact(&mut scene).expect("the merge station opens");
        drive(
            &mut scene,
            &mut clock,
            "the review's Merge control",
            |scene| offers(scene, "studio-merge"),
        );
        let shown = super::texts(&scene);
        assert!(
            shown.iter().any(|text| text.contains("Hello, studio")),
            "the diff shows the change: {shown:?}"
        );
        assert!(offers(&scene, "studio-reject"));
        tap(&mut scene, "studio-merge");
        drive(&mut scene, &mut clock, "the merge's record", |scene| {
            answered(scene, "studio.merge.decide").is_some()
        });
        // The frame that took the answer rebuilt the panel with it as the
        // first row. A later read of the merged task's review can replace
        // it, so the row is read at once.
        let shown = super::texts(&scene);
        assert!(
            shown
                .iter()
                .any(|text| text.starts_with("**Merge** ·") && text.contains("nothing was pushed")),
            "{shown:?}"
        );
        let merged = match answered(&scene, "studio.merge.decide")
            .expect("an answer")
            .expect("the computer takes the merge")
        {
            Outcome::Merged { merged } => merged,
            other => panic!("expected the merge record, got {other:?}"),
        };
        assert_eq!(merged.task, greet);
        assert_eq!(merged.verdict, Verdict::Merge);
        let publication = merged.publication.expect("the landing's record");
        assert_eq!(
            publication.state,
            PublishState::Published,
            "{}",
            publication.note
        );
        assert!(publication.note.contains("nothing was pushed"));
        // The change is on the checkout's branch, and the remote is as it
        // was.
        assert_ne!(git(&host.checkout, &["rev-parse", "HEAD"]), landed_before);
        assert_eq!(
            std::fs::read_to_string(host.checkout.join("greeting.txt")).expect("the greeting"),
            GREETING
        );
        assert_eq!(
            git(&host.origin, &["rev-parse", studio_sim::BRANCH]),
            origin_before
        );
        scene.activate(false).expect("the surface pauses");
    }

    #[test]
    fn the_phone_answers_a_decision_and_merges_a_task_on_a_scratch_host() {
        let mut host = host();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&host)));
        let archived = host.archive();
        host.shutdown();
        if let Err(panic) = outcome {
            std::panic::resume_unwind(panic);
        }
        assert!(
            archived.expect("every task the test created is archived") >= 3,
            "the lead and two plan tasks were created"
        );
    }
}
