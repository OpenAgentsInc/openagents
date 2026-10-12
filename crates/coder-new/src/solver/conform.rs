//! The conformance check between `drive()` and the `solve-issue` program
//! (#11392). `coder solver conform --run DIR` runs the pinned
//! `programs/solve-issue.json` through the program host ([`super::host`])
//! with a replay boundary: each delegate step takes the next hand the
//! recording holds, each check answers what the recording shows the
//! harness found, and the verdict is the program's own checker. The run
//! conforms when the program asked for exactly the recorded hands, in
//! order, and came to the same rounds and outcome. The hand-coded
//! `drive()` stays the reference until every recorded run conforms.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::hands::Verdict;
use super::host::{self, Effects, Pinned, Ran};

/// Where the shipped program lives, relative to the repository root.
pub const PROGRAM: &str = "programs/solve-issue.json";

/// The digest the shipped program is pinned at. Changing the program
/// changes this, and the corpus has to agree with the new one.
pub const PINNED: &str = "sha256:f474909d49642a2815fb98260f53acd1d1b799c1ac7a2f22223046be52d09883";

/// The checker that settles a run's outcome from its steps' envelopes.
pub const VERDICT_CHECKER: &str = "solver-verdict";

/// The hand a delegate step's executor names: `solver-coder` is `coder`.
#[must_use]
pub fn hand_of(executor: &str) -> &str {
    executor.strip_prefix("solver-").unwrap_or(executor)
}

/// The outcome the program comes to, from the envelopes of intake, the
/// code brief, and each round's code, check, and diagnosis. This is
/// `drive()`'s judgment written as a checker: intake and the brief can
/// stop the run, a Coder error fails it, and otherwise the last round's
/// check decides. `rounds` is the index of the last coding round.
#[must_use]
pub fn settle(input: &Value) -> Value {
    let status = |envelope: &Value| envelope["value"]["status"].as_str().map(str::to_owned);
    let stopped = |status: &str, rounds: usize, reason: &str| json!({"status": status, "rounds": rounds, "reason": reason});
    match status(&input["intake"]).as_deref() {
        Some("ready") => {}
        Some(other) => return stopped(other, 0, "intake stopped the run"),
        None => return stopped("setup_failed", 0, "intake did not complete"),
    }
    match status(&input["plan"]).as_deref() {
        Some("briefed") => {}
        Some(other) => return stopped(other, 0, "the code brief stopped the run"),
        None => return stopped("decision_failed", 0, "the code brief did not complete"),
    }
    let rounds: Vec<&Value> = input["rounds"]
        .as_array()
        .map(|rounds| {
            rounds
                .iter()
                .filter(|round| round["code"]["outcome"] != "skipped")
                .collect()
        })
        .unwrap_or_default();
    let index = rounds.len().saturating_sub(1);
    let Some(last) = rounds.last() else {
        return stopped("failed", 0, "no coding round ran");
    };
    if status(&last["code"]).as_deref() != Some("done") {
        return stopped("failed", index, "the Coder stopped");
    }
    match status(&last["check"]).as_deref() {
        Some("passed") => json!({"status": "passed", "rounds": index, "reason": null}),
        Some("unchecked") => stopped("unchecked", index, "no required check ran"),
        Some("cancelled") => stopped("cancelled", index, "the run was stopped"),
        Some("environment") => stopped("failed", index, "the computer failed a check"),
        Some(_) if last["diagnose"]["value"]["route"] == "repeat" => stopped(
            "failed",
            index,
            "the same checks failed twice with the same diagnosis",
        ),
        Some(_) => stopped(
            "failed",
            index,
            "the checks failed after the last hand-back",
        ),
        None => stopped("failed", index, "the checks did not run"),
    }
}

/// One recorded hand.
#[derive(Clone, Debug)]
struct Hand {
    hand: String,
    round: u64,
    verdict: String,
    error: bool,
}

/// The replay boundary: answers each effect the way the recorded run
/// went, and notes where the program asked for something the recording
/// does not hold.
pub struct Recorded {
    hands: VecDeque<Hand>,
    last_tester: usize,
    taken: usize,
    status: String,
    reason: String,
    pull_request: Value,
    /// Where the program and the recording disagreed.
    pub divergence: Vec<String>,
}

