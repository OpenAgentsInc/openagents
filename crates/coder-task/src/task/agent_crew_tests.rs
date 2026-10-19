//! The crew on the same machinery (#10806): Alice and Bob side by side on
//! one scratch host, each with a key, engrams, journal, steering loop, and
//! Coder session of their own, made through `studio.agent.new`.

use super::*;
use crate::task::agent_engrams::{EngramStore, Opened};
use crate::task::agent_steer::{Judgment, Mind, Move, Plan, ScriptedJudge, ScriptedPlanner, Step};
use serde_json::json;
use std::time::Instant;

fn clock() -> u64 {
    1_791_158_400
}

fn owner_key() -> secp256k1::SecretKey {
    secp256k1::SecretKey::from_byte_array([7; 32]).unwrap()
}

fn owner() -> Principal {
    Principal {
        device: "owner".into(),
        grant: None,
        epoch: None,
    }
}

fn run(command: &str, output: &str) -> Vec<CoderEvent> {
    vec![
        CoderEvent::Tool {
            name: "Run".into(),
            input: json!(command),
            output: serde_json::Value::Null,
            running: true,
            delegation: None,
        },
        CoderEvent::Tool {
            name: "Run".into(),
            input: json!(command),
            output: json!({"command": command, "exit": 0, "output": output}),
            running: false,
            delegation: None,
        },
    ]
}

fn turn(command: &str, reply: &str) -> coder_v1::Scripted {
    coder_v1::Scripted {
        events: run(command, reply),
        ended: Some(Ended::Finished {
            reply: reply.into(),
            tokens: 0,
        }),
        ..coder_v1::Scripted::default()
    }
}

fn plan(prompt: &str) -> Plan {
    Plan {
        understanding: "The owner wants this done.".into(),
        answer_directly: false,
        reply_if_direct: None,
        steps: vec![Step {
            prompt: prompt.into(),
            done_when: "the command ran".into(),
        }],
        verify: None,
    }
}

/// One crew member's script: its plan, Coder turn, and report.
struct Script {
    plan: Plan,
    turn: coder_v1::Sequence,
    report: &'static str,
    asked: Arc<Mutex<Vec<crate::task::agent_steer::Ask>>>,
}

/// A scratch host where `studio.agent.new` made Alice and Bob in one
/// checkout, each playing their own script.
fn crew(dir: &tempfile::TempDir) -> (Agents, BTreeMap<&'static str, Script>) {
    let root = dir.path().join("host");
    let workspace = dir.path().join("work");
    std::fs::create_dir_all(workspace.join(".git")).unwrap();
    let mut scripts = BTreeMap::new();
    scripts.insert(
        "alice",
        Script {
            plan: plan("Run the atif tests."),
            turn: coder_v1::Sequence::new(vec![turn("cargo test -p atif", "31 passed")]),
            report: "The atif tests pass.",
            asked: Arc::default(),
        },
    );
    scripts.insert(
        "bob",
        Script {
            plan: plan("List the townsfolk files."),
            turn: coder_v1::Sequence::new(vec![turn("ls townsfolk", "baker.json")]),
            report: "The town has one villager file, the baker.",
            asked: Arc::default(),
        },
    );
    let turns: BTreeMap<String, coder_v1::Sequence> = scripts
        .iter()
        .map(|(name, s)| ((*name).to_string(), s.turn.clone()))
        .collect();
    let engine: EngineFactory = Arc::new(move |record: &Record| {
        Ok((
            Box::new(turns[&record.name].clone()) as Box<dyn coder_v1::Engine>,
            "Coder V1 (recorded)".to_string(),
        ))
    });
    let minds: BTreeMap<String, (Plan, String, Arc<Mutex<Vec<_>>>)> = scripts
        .iter()
        .map(|(name, s)| {
            (
                (*name).to_string(),
                (s.plan.clone(), s.report.to_string(), s.asked.clone()),
            )
        })
        .collect();
    let mind: crate::task::agent_steer::MindFactory = Arc::new(move |record: &Record| {
        let (plan, report, asked) = minds[&record.name].clone();
        let judgment = Judgment {
            done: 0.95,
            unsupported: 0.02,
            next: Move::Continue,
            by: String::new(),
        };
        Ok(Mind {
            planner: Box::new(ScriptedPlanner {
                plan: Some(plan),
                report: Some(report),
                asked,
            }),
            judge: Some(Box::new(ScriptedJudge {
                judgments: vec![judgment].into(),
                states: Arc::default(),
            })),
            unjudged: String::new(),
        })
    });
    let agents = Agents::new(&root, dir.path().join("tasks"), BTreeMap::new())
        .with_engine(engine)
        .with_mind(mind)
        .with_coder_state(dir.path().join("coder"))
        .with_clock(clock);
    for name in ["alice", "bob"] {
        agents.create(name, &workspace, Some(&owner_key())).unwrap();
    }
    (agents, scripts)
}

