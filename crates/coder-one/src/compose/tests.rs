//! The composition on Terminal-Bench-shaped tasks with scripted executors:
//! no model, no network, virtual time.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::*;
use crate::judge::JevJudge;
use crate::policy::{Manifest, REFERENCE};
use crate::scripted::{Act, SCRIPT_SCHEMA, Script, Timed};
use crate::session::{Capabilities, Controls};
use crate::shell::Checkout;
use crate::state::{Environment, Issue};
use crate::stream::Format;

const INSTRUCTION: &str = "The file numbers.txt holds one integer per line.\n\
Write their sum to answer.json as a JSON object with the key sum.\n\
Run `python3 test_answer.py` to check your work.";

const TEST_ANSWER: &str = "import json\n\
total = sum(int(line) for line in open('numbers.txt') if line.strip())\n\
assert json.load(open('answer.json'))['sum'] == total, 'wrong sum'\n\
print('ok')\n";

struct NoGenerator;

impl Generate for NoGenerator {
    async fn generate(&mut self, _prompt: &str) -> Result<String, String> {
        Err("no explore steps".to_string())
    }
}

/// Hands out one script per dispatch, in order, and remembers what it
/// was asked to make.
struct Queue {
    scripts: VecDeque<Script>,
    workdir: PathBuf,
    artifacts: PathBuf,
    recorder: Recorder,
    made: Vec<(Tier, Duration)>,
}

impl Factory for Queue {
    fn make(&mut self, tier: &Tier, deadline: Duration, runs: u32) -> Result<Exec, String> {
        let script = self
            .scripts
            .pop_front()
            .ok_or_else(|| format!("no script left for {}", tier.label()))?;
        self.made.push((tier.clone(), deadline));
        let mut scripted = Scripted::new(script, self.workdir.clone());
        scripted.artifacts = Some(self.artifacts.clone());
        scripted.deadline = deadline;
        scripted.recorder = self.recorder.clone();
        scripted.runs = runs;
        scripted.controls = Controls {
            deadline_ms: u64::try_from(deadline.as_millis()).unwrap_or(u64::MAX),
            tick_ms: 100,
            ..Controls::default()
        };
        Ok(Exec::Scripted(Box::new(scripted)))
    }
}

fn script(name: &str, sum: i64, claimed_exit: i64) -> Script {
    let at = |at_ms: u64, act: Act| Timed { at_ms, act };
    Script {
        schema: SCRIPT_SCHEMA.to_string(),
        name: name.to_string(),
        format: Format::Codex,
        model: name.to_string(),
        capabilities: Capabilities::all(),
        events: vec![
            at(
                0,
                Act::Claim {
                    text: "Reading numbers.txt.".to_string(),
                },
            ),
            at(
                1_000,
                Act::Write {
                    path: "answer.json".to_string(),
                    content: format!("{{\"sum\": {sum}}}\n"),
                    announce: true,
                },
            ),
            at(
                2_000,
                Act::Command {
                    command: "python3 test_answer.py".to_string(),
                    output: "ok".to_string(),
                    exit_code: claimed_exit,
                },
            ),
            at(
                3_000,
                Act::Claim {
                    text: "Done: answer.json holds the sum.".to_string(),
                },
            ),
            at(3_000, Act::End { error: false }),
        ],
        on_steer: Vec::new(),
        on_resume: Vec::new(),
        rebind: Vec::new(),
        opening: true,
        briefed: None,
    }
}

fn manifest(file: &str) -> Manifest {
    let (_, text) = REFERENCE.iter().find(|(name, _)| *name == file).unwrap();
    Manifest::parse(text).unwrap()
}

fn workspace(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "coder-one-compose-{label}-{}-{}",
        std::process::id(),
        atif::now_ms()
    ));
    let work = dir.join("work");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::create_dir_all(dir.join("out/artifacts")).unwrap();
    std::fs::write(work.join("numbers.txt"), "1\n2\n3\n").unwrap();
    std::fs::write(work.join("test_answer.py"), TEST_ANSWER).unwrap();
    dir
}

fn easy() -> crate::profile::Profile {
    crate::profile::Profile {
        features: crate::profile::NOULS
            .iter()
            .map(|(id, _)| ((*id).to_string(), Some(0.1)))
            .collect::<BTreeMap<_, _>>(),
        difficulty: Some(0.25),
        executor: None,
        executor_probabilities: BTreeMap::new(),
    }
}

struct Ran {
    record: Value,
    made: Vec<(Tier, Duration)>,
    work: PathBuf,
    out: PathBuf,
}

