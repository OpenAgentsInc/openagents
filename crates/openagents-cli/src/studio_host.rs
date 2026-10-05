//! `openagents studio` against the running host: every Agent Studio read
//! and intent Everglade's panels send, carried as NIP-HOST `studio.*`
//! operations over the host's same-user control socket
//! (`openagents_connect::control`, `Op::Task`). On the host's own computer
//! the socket's peer is the host's owner, so the host takes every studio
//! operation from it and still checks each one. Nothing here opens the
//! task store; a refusal reaches the person with the host's code and
//! message.
//!
//! Each intent goes out under a fresh 64-hex request identity. When its
//! answer is lost, it is sent once more under the same identity: the host
//! answers an exact retry with the same outcome and repeats no effect.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use coder::argv::Args;
use coder_access::protocol::{Outcome, Receipt};
use coder_access::review::{Completeness, PublishState, TaskReview};
use coder_access::studio::{
    Decision, DecisionKind, MergeDecision, Merged, Mirror, Snapshot, Update, Verdict, View,
};
use coder_access::{Code, Operation};
use openagents_connect::control::{self, Op, Reply, Request};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::Output;

/// The switches the host commands take, beside `studio up`'s.
pub(crate) const SWITCHES: &[&str] = &["diff", "always"];

/// How long one exchange with the host may take.
const TIMEOUT: Duration = Duration::from_secs(30);

/// A refusal: the host's own code and message, or `unavailable` when no
/// host answered.
#[derive(Clone, Debug)]
pub(crate) struct Refusal {
    pub(crate) operation: &'static str,
    pub(crate) code: String,
    pub(crate) message: String,
}

impl Refusal {
    fn new(operation: &'static str, code: &str, message: impl Into<String>) -> Self {
        Self {
            operation,
            code: code.to_owned(),
            message: message.into(),
        }
    }

    fn access(operation: &'static str, error: &coder_access::Error) -> Self {
        Self::new(operation, code_word(error.code), error.message.clone())
    }

    /// Prints the refusal and returns the failure exit code. Under
    /// `--json`, stdout carries `{"error", "code", "operation"}`.
    pub(crate) fn report(&self, output: &Output) -> u8 {
        eprintln!(
            "openagents studio: the host refused `{}` ({}): {}",
            self.operation, self.code, self.message
        );
        if output.json() {
            println!(
                "{}",
                json!({"error": self.message, "code": self.code, "operation": self.operation})
            );
        }
        crate::out::EXIT_FAILURE
    }
}

/// A NIP-HOST code's wire spelling.
fn code_word(code: Code) -> &'static str {
    match code {
        Code::Malformed => "malformed",
        Code::Unsupported => "unsupported",
        Code::Forbidden => "forbidden",
        Code::MissingRight => "missing_right",
        Code::Expired => "expired",
        Code::Revoked => "revoked",
        Code::Stale => "stale",
        Code::Conflict => "conflict",
        Code::Bounds => "bounds",
        Code::Unavailable => "unavailable",
        Code::Transport => "transport",
        Code::RateLimited => "rate_limited",
        Code::WrongCode => "wrong_code",
        Code::Denied => "denied",
    }
}

/// Why a host command stopped.
enum Fail {
    Usage(String),
    Refused(Refusal),
    Failed(String),
}

impl From<Refusal> for Fail {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(refusal)
    }
}

/// The host's control socket.
pub(crate) struct Host {
    socket: PathBuf,
    runtime: tokio::runtime::Runtime,
    next: u64,
}

