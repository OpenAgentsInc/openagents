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
    recorder: Recorder,
}

async fn compose(
    label: &str,
    manifest: &Manifest,
    scripts: Vec<Script>,
    profile: Option<crate::profile::Profile>,
    total: Duration,
) -> Ran {
    compose_task(label, manifest, scripts, profile, total, None).await
}

/// [`compose`] on a mini-task's instruction and files instead of the sum
/// task.
async fn compose_task(
    label: &str,
    manifest: &Manifest,
    scripts: Vec<Script>,
    profile: Option<crate::profile::Profile>,
    total: Duration,
    task: Option<&crate::minitask::MiniTask>,
) -> Ran {
    let dir = workspace(label);
    let work = dir.join("work");
    let instruction = match task {
        Some(task) => {
            std::fs::remove_dir_all(&work).unwrap();
            crate::minitask::setup(task, &work).unwrap();
            task.instruction
        }
        None => INSTRUCTION,
    };
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
            title: task.map_or("Sum the numbers", |t| t.id).to_string(),
            body: instruction.to_string(),
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
        instruction,
        directions: brief.directions.text(),
        cap: brief.cap,
        packer: brief.packer,
        pack: brief.pack_params(),
        isolation: "a scratch directory",
        base: None,
    };
    let setup = Setup {
        manifest,
        instruction,
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
        recorder,
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
async fn control_effort_picks_a_long_tasks_effort_and_records_why() {
    if !python() {
        return;
    }
    // With Jev off the features are unknown, and an unknown task runs at
    // the raised effort rather than risk a pass.
    let ran = compose(
        "effort",
        &manifest("tunable-v9.json"),
        vec![script("opus", 6, 0)],
        Some(easy()),
        Duration::from_secs(8 * 3600 - 120),
    )
    .await;
    let record = &ran.record;
    assert_eq!(record["effort"]["effort"], "xhigh", "{record:#}");
    assert_eq!(record["effort"]["jev"], "off");
    assert_eq!(record["effort"]["score"], Value::Null);
    assert_eq!(record["effort"]["workspace_files"], 2);
    assert_eq!(ran.made[0].0.effort.as_deref(), Some("xhigh"));
    assert!(ran.recorder.steps().iter().any(|step| {
        step.extensions
            .values()
            .any(|v| v.to_string().contains("effort xhigh"))
    }));

    // A short task keeps the horizon's effort and asks nothing.
    let short = compose(
        "effort-short",
        &manifest("tunable-v9.json"),
        vec![script("luna", 6, 0)],
        Some(easy()),
        Duration::from_secs(900),
    )
    .await;
    assert_eq!(short.record["effort"], Value::Null);
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

// ---------------------------------------------------------------------------
// v4: self-report, profile-v2 and families, and verify.second.
// ---------------------------------------------------------------------------

fn at(at_ms: u64, act: Act) -> Timed {
    Timed { at_ms, act }
}

/// A script that writes `sum` to answer.json and each of `extra`, runs the
/// public test, and ends with `claim`.
fn script_saying(name: &str, sum: i64, extra: &[&str], claim: &str) -> Script {
    let mut script = script(name, sum, 0);
    let mut events = vec![at(
        500,
        Act::Write {
            path: "answer.json".to_string(),
            content: format!("{{\"sum\": {sum}}}\n"),
            announce: true,
        },
    )];
    for (i, path) in extra.iter().enumerate() {
        events.push(at(
            600 + i as u64,
            Act::Write {
                path: (*path).to_string(),
                content: format!("{name}\n"),
                announce: true,
            },
        ));
    }
    events.push(at(
        2_000,
        Act::Command {
            command: "python3 test_answer.py".to_string(),
            output: "ok".to_string(),
            exit_code: 0,
        },
    ));
    events.push(at(
        3_000,
        Act::Claim {
            text: claim.to_string(),
        },
    ));
    events.push(at(3_000, Act::End { error: false }));
    script.events = events;
    script
}

/// v4 without its route, so the manifest's executor (lean Opus) starts,
/// and with verify.second's time floor lowered for a fifteen-minute test.
fn v4_unrouted() -> Manifest {
    let mut manifest = manifest("tunable-v4.json");
    manifest.policy.control.route = None;
    if let Some(second) = manifest
        .policy
        .verify
        .as_mut()
        .and_then(|v| v.second.as_mut())
    {
        second.min_remaining_sec = 60;
    }
    manifest
}

#[tokio::test(flavor = "current_thread")]
async fn a_self_reported_guess_triggers_the_repair_only_under_v4() {
    if !python() {
        return;
    }
    let claim = "Done. The task doesn't pin down the rounding, so I guessed at it.";
    let mut v4 = v4_unrouted();
    v4.policy.verify.as_mut().unwrap().second = None;
    let ran = compose(
        "self-report",
        &v4,
        vec![
            script_saying("opus", 6, &[], claim),
            script_saying("opus-repair", 6, &[], "Done: answer.json holds the sum."),
        ],
        None,
        Duration::from_secs(900),
    )
    .await;
    let record = &ran.record;
    let reported = &record["checks"][0]["self_report"];
    assert_eq!(reported["verdict"], "failed", "{record:#}");
    let signals: Vec<&str> = reported["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["signal"].as_str().unwrap())
        .collect();
    assert_eq!(signals, ["underdetermined", "guess"]);
    assert_eq!(record["repair"]["ran"], true, "{record:#}");
    let brief = std::fs::read_to_string(ran.out.join("artifacts/repair-1.brief.md")).unwrap();
    assert!(brief.contains("generic.self-report"), "{brief}");

    // The canary: the same session under the Opus arm without
    // verify.self_report certifies it, as it always has.
    let ran = compose(
        "self-report-canary",
        &manifest("tunable-opus.json"),
        vec![script_saying("opus", 6, &[], claim)],
        None,
        Duration::from_secs(900),
    )
    .await;
    assert_eq!(ran.record["checks"][0]["self_report"], Value::Null);
    assert_eq!(ran.record["repair"]["ran"], false);
    assert_eq!(ran.made.len(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn a_second_executor_that_checks_better_replaces_the_first_candidate() {
    if !python() {
        return;
    }
    let ran = compose(
        "second-wins",
        &v4_unrouted(),
        vec![
            script_saying("opus", 7, &["first.txt"], "Done."),
            // The repair changes nothing, so the check still fails.
            script_saying("opus-repair", 7, &["first.txt"], "Done."),
            script_saying("astra", 6, &["second.txt"], "Done."),
        ],
        None,
        Duration::from_secs(900),
    )
    .await;
    let record = &ran.record;
    let labels: Vec<String> = ran.made.iter().map(|(tier, _)| tier.label()).collect();
    assert_eq!(
        labels,
        [
            "claude-code/claude-opus-5-5",
            "claude-code/claude-opus-5-5",
            "codex/gpt-6-astra"
        ]
    );
    let second = &record["second"];
    assert_eq!(second["kept"], "second", "{record:#}");
    assert!(second["trigger"].as_str().unwrap().starts_with("failed"));
    assert_eq!(second["first"]["failed"], 1);
    assert_eq!(second["second"]["failed"], 0);
    assert_eq!(sum_in(&ran.work), 6);
    // The second executor ran on the original state, not on the first
    // candidate: the first's extra file is gone.
    assert!(!ran.work.join("first.txt").exists());
    assert!(ran.work.join("second.txt").is_file());
    assert_eq!(record["final_tier"]["model"], "gpt-6-astra");
    assert!(ran.out.join(SECOND_CHECKS).is_file());
    let roles: Vec<&str> = record["branches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["role"].as_str().unwrap())
        .collect();
    assert_eq!(roles, ["primary", "second"]);
}

#[tokio::test(flavor = "current_thread")]
async fn a_second_executor_that_checks_no_better_leaves_the_first_candidate() {
    if !python() {
        return;
    }
    let ran = compose(
        "second-loses",
        &v4_unrouted(),
        vec![
            script_saying("opus", 7, &["first.txt"], "Done."),
            script_saying("opus-repair", 7, &["first.txt"], "Done."),
            script_saying("astra", 5, &["second.txt"], "Done."),
        ],
        None,
        Duration::from_secs(900),
    )
    .await;
    let record = &ran.record;
    assert_eq!(record["second"]["kept"], "first", "{record:#}");
    assert_eq!(sum_in(&ran.work), 7);
    assert!(ran.work.join("first.txt").is_file());
    assert!(!ran.work.join("second.txt").exists());
    assert_eq!(record["final_tier"]["model"], "claude-opus-5-5");
}

#[tokio::test(flavor = "current_thread")]
async fn a_confirmed_result_runs_no_second_executor() {
    if !python() {
        return;
    }
    let ran = compose(
        "second-skipped",
        &v4_unrouted(),
        vec![script_saying("opus", 6, &[], "Done.")],
        None,
        Duration::from_secs(900),
    )
    .await;
    assert_eq!(ran.made.len(), 1);
    assert_eq!(
        ran.record["second"]["skipped"],
        "the checks confirmed the result"
    );
    assert_eq!(sum_in(&ran.work), 6);
}

fn with_difficulty(difficulty: f64) -> crate::profile::Profile {
    let mut profile = easy();
    profile.difficulty = Some(difficulty);
    profile
}

#[test]
fn under_profile_v2_a_long_deadline_lowers_the_bar_but_does_not_decide() {
    let manifest = manifest("tunable-v4.json");
    let route = manifest.policy.control.route.as_ref().unwrap();
    assert_eq!(route.rule, "profile-v2");
    let long = Some(28_680);
    let routed = decide(route, &with_difficulty(0.25), long);
    assert_eq!(routed.start, "cheap", "{}", routed.reason);
    assert!(routed.reason.contains("the deadline is long"));
    let routed = decide(route, &with_difficulty(0.4), long);
    assert_eq!(routed.start, "strong");
    assert!(routed.reason.contains("the bar for a long task"));
    // On a short task the ordinary bar holds.
    assert_eq!(
        decide(route, &with_difficulty(0.4), Some(840)).start,
        "cheap"
    );
    // profile-v1 is unchanged: the deadline alone decides.
    let v1 = manifest_route("tunable.json");
    assert_eq!(decide(&v1, &with_difficulty(0.25), long).start, "strong");
}

fn manifest_route(file: &str) -> RoutePolicy {
    manifest(file).policy.control.route.unwrap()
}

#[test]
fn the_family_table_picks_the_profile_that_passed_more() {
    let route = manifest_route("tunable-v4.json");
    let hard = with_difficulty(0.9);
    let cad = "I'd like you to output a STEP file in `/app/out.step` which contains the object described by the 2d schematic in `/app/schematic.png`.";
    let (routed, record) = decide_with_families(&route, &hard, Some(28_680), cad);
    assert_eq!(routed.start, "family", "{record:#}");
    assert_eq!(routed.tier.label(), "codex/gpt-6-astra");
    assert_eq!(record["family"], "cad-from-drawing");
    assert_eq!(record["picked"], "astra");
    // A family where the rule's Opus already passed more keeps it.
    let genomics =
        "You are provided with a region of genomic DNA representing the human ATRX locus.";
    let (routed, record) = decide_with_families(&route, &hard, Some(28_680), genomics);
    assert_eq!(routed.start, "strong");
    assert_eq!(record["family"], "genomics");
    assert!(record["picked"].is_null());
    // No family: the rule decides, and the record says why.
    let (routed, record) = decide_with_families(
        &route,
        &hard,
        Some(28_680),
        "Sum the numbers in numbers.txt.",
    );
    assert_eq!(routed.start, "strong");
    assert!(record["family"].is_null());
    // A route without a table records nothing.
    let (_, record) = decide_with_families(&manifest_route("tunable.json"), &hard, Some(840), cad);
    assert!(record.is_null());
}

#[test]
fn a_family_pick_needs_trials_and_a_gap() {
    let mut route = manifest_route("tunable-v4.json");
    let families = route.families.as_mut().unwrap();
    let cad = "Write a STEP file for the 2d schematic in schematic.png.";
    let strong = route.strong.clone();
    families.min_trials = 11;
    let (picked, record) = families.pick(cad, &strong);
    assert!(picked.is_none());
    assert!(record["why"].as_str().unwrap().contains("11 trials"));
    families.min_trials = 10;
    families.min_gap = 0.25;
    let (picked, record) = families.pick(cad, &strong);
    assert!(picked.is_none(), "{record:#}");
    assert!(
        record["why"]
            .as_str()
            .unwrap()
            .contains("less than the gap")
    );
}

#[test]
fn every_v4_tier_is_listed_for_the_doctor() {
    let tiers = tiers(&manifest("tunable-v4.json"));
    assert!(tiers.iter().any(|t| t.model == "gpt-6-astra"));
    assert!(tiers.iter().all(|t| t.version.is_some()));
    let verify = manifest("tunable-v4.json").policy.verify.unwrap();
    assert!(verify.validate().is_empty());
    assert_eq!(verify.support_params(true).max_requirements, 8);
    assert_eq!(verify.support_params(false).max_requirements, 3);
    assert_eq!(
        verify.support_params(true).order,
        crate::support::Order::BehaviorFirst
    );
    // v2's and v3's verify serialize as they always have.
    let v3: Value = serde_json::from_str(include_str!("../../policies/tunable-v3.json")).unwrap();
    let parsed: VerifyPolicy = serde_json::from_value(v3["policy"]["verify"].clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&parsed).unwrap(),
        v3["policy"]["verify"]
    );
    assert_eq!(
        parsed.support_params(true),
        crate::support::Params::default()
    );
    assert!(parsed.check_options().is_default());
}

#[test]
fn a_standing_prefers_fewer_failures_then_more_confirmed_requirements() {
    let standing = |failed, contradicted, confirmed| Standing {
        failed,
        failed_checks: failed,
        self_reported: 0,
        contradicted,
        confirmed,
        passed_scenarios: 1,
        unresolved: 0,
        verdict_fail: false,
    };
    assert!(standing(1, 0, 3).beaten_by(&standing(0, 0, 1)));
    assert!(standing(0, 0, 1).beaten_by(&standing(0, 0, 2)));
    assert!(!standing(0, 0, 2).beaten_by(&standing(0, 0, 2)));
    assert!(!standing(0, 0, 2).beaten_by(&standing(0, 1, 5)));
    let on = vec!["unconfirmed".to_string(), "failed".to_string()];
    assert!(standing(0, 0, 2).triggers(&on).is_empty());
    let mut unconfirmed = standing(0, 0, 0);
    unconfirmed.passed_scenarios = 0;
    assert_eq!(unconfirmed.triggers(&on).len(), 1);
    assert!(
        standing(1, 0, 0)
            .triggers(&["unconfirmed".to_string()])
            .is_empty()
    );
}

// ---------------------------------------------------------------------------
// v5: control.persist.
// ---------------------------------------------------------------------------

const EIGHT_HOURS: Duration = Duration::from_secs(8 * 3600 - 120);

/// v5 without its route or its second executor, so lean Opus starts and
/// every dispatch after the first is the repair or a persist round.
fn v5_unrouted() -> Manifest {
    let mut manifest = manifest("tunable-v5.json");
    manifest.policy.control.route = None;
    manifest.policy.verify.as_mut().unwrap().second = None;
    manifest
}

/// A session that reads, says it is done, and changes nothing.
fn idle(name: &str) -> Script {
    let mut script = script(name, 0, 0);
    script.events = vec![
        at(
            0,
            Act::Claim {
                text: "My tests pass; nothing to change.".to_string(),
            },
        ),
        at(100, Act::End { error: false }),
    ];
    script
}

fn log_task() -> crate::minitask::MiniTask {
    crate::minitask::find("log-severity").unwrap()
}

fn log_script(which: &str) -> Script {
    crate::minitask::scripts(&log_task())
        .into_iter()
        .find(|(name, _)| *name == which)
        .unwrap()
        .1
}

fn roles(record: &Value) -> Vec<String> {
    record["branches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["role"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn persist_fixes_a_subtly_wrong_answer_that_v4_leaves() {
    if !python() {
        return;
    }
    let task = log_task();
    // The first session counts any severity word on a line, not the
    // severity field; the repair makes the same mistake; the first persist
    // round writes its own tests and fixes it; the second changes nothing.
    let ran = compose_task(
        "persist-fixes",
        &v5_unrouted(),
        vec![
            log_script("bad"),
            log_script("bad"),
            log_script("good"),
            idle("opus-idle"),
        ],
        None,
        EIGHT_HOURS,
        Some(&task),
    )
    .await;
    let record = &ran.record;
    let grade = crate::minitask::grade(&task, &ran.work, &ran.out.join("grade")).await;
    assert_eq!(grade.verdict, "passed", "{grade:?}\n{record:#}");
    assert_eq!(record["repair"]["changed"], false, "{record:#}");
    let persist = &record["persist"];
    assert_eq!(persist["stopped"], "round 2 changed nothing", "{record:#}");
    let rounds = persist["rounds"].as_array().unwrap();
    assert_eq!(rounds.len(), 2);
    assert_eq!(rounds[0]["changed"], true);
    assert_eq!(rounds[0]["kept"], true);
    assert_eq!(rounds[0]["before"]["failed"], 1);
    assert_eq!(rounds[0]["after"]["failed"], 0);
    assert!(
        rounds[0]["files_changed"]
            .as_array()
            .unwrap()
            .contains(&json!({ "path": "summary.csv", "change": "modified" }))
    );
    assert_eq!(rounds[1]["changed"], false);
    assert_eq!(roles(record), ["primary", "persist-1", "persist-2"]);
    // Each round is a fresh session at the long effort.
    assert_eq!(rounds[0]["tier"]["effort"], "xhigh");
    assert_ne!(rounds[0]["session_id"], rounds[1]["session_id"]);
    assert_eq!(
        record["checks"].as_array().unwrap().last().unwrap()["after"],
        "persist-1"
    );
    assert!(ran.out.join(persist::checks_file(1)).is_file());
    // The brief is built by code from the task, the requirement states,
    // the changes, the packets, and the previous report.
    let brief = std::fs::read_to_string(ran.out.join(persist::brief_path(1))).unwrap();
    for part in [
        "The severity levels to count are exactly",
        "## Requirements and what the host's checks know about them",
        "- `summary.csv`: added",
        "It observed:",
        "## The previous session's final report",
        "the CSV has the header and 9 integer rows",
        "Write your own rigorous tests",
        "anything under /tests",
    ] {
        assert!(brief.contains(part), "{part} missing from\n{brief}");
    }
    // One control.persist invocation, with each round a child and each
    // round's session a child of the round.
    let invocations = crate::record::invocations(&ran.recorder.steps());
    let parent = invocations
        .iter()
        .find(|i| i.component == persist::COMPONENT && i.name.as_deref() == Some("persist"))
        .unwrap();
    let children: Vec<&crate::record::Invocation> = invocations
        .iter()
        .filter(|i| i.parent.as_deref() == Some(parent.id.as_str()))
        .collect();
    assert_eq!(children.len(), 2);
    assert!(children.iter().all(|c| c.component == persist::COMPONENT));
    assert!(
        invocations.iter().any(|i| i.component == "exec.session"
            && i.parent.as_deref() == Some(children[0].id.as_str()))
    );

    // The canary: v4, without control.persist, stops after the repair
    // with the wrong counts.
    let mut v4 = v4_unrouted();
    v4.policy.verify.as_mut().unwrap().second = None;
    let ran = compose_task(
        "persist-canary",
        &v4,
        vec![log_script("bad"), log_script("bad")],
        None,
        EIGHT_HOURS,
        Some(&task),
    )
    .await;
    let grade = crate::minitask::grade(&task, &ran.work, &ran.out.join("grade")).await;
    assert_eq!(grade.verdict, "failed");
    assert_eq!(ran.record["persist"], Value::Null);
    assert_eq!(ran.made.len(), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn persist_alternates_executors_up_to_the_round_cap() {
    if !python() {
        return;
    }
    let mut manifest = v5_unrouted();
    let policy = manifest.policy.control.persist.as_mut().unwrap();
    policy.max_rounds = 2;
    policy.alternate = vec![Tier::new("codex", "gpt-6-astra")];
    let ran = compose(
        "persist-alternate",
        &manifest,
        vec![
            script_saying("opus", 7, &[], "Done."),
            script_saying("opus-repair", 7, &[], "Done."),
            // Still wrong, but no worse: the round stays.
            script_saying("opus-persist", 8, &[], "Done."),
            script_saying("astra-persist", 6, &[], "Done."),
        ],
        None,
        EIGHT_HOURS,
    )
    .await;
    let record = &ran.record;
    let labels: Vec<String> = ran.made.iter().map(|(tier, _)| tier.label()).collect();
    assert_eq!(
        labels,
        [
            "claude-code/claude-opus-5-5",
            "claude-code/claude-opus-5-5",
            "claude-code/claude-opus-5-5",
            "codex/gpt-6-astra"
        ],
        "{record:#}"
    );
    let persist = &record["persist"];
    assert_eq!(
        persist["stopped"], "reached the cap of 2 rounds",
        "{record:#}"
    );
    assert_eq!(persist["rounds"][0]["kept"], true);
    assert_eq!(persist["rounds"][1]["after"]["failed"], 0);
    assert_eq!(sum_in(&ran.work), 6);
    assert_eq!(record["final_tier"]["model"], "gpt-6-astra");
    // The rounds drew on the one episode deadline: each asked half of
    // what was left, never more than the whole.
    let asked: Vec<u64> = ran.made.iter().map(|(_, d)| d.as_secs()).collect();
    assert!(asked[2] <= asked[0], "{asked:?}");
}

#[tokio::test(flavor = "current_thread")]
async fn persist_puts_back_a_round_whose_checks_come_out_worse() {
    if !python() {
        return;
    }
    let mut manifest = v5_unrouted();
    manifest
        .policy
        .control
        .persist
        .as_mut()
        .unwrap()
        .stop_when_confirmed = false;
    let ran = compose(
        "persist-guard",
        &manifest,
        vec![
            script_saying("opus", 6, &[], "Done."),
            script_saying("opus-persist", 5, &["scratch.txt"], "Done."),
            idle("opus-idle"),
        ],
        None,
        EIGHT_HOURS,
    )
    .await;
    let record = &ran.record;
    let round = &record["persist"]["rounds"][0];
    assert_eq!(round["kept"], false, "{record:#}");
    assert!(
        round["why"]
            .as_str()
            .unwrap()
            .contains("put the workspace back")
    );
    assert_eq!(sum_in(&ran.work), 6);
    assert!(!ran.work.join("scratch.txt").exists());
    assert_eq!(record["persist"]["stopped"], "round 2 changed nothing");
    // The final checks are the ones from before the round that was put
    // back.
    assert!(record["final_checks"]["verdicts"]["failed"].is_null());
}

#[tokio::test(flavor = "current_thread")]
async fn persist_skips_a_short_task_and_an_episode_near_its_deadline() {
    if !python() {
        return;
    }
    // Fifteen minutes is not a long task.
    let ran = compose(
        "persist-short",
        &v5_unrouted(),
        vec![script_saying("opus", 7, &[], "Done."), idle("repair")],
        None,
        Duration::from_secs(900),
    )
    .await;
    assert_eq!(ran.record["persist"]["skipped"], "not a long task");
    assert_eq!(ran.made.len(), 2);
    // A long task whose deadline has less than the floor left.
    let mut manifest = v5_unrouted();
    manifest
        .policy
        .control
        .persist
        .as_mut()
        .unwrap()
        .min_remaining_sec = 9 * 3600;
    let ran = compose(
        "persist-floor",
        &manifest,
        vec![script_saying("opus", 7, &[], "Done."), idle("repair")],
        None,
        EIGHT_HOURS,
    )
    .await;
    assert_eq!(
        ran.record["persist"]["stopped"],
        "less than 32400 s left in the episode"
    );
    assert_eq!(ran.record["persist"]["rounds"], json!([]));
    assert_eq!(ran.made.len(), 2);
}

#[test]
fn a_persist_round_is_gated_on_confirmation_of_every_binding_requirement() {
    use crate::requirements::Binding;
    let map = crate::requirements::mechanical(INSTRUCTION);
    let binding: Vec<String> = map
        .requirements
        .iter()
        .filter(|r| r.binding != Binding::Uncertain)
        .map(|r| r.id.clone())
        .collect();
    assert!(!binding.is_empty());
    let report = |states: &[(&str, &str)], failed: bool| {
        let mut report: checks::Report = serde_json::from_value(json!({
            "schema": "x",
            "implementation": checks::implementation(),
            "candidate": { "digest": "c1" },
            "requirements_method": "rule",
            "ineligible": [],
            "scenarios": [],
            "selection": null,
            "verdicts": [],
            "coverage": [],
            "packets": [],
        }))
        .unwrap();
        for (id, state) in states {
            report.coverage.push(checks::Covered {
                id: (*id).to_string(),
                text: String::new(),
                kind: "behavior".to_string(),
                state: (*state).to_string(),
                scenarios: Vec::new(),
            });
        }
        if failed {
            report.verdicts.push(
                serde_json::from_value(json!({ "scenario": "generic.output", "verdict": "failed", "observations": [], "coverage": [] }))
                    .unwrap(),
            );
        }
        report
    };
    let all: Vec<(&str, &str)> = binding.iter().map(|id| (id.as_str(), "observed")).collect();
    assert!(persist::confirmed(Some(&map), &report(&all, false), None));
    assert!(!persist::confirmed(Some(&map), &report(&all, true), None));
    assert!(!persist::confirmed(
        Some(&map),
        &report(&all[1..], false),
        None
    ));
    // A map with no binding requirement confirms nothing.
    let mut empty = map.clone();
    empty.requirements.clear();
    assert!(!persist::confirmed(
        Some(&empty),
        &report(&all, false),
        None
    ));
}

#[test]
fn persist_cycles_the_producing_executor_with_its_alternates() {
    let opus = Tier::new("claude-code", "claude-opus-5-5");
    let astra = Tier::new("codex", "gpt-6-astra");
    let mut policy = PersistPolicy::default();
    assert_eq!(policy.tier_for(1, &opus), opus);
    assert_eq!(policy.tier_for(3, &opus), opus);
    policy.alternate = vec![astra.clone(), opus.clone()];
    let labels: Vec<String> = (1..=4).map(|n| policy.tier_for(n, &opus).label()).collect();
    assert_eq!(
        labels,
        [
            "claude-code/claude-opus-5-5",
            "codex/gpt-6-astra",
            "claude-code/claude-opus-5-5",
            "codex/gpt-6-astra"
        ]
    );
    // A candidate Astra produced alternates with nothing new.
    assert_eq!(policy.tier_for(2, &astra), opus);
}

#[test]
fn the_v5_manifest_is_v4_plus_persist_and_v4_keeps_its_digest() {
    let v5 = manifest("tunable-v5.json");
    v5.validate().unwrap();
    let persist = v5.policy.control.persist.as_ref().unwrap();
    assert_eq!(persist.max_rounds, 3);
    assert_eq!(persist.min_remaining_sec, 1_800);
    assert!(persist.long_only && persist.guard && persist.alternate.is_empty());
    let mut stripped = v5.policy.clone();
    stripped.control.persist = None;
    assert_eq!(stripped, manifest("tunable-v4.json").policy);
    // v4 serializes without the field, so its digest is what it was.
    let v4: Value = serde_json::from_str(include_str!("../../policies/tunable-v4.json")).unwrap();
    assert!(v4["policy"]["control"].get("persist").is_none());
    let parsed = manifest("tunable-v4.json");
    assert_eq!(
        serde_json::to_value(&parsed.policy.control).unwrap(),
        v4["policy"]["control"]
    );
    // A bad policy is refused.
    let mut bad = v5.clone();
    let policy = bad.policy.control.persist.as_mut().unwrap();
    policy.max_rounds = 0;
    policy.share = 1.5;
    let problems = bad.validate().unwrap_err();
    assert!(problems.contains("max_rounds"), "{problems}");
    assert!(problems.contains("share"), "{problems}");
    let mut unlong = v5.clone();
    unlong.policy.control.horizon = None;
    assert!(unlong.validate().unwrap_err().contains("long_after_sec"));
    // An alternate executor is listed for the doctor.
    let mut alternating = v5;
    alternating
        .policy
        .control
        .persist
        .as_mut()
        .unwrap()
        .alternate = vec![Tier::new("codex", "gpt-6-astra")];
    assert!(tiers(&alternating).iter().any(|t| t.model == "gpt-6-astra"));
}

/// A session the subscription limit throttled: a retained Claude Code
/// stream replayed, then the CLI's exit 1.
fn throttled(name: &str) -> Script {
    let mut script = script(name, 0, 0);
    script.format = Format::Claude;
    script.opening = false;
    script.events = crate::delegate::tests::THROTTLED
        .lines()
        .map(|line| {
            at(
                0,
                Act::Raw {
                    line: line.to_string(),
                },
            )
        })
        .chain([at(10, Act::Exit { code: 1 })])
        .collect();
    script
}

#[tokio::test(flavor = "current_thread")]
async fn a_throttled_first_session_stops_the_composition() {
    // v5 would repair, run a second executor, and persist; none of them
    // runs on an exhausted quota.
    let ran = compose(
        "usage-limit-first",
        &manifest("tunable-v5.json"),
        vec![throttled("opus"), idle("never-1"), idle("never-2")],
        None,
        EIGHT_HOURS,
    )
    .await;
    let record = &ran.record;
    assert_eq!(ran.made.len(), 1, "{record:#}");
    assert_eq!(roles(record), ["primary"]);
    assert_eq!(record["branches"][0]["status"], "refused");
    let limit = &record["usage_limited"];
    assert_eq!(limit["provider"], "anthropic", "{record:#}");
    assert_eq!(limit["resets_at"], 1_790_164_200);
    assert_eq!(record["repair"], Value::Null);
    assert_eq!(record["second"], Value::Null);
    assert_eq!(record["persist"], Value::Null);
    assert!(record["checks"].as_array().unwrap().is_empty());
    assert_eq!(
        crate::limit::from_steps(&ran.recorder.steps()).unwrap()["resets_at"],
        1_790_164_200
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_throttled_persist_round_ends_persist_and_the_composition() {
    if !python() {
        return;
    }
    let task = log_task();
    let ran = compose_task(
        "usage-limit-persist",
        &v5_unrouted(),
        vec![
            log_script("bad"),
            log_script("bad"),
            throttled("opus-persist"),
            idle("never"),
        ],
        None,
        EIGHT_HOURS,
        Some(&task),
    )
    .await;
    let record = &ran.record;
    assert_eq!(ran.made.len(), 3, "{record:#}");
    assert_eq!(roles(record), ["primary", "persist-1"]);
    let persist = &record["persist"];
    assert_eq!(persist["stopped"], "round 1's session hit a usage limit");
    assert_eq!(
        persist["rounds"][0]["usage_limit"]["resets_at"],
        1_790_164_200
    );
    assert_eq!(record["usage_limited"]["provider"], "anthropic");
}

// ---------------------------------------------------------------------------
// v8: persistence that stops when it stops helping, and runs cheaper.
// ---------------------------------------------------------------------------

/// A runner path outside the workspace, as `/tmp/persist-tests/run.sh` is
/// in a task container.
fn runner_path(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "coder-one-persist-tests-{label}-{}-{}",
        std::process::id(),
        atif::now_ms()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("run.sh")
}

/// v8 without its route or its second executor, with its runner at
/// `runner` and the confirmation gate off, so the ladder alone decides.
fn v8_unrouted(runner: &Path) -> Manifest {
    let mut manifest = manifest("tunable-v8.json");
    manifest.policy.control.route = None;
    manifest.policy.verify.as_mut().unwrap().second = None;
    let persist = manifest.policy.control.persist.as_mut().unwrap();
    persist.own_tests.as_mut().unwrap().runner = runner.display().to_string();
    persist.stop_when_confirmed = false;
    manifest
}

/// The runner the sessions keep: `sum` passes when answer.json holds 6,
/// and `notes` when notes.txt says ok.
const RUNNER: &str = "if grep -q '\"sum\": 6' answer.json; then echo 'PASS sum'; else echo 'FAIL sum'; fi\n\
if grep -q ok notes.txt 2>/dev/null; then echo 'PASS notes'; else echo 'FAIL notes'; fi\n";

/// `script` that also writes the runner first, as a persist round's
/// executor would.
fn with_runner(mut script: Script, runner: &Path) -> Script {
    let dir = runner.parent().unwrap().display().to_string();
    script.events.insert(
        0,
        at(
            100,
            Act::Run {
                command: format!(
                    "mkdir -p '{dir}' && cat > '{}' <<'RUNNER'\n{RUNNER}RUNNER",
                    runner.display()
                ),
            },
        ),
    );
    script
}

#[tokio::test(flavor = "current_thread")]
async fn a_cheap_round_without_progress_escalates_and_opus_finishes() {
    if !python() {
        return;
    }
    let runner = runner_path("ladder");
    let ran = compose(
        "persist-ladder",
        &v8_unrouted(&runner),
        vec![
            script_saying("opus", 7, &[], "Done."),
            script_saying("opus-repair", 7, &[], "Done."),
            // Round 1, Opus: writes its tests and a still-wrong answer.
            with_runner(script_saying("opus-persist", 8, &[], "Done."), &runner),
            // Round 2, Sol: another wrong answer, no test fixed.
            script_saying("sol-persist", 9, &[], "Done."),
            // Round 3, Opus again: fixes it.
            script_saying("opus-escalated", 6, &[], "Done."),
            // Round 4, Sol: changes nothing, and no escalation is left.
            idle("sol-idle"),
        ],
        None,
        EIGHT_HOURS,
    )
    .await;
    let record = &ran.record;
    let labels: Vec<String> = ran.made.iter().map(|(tier, _)| tier.label()).collect();
    assert_eq!(
        labels,
        [
            "claude-code/claude-opus-5-5",
            "claude-code/claude-opus-5-5",
            "claude-code/claude-opus-5-5",
            "codex/gpt-6-sol",
            "claude-code/claude-opus-5-5",
            "codex/gpt-6-sol",
        ],
        "{record:#}"
    );
    assert_eq!(sum_in(&ran.work), 6);
    let persist = &record["persist"];
    assert_eq!(
        persist["stopped"],
        "round 4 changed nothing; round 4 on a cheap executor made no progress, and no escalation is left",
        "{record:#}"
    );
    let rounds = persist["rounds"].as_array().unwrap();
    let classes: Vec<&str> = rounds
        .iter()
        .map(|r| r["class"].as_str().unwrap())
        .collect();
    assert_eq!(classes, ["strong", "cheap", "escalated", "cheap"]);
    // Round 1 set the tests' baseline; round 2 fixed none and escalated;
    // round 3 fixed the sum.
    assert_eq!(rounds[0]["delta"]["tests_added"], 2);
    assert_eq!(rounds[0]["delta"]["progress"], Value::Null);
    assert_eq!(rounds[0]["tests"]["failing"], json!(["notes", "sum"]));
    assert_eq!(rounds[1]["delta"]["tests_fixed"], 0);
    assert_eq!(rounds[1]["delta"]["progress"], false);
    assert_eq!(rounds[1]["next"], "escalate");
    assert_eq!(rounds[1]["executor"], "codex/gpt-6-sol");
    // The cheap tier keeps its own effort; Opus runs at the long effort.
    assert_eq!(rounds[1]["tier"]["effort"], "high");
    assert_eq!(rounds[2]["tier"]["effort"], "xhigh");
    assert_eq!(rounds[2]["delta"]["tests_fixed"], 1);
    assert_eq!(rounds[2]["delta"]["progress"], true);
    assert_eq!(rounds[2]["after"]["failed"], 0);
    assert!(rounds.iter().all(|r| r.get("cost_usd").is_some()));
    assert_eq!(persist["totals"]["tests_fixed"], 1);
    assert_eq!(persist["totals"]["escalations"], 1);
    // A scripted session costs nothing, so the cap never binds here.
    assert_eq!(persist["spend"]["cap_usd"], 5.0);
    // Round 2's brief lists the failing tests and the runner's contract.
    let brief = std::fs::read_to_string(ran.out.join(persist::brief_path(2))).unwrap();
    for part in [
        "## The earlier sessions' own tests",
        "0 passed, 2 failed",
        "- failing: sum",
        "`PASS <name>` or `FAIL <name>`",
    ] {
        assert!(brief.contains(part), "{part} missing from\n{brief}");
    }
    assert_eq!(record["schema"], SCHEMA);
    let _ = std::fs::remove_dir_all(runner.parent().unwrap());
}

#[tokio::test(flavor = "current_thread")]
async fn a_round_that_moves_no_test_or_check_ends_the_rounds() {
    if !python() {
        return;
    }
    let runner = runner_path("no-progress");
    let mut manifest = v8_unrouted(&runner);
    manifest.policy.control.persist.as_mut().unwrap().cheap = None;
    let ran = compose(
        "persist-no-progress",
        &manifest,
        vec![
            script_saying("opus", 7, &[], "Done."),
            script_saying("opus-repair", 7, &[], "Done."),
            with_runner(script_saying("opus-persist-1", 8, &[], "Done."), &runner),
            // Changes the answer, but fixes no test and no check.
            script_saying("opus-persist-2", 9, &[], "Done."),
            script_saying("never", 6, &[], "Done."),
        ],
        None,
        EIGHT_HOURS,
    )
    .await;
    let record = &ran.record;
    assert_eq!(ran.made.len(), 4, "{record:#}");
    let persist = &record["persist"];
    assert_eq!(
        persist["stopped"], "round 2 changed no test or check outcome",
        "{record:#}"
    );
    assert_eq!(persist["rounds"][1]["changed"], true);
    assert_eq!(persist["rounds"][1]["delta"]["progress"], false);
    let _ = std::fs::remove_dir_all(runner.parent().unwrap());
}

#[tokio::test(flavor = "current_thread")]
async fn a_round_that_breaks_its_own_tests_is_put_back() {
    if !python() {
        return;
    }
    let runner = runner_path("broken");
    let ran = compose(
        "persist-broken",
        &v8_unrouted(&runner),
        vec![
            script_saying("opus", 6, &[], "Done."),
            // Round 1: notes.txt says ok, and every own test passes.
            with_runner(script_saying("ok", 6, &["notes.txt"], "Done."), &runner),
            // Round 2: breaks the notes test, which no check reads.
            script_saying("sol-persist", 6, &["notes.txt"], "Done."),
            // Round 3, escalated: changes nothing.
            idle("opus-idle"),
        ],
        None,
        EIGHT_HOURS,
    )
    .await;
    let record = &ran.record;
    let rounds = record["persist"]["rounds"].as_array().unwrap();
    assert_eq!(rounds.len(), 3, "{record:#}");
    assert_eq!(rounds[0]["tests"]["failed"], 0, "{record:#}");
    assert_eq!(rounds[1]["kept"], false, "{record:#}");
    assert_eq!(rounds[1]["delta"]["tests_broken"], 1);
    assert!(
        rounds[1]["why"]
            .as_str()
            .unwrap()
            .contains("broke 1 of its own tests and fixed 0")
    );
    assert_eq!(rounds[1]["next"], "escalate");
    assert_eq!(
        std::fs::read_to_string(ran.work.join("notes.txt")).unwrap(),
        "ok\n"
    );
    assert_eq!(
        record["persist"]["stopped"],
        "round 3 changed nothing; round 3 on the strong executor made no progress"
    );
    let _ = std::fs::remove_dir_all(runner.parent().unwrap());
}

#[test]
fn the_v8_manifest_is_v7_with_a_cheaper_ladder() {
    let v8 = manifest("tunable-v8.json");
    v8.validate().unwrap();
    let persist = v8.policy.control.persist.as_ref().unwrap();
    assert_eq!(persist.max_rounds, 4);
    assert!(persist.stop_when_no_progress);
    assert_eq!(
        persist.own_tests.as_ref().unwrap().runner,
        "/tmp/persist-tests/run.sh"
    );
    let cheap = persist.cheap.as_ref().unwrap();
    assert_eq!(cheap.tiers[0].label(), "codex/gpt-6-sol");
    assert_eq!((cheap.from_round, cheap.max_escalations), (2, 1));
    assert_eq!(persist.spend.as_ref().unwrap().budget_usd, Some(10.0));
    // Everything else is v7's.
    let v7 = Manifest::parse(include_str!("../../policies/tunable-v7.json")).unwrap();
    let mut stripped = v8.policy.clone();
    let p = stripped.control.persist.as_mut().unwrap();
    p.max_rounds = 2;
    p.own_tests = None;
    p.stop_when_no_progress = false;
    p.cheap = None;
    p.spend = None;
    assert_eq!(stripped, v7.policy);
    // v7 serializes without the new fields, so its digest is what it was.
    let raw: Value = serde_json::from_str(include_str!("../../policies/tunable-v7.json")).unwrap();
    assert_eq!(
        serde_json::to_value(&v7.policy.control).unwrap(),
        raw["policy"]["control"]
    );
    // The doctor sees the cheap tier.
    assert!(tiers(&v8).iter().any(|t| t.model == "gpt-6-sol"));
}

// ---------------------------------------------------------------------------
// v9-escalate: verify.second on a failed check or a self-reported failure.
// ---------------------------------------------------------------------------

/// v9-escalate without its route, so the manifest's executor (lean Opus)
/// starts, and with verify.second's time floor lowered for a
/// fifteen-minute test.
fn escalate_unrouted() -> Manifest {
    let mut manifest = manifest("tunable-v9-escalate.json");
    manifest.policy.control.route = None;
    manifest
        .policy
        .verify
        .as_mut()
        .and_then(|v| v.second.as_mut())
        .unwrap()
        .min_remaining_sec = 60;
    manifest
}

fn standing(failed_checks: usize, self_reported: usize, contradicted: usize) -> Standing {
    Standing {
        failed: failed_checks + self_reported,
        failed_checks,
        self_reported,
        contradicted,
        confirmed: 1,
        passed_scenarios: 1,
        unresolved: 0,
        verdict_fail: false,
    }
}

#[test]
fn escalation_fires_on_a_failed_check_or_a_self_report_and_nothing_else() {
    let on: Vec<String> = manifest("tunable-v9-escalate.json")
        .policy
        .verify
        .unwrap()
        .second
        .unwrap()
        .on;
    assert_eq!(on, ["check", "self_report"]);
    assert_eq!(standing(2, 0, 0).fired(&on), ["check"]);
    assert_eq!(standing(0, 1, 0).fired(&on), ["self_report"]);
    assert_eq!(standing(1, 1, 0).fired(&on), ["check", "self_report"]);
    assert_eq!(
        standing(2, 0, 0).triggers(&on),
        ["check: 2 scenario(s) failed"]
    );
    // A contradiction verify.support reads alone is not a failed check.
    assert!(standing(0, 0, 1).fired(&on).is_empty());
    // Nor is a result the checks can't confirm.
    let mut unconfirmed = standing(0, 0, 0);
    unconfirmed.passed_scenarios = 0;
    unconfirmed.confirmed = 0;
    unconfirmed.unresolved = 3;
    assert!(unconfirmed.fired(&on).is_empty());
    assert!(unconfirmed.triggers(&on).is_empty());
    // The older words mean what they did.
    let older = vec!["failed".to_string(), "unconfirmed".to_string()];
    assert_eq!(standing(0, 0, 1).fired(&older), ["failed"]);
    assert_eq!(unconfirmed.fired(&older), ["unconfirmed"]);
}

#[test]
fn the_better_candidate_is_the_one_whose_checks_fail_less() {
    // Fewer failures and contradictions win, whichever kind.
    assert!(standing(1, 0, 0).beaten_by(&standing(0, 0, 0)));
    assert!(standing(0, 1, 0).beaten_by(&standing(0, 0, 0)));
    assert!(standing(2, 1, 0).beaten_by(&standing(1, 0, 0)));
    // A second candidate that fails as much doesn't replace the first
    // unless it confirms more; one that fails more never does.
    assert!(!standing(1, 0, 0).beaten_by(&standing(0, 1, 0)));
    let mut confirms_more = standing(0, 1, 0);
    confirms_more.confirmed = 4;
    assert!(standing(1, 0, 0).beaten_by(&confirms_more));
    assert!(!standing(0, 0, 0).beaten_by(&standing(1, 0, 0)));
}

#[test]
fn the_escalate_manifest_is_v9_with_v7s_checks_and_an_astra_second() {
    let escalate = manifest("tunable-v9-escalate.json");
    escalate.validate().unwrap();
    let v9 = manifest("tunable-v9.json");
    assert_eq!(escalate.policy.control, v9.policy.control);
    assert_eq!(escalate.policy.executor, v9.policy.executor);
    assert_eq!(escalate.policy.brief, v9.policy.brief);
    let verify = escalate.policy.verify.as_ref().unwrap();
    let v7 = Manifest::parse(include_str!("../../policies/tunable-v7.json"))
        .unwrap()
        .policy
        .verify
        .unwrap();
    assert_eq!(
        (
            verify.self_report,
            verify.optional_outputs,
            verify.behavior,
            verify.support_budget
        ),
        (
            v7.self_report,
            v7.optional_outputs,
            v7.behavior,
            v7.support_budget
        )
    );
    // Escalation, not a repair, answers a failed check.
    assert!(v9.policy.verify.as_ref().unwrap().repair.is_some());
    assert_eq!(verify.repair, None);
    let second = verify.second.as_ref().unwrap();
    let labels: Vec<String> = second.to.iter().map(Tier::label).collect();
    assert_eq!(labels, ["codex/gpt-6-astra"]);
    assert!(tiers(&escalate).iter().any(|t| t.model == "gpt-6-astra"));

    // A self_report trigger needs the self-report scenario, and a word
    // outside the vocabulary is refused.
    let mut broken = verify.clone();
    broken.self_report = false;
    assert!(
        broken
            .validate()
            .iter()
            .any(|p| p.contains("self_report needs verify.self_report")),
        "{:?}",
        broken.validate()
    );
    let mut broken = verify.clone();
    broken.second.as_mut().unwrap().on = vec!["unsure".to_string()];
    assert!(
        broken
            .validate()
            .iter()
            .any(|p| p.contains("must be one of check, self_report, failed, unconfirmed"))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_failed_check_escalates_to_astra_and_the_better_candidate_stays() {
    if !python() {
        return;
    }
    let ran = compose(
        "escalate-check",
        &escalate_unrouted(),
        vec![
            script_saying("opus", 7, &["first.txt"], "Done."),
            script_saying("astra", 6, &["second.txt"], "Done."),
        ],
        None,
        Duration::from_secs(900),
    )
    .await;
    let record = &ran.record;
    assert_eq!(record["schema"], SCHEMA);
    let labels: Vec<String> = ran.made.iter().map(|(tier, _)| tier.label()).collect();
    assert_eq!(labels, ["claude-code/claude-opus-5-5", "codex/gpt-6-astra"]);
    let second = &record["second"];
    assert_eq!(second["fired"], json!(["check"]), "{record:#}");
    assert_eq!(second["outcome"], "kept_second");
    assert_eq!(second["kept"], "second");
    assert_eq!(second["tier"]["model"], "gpt-6-astra");
    assert_eq!(second["status"], "answered");
    assert!(second["milliseconds"].is_u64());
    assert!(second.get("cost_usd").is_some());
    assert_eq!(second["first"]["failed_checks"], 1);
    assert_eq!(second["second"]["failed"], 0);
    assert_eq!(sum_in(&ran.work), 6);
    assert!(!ran.work.join("first.txt").exists());
    assert_eq!(record["final_tier"]["model"], "gpt-6-astra");
}

#[tokio::test(flavor = "current_thread")]
async fn a_self_reported_failure_escalates_even_when_every_check_passes() {
    if !python() {
        return;
    }
    let admits = "Done, but the task doesn't pin down the rounding, so I guessed at it.";
    let ran = compose(
        "escalate-self-report",
        &escalate_unrouted(),
        vec![
            script_saying("opus", 6, &[], admits),
            script_saying(
                "astra",
                6,
                &["second.txt"],
                "Done: answer.json holds the sum.",
            ),
        ],
        None,
        Duration::from_secs(900),
    )
    .await;
    let record = &ran.record;
    let second = &record["second"];
    assert_eq!(second["fired"], json!(["self_report"]), "{record:#}");
    assert_eq!(second["first"]["failed_checks"], 0);
    assert_eq!(second["first"]["self_reported"], 1);
    assert_eq!(second["outcome"], "kept_second");
    assert!(ran.work.join("second.txt").is_file());
}

#[tokio::test(flavor = "current_thread")]
async fn an_escalation_that_checks_no_better_keeps_the_first_candidate() {
    if !python() {
        return;
    }
    let ran = compose(
        "escalate-loses",
        &escalate_unrouted(),
        vec![
            script_saying("opus", 7, &["first.txt"], "Done."),
            script_saying("astra", 5, &["second.txt"], "Done."),
        ],
        None,
        Duration::from_secs(900),
    )
    .await;
    let second = &ran.record["second"];
    assert_eq!(second["fired"], json!(["check"]), "{:#}", ran.record);
    assert_eq!(second["outcome"], "kept_first");
    assert_eq!(sum_in(&ran.work), 7);
    assert!(ran.work.join("first.txt").is_file());
    assert!(!ran.work.join("second.txt").exists());
    assert_eq!(ran.record["final_tier"]["model"], "claude-opus-5-5");
}

#[tokio::test(flavor = "current_thread")]
async fn a_passing_result_never_escalates() {
    if !python() {
        return;
    }
    let ran = compose(
        "escalate-skipped",
        &escalate_unrouted(),
        vec![script_saying("opus", 6, &[], "Done.")],
        None,
        Duration::from_secs(900),
    )
    .await;
    assert_eq!(ran.made.len(), 1);
    let second = &ran.record["second"];
    assert_eq!(
        second["skipped"],
        "no check failed and the executor reported no failure"
    );
    assert!(second.get("fired").is_none());
}

// ---------------------------------------------------------------------------
// v10: persistence judged against what the checks flag, cheap from round
// 1, and a second candidate that must resolve a failure.
// ---------------------------------------------------------------------------

/// v10 without its route, with its runner at `runner`, the confirmation
/// gate off, and verify.second's time floor lowered. `second` keeps or
/// drops the second executor.
fn v10_unrouted(runner: &Path, second: bool) -> Manifest {
    let mut manifest = manifest("tunable-v10.json");
    manifest.policy.control.route = None;
    let verify = manifest.policy.verify.as_mut().unwrap();
    if second {
        verify.second.as_mut().unwrap().min_remaining_sec = 60;
    } else {
        verify.second = None;
    }
    let persist = manifest.policy.control.persist.as_mut().unwrap();
    persist.own_tests.as_mut().unwrap().runner = runner.display().to_string();
    persist.stop_when_confirmed = false;
    manifest
}

/// A runner with one test: `sum` passes when answer.json holds 6.
const SUM_RUNNER: &str =
    "if grep -q '\"sum\": 6' answer.json; then echo 'PASS sum'; else echo 'FAIL sum'; fi\n";

/// `script` that also writes [`SUM_RUNNER`] first.
fn with_sum_runner(mut script: Script, runner: &Path) -> Script {
    let dir = runner.parent().unwrap().display().to_string();
    script.events.insert(
        0,
        at(
            100,
            Act::Run {
                command: format!(
                    "mkdir -p '{dir}' && cat > '{}' <<'RUNNER'\n{SUM_RUNNER}RUNNER",
                    runner.display()
                ),
            },
        ),
    );
    script
}

#[test]
fn the_v10_manifest_is_v8_judged_by_the_checks_and_cheap_from_round_one() {
    let v10 = manifest("tunable-v10.json");
    v10.validate().unwrap();
    let persist = v10.policy.control.persist.as_ref().unwrap();
    assert_eq!(persist.judge, persist::Judge::Checks);
    assert_eq!(persist.cheap.as_ref().unwrap().from_round, 1);
    assert_eq!(persist.cheap.as_ref().unwrap().max_escalations, 1);
    let second = v10.policy.verify.as_ref().unwrap().second.as_ref().unwrap();
    assert_eq!(second.keep, Keep::Resolved);
    // Everything else is v8's.
    let v8 = manifest("tunable-v8.json");
    let mut stripped = v10.policy.clone();
    let p = stripped.control.persist.as_mut().unwrap();
    p.judge = persist::Judge::Outcome;
    p.cheap.as_mut().unwrap().from_round = 2;
    stripped
        .verify
        .as_mut()
        .unwrap()
        .second
        .as_mut()
        .unwrap()
        .keep = Keep::FewerFailures;
    assert_eq!(stripped, v8.policy);
    // v8 serializes without the new fields, so its digest is what it was.
    let raw: Value = serde_json::from_str(include_str!("../../policies/tunable-v8.json")).unwrap();
    assert_eq!(serde_json::to_value(&v8.policy).unwrap(), raw["policy"]);
    // A word outside the vocabulary is refused.
    let text = include_str!("../../policies/tunable-v10.json")
        .replace("\"judge\": \"checks\"", "\"judge\": \"verifier\"");
    assert!(Manifest::parse(&text).is_err());
}

#[tokio::test(flavor = "current_thread")]
async fn a_second_candidate_that_resolves_the_failure_replaces_the_first() {
    if !python() {
        return;
    }
    let runner = runner_path("v10-second-wins");
    let ran = compose(
        "v10-second-wins",
        &v10_unrouted(&runner, true),
        vec![
            script_saying("opus", 7, &["first.txt"], "Done."),
            script_saying("opus-repair", 7, &["first.txt"], "Done."),
            script_saying("astra", 6, &["second.txt"], "Done."),
        ],
        None,
        Duration::from_secs(900),
    )
    .await;
    let second = &ran.record["second"];
    assert_eq!(second["keep"], "resolved", "{:#}", ran.record);
    assert_eq!(second["outcome"], "kept_second");
    assert_eq!(second["regressed"], json!([]));
    let resolved = second["resolved"].as_array().unwrap();
    assert!(
        resolved
            .iter()
            .any(|r| r.as_str().unwrap().ends_with("failed → passed")),
        "{second:#}"
    );
    assert_eq!(sum_in(&ran.work), 6);
    let _ = std::fs::remove_dir_all(runner.parent().unwrap());
}

#[tokio::test(flavor = "current_thread")]
async fn a_second_candidate_that_resolves_nothing_leaves_the_first() {
    if !python() {
        return;
    }
    let runner = runner_path("v10-second-loses");
    let ran = compose(
        "v10-second-loses",
        &v10_unrouted(&runner, true),
        vec![
            script_saying("opus", 7, &["first.txt"], "Done."),
            script_saying("opus-repair", 7, &["first.txt"], "Done."),
            script_saying("astra", 5, &["second.txt"], "Done."),
        ],
        None,
        Duration::from_secs(900),
    )
    .await;
    let second = &ran.record["second"];
    assert_eq!(second["outcome"], "kept_first", "{:#}", ran.record);
    assert!(
        second["why"]
            .as_str()
            .unwrap()
            .starts_with("the second candidate resolved 0 of the first's failures"),
        "{second:#}"
    );
    assert_eq!(sum_in(&ran.work), 7);
    assert!(ran.work.join("first.txt").is_file());
    let _ = std::fs::remove_dir_all(runner.parent().unwrap());
}

#[tokio::test(flavor = "current_thread")]
async fn a_flagged_failure_runs_sol_first_then_opus_once_until_nothing_is_flagged() {
    if !python() {
        return;
    }
    let runner = runner_path("v10-ladder");
    let ran = compose(
        "v10-ladder",
        &v10_unrouted(&runner, false),
        vec![
            script_saying("opus", 7, &[], "Done."),
            script_saying("opus-repair", 7, &[], "Done."),
            // Round 1, Sol: writes its test and another wrong answer.
            with_sum_runner(script_saying("sol-persist", 8, &[], "Done."), &runner),
            // Round 2, Opus: fixes it.
            script_saying("opus-escalated", 6, &[], "Done."),
            script_saying("never", 6, &[], "Done."),
        ],
        None,
        EIGHT_HOURS,
    )
    .await;
    let record = &ran.record;
    let labels: Vec<String> = ran.made.iter().map(|(tier, _)| tier.label()).collect();
    assert_eq!(
        labels,
        [
            "claude-code/claude-opus-5-5",
            "claude-code/claude-opus-5-5",
            "codex/gpt-6-sol",
            "claude-code/claude-opus-5-5",
        ],
        "{record:#}"
    );
    assert_eq!(sum_in(&ran.work), 6);
    let persist = &record["persist"];
    assert_eq!(persist["stopped"], persist::NOTHING_LEFT, "{record:#}");
    let rounds = persist["rounds"].as_array().unwrap();
    let classes: Vec<&str> = rounds
        .iter()
        .map(|r| r["class"].as_str().unwrap())
        .collect();
    assert_eq!(classes, ["cheap", "escalated"]);
    // Round 1 resolved nothing the checks flagged: no progress, so Opus
    // runs next.
    assert_eq!(rounds[0]["delta"]["progress"], false);
    assert_eq!(rounds[0]["next"], "escalate");
    assert_eq!(rounds[0]["kept"], true);
    assert!(
        rounds[0]["flagged_after"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f == "own test sum"),
        "{:#}",
        rounds[0]
    );
    // Round 2 resolved the failed check and the own test.
    assert_eq!(rounds[1]["delta"]["progress"], true);
    let resolved: Vec<&str> = rounds[1]["delta"]["resolved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap())
        .collect();
    assert!(
        resolved.contains(&"own test sum: fail → pass"),
        "{resolved:?}"
    );
    assert!(resolved.iter().any(|r| r.starts_with("scenario ")));
    assert_eq!(rounds[1]["flagged_after"], json!([]));
    assert_eq!(persist["totals"]["escalations"], 1);
    // Round 1's brief lists what the checks flag and the rule.
    let brief = std::fs::read_to_string(ran.out.join(persist::brief_path(1))).unwrap();
    for part in [
        "## What the host's checks and your tests flag",
        "- scenario ",
        "named `check <scenario>`",
        "Your tests passing while a check still fails is not progress.",
    ] {
        assert!(brief.contains(part), "{part} missing from\n{brief}");
    }
    let brief = std::fs::read_to_string(ran.out.join(persist::brief_path(2))).unwrap();
    assert!(brief.contains("- own test sum"), "{brief}");
    let _ = std::fs::remove_dir_all(runner.parent().unwrap());
}

#[tokio::test(flavor = "current_thread")]
async fn with_nothing_flagged_one_sol_round_runs_and_opus_never_does() {
    if !python() {
        return;
    }
    let runner = runner_path("v10-nothing");
    let ran = compose(
        "v10-nothing",
        &v10_unrouted(&runner, false),
        vec![
            script_saying("opus", 6, &[], "Done."),
            // Round 1, Sol: its tests pass and it changes nothing.
            with_sum_runner(idle("sol-idle"), &runner),
            script_saying("never", 6, &[], "Done."),
        ],
        None,
        EIGHT_HOURS,
    )
    .await;
    let record = &ran.record;
    assert_eq!(ran.made.len(), 2, "{record:#}");
    assert_eq!(ran.made[1].0.label(), "codex/gpt-6-sol");
    let persist = &record["persist"];
    assert_eq!(
        persist["stopped"],
        format!("round 1 changed nothing; {}", persist::NOTHING_LEFT),
        "{record:#}"
    );
    assert_eq!(persist["totals"]["escalations"], 0);
    let round = &persist["rounds"][0];
    assert_eq!(round["changed"], false);
    assert!(round.get("next").is_none(), "{round:#}");
    let brief = std::fs::read_to_string(ran.out.join(persist::brief_path(1))).unwrap();
    assert!(brief.contains(persist::NOTHING_FLAGGED), "{brief}");
    let _ = std::fs::remove_dir_all(runner.parent().unwrap());
}

#[tokio::test(flavor = "current_thread")]
async fn a_round_that_makes_a_passing_check_fail_is_put_back() {
    if !python() {
        return;
    }
    let runner = runner_path("v10-regress");
    let ran = compose(
        "v10-regress",
        &v10_unrouted(&runner, false),
        vec![
            script_saying("opus", 6, &[], "Done."),
            // Round 1, Sol: breaks the answer.
            with_sum_runner(
                script_saying("sol-persist", 5, &["scratch.txt"], "Done."),
                &runner,
            ),
            script_saying("never", 6, &[], "Done."),
        ],
        None,
        EIGHT_HOURS,
    )
    .await;
    let record = &ran.record;
    let round = &record["persist"]["rounds"][0];
    assert_eq!(round["kept"], false, "{record:#}");
    assert!(
        round["why"]
            .as_str()
            .unwrap()
            .starts_with("the round regressed 1 (scenario "),
        "{round:#}"
    );
    assert_eq!(round["delta"]["progress"], false);
    assert_eq!(sum_in(&ran.work), 6);
    assert!(!ran.work.join("scratch.txt").exists());
    assert_eq!(record["persist"]["stopped"], persist::NOTHING_LEFT);
    assert_eq!(ran.made.len(), 2);
    let _ = std::fs::remove_dir_all(runner.parent().unwrap());
}

#[test]
fn the_verdict_trigger_fires_only_on_a_failed_verdict() {
    let on = vec!["verdict".to_string()];
    let mut first = standing(0, 0, 0);
    assert!(first.fired(&on).is_empty());
    first.verdict_fail = true;
    assert_eq!(first.fired(&on), ["verdict"]);
    // A failed check alone doesn't fire it.
    assert!(standing(2, 1, 0).fired(&on).is_empty());
    // And a standing without a verdict serializes as it always did.
    assert!(
        serde_json::to_value(standing(0, 0, 0))
            .unwrap()
            .get("verdict_fail")
            .is_none()
    );
}

#[test]
fn verify_second_on_verdict_needs_verify_verdict() {
    let mut verify = manifest("tunable-v9-escalate.json").policy.verify.unwrap();
    verify.second.as_mut().unwrap().on = vec!["verdict".to_string()];
    assert!(
        verify
            .validate()
            .iter()
            .any(|p| p.contains("needs verify.verdict"))
    );
    verify.verdict = true;
    assert!(verify.validate().is_empty(), "{:?}", verify.validate());
    verify.distrust = vec![" ".to_string()];
    assert!(
        verify
            .validate()
            .iter()
            .any(|p| p.contains("empty scenario kind"))
    );
}

// ---------------------------------------------------------------------------
// control.best_of.
// ---------------------------------------------------------------------------

fn delegate_calls(recorder: &Recorder) -> usize {
    recorder
        .steps()
        .iter()
        .filter(|s| s.call.as_ref().is_some_and(|c| c.name == "delegate"))
        .count()
}

#[tokio::test(flavor = "current_thread")]
async fn best_of_three_keeps_the_candidate_whose_checks_pass_and_records_every_one() {
    if !python() {
        return;
    }
    let ran = compose(
        "best-of-3",
        &manifest("luna-best-of-3.json"),
        vec![
            // A wrong sum, then two right ones; each leaves a file of its own.
            script_saying("luna-1", 5, &["one.txt"], "Done."),
            script_saying("luna-2", 6, &["two.txt"], "Done."),
            script_saying("luna-3", 6, &["three.txt"], "Done."),
        ],
        None,
        Duration::from_secs(900),
    )
    .await;
    let record = &ran.record;
    let best = &record["best_of"];
    assert_eq!(best["n"], 3, "{record:#}");
    assert_eq!(
        best["order"],
        json!(["verdict", "scorecard", "cost", "number"])
    );
    // Jev is off, so every verdict is unknown and the checks decide: the
    // wrong sum fails the public test, and of the two right ones the lower
    // number stays.
    assert_eq!(best["verdicts"], json!(["unknown", "unknown", "unknown"]));
    assert_eq!(best["kept"], 2, "{best:#}");
    let candidates = best["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 3);
    assert!(candidates[0]["score"]["failures"].as_u64().unwrap() > 0);
    assert_eq!(candidates[1]["score"]["failures"], 0);
    for (i, candidate) in candidates.iter().enumerate() {
        assert_eq!(candidate["number"], i + 1);
        assert_eq!(candidate["status"], "answered");
        assert!(ran.out.join(best_of::checks_file(i + 1)).is_file());
        let archive = candidate["archive"]["path"].as_str().unwrap();
        assert!(ran.out.join(archive).is_file(), "{archive}");
    }
    assert_eq!(best["leaked"], false);
    // The kept candidate's copy is the workspace, and only its own file is
    // in it.
    assert_eq!(sum_in(&ran.work), 6);
    assert!(ran.work.join("two.txt").is_file());
    assert!(!ran.work.join("one.txt").exists());
    assert!(!ran.work.join("three.txt").exists());
    // Each candidate is a dispatch and a delegate call, so each is charged.
    assert_eq!(roles(record), ["candidate-1", "candidate-2", "candidate-3"]);
    let kept: Vec<bool> = record["branches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["kept"] == true)
        .collect();
    assert_eq!(kept, [false, true, false]);
    assert_eq!(delegate_calls(&ran.recorder), 3);
    assert_eq!(ran.made.len(), 3);
    // The kept candidate's checks are the first checks.
    assert_eq!(record["checks"].as_array().unwrap().len(), 3);
    assert!(
        record["final_checks"]["verdicts"]["failed"].is_null(),
        "{record:#}"
    );
    let first = std::fs::read_to_string(ran.out.join(checks::COVERAGE_FILE)).unwrap();
    let kept_checks = std::fs::read_to_string(ran.out.join(best_of::checks_file(2))).unwrap();
    assert_eq!(first, kept_checks);
    // The verdict on the kept candidate is the composition's.
    assert_eq!(record["verdict"]["first"]["call"], "unknown");
}

#[tokio::test(flavor = "current_thread")]
async fn each_candidate_works_in_its_own_copy_until_one_is_kept() {
    if !python() {
        return;
    }
    // Every candidate writes answer.json; were they sharing the workspace,
    // the last write would win whichever candidate the checks keep.
    let ran = compose(
        "best-of-copies",
        &manifest("luna-best-of-3.json"),
        vec![
            script_saying("luna-1", 7, &[], "Done."),
            script_saying("luna-2", 6, &[], "Done."),
            script_saying("luna-3", 8, &[], "Done."),
        ],
        None,
        Duration::from_secs(900),
    )
    .await;
    let best = &ran.record["best_of"];
    assert_eq!(best["kept"], 2, "{best:#}");
    assert_eq!(sum_in(&ran.work), 6);
    // Every copy is gone once one is kept.
    let copies = std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(&format!("coder-one-best-of-{}-", std::process::id()))
        })
        .count();
    assert_eq!(copies, 0);
}

#[tokio::test(flavor = "current_thread")]
async fn a_workspace_too_large_to_copy_runs_one_candidate_and_says_why() {
    if !python() {
        return;
    }
    let mut manifest = manifest("luna-best-of-3.json");
    manifest
        .policy
        .control
        .best_of
        .as_mut()
        .unwrap()
        .max_copy_mb = 0;
    let ran = compose(
        "best-of-too-large",
        &manifest,
        vec![script_saying("luna", 6, &[], "Done.")],
        None,
        Duration::from_secs(900),
    )
    .await;
    let record = &ran.record;
    assert!(
        record["best_of"]["skipped"]
            .as_str()
            .unwrap()
            .contains("too large to copy"),
        "{record:#}"
    );
    assert_eq!(roles(record), ["primary"]);
    assert_eq!(ran.made.len(), 1);
    assert_eq!(sum_in(&ran.work), 6);
}

#[test]
fn the_best_of_manifests_differ_from_their_single_arm_only_in_n() {
    let single = manifest("luna-best-of-1.json");
    assert!(single.policy.control.best_of.is_none());
    assert!(single.policy.control.monitor.is_none());
    let verify = single.policy.verify.clone().unwrap();
    assert!(verify.checks && verify.verdict && verify.repair.is_none());
    for (file, n) in [("luna-best-of-3.json", 3), ("luna-best-of-5.json", 5)] {
        let mut many = manifest(file);
        assert_eq!(many.policy.control.best_of.take().unwrap().n, n);
        assert_eq!(many.policy, single.policy, "{file}");
    }
    let micro = manifest("microluna-best-of-1.json");
    let mut many = manifest("microluna-best-of-3.json");
    assert_eq!(many.policy.control.best_of.take().unwrap().n, 3);
    assert_eq!(many.policy, micro.policy);
    // The suite arm differs from best-of-3 only in its selection key.
    let mut suite = manifest("microluna-best-of-3-suite.json");
    let best_of = suite.policy.control.best_of.take().unwrap();
    assert_eq!(
        (best_of.n, best_of.select),
        (3, super::best_of::Select::Suite)
    );
    assert_eq!(suite.policy, micro.policy);
    // The accept arm adds only accept.define to the suite arm.
    let mut accept = manifest("microluna-best-of-3-accept.json");
    let best_of = accept.policy.control.best_of.take().unwrap();
    assert!(best_of.accept);
    assert_eq!(best_of.select, super::best_of::Select::Suite);
    assert_eq!(accept.policy, micro.policy);
    assert_eq!(
        micro.policy.executor.agent,
        crate::policy::AgentName::Microluna
    );
}

#[test]
fn best_of_needs_the_verdict_and_takes_no_handoff() {
    let mut unverdicted = manifest("luna-best-of-3.json");
    unverdicted.policy.verify.as_mut().unwrap().verdict = false;
    let error = unverdicted.validate().unwrap_err();
    assert!(
        error.contains("needs verify.checks and verify.verdict"),
        "{error}"
    );
    let mut one = manifest("luna-best-of-3.json");
    one.policy.control.best_of.as_mut().unwrap().n = 1;
    assert!(one.validate().unwrap_err().contains("from 2 to 8"));
    let mut handed = manifest("luna-best-of-3.json");
    handed.policy.control.handoff = manifest("handoff-escalate.json").policy.control.handoff;
    assert!(
        handed
            .validate()
            .unwrap_err()
            .contains("takes no control.handoff")
    );
}