fn ask(agents: &Agents, name: &str, text: &str) {
    agents
        .answer(
            &format!("{name}-1"),
            &owner(),
            &Operation::AskAgent {
                agent: name.into(),
                text: text.into(),
                workspace: None,
                context: String::new(),
                mode: Mode::Terminal,
                typist: false,
                computer: None,
            },
        )
        .unwrap();
}

fn finished(agents: &Agents, name: &str) -> wire::AgentView {
    let start = Instant::now();
    loop {
        let seen = agents
            .list()
            .agents
            .into_iter()
            .find(|a| a.name == name)
            .unwrap();
        if !seen.busy && !seen.headline.is_empty() {
            return seen;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "{seen:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn store(dir: &tempfile::TempDir, name: &str) -> Store {
    Store::new(&dir.path().join("host"), name).unwrap()
}

fn journal_text(dir: &tempfile::TempDir, name: &str) -> String {
    store(dir, name)
        .journal(500)
        .unwrap()
        .into_iter()
        .map(|e| e.text)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether `text` says `word` as a word.
fn says(text: &str, word: &str) -> bool {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|w| w == word)
}

#[test]
fn alice_and_bob_run_side_by_side_with_their_own_everything() {
    let dir = tempfile::tempdir().unwrap();
    let (agents, scripts) = crew(&dir);
    let (alice, bob) = (store(&dir, "alice"), store(&dir, "bob"));
    let (a, b) = (alice.load().unwrap().unwrap(), bob.load().unwrap().unwrap());

    // Each starts from its own preset.
    assert_eq!(a.charter, agent::DEFAULT_CHARTER);
    assert_eq!(a.look, "alice");
    assert_eq!(a.display_name(), "Alice");
    assert_eq!(b.charter, agent::preset("bob").unwrap().charter);
    assert_eq!(b.look, "bob");
    assert_eq!(b.display_name(), "Bob");
    assert_eq!(b.definition().pronouns, Some(agent::Pronouns::He));

    // Separate keys, each attested by the same owner.
    let (ka, kb) = (alice.key().unwrap().unwrap(), bob.key().unwrap().unwrap());
    assert_ne!(ka, kb);
    assert_eq!(a.pubkey.as_deref(), Some(agent::public_hex(&ka).as_str()));
    assert_eq!(b.pubkey.as_deref(), Some(agent::public_hex(&kb).as_str()));
    let owner_hex = agent::public_hex(&owner_key());
    assert_eq!(a.attestation.as_ref().unwrap().owner, owner_hex);
    assert_eq!(b.attestation.as_ref().unwrap().owner, owner_hex);

    // Separate engrams: each core names its own member and role.
    let screen = secret_screen::Screen::shapes();
    let core = |s: &Store| match EngramStore::open(s, &screen, clock()) {
        Opened::Ready(engrams) => engrams.core().unwrap().to_string(),
        other => panic!("{other:?}"),
    };
    assert!(core(&alice).starts_with("I am alice, my owner's workshop agent."));
    let bob_core = core(&bob);
    assert!(bob_core.starts_with("I am bob, my owner's town builder."));
    assert!(!bob_core.to_lowercase().contains("alice"));

    // A request to each runs its own loop in its own Coder session.
    ask(&agents, "alice", "run the atif tests");
    ask(&agents, "bob", "what villagers does the town have?");
    let (va, vb) = (finished(&agents, "alice"), finished(&agents, "bob"));
    assert_eq!(va.headline, "ok exit 0");
    assert_eq!(vb.headline, "ok exit 0");
    let sessions = |name: &str| -> Vec<(String, String)> {
        scripts[name]
            .turn
            .given
            .lock()
            .unwrap()
            .iter()
            .map(|t| (t.session.clone(), t.prompt.clone()))
            .collect()
    };
    assert_eq!(sessions("alice").len(), 1);
    assert_eq!(sessions("alice")[0].0, "alice-coder");
    assert!(sessions("alice")[0].1.starts_with("Run the atif tests."));
    assert_eq!(sessions("bob").len(), 1);
    assert_eq!(sessions("bob")[0].0, "bob-coder");
    assert!(
        sessions("bob")[0]
            .1
            .starts_with("List the townsfolk files.")
    );

    // Each plans from its own definition.
    let system = |name: &str| scripts[name].asked.lock().unwrap()[0].system.clone();
    assert!(system("alice").contains("You are alice, the owner's workshop agent"));
    assert!(!system("alice").contains("Your voice:"));
    let bob_system = system("bob");
    assert!(bob_system.contains("You are bob, the owner's town builder"));
    assert!(bob_system.contains("Your voice: Practical and concrete"));
    assert!(bob_system.contains("only in his own worktree"));

    // Separate journals.
    let (ja, jb) = (journal_text(&dir, "alice"), journal_text(&dir, "bob"));
    assert!(ja.contains("run the atif tests") && !ja.contains("villagers"));
    assert!(jb.contains("villagers") && !jb.contains("atif"));

    // Stopping Bob journals each step about him, and leaves Alice alone.
    agents.stop("bob", "end of the day", "owner").unwrap();
    let jb = journal_text(&dir, "bob");
    assert!(jb.contains("stop 4 of 4: he holds no grants"), "{jb}");
    assert!(
        jb.contains("the owner set him up at his workstation"),
        "{jb}"
    );
    assert_eq!(alice.load().unwrap().unwrap().state, agent::State::Active);
    assert!(journal_text(&dir, "alice").contains("the owner set her up at her workstation"));

    // Nothing Bob's says Alice or calls him her.
    let reports: Vec<String> = agents
        .reports()
        .into_iter()
        .filter(|r| r.headline.starts_with("bob"))
        .map(|r| format!("{} {}", r.headline, r.text))
        .collect();
    assert_eq!(reports.len(), 1, "{reports:?}");
    let profile = std::fs::read_to_string(bob.dir().join("profile.json")).unwrap_or_default();
    for text in [
        jb.as_str(),
        &reports.join("\n"),
        &vb.lines.join("\n"),
        &profile,
    ] {
        assert!(!text.to_lowercase().contains("alice"), "{text}");
        assert!(!says(text, "her") && !says(text, "she"), "{text}");
    }
    assert!(profile.contains("Bob is a town builder. He answers only his owner."));
}

#[test]
fn a_member_without_a_preset_is_referred_to_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("work");
    std::fs::create_dir_all(workspace.join(".git")).unwrap();
    let agents = Agents::new(
        &dir.path().join("host"),
        dir.path().join("tasks"),
        BTreeMap::new(),
    )
    .with_clock(clock);
    agents
        .create("carol", &workspace, Some(&owner_key()))
        .unwrap();
    let carol = store(&dir, "carol");
    let record = carol.load().unwrap().unwrap();
    assert_eq!(record.look, "carol");
    assert!(!record.charter.split_whitespace().any(|w| w == "her"));
    agents.stop("carol", "test", "owner").unwrap();
    let text = journal_text(&dir, "carol");
    assert!(
        text.contains("the owner set Carol up at Carol's workstation"),
        "{text}"
    );
    assert!(
        text.contains("stop 4 of 4: Carol holds no grants"),
        "{text}"
    );
    assert!(!says(&text, "her") && !says(&text, "she"), "{text}");
}

#[test]
fn a_preset_makes_another_name_from_bobs_definition() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir, "bob-two");
    let record = store
        .open_as(dir.path(), clock(), agent::preset("bob"))
        .unwrap();
    assert_eq!(record.charter, agent::preset("bob").unwrap().charter);
    assert_eq!(record.display_name(), "Bob-two");
    assert_eq!(record.refer().they(), "he");
    // Opening it again keeps its record.
    let again = store.open(dir.path(), clock()).unwrap();
    assert_eq!(again, record);
}