impl Host {
    /// A client of the host whose control socket is at `socket`. Nothing
    /// connects until the first call.
    ///
    /// # Errors
    /// The runtime does not start.
    pub(crate) fn new(socket: PathBuf) -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("cannot start the runtime: {error}"))?;
        Ok(Self {
            socket,
            runtime,
            next: 1,
        })
    }

    /// Whether a host answers on the socket now.
    pub(crate) fn answers(&self) -> bool {
        crate::host_answers_at(&self.socket)
    }

    /// Sends `operation` and returns the host's outcome, which answers it.
    ///
    /// # Errors
    /// The host's refusal with its code, or `unavailable` when nothing
    /// answers.
    pub(crate) fn call(&mut self, operation: &Operation) -> Result<Outcome, Refusal> {
        let name = operation.name();
        operation
            .validate()
            .map_err(|error| Refusal::access(name, &error))?;
        let request = mint();
        let outcome = match self.exchange(&request, operation) {
            Err(Failure::Lost(_)) => self.exchange(&request, operation),
            other => other,
        }
        .map_err(|failure| match failure {
            Failure::Lost(refusal) | Failure::Final(refusal) => refusal,
        })?;
        if !outcome.answers(operation) {
            return Err(Refusal::new(
                name,
                "malformed",
                "the host answered another operation",
            ));
        }
        outcome
            .validate()
            .map_err(|error| Refusal::access(name, &error))?;
        Ok(outcome)
    }

    /// One request on a fresh connection. A refusal the host sent is
    /// final; a failure of the connection may have lost the answer.
    fn exchange(&mut self, request: &str, operation: &Operation) -> Result<Outcome, Failure> {
        let name = operation.name();
        let id = self.next;
        self.next += 1;
        let op = Op::Task {
            request: request.to_owned(),
            operation: operation.clone(),
        };
        let socket = self.socket.clone();
        let reply: Result<Reply, Refusal> = self.runtime.block_on(async move {
            let Ok(mut stream) = crate::dial_control(&socket).await else {
                return Err(Refusal::new(name, "unavailable", no_host(&socket)));
            };
            match tokio::time::timeout(TIMEOUT, control::call(&mut stream, &Request::new(id, op)))
                .await
            {
                Err(_) => Err(Refusal::new(
                    name,
                    "unavailable",
                    "the host did not answer in time",
                )),
                Ok(Err(error)) => Err(Refusal::new(
                    name,
                    error.code.as_str(),
                    format!("the host did not answer: {}", error.detail),
                )),
                Ok(Ok(reply)) => Ok(reply),
            }
        });
        match reply {
            Ok(Reply::Task { outcome }) => Ok(outcome),
            Ok(Reply::Refused { code, message }) => {
                Err(Failure::Final(Refusal::new(name, &code, message)))
            }
            Ok(_) => Err(Failure::Final(Refusal::new(
                name,
                "malformed",
                "the host answered another operation",
            ))),
            Err(refusal) => Err(Failure::Lost(refusal)),
        }
    }
}

/// Why one exchange failed.
enum Failure {
    /// The connection failed, so the host's answer may be lost: the same
    /// request is sent once more.
    Lost(Refusal),
    /// The host refused; a retry would only repeat it.
    Final(Refusal),
}

fn no_host(socket: &Path) -> String {
    format!(
        "no host answers at {}; start it with `openagents host serve --control`, open the \
         OpenAgents app, or pass --control-socket PATH",
        socket.display()
    )
}

/// A fresh 64-hex identity: a NIP-HOST request ID, or the command ID an
/// answer or a merge decision carries. Unique within this process and,
/// through the time and process ID it digests, across processes.
fn mint() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let mut hash = Sha256::new();
    hash.update(b"openagents-studio-identity");
    hash.update(nanos.to_be_bytes());
    hash.update(std::process::id().to_be_bytes());
    hash.update(NEXT.fetch_add(1, Ordering::Relaxed).to_be_bytes());
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The host's control socket for `args`: `--control-socket PATH`, or the
/// platform's default. The second value says whether the person named it.
pub(crate) fn socket(args: &Args) -> (Option<PathBuf>, bool) {
    match args.option("control-socket") {
        Some(path) => (Some(PathBuf::from(path)), true),
        None => (control::socket_path(), false),
    }
}