impl Recorded {
    /// The boundary over a run's `summary.json`.
    #[must_use]
    pub fn of(summary: &Value) -> Self {
        let hands: VecDeque<Hand> = summary["solver"]["hands"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|turn| Hand {
                hand: turn["hand"].as_str().unwrap_or_default().to_owned(),
                round: turn["round"].as_u64().unwrap_or_default(),
                verdict: turn["verdict"].as_str().unwrap_or_default().to_owned(),
                error: !turn["error"].is_null() && turn["error"] != false,
            })
            .collect();
        let last_tester = hands
            .iter()
            .rposition(|hand| hand.hand == "tester")
            .unwrap_or(usize::MAX);
        Self {
            hands,
            last_tester,
            taken: 0,
            status: summary["outcome"]["status"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            reason: summary["outcome"]["reason"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            pull_request: summary["solver"]["pull_request"].clone(),
            divergence: Vec::new(),
        }
    }

    /// The recorded hands the program never asked for.
    #[must_use]
    pub fn leftover(&self) -> Vec<String> {
        self.hands.iter().map(|hand| hand.hand.clone()).collect()
    }

    fn next_is(&self, hand: &str) -> bool {
        self.hands.front().is_some_and(|next| next.hand == hand)
    }
}

impl Effects for Recorded {
    fn delegate(&mut self, step: &str, executor: &str, _: &Value) -> Result<Value, String> {
        let want = hand_of(executor);
        let Some(hand) = self.hands.front().cloned() else {
            let why = format!("{step} asks for the {want}, and the recording has no more hands");
            self.divergence.push(why.clone());
            return Err(why);
        };
        if hand.hand != want {
            let why = format!(
                "{step} asks for the {want}, and the recording's next hand is the {}",
                hand.hand
            );
            self.divergence.push(why.clone());
            return Err(why);
        }
        self.hands.pop_front();
        let index = self.taken;
        self.taken += 1;
        Ok(match want {
            "intaker" => {
                let stopped = matches!(self.status.as_str(), "setup_failed" | "already_done")
                    || (self.status == "cancelled" && self.hands.is_empty());
                let status = if stopped {
                    self.status.as_str()
                } else {
                    "ready"
                };
                json!({"status": status})
            }
            "briefer" => json!({"status": "briefed", "round": hand.round}),
            "coder" => json!({
                "status": if hand.error { "error" } else { "done" },
                "round": hand.round,
            }),
            "tester" => {
                let repeat =
                    index == self.last_tester && self.reason.starts_with("The same checks");
                let route = match Verdict::parse(&hand.verdict) {
                    _ if repeat => "repeat",
                    Verdict::Briefer(_) => "briefer",
                    Verdict::Pass | Verdict::Coder(_) => "coder",
                };
                json!({"route": route, "again": !repeat, "round": hand.round})
            }
            _ => json!({"round": hand.round}),
        })
    }

    fn check(&mut self, _: &str, checker: &str, input: &Value) -> Result<Value, String> {
        if checker == VERDICT_CHECKER {
            return Ok(settle(input));
        }
        // A Tester reads the failures of the round it follows, so a
        // recorded Tester next means this round's checks failed.
        if self.next_is("tester") {
            return Ok(json!({"status": "failed"}));
        }
        let status = match self.status.as_str() {
            "passed" | "unchecked" | "cancelled" => self.status.as_str(),
            "failed" if self.reason.starts_with("The computer, not the change") => "environment",
            _ => "failed",
        };
        Ok(json!({"status": status}))
    }

    fn invoke(&mut self, _: &str, operation: &str, _: &Value) -> Result<Value, String> {
        match operation {
            "code-brief" => {
                let status = match self.status.as_str() {
                    _ if self.next_is("briefer") => "briefed",
                    "decision_failed" | "cancelled" => self.status.as_str(),
                    _ => "briefed",
                };
                Ok(json!({"status": status}))
            }
            "open-pull-request" => Ok(json!({"pull_request": self.pull_request})),
            other => Err(format!("the recording holds no {other} operation")),
        }
    }

    fn decide(&mut self, _: &str, question: &str, _: &Value) -> Result<(Value, Value), String> {
        // The recording holds no door answer for the done judgment, so the
        // replay answers it from the recorded outcome, marked as such.
        let complete = self.status == "passed";
        Ok((
            json!({"question": question, "complete": complete}),
            json!([{"answer": if complete { "yes" } else { "no" }, "p": 1.0, "from": "recording"}]),
        ))
    }
}

/// The hands the program asked for: each dispatched delegate step's
/// executor, in order.
#[must_use]
pub fn program_hands(ran: &Ran) -> Vec<String> {
    ran.steps
        .iter()
        .filter(|reached| reached.kind == "delegate" && reached.outcome() == "completed")
        .map(|reached| hand_of(&reached.target).to_owned())
        .collect()
}

/// One recorded run replayed through the program and judged.
#[must_use]
pub fn judge(run: &Path, summary: &Value, pinned: &Pinned) -> Value {
    let mut boundary = Recorded::of(summary);
    let ran = host::run(
        pinned,
        &json!({"issue": summary["issue"], "repository": summary["repo"]}),
        &mut boundary,
    );
    let recorded: Vec<String> = summary["solver"]["hands"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|turn| turn["hand"].as_str().map(str::to_owned))
        .collect();
    let program = program_hands(&ran);
    let result = ran.result.clone().unwrap_or(Value::Null);
    let mut why = boundary.divergence.clone();
    let leftover = boundary.leftover();
    if !leftover.is_empty() {
        why.push(format!(
            "the recording holds hands the program never asked for: {leftover:?}"
        ));
    }
    if let Some(stopped) = &ran.stopped {
        why.push(format!("the program stopped: {stopped}"));
    }
    let hands_match = recorded == program;
    let rounds_match = summary["solver"]["rounds"] == result["rounds"];
    let outcome_match = summary["outcome"]["status"] == result["status"];
    if !rounds_match {
        why.push(format!(
            "rounds {} recorded, {} by the program",
            summary["solver"]["rounds"], result["rounds"]
        ));
    }
    if !outcome_match {
        why.push(format!(
            "outcome {} recorded, {} by the program",
            summary["outcome"]["status"], result["status"]
        ));
    }
    let conforms = hands_match && rounds_match && outcome_match && why.is_empty();
    let steps: Vec<Value> = ran
        .steps
        .iter()
        .filter(|reached| reached.outcome() != "skipped")
        .map(|reached| json!([reached.step, reached.outcome()]))
        .collect();
    json!({
        "run": run.display().to_string(),
        "issue": summary["issue"],
        "program": pinned.digest,
        "recorded": {"hands": recorded, "rounds": summary["solver"]["rounds"], "outcome": summary["outcome"]["status"]},
        "replayed": {"hands": program, "rounds": result["rounds"], "outcome": result["status"]},
        "steps": steps,
        "conforms": conforms,
        "why": if conforms { json!("the program asked for the recorded hands and came to the same rounds and outcome") } else { json!(why) },
    })
}

/// `coder solver conform --run DIR [--run DIR ...] [--program FILE]
/// [--pin sha256:...]`, one JSON line per run; exits with an error when
/// any run does not conform. A `DIR` may also be a `summary.json` or a
/// corpus case file.
///
/// # Errors
/// An option is unknown, the program does not parse or is not the pinned
/// one, a run has no `summary.json`, or a run does not conform.
pub fn command(args: &[String]) -> Result<String, String> {
    let mut runs: Vec<PathBuf> = Vec::new();
    let mut program = PathBuf::from(PROGRAM);
    let mut pin: Option<String> = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--run" => runs.push(args.next().ok_or("--run needs a directory")?.into()),
            "--program" => program = args.next().ok_or("--program needs a file")?.into(),
            "--pin" => pin = Some(args.next().ok_or("--pin needs a digest")?.clone()),
            other => {
                return Err(format!(
                    "unknown option {other}; coder solver conform takes --run DIR [--program FILE] [--pin sha256:...]"
                ));
            }
        }
    }
    if runs.is_empty() {
        return Err("coder solver conform needs at least one --run DIR".into());
    }
    let pinned = Pinned::load(&program, pin.as_deref())?;
    let mut out = String::new();
    let mut failed = 0;
    for run in &runs {
        let file = if run.is_dir() {
            run.join("summary.json")
        } else {
            run.clone()
        };
        let summary: Value = std::fs::read(&file)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .ok_or_else(|| format!("{} is not a readable summary.json", file.display()))?;
        // A corpus case carries its summary under `summary`.
        let summary = summary.get("summary").cloned().unwrap_or(summary);
        let row = judge(run, &summary, &pinned);
        if row["conforms"] != true {
            failed += 1;
        }
        out.push_str(&row.to_string());
        out.push('\n');
    }
    if failed > 0 {
        return Err(format!(
            "{out}{failed} run(s) do not conform to {}",
            program.display()
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn shipped() -> Pinned {
        Pinned::load(&root().join(PROGRAM), Some(PINNED)).expect("the shipped program, pinned")
    }

    /// The corpus: recorded `drive()` runs (trimmed `summary.json`s) and a
    /// few constructed ones for paths no recording covers yet, each with
    /// whether it conforms.
    fn corpus() -> Vec<(String, Value)> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/solver-corpus");
        let mut cases: Vec<(String, Value)> = std::fs::read_dir(&dir)
            .expect("the solver corpus")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .map(|path| {
                let bytes = std::fs::read(&path).unwrap();
                (
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    serde_json::from_slice(&bytes).unwrap(),
                )
            })
            .collect();
        cases.sort_by(|a, b| a.0.cmp(&b.0));
        cases
    }

    #[test]
    fn every_corpus_case_replays_through_the_pinned_program_as_expected() {
        let pinned = shipped();
        let cases = corpus();
        assert!(cases.len() >= 10, "{} cases", cases.len());
        for (name, case) in cases {
            let row = judge(Path::new(&name), &case["summary"], &pinned);
            assert_eq!(row["conforms"], case["conforms"], "{name}: {row}");
            if let Some(steps) = case.get("steps") {
                assert_eq!(&row["steps"], steps, "{name}");
            }
        }
    }

    #[test]
    fn the_done_judgment_writes_its_answers_beside_its_decision() {
        let pinned = shipped();
        let case = corpus()
            .into_iter()
            .find(|(name, _)| name == "recorded-11348-1791760489.json")
            .expect("a recorded pass")
            .1;
        let mut boundary = Recorded::of(&case["summary"]);
        let ran = host::run(&pinned, &json!({}), &mut boundary);
        assert_eq!(ran.envelope("done").unwrap()["value"]["complete"], true);
        assert_eq!(ran.sidecars["done$answers"][0]["answer"], "yes");
        assert_eq!(
            ran.envelope("land").unwrap()["value"]["pull_request"],
            case["summary"]["solver"]["pull_request"]
        );
    }

    #[test]
    fn another_digest_is_refused() {
        let error = Pinned::load(&root().join(PROGRAM), Some("sha256:00")).unwrap_err();
        assert!(error.contains("not the pinned"), "{error}");
    }

    #[test]
    fn a_failed_run_with_a_coder_error_settles_failed_at_its_round() {
        let rounds = json!([
            {"code": {"outcome": "completed", "value": {"status": "done"}},
             "check": {"outcome": "completed", "value": {"status": "failed"}},
             "diagnose": {"outcome": "completed", "value": {"route": "coder"}}},
            {"code": {"outcome": "completed", "value": {"status": "error"}},
             "check": {"outcome": "skipped", "value": null},
             "diagnose": {"outcome": "skipped", "value": null}},
            {"code": {"outcome": "skipped", "value": null},
             "check": {"outcome": "skipped", "value": null},
             "diagnose": {"outcome": "skipped", "value": null}},
        ]);
        let settled = settle(&json!({
            "intake": {"outcome": "completed", "value": {"status": "ready"}},
            "plan": {"outcome": "completed", "value": {"status": "briefed"}},
            "rounds": rounds,
        }));
        assert_eq!(settled["status"], "failed");
        assert_eq!(settled["rounds"], 1);
    }
}