async fn compose(
    label: &str,
    manifest: &Manifest,
    scripts: Vec<Script>,
    profile: Option<crate::profile::Profile>,
    total: Duration,
) -> Ran {
    let dir = workspace(label);
    let work = dir.join("work");
    let out = dir.join("out");
    let recorder = Recorder::default();
    let deadline = Deadline::new(Some(total), Duration::from_secs(5));
    let mut state = State::new(
        Environment {
            repository: String::new(),
            workdir: work.to_string_lossy().into_owned(),
            os: std::env::consts::OS.to_string(),
        },
        Issue {
            url: String::new(),
            title: "Sum the numbers".to_string(),
            body: INSTRUCTION.to_string(),
            labels: Vec::new(),
        },
    );
    let mut judge = JevJudge::new(None, work.clone(), &state.issue, recorder.clone());
    let mut shell = Checkout {
        workdir: work.clone(),
        deadline: Duration::from_secs(60),
        recorder: recorder.clone(),
        commands: 0,
        episode: Deadline::unbounded(),
    };
    let brief = &manifest.policy.brief;
    let plan = Plan {
        mode: Mode::Always,
        policy: delegate::Policy {
            explore_steps: 0,
            ..delegate::Policy::default()
        },
        max_steps: 0,
        prompt: "Complete this task.",
        instruction: INSTRUCTION,
        directions: brief.directions.text(),
        cap: brief.cap,
        packer: brief.packer,
        pack: brief.pack_params(),
        isolation: "a scratch directory",
        base: None,
    };
    let setup = Setup {
        manifest,
        instruction: INSTRUCTION,
        workdir: &work,
        dir: &out,
        recorder: &recorder,
        deadline: &deadline,
        jev: JevMode::Off,
        profile,
        base: None,
    };
    let mut factory = Queue {
        scripts: scripts.into(),
        workdir: work.clone(),
        artifacts: out.join("artifacts"),
        recorder: recorder.clone(),
        made: Vec::new(),
    };
    let composed = run(
        &setup,
        &mut state,
        &plan,
        &mut judge,
        &mut NoGenerator,
        &mut shell,
        &mut factory,
        &mut |_| {},
    )
    .await
    .unwrap();
    Ran {
        record: composed.record,
        made: factory.made,
        work,
        out,
    }
}

fn sum_in(work: &Path) -> i64 {
    let text = std::fs::read_to_string(work.join("answer.json")).unwrap();
    serde_json::from_str::<Value>(&text).unwrap()["sum"]
        .as_i64()
        .unwrap()
}

fn python() -> bool {
    crate::minitask::process::python().is_some()
}

#[tokio::test(flavor = "current_thread")]
async fn a_cheap_start_escalates_to_opus_when_the_public_check_fails() {
    if !python() {
        return;
    }
    let ran = compose(
        "escalate",
        &manifest("tunable.json"),
        // Luna writes the wrong sum and says its test passed.
        vec![script("luna", 5, 0), script("opus", 6, 0)],
        Some(easy()),
        Duration::from_secs(900),
    )
    .await;
    let record = &ran.record;
    assert_eq!(record["route"]["start"], "cheap", "{record:#}");
    let labels: Vec<String> = ran.made.iter().map(|(tier, _)| tier.label()).collect();
    assert_eq!(labels, ["codex/gpt-6-luna", "claude-code/claude-opus-5-5"]);
    assert_eq!(record["escalated"], true);
    assert!(
        record["handoffs"][0]["trigger"]
            .as_str()
            .unwrap()
            .contains("a check failed"),
        "{record:#}"
    );
    // The first check found the wrong sum; the one after the escalation
    // found none.
    assert_eq!(record["checks"][0]["summary"]["verdicts"]["failed"], 1);
    assert!(record["checks"][1]["summary"]["verdicts"]["failed"].is_null());
    assert_eq!(sum_in(&ran.work), 6);
    // Nothing was left for the repair.
    assert_eq!(record["repair"]["ran"], false);
    assert!(ran.out.join(ESCALATED_CHECKS).is_file());
    // Both dispatches drew from one deadline, the first asking for about
    // half of it so the escalation had room.
    let first = ran.made[0].1.as_secs();
    assert!((300..=600).contains(&first), "{first}");
}

#[tokio::test(flavor = "current_thread")]
async fn a_strong_start_repairs_once_from_the_packet() {
    if !python() {
        return;
    }
    let ran = compose(
        "repair",
        &manifest("tunable-opus.json"),
        vec![script("opus", 7, 0), script("opus-repair", 6, 0)],
        None,
        Duration::from_secs(900),
    )
    .await;
    let record = &ran.record;
    assert_eq!(record["route"], Value::Null);
    assert_eq!(record["first"]["start"], "manifest");
    assert_eq!(record["escalated"], false);
    assert_eq!(record["repair"]["ran"], true, "{record:#}");
    assert_eq!(record["repair"]["changed"], true);
    assert_eq!(record["repair"]["recheck"]["gaps"], json!([]));
    assert_eq!(sum_in(&ran.work), 6);
    let brief = std::fs::read_to_string(ran.out.join("artifacts/repair-1.brief.md")).unwrap();
    assert!(brief.contains(crate::repair::PACKET_MARK));
    assert!(brief.contains("python3 test_answer.py"));
}