/// Runs `words` against the host when it is a host command, returning its
/// exit code; `None` when `words` is a local command. `goal submit` and
/// `message` go through the host when one answers, or always when the
/// person named `--control-socket`; otherwise they stay local.
pub(crate) fn dispatch(output: &Output, words: &[&str], args: &Args) -> Option<u8> {
    let host_only = matches!(
        words,
        ["status"]
            | ["tasks"]
            | ["tasks", _]
            | ["log", _]
            | ["decisions"]
            | ["answer", _, ..]
            | ["review", _]
            | ["merge", _]
            | ["request-changes", _, ..]
            | ["reject", _, ..]
            | ["seat", "pause" | "resume" | "stop", _]
            | ["task", "cancel" | "retry" | "prioritize", _]
            | ["task", "reassign", _, _]
            | ["watch"]
    );
    let either = matches!(words, ["goal", "submit", _, ..] | ["message", _, _, ..]);
    if !host_only && !either {
        return None;
    }
    let (path, named) = socket(args);
    let Some(path) = path else {
        if either {
            return None;
        }
        return Some(output.fail(
            "studio",
            "no control socket path on this platform; pass --control-socket PATH",
        ));
    };
    let mut host = match Host::new(path) {
        Ok(host) => host,
        Err(message) => return Some(output.fail("studio", &message)),
    };
    if either && !named && !host.answers() {
        return None;
    }
    let result = match words {
        ["status"] => status(output, &mut host),
        ["tasks"] => tasks(output, &mut host, None),
        ["tasks", goal] => tasks(output, &mut host, Some(*goal)),
        ["log", seat] => log(output, &mut host, seat),
        ["decisions"] => decisions(output, &mut host),
        ["answer", decision, text @ ..] => answer(output, &mut host, decision, text, args),
        ["review", task] => review(output, &mut host, task, args.switch("diff")),
        ["merge", task] => decide(output, &mut host, task, Verdict::Merge, "", args),
        ["request-changes", task, text @ ..] => decide(
            output,
            &mut host,
            task,
            Verdict::RequestChanges,
            &text.join(" "),
            args,
        ),
        ["reject", task, reason @ ..] => decide(
            output,
            &mut host,
            task,
            Verdict::Reject,
            &reason.join(" "),
            args,
        ),
        ["seat", verb, seat] => seat_intent(output, &mut host, verb, seat),
        ["task", "reassign", task, seat] => task_intent(output, &mut host, "reassign", task, seat),
        ["task", verb, task] => task_intent(output, &mut host, verb, task, ""),
        ["watch"] => watch(output, &mut host, args),
        ["goal", "submit", text @ ..] => goal_submit(output, &mut host, &text.join(" "), args),
        ["message", seat, text @ ..] => message(output, &mut host, seat, &text.join(" ")),
        _ => Err(Fail::Usage("unknown or incomplete command".into())),
    };
    Some(match result {
        Ok(code) => code,
        Err(Fail::Usage(message)) => output.usage("studio", &message, crate::studio::USAGE),
        Err(Fail::Refused(refusal)) => refusal.report(output),
        Err(Fail::Failed(message)) => output.fail("studio", &message),
    })
}

fn now() -> u64 {
    coder::task::autostart::unix_now()
}

/// A value's JSON word: an enum's wire spelling.
fn word<T: serde::Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(Value::String(word)) => word,
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

/// A host-issued 64-hex identity, shortened for a table; any other kept.
fn short(id: &str) -> &str {
    if id.len() == 64 { &id[..12] } else { id }
}

fn unexpected(operation: &'static str) -> Fail {
    Fail::Refused(Refusal::new(
        operation,
        "malformed",
        "the host answered another operation",
    ))
}

/// The studio now.
fn snapshot(host: &mut Host) -> Result<Snapshot, Fail> {
    match host.call(&Operation::StudioSnapshot {})? {
        Outcome::Studio { snapshot } => Ok(*snapshot),
        _ => Err(unexpected("studio.snapshot")),
    }
}

/// The receipt an intent answers with.
fn dispatched(outcome: Outcome, operation: &'static str) -> Result<Receipt, Fail> {
    match outcome {
        Outcome::Dispatched { receipt } => Ok(receipt),
        _ => Err(unexpected(operation)),
    }
}

/// The one identity in `ids` that is `given` or starts with it, so a
/// table's shortened task ID names its task. An identity the view does
/// not hold is passed on as given: the host decides.
fn resolve<'a>(
    given: &str,
    ids: impl Iterator<Item = &'a str>,
    what: &str,
) -> Result<String, Fail> {
    let mut found: Vec<&str> = Vec::new();
    for id in ids {
        if id == given {
            return Ok(id.to_owned());
        }
        if id.starts_with(given) && !found.contains(&id) {
            found.push(id);
        }
    }
    match found.as_slice() {
        [] => Ok(given.to_owned()),
        [one] => Ok((*one).to_owned()),
        _ => Err(Fail::Failed(format!(
            "`{given}` names more than one {what}: {}",
            found.join(", ")
        ))),
    }
}

fn task_id(view: &View, given: &str) -> Result<String, Fail> {
    resolve(
        given,
        view.tasks.iter().map(|task| task.task.as_str()),
        "task",
    )
}

/// Prints an intent's receipt: `{"operation", "reference"}` and `extra`.
fn receipt(output: &Output, receipt: &Receipt, extra: Value, sentence: String) {
    let mut value = json!({"operation": receipt.operation, "reference": receipt.reference});
    if let (Value::Object(map), Value::Object(more)) = (&mut value, extra) {
        map.extend(more);
    }
    output.emit(&value, |_| sentence);
}

