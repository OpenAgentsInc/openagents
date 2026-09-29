//! How fast the Coder tab gets to work: launch to a ready composer, a sent
//! message to its first visible words and to its end, and a kept Coder chat
//! list. These time the app's Rust side through `App::call`, as the iOS and
//! Android hosts drive it. The hosts ask for a packet when Rust rings
//! (`wake`); `coder_tab_to_composer_timings` times that wake and the
//! `changed` packet it asks for (see
//! `docs/coder/runtime/chat-load-benchmark.md`).
//!
//! `coder_tab_to_composer_timings` needs no network and runs with the other
//! tests. The live ones print their timings:
//!
//! ```sh
//! cargo test --release --manifest-path crates/openagents-mobile/Cargo.toml \
//!   speed_tests -- --include-ignored --nocapture --test-threads 1
//! ```
//!
//! `live_basic_coder_speed` sends three short messages to the OpenAgents
//! chat worker, each from a new device key. `live_computer_coder_speed`
//! needs `OPENAGENTS_TEST_ADMISSION` (a host with tailnet admission and
//! auto-start on) and archives the task it creates.

use crate::app::{App, Config, Launch, Packet, Request};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn children_of(node: &Value) -> Vec<Value> {
    let props = &node["element"]["props"];
    if let Some(name) = props["source"].as_str() {
        return rust_native::layout::source::get(name)
            .map(|snapshot| {
                snapshot
                    .rows()
                    .filter_map(|row| serde_json::to_value(row).ok())
                    .collect()
            })
            .unwrap_or_default();
    }
    props["children"].as_array().cloned().unwrap_or_default()
}

fn nodes_of(view: &Value, kind: &str) -> Vec<Value> {
    let mut out = vec![];
    let mut pending = vec![view["root"].clone()];
    while let Some(node) = pending.pop() {
        pending.extend(children_of(&node).into_iter().rev());
        if node["element"]["kind"] == kind {
            out.push(node);
        }
    }
    out
}

fn has_key(view: &Value, key: &str) -> bool {
    let mut pending = vec![view["root"].clone()];
    while let Some(node) = pending.pop() {
        if node["key"] == key {
            return true;
        }
        pending.extend(children_of(&node));
    }
    false
}

fn assistant_messages(view: &Value) -> usize {
    nodes_of(view, "message")
        .iter()
        .filter(|node| node["element"]["props"]["role"] == "assistant")
        .count()
}

fn tap(app: &mut App, view: &Value, key: &str) -> Packet {
    app.call(Request::CoderActivate {
        instance: view["instance"].as_str().unwrap_or_default().into(),
        revision: view["revision"].as_u64().unwrap_or_default(),
        node: key.into(),
    })
}

fn launch() -> Launch {
    // As the iOS host opens it.
    Launch {
        native_computers: true,
        pulled_transcripts: true,
        ..Launch::default()
    }
}