#[tokio::test(flavor = "current_thread")]
async fn an_eight_hour_task_starts_strong_at_the_long_effort_with_hours_to_work() {
    if !python() {
        return;
    }
    let ran = compose(
        "long",
        &manifest("tunable.json"),
        vec![script("opus", 6, 0)],
        Some(easy()),
        Duration::from_secs(8 * 3600 - 120),
    )
    .await;
    let record = &ran.record;
    assert_eq!(record["route"]["start"], "strong");
    assert!(
        record["route"]["reason"]
            .as_str()
            .unwrap()
            .contains("long-task bound")
    );
    assert_eq!(record["horizon"]["long"], true);
    let (tier, deadline) = &ran.made[0];
    assert_eq!(tier.effort.as_deref(), Some("medium"));
    // Only a repair can follow a strong start, so it asks for three
    // quarters of the eight hours.
    assert!(deadline.as_secs() > 5 * 3600, "{deadline:?}");
    let command = record["horizon"]["command_sec"].as_u64().unwrap();
    assert!((470..=480).contains(&command), "{command}");
    assert_eq!(record["repair"]["ran"], false);
}

#[tokio::test(flavor = "current_thread")]
async fn a_planner_writes_the_plan_in_a_scratch_copy_before_the_worker() {
    let mut manifest = manifest("tunable-luna.json");
    let handoff = manifest.policy.control.handoff.as_mut().unwrap();
    handoff.pattern = Pattern::PlannerWorker;
    manifest.policy.verify = None;
    let mut planner = script("opus-planner", 0, 0);
    planner.events = vec![
        Timed {
            at_ms: 0,
            act: Act::Write {
                path: "answer.json".to_string(),
                content: "{\"sum\": 99}".to_string(),
                announce: true,
            },
        },
        Timed {
            at_ms: 1_000,
            act: Act::Claim {
                text: "1. Sum numbers.txt.\n2. Write answer.json.\nScenario: python3 test_answer.py passes.".to_string(),
            },
        },
        Timed {
            at_ms: 1_000,
            act: Act::End { error: false },
        },
    ];
    let ran = compose(
        "planner",
        &manifest,
        vec![planner, script("luna", 6, 0)],
        None,
        Duration::from_secs(900),
    )
    .await;
    let record = &ran.record;
    assert_eq!(record["planner"]["status"], "answered", "{record:#}");
    assert!(
        record["planner"]["plan"]
            .as_str()
            .unwrap()
            .contains("Scenario:")
    );
    assert_eq!(record["handoffs"][0]["action"], "plan");
    // The planner's write stayed in its scratch copy.
    assert_eq!(sum_in(&ran.work), 6);
    let roles: Vec<&str> = record["branches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["role"].as_str().unwrap())
        .collect();
    assert_eq!(roles, ["planner", "primary"]);
}

#[test]
fn the_route_starts_cheap_only_on_a_readable_easy_short_task() {
    let manifest = manifest("tunable.json");
    let route = manifest.policy.control.route.as_ref().unwrap();
    assert_eq!(decide(route, &easy(), Some(840)).start, "cheap");
    assert_eq!(
        decide(route, &crate::profile::Profile::default(), Some(840)).start,
        "strong"
    );
    let mut hard = easy();
    hard.difficulty = Some(0.75);
    assert_eq!(decide(route, &hard, Some(840)).start, "strong");
    let mut builds = easy();
    builds.features.insert("builds_code".to_string(), Some(0.9));
    assert!(
        decide(route, &builds, Some(840))
            .reason
            .contains("builds_code")
    );
    assert_eq!(decide(route, &easy(), Some(28_680)).start, "strong");
}

#[test]
fn the_horizon_scales_dispatches_and_checks_with_the_deadline() {
    let horizon = Horizon::default();
    assert_eq!(horizon.dispatch_sec(None, 600, 0.55), 600);
    assert_eq!(
        horizon.dispatch_sec(Some(Duration::from_secs(780)), 600, 0.55),
        429
    );
    // Never less than the minimum, never more than what is left.
    assert_eq!(
        horizon.dispatch_sec(Some(Duration::from_secs(400)), 600, 0.1),
        300
    );
    assert_eq!(
        horizon.dispatch_sec(Some(Duration::from_secs(200)), 600, 0.1),
        200
    );
    assert_eq!(
        horizon.dispatch_sec(Some(Duration::from_secs(28_000)), 600, 0.75),
        21_000
    );
    let (budget, command) = horizon.checks(Some(28_680));
    assert_eq!((budget.seconds, command), (1_434, 478));
    let (budget, command) = horizon.checks(Some(840));
    assert_eq!((budget.seconds, command), (180, 60));
}

#[test]
fn every_tunable_tier_is_listed_for_the_doctor() {
    let tiers = tiers(&manifest("tunable.json"));
    let agents: Vec<&str> = tiers.iter().map(|t| t.agent.as_str()).collect();
    assert!(agents.contains(&"codex") && agents.contains(&"claude-code"));
    assert!(tiers.iter().all(|t| t.version.is_some()));
    assert!(composes(&manifest("tunable-opus.json")));
    assert!(!composes(&manifest("jevprobe3-luna.json")));
}