fn status(output: &Output, host: &mut Host) -> Result<u8, Fail> {
    let snapshot = snapshot(host)?;
    let view = &snapshot.view;
    output.emit(&json!(snapshot), |_| {
        let mut out = Vec::new();
        if view.goals.is_empty() {
            out.push("No goals yet.".to_owned());
        } else {
            out.push("Goals:".to_owned());
            for goal in &view.goals {
                out.push(format!(
                    "  {}  {}  {}/{} tasks over  {}  {}",
                    goal.goal,
                    word(&goal.status),
                    goal.final_tasks,
                    goal.total_tasks,
                    goal.spend.label(),
                    goal.text.lines().next().unwrap_or("")
                ));
            }
        }
        if view.seats.is_empty() {
            out.push("No seats yet: `openagents studio seat set NAME --route ROUTE`.".to_owned());
        } else {
            let mut rows = vec![vec![
                "SEAT".to_owned(),
                "ROLE".into(),
                "ROUTE".into(),
                "ACTIVITY".into(),
                "STATION".into(),
                "SPENT".into(),
                "TASK".into(),
            ]];
            for seat in &view.seats {
                let activity = if seat.paused {
                    format!("{} (paused)", word(&seat.activity))
                } else {
                    word(&seat.activity)
                };
                rows.push(vec![
                    seat.seat.clone(),
                    word(&seat.role),
                    seat.route.clone(),
                    activity,
                    word(&seat.station),
                    seat.spend.label(),
                    seat.task.as_deref().map_or("idle", short).to_owned(),
                ]);
            }
            out.push(crate::out::table(&rows));
        }
        out.push(match view.decisions.len() {
            0 => "No open decisions.".to_owned(),
            n => format!("{n} open decision(s): `openagents studio decisions`."),
        });
        out.join("\n")
    });
    Ok(0)
}

fn tasks(output: &Output, host: &mut Host, goal: Option<&str>) -> Result<u8, Fail> {
    let view = snapshot(host)?.view;
    let goal = match goal {
        Some(given) => Some(resolve(
            given,
            view.goals.iter().map(|goal| goal.goal.as_str()),
            "goal",
        )?),
        None => None,
    };
    let rows: Vec<_> = view
        .tasks
        .iter()
        .filter(|task| goal.as_ref().is_none_or(|goal| &task.goal == goal))
        .collect();
    output.emit(&json!({"tasks": rows}), |_| {
        if rows.is_empty() {
            return "No tasks.".into();
        }
        let mut table = vec![vec![
            "TASK".to_owned(),
            "GOAL".into(),
            "ENTRY".into(),
            "SEAT".into(),
            "STATUS".into(),
            "AFTER".into(),
            "SPENT".into(),
            "TITLE".into(),
        ]];
        for task in &rows {
            table.push(vec![
                short(&task.task).to_owned(),
                task.goal.clone(),
                task.entry.clone(),
                task.seat.clone(),
                word(&task.status),
                task.depends_on.join(","),
                task.spend.label(),
                task.title.clone(),
            ]);
        }
        crate::out::table(&table)
    });
    Ok(0)
}

/// `at`, Unix seconds, as a UTC time of day.
fn clock(at: u64) -> String {
    let day = at % 86_400;
    format!("{:02}:{:02}:{:02}", day / 3600, (day % 3600) / 60, day % 60)
}

fn log(output: &Output, host: &mut Host, seat: &str) -> Result<u8, Fail> {
    let view = snapshot(host)?.view;
    let seat = seat.trim_start_matches('@');
    if !view.seats.iter().any(|item| item.seat == seat) {
        return Err(Fail::Failed(format!(
            "no seat is `{seat}`; `openagents studio status` lists them"
        )));
    }
    let found = view.logs.iter().find(|log| log.seat == seat);
    output.emit(&json!({"seat": seat, "log": found}), |_| {
        let Some(log) = found.filter(|log| !log.lines.is_empty()) else {
            return format!("Seat {seat} has no log yet.");
        };
        let mut out = vec![match &log.task {
            Some(task) => format!("Seat {seat}, task {}:", short(task)),
            None => format!("Seat {seat}:"),
        }];
        for line in &log.lines {
            out.push(format!(
                "  {} UTC  {:<8}  {}",
                clock(line.at),
                word(&line.activity),
                line.text
            ));
        }
        out.join("\n")
    });
    Ok(0)
}

fn render_decision(decision: &Decision) -> String {
    let mut out = format!(
        "{}  {}  goal {}",
        short(&decision.decision),
        word(&decision.kind),
        decision.goal
    );
    if let Some(seat) = &decision.seat {
        out.push_str(&format!("  seat {seat}"));
    }
    for line in decision.text.lines() {
        out.push_str(&format!("\n    {line}"));
    }
    if let Some(step) = &decision.approval {
        out.push_str(&format!(
            "\n    step: {} `{}` in {} ({})",
            step.tool,
            step.command,
            step.cwd,
            step.risk.label()
        ));
        if let Some(rule) = &step.always {
            out.push_str(&format!("\n    --always keeps: {rule}"));
        }
    }
    out
}