#[test]
fn an_agent_key_counts_toward_its_owner_once_both_sides_link() {
    use nostr::domain::RelaySigner;
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join("work");
    std::fs::create_dir_all(workspace.join(".git")).unwrap();
    let agents = Agents::new(
        &dir.path().join("host"),
        dir.path().join("tasks"),
        BTreeMap::new(),
    )
    .with_clock(clock);
    agents
        .create("bob", &workspace, Some(&owner_key()))
        .unwrap();
    let bob = store(&dir, "bob");
    let owner_hex = agent::public_hex(&owner_key());

    // Another owner's key is refused.
    let stranger = agent::public_hex(&secp256k1::SecretKey::from_byte_array([9; 32]).unwrap());
    assert!(crate::task::agent_profile::xp_link(&bob, &stranger, clock()).is_err());

    let linked = crate::task::agent_profile::xp_link(&bob, &owner_hex, clock()).unwrap();
    assert_eq!(linked.link.pubkey, linked.key);
    assert_eq!(
        Some(linked.key.as_str()),
        bob.load().unwrap().unwrap().pubkey.as_deref()
    );

    // One side alone links nothing.
    let alone = knowledge::xp::Trainers::read(std::slice::from_ref(&linked.link));
    assert!(alone.linked.is_empty());

    // The owner's trainer profile lists the key: now it counts.
    let secret: String = owner_key()
        .secret_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let unsigned = nostr::xp::profile(&owner_hex, true, std::slice::from_ref(&linked.key)).unwrap();
    let profile = RelaySigner::from_secret_hex(&secret).unwrap().sign(
        clock(),
        unsigned.kind,
        unsigned.tags,
        unsigned.content,
    );
    let both = knowledge::xp::Trainers::read(&[profile, linked.link]);
    assert_eq!(both.linked.get(&linked.key), Some(&owner_hex));
}