fn open(dir: &std::path::Path, secret_hex: &str) -> App {
    App::open(
        Config {
            state_dir: dir.to_path_buf(),
            secret_hex: secret_hex.into(),
        },
        launch(),
    )
    .expect("app")
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// Median and 95th percentile (nearest rank), in milliseconds.
fn summary(name: &str, samples: &mut [Duration]) -> String {
    samples.sort();
    let at = |p: usize| samples[((p * samples.len()).div_ceil(100)).max(1) - 1];
    format!(
        "| {name} | {} | {:.1} | {:.1} | {:.1} |",
        samples.len(),
        ms(at(50)),
        ms(at(95)),
        ms(samples[samples.len() - 1])
    )
}

/// Keep `count` basic conversations of `turns` turns each in the app's
/// store, as a phone that has been used for a while holds them.
fn seed(dir: &std::path::Path, secret: &secp256k1::SecretKey, count: usize, turns: usize) {
    let store = coder_computers::cache::Cache::open(&dir.join("basic-chats"), secret)
        .expect("basic chats store");
    let mut index = vec![];
    for n in 0..count {
        // Conversation IDs are random; rows are keyed by their first 16
        // hex digits, so those differ.
        let id = format!(
            "{:016x}{:016x}",
            (n as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15),
            n
        );
        let talk: Vec<Value> = (0..turns)
            .map(|t| {
                json!({
                    "role": if t % 2 == 0 { "user" } else { "assistant" },
                    "text": format!("Turn {t} of conversation {n}. ").repeat(if t % 2 == 0 { 4 } else { 30 }),
                })
            })
            .collect();
        store
            .write(&format!("basic-{id}"), &json!({ "turns": talk }))
            .expect("a conversation");
        index.push(json!({
            "id": id,
            "title": format!("Conversation {n}"),
            "started": 1_790_000_000 + n as u64,
            "updated": 1_790_000_000 + n as u64 * 60,
        }));
    }
    store.write("basic-index", &index).expect("the index");
}

/// Launch to the New chat composer, cold (a new install) and warm (a store
/// of 50 kept conversations), and a tab switch in a running app.
#[test]
fn coder_tab_to_composer_timings() {
    const RUNS: usize = 10;
    let secret_hex = "22".repeat(32);
    let secret: secp256k1::SecretKey = secret_hex.parse().expect("key");
    let warm_dir = tempfile::tempdir().expect("temp dir");
    seed(warm_dir.path(), &secret, 50, 20);
    let mut rows = vec![];
    for (label, warm) in [
        ("cold (new install)", false),
        ("warm (50 kept conversations)", true),
    ] {
        let (mut opened, mut first, mut composer, mut total, mut bytes) =
            (vec![], vec![], vec![], vec![], 0);
        for _ in 0..RUNS {
            let fresh = tempfile::tempdir().expect("temp dir");
            let dir = if warm { warm_dir.path() } else { fresh.path() };
            let started = Instant::now();
            let mut app = open(dir, &secret_hex);
            opened.push(started.elapsed());
            let at = Instant::now();
            let packet = app.call(Request::Snapshot);
            first.push(at.elapsed());
            bytes = serde_json::to_vec(&packet).map_or(0, |b| b.len());
            let coder = packet.coder.expect("the Coder view");
            // The Coder tab opens on a new chat, ready to type; an older
            // build opens on its list, with New chat one tap away.
            let at = Instant::now();
            let screen = if nodes_of(&coder, "composer").is_empty() {
                assert!(has_key(&coder, "coder-new"), "no composer and no New chat");
                tap(&mut app, &coder, "coder-new").coder.expect("New chat")
            } else {
                coder
            };
            composer.push(at.elapsed());
            total.push(started.elapsed());
            assert!(
                !nodes_of(&screen, "composer").is_empty(),
                "the Coder tab shows no composer"
            );
        }
        rows.push(summary(&format!("App::open, {label}"), &mut opened));
        rows.push(summary(
            &format!(
                "first packet (every surface rendered), {label}, {} KB",
                bytes / 1024
            ),
            &mut first,
        ));
        rows.push(summary(
            &format!("first packet to composer (0 when it opens on one), {label}"),
            &mut composer,
        ));
        rows.push(summary(
            &format!("launch to ready composer, {label}"),
            &mut total,
        ));
    }
    // Parts of a first launch, each on a new directory: what App::open
    // creates once.
    let (mut runtimes, mut stores, mut tailnets) = (vec![], vec![], vec![]);
    for _ in 0..RUNS {
        let fresh = tempfile::tempdir().expect("temp dir");
        let at = Instant::now();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("runtime");
        runtimes.push(at.elapsed());
        drop(runtime);
        let at = Instant::now();
        let store = coder_computers::cache::Cache::open(&fresh.path().join("store"), &secret)
            .expect("store");
        store.write("first", &json!({"rows": []})).expect("write");
        stores.push(at.elapsed());
        let at = Instant::now();
        crate::tailnet::Client::open(&fresh.path().join("tailscale")).expect("tailnet keys");
        tailnets.push(at.elapsed());
    }
    rows.push(summary("  part: tokio runtime (2 workers)", &mut runtimes));
    rows.push(summary(
        "  part: one new encrypted store + first write (fsync)",
        &mut stores,
    ));
    rows.push(summary(
        "  part: Tailscale node keys created and saved",
        &mut tailnets,
    ));
    // A running app: the tab bar tap sends nothing; the next poll's packet
    // is the whole cost.
    let mut app = open(warm_dir.path(), &secret_hex);
    let mut snapshots = vec![];
    for _ in 0..40 {
        let at = Instant::now();
        app.call(Request::Snapshot);
        snapshots.push(at.elapsed());
    }
    rows.push(summary(
        "snapshot packet in a running app (warm)",
        &mut snapshots,
    ));
    // A change in Rust to the host's `changed` packet: the host's thread
    // waits in `wake::wait`, as `openagents_mobile_wait` does.
    let (mut wakes, mut changed) = (vec![], vec![]);
    for _ in 0..40 {
        let seen = crate::wake::count();
        let waiter = std::thread::spawn(move || {
            crate::wake::wait(seen, Duration::from_secs(5));
            Instant::now()
        });
        std::thread::sleep(Duration::from_millis(2));
        let rung = Instant::now();
        crate::wake::ring();
        let woke = waiter.join().expect("waiter");
        wakes.push(woke.duration_since(rung));
        app.call(Request::Changed);
        changed.push(rung.elapsed());
    }
    rows.push(summary(
        "Rust change to the host's waiting thread",
        &mut wakes,
    ));
    rows.push(summary("Rust change to its `changed` packet", &mut changed));
    eprintln!("| Phase | n | median ms | p95 ms | max ms |\n| --- | ---: | ---: | ---: | ---: |");
    for row in rows {
        eprintln!("{row}");
    }
}

/// Send from New chat to the basic Coder and time the first visible words
/// and the end, polling the app every 5 ms (the hosts ask for a packet
/// when Rust rings with each partial).
#[test]
#[ignore = "network: sends three messages to the OpenAgents chat worker"]
fn live_basic_coder_speed() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let (mut sent_to_thinking, mut first_words, mut done) = (vec![], vec![], vec![]);
    for _ in 0..3 {
        let dir = tempfile::tempdir().expect("temp dir");
        let secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
        let mut app = open(dir.path(), &secret.display_secret().to_string());
        let coder = app.call(Request::Snapshot).coder.expect("Coder");
        let screen = if nodes_of(&coder, "composer").is_empty() {
            tap(&mut app, &coder, "coder-new").coder.expect("New chat")
        } else {
            coder
        };
        let token = nodes_of(&screen, "composer")[0]["element"]["props"]["token"]
            .as_str()
            .expect("a composer token")
            .to_owned();
        let started = Instant::now();
        let packet = app.call(Request::CoderInput {
            token,
            value: "In two short sentences, what is a Nostr relay?".into(),
        });
        sent_to_thinking.push(started.elapsed());
        let _ = packet;
        let mut first = None;
        loop {
            let packet = app.call(Request::Snapshot);
            let view = packet.coder.expect("Coder");
            if first.is_none() && assistant_messages(&view) > 0 {
                first = Some(started.elapsed());
            }
            if !packet.chat_streaming && first.is_some() {
                done.push(started.elapsed());
                break;
            }
            assert!(
                !has_key(&view, "talk-failed"),
                "the basic Coder failed: {:?}",
                nodes_of(&view, "text")
            );
            assert!(started.elapsed() < Duration::from_secs(90), "no answer");
            std::thread::sleep(Duration::from_millis(5));
        }
        first_words.push(first.expect("first words"));
    }
    eprintln!("| Phase | n | median ms | p95 ms | max ms |\n| --- | ---: | ---: | ---: | ---: |");
    eprintln!(
        "{}",
        summary(
            "Send to the message and thinking row shown",
            &mut sent_to_thinking
        )
    );
    eprintln!(
        "{}",
        summary("Send to first visible words (Rust side)", &mut first_words)
    );
    eprintln!("{}", summary("Send to reply done", &mut done));
}