fn decisions(output: &Output, host: &mut Host) -> Result<u8, Fail> {
    let view = snapshot(host)?.view;
    output.emit(&json!({"decisions": view.decisions}), |_| {
        if view.decisions.is_empty() {
            return "No open decisions.".into();
        }
        let mut out: Vec<String> = view.decisions.iter().map(render_decision).collect();
        out.push(
            "Answer with `openagents studio answer DECISION TEXT`: `allow` or `deny` for an \
             approval, a plan file (`--file PATH`) for a goal's plan decision."
                .into(),
        );
        out.join("\n")
    });
    Ok(0)
}

/// The answer's text: `--file PATH` (`-` for stdin), or the words.
fn answer_text(words: &[&str], args: &Args) -> Result<String, Fail> {
    match args.option("file") {
        Some(_) if !words.is_empty() => Err(Fail::Usage(
            "answer takes TEXT or --file PATH, not both".into(),
        )),
        Some("-") => {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin().lock(), &mut text)
                .map_err(|error| Fail::Failed(format!("cannot read stdin: {error}")))?;
            Ok(text)
        }
        Some(file) => std::fs::read_to_string(file)
            .map_err(|error| Fail::Failed(format!("cannot read {file}: {error}"))),
        None => Ok(words.join(" ")),
    }
}

fn answer(
    output: &Output,
    host: &mut Host,
    given: &str,
    words: &[&str],
    args: &Args,
) -> Result<u8, Fail> {
    let view = snapshot(host)?.view;
    let id = resolve(
        given,
        view.decisions.iter().map(|open| open.decision.as_str()),
        "decision",
    )?;
    let open = view
        .decisions
        .iter()
        .find(|open| open.decision == id)
        .ok_or_else(|| {
            Fail::Failed(format!(
                "no open decision is `{given}`; `openagents studio decisions` lists them"
            ))
        })?;
    let operation = if args.switch("always") {
        if !words.is_empty() || args.option("file").is_some() {
            return Err(Fail::Usage(
                "--always approves the step and takes no answer text".into(),
            ));
        }
        let step = open
            .approval
            .as_ref()
            .filter(|_| open.kind == DecisionKind::Approval)
            .ok_or_else(|| {
                Fail::Failed(format!(
                    "decision {} is not an approval of a named step; only one offers --always",
                    short(&open.decision)
                ))
            })?;
        let rule = step.always.clone().ok_or_else(|| {
            Fail::Failed(format!(
                "the host keeps no standing rule for this {} risk step; answer `allow` to \
                 approve it once",
                step.risk.label()
            ))
        })?;
        Operation::AllowAlways {
            decision: open.decision.clone(),
            based_on: open.based_on,
            rule,
            command: mint(),
            issued_at: now(),
        }
    } else {
        let text = answer_text(words, args)?;
        if text.trim().is_empty() {
            return Err(Fail::Usage("answer needs TEXT or --file PATH".into()));
        }
        Operation::AnswerDecision {
            decision: open.decision.clone(),
            based_on: open.based_on,
            text,
            command: mint(),
            issued_at: now(),
        }
    };
    let name = operation.name();
    let sent = dispatched(host.call(&operation)?, name)?;
    let always = args.switch("always");
    receipt(
        output,
        &sent,
        json!({"decision": open.decision, "kind": open.kind, "always": always}),
        if always {
            format!(
                "Approved {}; seat {} keeps the rule.",
                short(&open.decision),
                open.seat.as_deref().unwrap_or("its seat")
            )
        } else {
            format!("Answered {}.", short(&open.decision))
        },
    );
    Ok(0)
}

/// The task's review, read now.
fn open_review(host: &mut Host, task: &str) -> Result<TaskReview, Fail> {
    match host.call(&Operation::OpenReview {
        task: task.to_owned(),
    })? {
        Outcome::Review { review } => Ok(*review),
        _ => Err(unexpected("studio.review.open")),
    }
}