/// Send from New chat to Coder on a real computer, as the owner does, and
/// time the task's start, its first reply, and its end. Needs a host with
/// tailnet admission and auto-start; set `OPENAGENTS_TEST_ADMISSION` to its
/// tailnet address. The task is archived whatever the outcome.
#[test]
#[ignore = "network: runs one real turn on a real host"]
fn live_computer_coder_speed() {
    use crate::tailnet::{Device, Tailnet};
    use crate::tailnet_view::Screen;
    let address = std::env::var("OPENAGENTS_TEST_ADMISSION").expect("OPENAGENTS_TEST_ADMISSION");
    let dir = tempfile::tempdir().expect("temp dir");
    let started = Instant::now();
    let mut app = open(dir.path(), &"33".repeat(32));
    app.call(Request::Lifecycle { active: true });
    app.set_tailnet(Screen::Devices(Tailnet {
        name: None,
        this_device: None,
        devices: vec![Device {
            name: "test-computer".into(),
            os: "macOS".into(),
            address,
            online: Some(true),
        }],
    }));
    let at = |what: &str, since: Instant| eprintln!("{:>8.1} ms  {what}", ms(since.elapsed()));
    // New chat, on the computer, once the computer can take it.
    let composer = loop {
        let coder = app.call(Request::ComputersRefresh).coder.expect("Coder");
        // An older build opens on the list; New chat is one tap away.
        if nodes_of(&coder, "composer").is_empty() && has_key(&coder, "coder-new") {
            tap(&mut app, &coder, "coder-new");
            continue;
        }
        // LOCAL MEASUREMENT PATCH: the new chat's target selector.
        let label = nodes_of(&coder, "button")
            .into_iter()
            .find(|node| node["key"] == "coder-target")
            .and_then(|node| node["element"]["props"]["label"].as_str().map(str::to_owned))
            .unwrap_or_default();
        if label.contains(" · ") && !nodes_of(&coder, "composer").is_empty() {
            break coder;
        }
        if started.elapsed() > Duration::from_secs(40) {
            let texts: Vec<String> = nodes_of(&coder, "text").iter().filter_map(|n| n["element"]["props"]["value"].as_str().map(str::to_owned)).collect();
            let buttons: Vec<String> = nodes_of(&coder, "button").iter().map(|n| format!("{}={}", n["key"], n["element"]["props"]["label"])).collect();
            let account = app.call(Request::ComputersRefresh);
            panic!("DIAG label {label} texts {texts:?} buttons {buttons:?} computers {}", serde_json::to_string(&account.computers).unwrap_or_default().chars().take(1500).collect::<String>());
        }
        if label == "Cloud" {
            let picking = tap(&mut app, &coder, "coder-target");
            let _ = picking;
            let coder2 = app.call(Request::ComputersRefresh).coder.expect("Coder");
            if has_key(&coder2, "coder-target-0") {
                tap(&mut app, &coder2, "coder-target-0");
            } else if has_key(&coder2, "coder-target-cloud") {
                tap(&mut app, &coder2, "coder-target-cloud");
            }
        }
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "the computer never admitted"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    at(
        "admission, pairing, and a composer on the computer",
        started,
    );
    let token = nodes_of(&composer, "composer")[0]["element"]["props"]["token"]
        .as_str()
        .expect("token")
        .to_owned();
    let sent = Instant::now();
    // Wall-clock time of the send, to line up with the host's own records.
    eprintln!(
        "sent at unix ms {}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis())
    );
    app.call(Request::CoderInput {
        token,
        value: "Reply with only the word ready.".into(),
    });
    at("send answered by the app", sent);
    let task = app.open_coder_task().expect("the chat opens on its task");
    eprintln!("task {}", &task.1[..16]);
    let mut working = None;
    let mut first = None;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        loop {
            let chat = app.call(Request::ComputersRefresh).coder.expect("chat");
            let place = nodes_of(&chat, "text")
                .iter()
                .filter_map(|n| n["element"]["props"]["value"].as_str().map(str::to_owned))
                .find(|t| {
                    t.starts_with("Working") || t.starts_with("Done") || t.starts_with("Queued")
                })
                .unwrap_or_default();
            if working.is_none() && place.starts_with("Working") {
                working = Some(sent.elapsed());
                at("task shows Working", sent);
            }
            if first.is_none() && assistant_messages(&chat) > 0 {
                first = Some(sent.elapsed());
                at("first reply row visible", sent);
            }
            if place.starts_with("Done") && first.is_some() {
                at("task shows Done with its reply", sent);
                break;
            }
            assert!(
                sent.elapsed() < Duration::from_secs(300),
                "no reply: {place}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }));
    let archived = app.archive_task_for_test(task, Duration::from_secs(300));
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
    archived.expect("the test's task is archived");
}