fn render_review(review: &TaskReview, diff: bool) -> String {
    let mut out = vec![format!(
        "Task {}: {} file(s) changed, +{} -{}{}",
        short(&review.task),
        review.files_total,
        review.added,
        review.removed,
        if review.uncounted > 0 {
            format!(", {} uncounted", review.uncounted)
        } else {
            String::new()
        }
    )];
    out.push(format!(
        "  base {}  head commit {}  tree {}",
        &review.base[..12.min(review.base.len())],
        &review.head_commit[..12.min(review.head_commit.len())],
        &review.head[..12.min(review.head.len())],
    ));
    for file in &review.files {
        let count = |n: Option<u64>| n.map_or_else(|| "?".to_owned(), |n| n.to_string());
        out.push(format!(
            "  {:<13} +{} -{}  {}",
            word(&file.status),
            count(file.added),
            count(file.removed),
            file.path
        ));
    }
    match &review.completeness {
        Completeness::Complete => {}
        other => out.push(format!("  The diff is not whole: {}", json!(other))),
    }
    if let Some(publication) = &review.publication {
        out.push(format!(
            "  Last publication: {}: {}",
            word(&publication.state),
            publication.note
        ));
    }
    if diff {
        out.push(String::new());
        out.push(review.diff.trim_end().to_owned());
    } else {
        out.push(format!(
            "Decide: `openagents studio merge {0}`, `request-changes {0} TEXT`, or `reject {0} \
             [REASON]`; --diff prints the diff.",
            short(&review.task)
        ));
    }
    out.join("\n")
}

fn review(output: &Output, host: &mut Host, given: &str, diff: bool) -> Result<u8, Fail> {
    let view = snapshot(host)?.view;
    let task = task_id(&view, given)?;
    let mut review = open_review(host, &task)?;
    let text = render_review(&review, diff);
    if !diff {
        review.diff.clear();
    }
    output.emit(&json!({"review": review}), |_| text);
    Ok(0)
}

/// Whether `given` names the review's tree or head commit, in full or
/// by a prefix of at least seven characters.
fn names_revision(review: &TaskReview, given: &str) -> bool {
    [&review.head, &review.head_commit]
        .iter()
        .any(|revision| *revision == given || (given.len() >= 7 && revision.starts_with(given)))
}

fn decide(
    output: &Output,
    host: &mut Host,
    given: &str,
    verdict: Verdict,
    text: &str,
    args: &Args,
) -> Result<u8, Fail> {
    if verdict == Verdict::RequestChanges && text.trim().is_empty() {
        return Err(Fail::Usage("request-changes needs TEXT".into()));
    }
    let view = snapshot(host)?.view;
    let task = task_id(&view, given)?;
    let review = open_review(host, &task)?;
    if let Some(head) = args.option("head")
        && !names_revision(&review, head)
    {
        return Err(Fail::Refused(Refusal::new(
            "studio.merge.decide",
            "stale",
            format!(
                "the task's change moved past {head}; read it again with `openagents studio \
                 review {}`",
                short(&task)
            ),
        )));
    }
    let operation = Operation::DecideMerge {
        decision: Box::new(MergeDecision {
            task: review.task.clone(),
            base: review.base.clone(),
            head_commit: review.head_commit.clone(),
            head: review.head.clone(),
            verdict,
            text: text.to_owned(),
            command: mint(),
            issued_at: now(),
        }),
    };
    let merged: Merged = match host.call(&operation)? {
        Outcome::Merged { merged } => *merged,
        _ => return Err(unexpected("studio.merge.decide")),
    };
    let refused = merged
        .publication
        .as_ref()
        .filter(|publication| publication.state == PublishState::Refused);
    output.emit(&json!({"merged": merged}), |_| match merged.verdict {
        Verdict::Merge => match &merged.publication {
            Some(publication) => format!(
                "Merge of task {} at {}: {}: {}",
                short(&merged.task),
                &merged.head_commit[..12.min(merged.head_commit.len())],
                word(&publication.state),
                publication.note
            ),
            None => format!("Merged task {}.", short(&merged.task)),
        },
        Verdict::RequestChanges => format!(
            "Sent the requested changes to task {}'s seat.",
            short(&merged.task)
        ),
        Verdict::Reject => format!(
            "Rejected task {}; its worktree stays until the task is archived.",
            short(&merged.task)
        ),
    });
    if let Some(publication) = refused {
        eprintln!(
            "openagents studio: the merge did not land: {}",
            publication.note
        );
        return Ok(crate::out::EXIT_FAILURE);
    }
    Ok(0)
}

fn seat_intent(output: &Output, host: &mut Host, verb: &str, seat: &str) -> Result<u8, Fail> {
    let seat = seat.trim_start_matches('@').to_owned();
    let (operation, done) = match verb {
        "pause" => (
            Operation::PauseSeat { seat: seat.clone() },
            "paused; it keeps its task and takes no new one",
        ),
        "resume" => (Operation::ResumeSeat { seat: seat.clone() }, "resumed"),
        "stop" => (
            Operation::StopSeat { seat: seat.clone() },
            "stopped; its task went back to the board and the seat is paused",
        ),
        other => return Err(Fail::Usage(format!("unknown seat command `{other}`"))),
    };
    let name = operation.name();
    let sent = dispatched(host.call(&operation)?, name)?;
    receipt(output, &sent, json!({}), format!("Seat {seat} {done}."));
    Ok(0)
}

fn task_intent(
    output: &Output,
    host: &mut Host,
    verb: &str,
    given: &str,
    seat: &str,
) -> Result<u8, Fail> {
    let view = snapshot(host)?.view;
    let task = task_id(&view, given)?;
    let operation = match verb {
        "cancel" => Operation::CancelStudioTask { task: task.clone() },
        "retry" => Operation::RetryTask { task: task.clone() },
        "prioritize" => Operation::PrioritizeTask { task: task.clone() },
        "reassign" => Operation::ReassignTask {
            task: task.clone(),
            seat: seat.trim_start_matches('@').to_owned(),
        },
        other => return Err(Fail::Usage(format!("unknown task command `{other}`"))),
    };
    let name = operation.name();
    let sent = dispatched(host.call(&operation)?, name)?;
    let sentence = match verb {
        "cancel" => format!("Cancelled task {}.", short(&task)),
        "retry" => format!(
            "Task {} is planned again as task {}.",
            short(&task),
            short(&sent.reference)
        ),
        "prioritize" => format!(
            "Task {} moves ahead of its goal's planned tasks.",
            short(&task)
        ),
        _ => format!("Task {} goes to seat {seat}.", short(&task)),
    };
    receipt(output, &sent, json!({"task": task}), sentence);
    Ok(0)
}

fn goal_submit(output: &Output, host: &mut Host, text: &str, args: &Args) -> Result<u8, Fail> {
    let workspace = args
        .option("workspace")
        .ok_or_else(|| Fail::Usage("goal submit needs --workspace LABEL".into()))?;
    let operation = Operation::SubmitGoal {
        text: text.to_owned(),
        workspace: workspace.to_owned(),
        lead: args.option("lead").map(str::to_owned),
    };
    let sent = dispatched(host.call(&operation)?, "studio.goal.submit")?;
    let goal = sent.reference.clone();
    // The lead's task, when the studio shows it already.
    let lead = snapshot(host).ok().and_then(|snapshot| {
        snapshot
            .view
            .tasks
            .into_iter()
            .find(|task| task.goal == goal && task.entry == "lead")
    });
    let sentence = match &lead {
        Some(task) => format!(
            "Goal {goal} submitted; seat {} plans it as task {}.",
            task.seat,
            short(&task.task)
        ),
        None => format!("Goal {goal} submitted; its lead plans it."),
    };
    receipt(
        output,
        &sent,
        json!({
            "goal_id": goal,
            "lead": lead.map(|task| json!({"seat": task.seat, "task_id": task.task})),
        }),
        sentence,
    );
    Ok(0)
}

fn message(output: &Output, host: &mut Host, seat: &str, text: &str) -> Result<u8, Fail> {
    let seat = match seat.trim_start_matches('@') {
        "everyone" | "all" => None,
        name => Some(name.to_owned()),
    };
    let operation = Operation::MessageSeat {
        seat: seat.clone(),
        text: text.to_owned(),
    };
    let sent = dispatched(host.call(&operation)?, "studio.seat.message")?;
    receipt(
        output,
        &sent,
        json!({}),
        format!(
            "Sent to {}: a running task reads it now when its engine reads steering; \
             otherwise its next briefing carries it.",
            seat.as_deref().unwrap_or("every seat")
        ),
    );
    Ok(0)
}

/// One `watch` line for a snapshot.
fn snapshot_line(snapshot: &Snapshot) -> (Value, String) {
    let view = &snapshot.view;
    (
        json!({
            "kind": "snapshot",
            "stream": snapshot.stream,
            "sequence": snapshot.sequence,
            "view": view,
        }),
        format!(
            "[{}] studio: {} goal(s), {} seat(s), {} task(s), {} open decision(s)",
            snapshot.sequence,
            view.goals.len(),
            view.seats.len(),
            view.tasks.len(),
            view.decisions.len()
        ),
    )
}

/// One `watch` line for an update: what changed, item by item.
fn update_line(update: &Update) -> (Value, String) {
    let put = &update.put;
    let mut out = Vec::new();
    let at = update.sequence;
    for goal in &put.goals {
        out.push(format!(
            "[{at}] goal {}: {} ({}/{} tasks over)",
            goal.goal,
            word(&goal.status),
            goal.final_tasks,
            goal.total_tasks
        ));
    }
    for seat in &put.seats {
        out.push(format!(
            "[{at}] seat {}: {} at the {}{}{}",
            seat.seat,
            word(&seat.activity),
            word(&seat.station),
            seat.task
                .as_deref()
                .map_or_else(String::new, |task| format!(", task {}", short(task))),
            if seat.paused { ", paused" } else { "" }
        ));
    }
    for task in &put.tasks {
        out.push(format!(
            "[{at}] task {} ({} on {}): {}",
            short(&task.task),
            task.entry,
            task.seat,
            word(&task.status)
        ));
    }
    for decision in &put.decisions {
        out.push(format!(
            "[{at}] decision {} ({}): {}",
            short(&decision.decision),
            word(&decision.kind),
            decision.text.lines().next().unwrap_or("")
        ));
    }
    for log in &put.logs {
        if let Some(line) = log.lines.last() {
            out.push(format!("[{at}] {}: {}", log.seat, line.text));
        }
    }
    for message in &put.messages {
        out.push(format!(
            "[{at}] message to {}: {}",
            message.seat,
            message.delivery()
        ));
    }
    for entry in &put.memory {
        out.push(format!("[{at}] memory: {}", entry.text));
    }
    for removed in &update.removed {
        out.push(format!(
            "[{at}] removed {} {}",
            word(&removed.kind),
            short(&removed.id)
        ));
    }
    if out.is_empty() {
        out.push(format!("[{at}] studio changed"));
    }
    (
        json!({
            "kind": "update",
            "stream": update.stream,
            "from": update.from,
            "sequence": update.sequence,
            "put": put,
            "removed": update.removed,
        }),
        out.join("\n"),
    )
}

/// `watch`: the studio as one snapshot line, then one line per change,
/// read every `--interval SECONDS` (default 1) until interrupted or
/// `--limit N` lines are printed. A host that started again, or a missed
/// update, prints a fresh snapshot line.
fn watch(output: &Output, host: &mut Host, args: &Args) -> Result<u8, Fail> {
    let interval: u64 = args.number("interval", 1).map_err(Fail::Usage)?;
    let limit: u64 = args.number("limit", 0).map_err(Fail::Usage)?;
    let mut mirror = Mirror::default();
    let mut printed = 0u64;
    let mut stale = 0u32;
    loop {
        let operation = mirror.next();
        let name = operation.name();
        match host.call(&operation) {
            Ok(outcome) => match mirror.accept(&outcome) {
                Ok(changed) => {
                    stale = 0;
                    let line = match &outcome {
                        Outcome::Studio { snapshot } => Some(snapshot_line(snapshot)),
                        Outcome::StudioUpdate { update } if changed => Some(update_line(update)),
                        _ => None,
                    };
                    if let Some((value, text)) = line {
                        output.line(&value, |_| text);
                        printed += 1;
                        if limit > 0 && printed >= limit {
                            return Ok(0);
                        }
                    }
                }
                Err(error) if error.code == Code::Stale && stale < 2 => {
                    stale += 1;
                    continue;
                }
                Err(error) => return Err(Fail::Refused(Refusal::access(name, &error))),
            },
            Err(refusal) if refusal.code == "stale" && stale < 2 => {
                stale += 1;
                mirror.refused(&coder_access::Error::new(Code::Stale, refusal.message));
                continue;
            }
            Err(refusal) => return Err(Fail::Refused(refusal)),
        }
        std::thread::sleep(Duration::from_secs(interval.max(1)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_unique_prefix_names_its_identity_and_an_ambiguous_one_is_refused() {
        let ids = ["abc123", "abd456", "zzz"];
        assert_eq!(
            resolve("abc", ids.iter().copied(), "task").ok().as_deref(),
            Some("abc123")
        );
        assert_eq!(
            resolve("zzz", ids.iter().copied(), "task").ok().as_deref(),
            Some("zzz")
        );
        // Unknown to the view: the host decides.
        assert_eq!(
            resolve("q1", ids.iter().copied(), "task").ok().as_deref(),
            Some("q1")
        );
        assert!(matches!(
            resolve("ab", ids.iter().copied(), "task"),
            Err(Fail::Failed(_))
        ));
    }

    #[test]
    fn identities_are_fresh_64_hex() {
        let (a, b) = (mint(), mint());
        assert_eq!(a.len(), 64);
        assert!(
            a.bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        );
        assert_ne!(a, b);
    }

    #[test]
    fn every_code_has_its_wire_spelling() {
        for code in [
            Code::Malformed,
            Code::Unsupported,
            Code::Forbidden,
            Code::MissingRight,
            Code::Expired,
            Code::Revoked,
            Code::Stale,
            Code::Conflict,
            Code::Bounds,
            Code::Unavailable,
            Code::Transport,
            Code::RateLimited,
            Code::WrongCode,
            Code::Denied,
        ] {
            assert_eq!(json!(code), json!(code_word(code)));
        }
    }
}
