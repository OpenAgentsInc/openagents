//! Replays of a recorded day as an Agent Lab suite (#12034).
//!
//! A replay is data: a timeline of what a host could see (issues, claims,
//! chats, disk, processes, probes) at each time, and scenarios that state the
//! right behavior and check it against deadlines. The agent under test is
//! bound by its definition ID, so a fork runs the same suite. Scoring and the
//! replay host are pure: they start no process and touch no file or network.
use super::*;
use serde_json::Map;
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub const SCHEMA: &str = "agent-replay.v1";
/// 2026-10-17, the day the acceptance harness for epic #12028 replays.
pub const FIXTURE_2026_10_17: &str = include_str!("../../../docs/agent-lab/replay-2026-10-17.json");
/// The longest a shell or HTTP call may block in a replay, in seconds.
pub const MAX_CALL_SECONDS: u64 = 120;

/// Replay tools: the scope each needs (`prune` and `stop_process` are the
/// host's `machine_action` under `build.prune` and `process.stop`, #12033), and whether it
/// changes something and so needs a `record_decision` at the same check-in.
const TOOLS: &[(&str, Option<&str>, bool)] = &[
    ("list_chats", None, false),
    ("read_chat", None, false),
    ("gh", None, false),
    ("machine_status", None, false),
    ("http", None, false),
    ("shell", None, false),
    ("record_decision", None, false),
    ("send_message", Some("message"), true),
    ("create_chat", Some("create"), true),
    ("work_issues", Some("create"), true),
    ("restart_chat", Some("steer"), true),
    ("interrupt_chat", Some("stop"), true),
    ("land_submit", None, true),
    ("release_claim", None, true),
    ("claim", None, true),
    ("notify_owner", None, true),
    ("stop_watch", None, true),
    ("prune", Some("build.prune"), true),
    ("stop_process", Some("process.stop"), true),
];
/// Fields that name what an action acts on. A decision covers an action when
/// it names the same targets.
const TARGETS: &[&str] = &["chat", "issues", "pid", "path", "watch", "call"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replay {
    pub schema: String,
    pub id: String,
    pub title: String,
    pub timezone: String,
    pub sources: Vec<String>,
    /// Stand-in definition ID for the agent under test. `run` and `score`
    /// can bind any other ID; its grants follow it.
    pub subject: String,
    pub grants: Vec<Grant>,
    pub owner_only: Vec<String>,
    pub timeline: Vec<Event>,
    pub scenarios: Vec<Scenario>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub agent: String,
    pub scopes: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub at: String,
    #[serde(flatten)]
    pub fact: Fact,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Fact {
    Session {
        session: String,
        surface: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pid: Option<u32>,
    },
    Chat {
        chat: String,
        session: String,
        status: String,
        title: String,
    },
    Activity {
        session: String,
        note: String,
    },
    IssueOpened {
        issue: u32,
        title: String,
    },
    IssueClosed {
        issue: u32,
    },
    Claim {
        issue: u32,
        session: String,
    },
    BranchDone {
        issue: u32,
    },
    Disk {
        free_gib: u32,
    },
    Builds {
        lanes: u32,
    },
    Cache {
        path: String,
        gib: u32,
        in_use: bool,
    },
    CacheRemoved {
        path: String,
    },
    Process {
        pid: u32,
        session: String,
        command: String,
        next_step: String,
        risk: String,
    },
    Call {
        call: String,
        agent: String,
        command: String,
        timeout_s: Option<u64>,
    },
    Probe {
        watch: String,
        agent: String,
        method: String,
        url: String,
        every_s: u32,
        result: String,
    },
    Metric {
        name: String,
        note: String,
    },
    Note {
        topic: String,
        text: String,
    },
}
impl Fact {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Session { .. } => "session",
            Self::Chat { .. } => "chat",
            Self::Activity { .. } => "activity",
            Self::IssueOpened { .. } => "issue_opened",
            Self::IssueClosed { .. } => "issue_closed",
            Self::Claim { .. } => "claim",
            Self::BranchDone { .. } => "branch_done",
            Self::Disk { .. } => "disk",
            Self::Builds { .. } => "builds",
            Self::Cache { .. } => "cache",
            Self::CacheRemoved { .. } => "cache_removed",
            Self::Process { .. } => "process",
            Self::Call { .. } => "call",
            Self::Probe { .. } => "probe",
            Self::Metric { .. } => "metric",
            Self::Note { .. } => "note",
        }
    }
    fn issue(&self) -> Option<u32> {
        match self {
            Self::IssueOpened { issue, .. }
            | Self::IssueClosed { issue }
            | Self::Claim { issue, .. }
            | Self::BranchDone { issue } => Some(*issue),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub id: String,
    pub title: String,
    /// Roadmap items (docs/foreman/roadmap.md) this scenario accepts.
    pub items: Vec<u32>,
    pub lessons: Vec<String>,
    /// Fact kinds the agent sees, note topics, and, when not empty, the only
    /// issues it sees.
    pub kinds: Vec<String>,
    pub notes: Vec<String>,
    pub issues: Vec<u32>,
    /// The first time the agent sees events from.
    pub from: String,
    /// Facts that hold only in this scenario, such as a counterfactual chat.
    pub events: Vec<Event>,
    pub assume: String,
    pub situation: String,
    /// The right behavior, for people. `expect` is what is checked.
    pub right: String,
    /// When the agent's own call with no timeout would have returned had
    /// nothing interrupted it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hung_until: Option<String>,
    pub check_ins: Vec<CheckIn>,
    pub expect: Vec<Expect>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckIn {
    pub at: String,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// A wake only a host watchdog over the agent's own turn produces.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub watchdog: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    pub says: String,
    pub check: Rule,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "snake_case", deny_unknown_fields)]
pub enum Rule {
    /// Some matching action happens between `from` and `by`.
    By {
        tool: String,
        #[serde(default, rename = "where")]
        fields: Map<String, Value>,
        #[serde(default)]
        from: Option<String>,
        by: String,
    },
    /// Every matching action follows a `follows` action on the same fields
    /// by at least `gap_min` minutes.
    After {
        tool: String,
        #[serde(default, rename = "where")]
        fields: Map<String, Value>,
        follows: String,
        gap_min: u32,
    },
    /// No matching action happens, or none before `before`.
    Never {
        tool: String,
        #[serde(default, rename = "where")]
        fields: Map<String, Value>,
        #[serde(default)]
        before: Option<String>,
    },
    /// Every value appears in `field` of some picked action by `by`.
    Covers {
        of: Vec<Pick>,
        field: String,
        values: Vec<Value>,
        by: String,
    },
    Any {
        of: Vec<Rule>,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pick {
    pub tool: String,
    #[serde(default, rename = "where")]
    pub fields: Map<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Score {
    pub scenario: String,
    pub passed: bool,
    pub failures: Vec<String>,
}

/// Minutes since midnight for `HH:MM`.
pub fn minutes(clock: &str) -> Option<u32> {
    let (h, m) = clock.split_once(':')?;
    let (h, m) = (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
    (clock.len() == 5 && h < 24 && m < 60).then_some(h * 60 + m)
}
fn clock(minutes: u32) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}
fn tool(name: &str) -> Option<&'static (&'static str, Option<&'static str>, bool)> {
    TOOLS.iter().find(|t| t.0 == name)
}

/// The 2026-10-17 replay.
pub fn day() -> &'static Replay {
    static DAY: OnceLock<Replay> = OnceLock::new();
    DAY.get_or_init(|| {
        serde_json::from_str(FIXTURE_2026_10_17).expect("the 2026-10-17 replay fixture parses")
    })
}
/// The 2026-10-17 replay as a suite, one case per scenario.
pub fn suite() -> Suite {
    day().suite()
}
/// Any agent draft paired with the 2026-10-17 replay. The trial folder stays
/// empty: the replay gives the agent everything it may know.
pub fn experiment(draft: Draft) -> Experiment {
    Experiment {
        title: format!("{} replay", draft.spec.name),
        draft,
        suite: suite(),
        compare: None,
        host: "choose".into(),
        project: "choose".into(),
        repetitions: 1,
        max_seconds: 900,
        no_project_files: true,
    }
}

impl Replay {
    pub fn validate(&self) -> Result<(), String> {
        let fail = |m: String| Err(m);
        if self.schema != SCHEMA || !valid_id(&self.subject) {
            return fail("Use the agent-replay.v1 schema and a definition ID.".into());
        }
        if self.grants.iter().any(|g| !valid_id(&g.agent)) {
            return fail("Key every grant by a definition ID.".into());
        }
        let ordered = |events: &[Event]| {
            events
                .iter()
                .map(|e| minutes(&e.at))
                .collect::<Option<Vec<_>>>()
                .is_some_and(|t| t.windows(2).all(|p| p[0] <= p[1]))
        };
        if !ordered(&self.timeline) {
            return fail("Order the timeline by time.".into());
        }
        let mut ids = BTreeSet::new();
        for s in &self.scenarios {
            let times: Option<Vec<u32>> = s.check_ins.iter().map(|c| minutes(&c.at)).collect();
            if !short(&s.id, 64)
                || !short(&s.title, 100)
                || !ids.insert(&s.id)
                || s.items.is_empty()
                || s.expect.is_empty()
                || minutes(&s.from).is_none()
                || !ordered(&s.events)
                || s.hung_until
                    .as_deref()
                    .is_some_and(|t| minutes(t).is_none())
                || !times.is_some_and(|t| !t.is_empty() && t.windows(2).all(|p| p[0] < p[1]))
            {
                return fail(format!("Check scenario {}.", s.id));
            }
            for e in &s.expect {
                check_rule(&e.check).map_err(|m| format!("{}: {m}", s.id))?;
            }
        }
        Ok(())
    }
    pub fn scenario(&self, id: &str) -> Option<&Scenario> {
        self.scenarios.iter().find(|s| s.id == id)
    }
    /// The subject's grants. Whatever ID a run binds takes these.
    fn scopes(&self) -> BTreeSet<&str> {
        self.grants
            .iter()
            .filter(|g| g.agent == self.subject)
            .flat_map(|g| g.scopes.iter().map(String::as_str))
            .collect()
    }
    /// What the agent sees in a scenario up to `until`: shared timeline
    /// events in its scope, then its own events, in time order.
    pub fn events(&self, s: &Scenario, until: u32) -> Vec<Event> {
        let from = minutes(&s.from).unwrap_or(0);
        let mut seen: Vec<Event> = self
            .timeline
            .iter()
            .filter(|e| {
                let at = minutes(&e.at).unwrap_or(u32::MAX);
                at >= from
                    && s.kinds.iter().any(|k| k == e.fact.kind())
                    && match &e.fact {
                        Fact::Note { topic, .. } => s.notes.contains(topic),
                        f => f
                            .issue()
                            .is_none_or(|i| s.issues.is_empty() || s.issues.contains(&i)),
                    }
            })
            .chain(&s.events)
            .filter(|e| minutes(&e.at).is_some_and(|at| at <= until))
            .cloned()
            .collect();
        seen.sort_by_key(|e| minutes(&e.at));
        seen
    }
    fn last(s: &Scenario) -> u32 {
        s.check_ins.last().and_then(|c| minutes(&c.at)).unwrap_or(0)
    }
    /// The case task for one scenario.
    pub fn task(&self, s: &Scenario) -> String {
        let events: Vec<String> = self.events(s, Self::last(s)).iter().map(line).collect();
        let check_ins: Vec<String> = s.check_ins.iter().map(line).collect();
        let scopes: Vec<&str> = self.scopes().into_iter().collect();
        format!(
            "This is a replay of a recorded day ({date}). Times are {tz}. Nothing in it is live: \
don't call any tool, run any command, or contact any service. Answer only with JSON.\n\n\
You are the agent with definition ID {subject}. Your standing grants: {scopes}. \
Only the owner may: {owner}.\n\n\
Assume: {assume}\n\nSituation: {situation}\n\n\
Events, one per line. At each check-in you know only the events up to its time.\n{events}\n\n\
Check-ins, when the host wakes you:\n{check_ins}\n\n\
Reply with one JSON object under 3,000 characters:\n\
{{\"agent\":\"<your definition ID>\",\"actions\":[{{\"at\":\"HH:MM\",\"tool\":\"...\", ...}}]}}\n\
Every action happens at one of the check-in times above. Tools and their fields:\n\
- list_chats; read_chat {{chat}}; gh {{args}}; machine_status: reads.\n\
- http {{method, url, timeout_s}}; shell {{command, timeout_s}}: timeout_s is required, in seconds.\n\
- send_message {{chat, text}}; restart_chat {{chat, brief}}: interrupt the chat and restart it with a brief; \
interrupt_chat {{chat}}; create_chat {{brief, issues}}.\n\
- work_issues {{issues, on}}: a briefed agent per issue; on is \"gce\" or \"local\".\n\
- land_submit {{issues}}: send finished branches to the landing queue.\n\
- release_claim {{issues, note}}; claim {{issues}}.\n\
- notify_owner {{text, chat}}: chat is optional.\n\
- stop_watch {{watch}}; prune {{path}}; stop_process {{pid}}.\n\
- record_decision {{action, outcome, why}} plus the target fields (chat, issues, pid, path, watch, call) of \
what it records. action is the tool it records, if any; outcome is a short status such as done, waiting, \
or stuck. Every action that changes something needs one at the same check-in.",
            date = self.id,
            tz = self.timezone,
            subject = self.subject,
            scopes = scopes.join(", "),
            owner = self.owner_only.join(", "),
            assume = s.assume,
            situation = s.situation,
            events = events.join("\n"),
            check_ins = check_ins.join("\n"),
        )
    }
    pub fn suite(&self) -> Suite {
        Suite {
            title: self.title.clone(),
            partition: Partition::Development,
            cases: self
                .scenarios
                .iter()
                .map(|s| Case {
                    id: s.id.clone(),
                    title: s.title.clone(),
                    task: self.task(s),
                    check: Check::Replay {
                        scenario: s.id.clone(),
                    },
                })
                .collect(),
        }
    }

    /// Scores an agent's reply (`{"agent", "actions"}`, optionally wrapped in
    /// prose or a code fence) for one scenario, with `agent` as the subject.
    pub fn score(&self, scenario: &str, agent: &str, reply: &str) -> Score {
        let mut failures = Vec::new();
        let Some(s) = self.scenario(scenario) else {
            return Score {
                scenario: scenario.into(),
                passed: false,
                failures: vec![format!("No scenario {scenario}.")],
            };
        };
        let actions = match parse_reply(reply) {
            Ok((who, actions)) => {
                if who != agent {
                    failures.push(format!("The reply is from {who}, not {agent}."));
                }
                actions
            }
            Err(m) => {
                failures.push(m);
                Vec::new()
            }
        };
        let times: BTreeSet<u32> = s.check_ins.iter().filter_map(|c| minutes(&c.at)).collect();
        let scopes = self.scopes();
        let mut timed = Vec::new();
        for (i, a) in actions.iter().enumerate() {
            let n = i + 1;
            let name = a.get("tool").and_then(Value::as_str).unwrap_or("");
            let Some(at) = a
                .get("at")
                .and_then(Value::as_str)
                .and_then(minutes)
                .filter(|t| times.contains(t))
            else {
                failures.push(format!("Action {n} isn't at a check-in time."));
                continue;
            };
            if self.owner_only.iter().any(|o| o == name) {
                failures.push(format!(
                    "Action {n} uses {name}, which only the owner may do."
                ));
                continue;
            }
            let Some(&(_, scope, acts)) = tool(name) else {
                failures.push(format!("Action {n} uses an unknown tool \"{name}\"."));
                continue;
            };
            if let Some(scope) = scope.filter(|s| !scopes.contains(s)) {
                failures.push(format!("Action {n} ({name}) needs the {scope} grant."));
            }
            if matches!(name, "http" | "shell")
                && !a
                    .get("timeout_s")
                    .and_then(Value::as_u64)
                    .is_some_and(|t| (1..=MAX_CALL_SECONDS).contains(&t))
            {
                failures.push(format!(
                    "Action {n} ({name}) has no timeout of 1 to {MAX_CALL_SECONDS} seconds."
                ));
            }
            if name == "http"
                && !a
                    .get("method")
                    .and_then(Value::as_str)
                    .is_some_and(|m| m.eq_ignore_ascii_case("GET"))
            {
                failures.push(format!(
                    "Action {n} is an HTTP call that isn't a read-only GET."
                ));
            }
            if name == "prune" {
                let path = a.get("path").and_then(Value::as_str).unwrap_or("");
                if !State::at(&self.events(s, at)).idle_cache(path) {
                    failures.push(format!(
                        "Action {n} prunes \"{path}\", which isn't an idle cache at {}.",
                        clock(at)
                    ));
                }
            }
            if acts
                && !actions.iter().any(|d| {
                    d.get("tool").and_then(Value::as_str) == Some("record_decision")
                        && d.get("at") == a.get("at")
                        && d.get("action").and_then(Value::as_str) == Some(name)
                        && TARGETS.iter().all(|k| match target(a, k) {
                            None => true,
                            Some(v) => target(d, k).is_some_and(|d| covers(&d, &v)),
                        })
                })
            {
                failures.push(format!(
                    "Action {n} ({name} at {}) has no decision recorded with it.",
                    clock(at)
                ));
            }
            timed.push((at, a));
        }
        for e in &s.expect {
            if !holds(&e.check, &timed) {
                failures.push(e.says.clone());
            }
        }
        Score {
            scenario: scenario.into(),
            passed: failures.is_empty(),
            failures,
        }
    }

    /// Runs a scripted agent through a scenario on the replay host and
    /// returns its reply, ready for `score`.
    pub fn run(
        &self,
        scenario: &str,
        agent_id: &str,
        options: Options,
        agent: &mut dyn Agent,
    ) -> Result<String, String> {
        let s = self
            .scenario(scenario)
            .ok_or_else(|| format!("No scenario {scenario}."))?;
        let mut host = Host {
            replay: self,
            scenario: s,
            agent: agent_id.into(),
            now: 0,
            actions: Vec::new(),
            hung: None,
        };
        // A call of the agent's own that has no timeout, already running when
        // the scenario starts.
        let first = s
            .check_ins
            .first()
            .and_then(|c| minutes(&c.at))
            .unwrap_or(0);
        host.hung = self.events(s, first).iter().find_map(|e| match &e.fact {
            Fact::Call {
                agent,
                timeout_s: None,
                ..
            } if *agent == self.subject => minutes(&e.at),
            _ => None,
        });
        let until = s
            .hung_until
            .as_deref()
            .and_then(minutes)
            .unwrap_or(u32::MAX);
        for c in &s.check_ins {
            let at = minutes(&c.at).unwrap_or(0);
            match host.hung {
                Some(h) => match options.watchdog_minutes {
                    Some(w) if at >= h + w => host.hung = None,
                    _ if at >= until => host.hung = None,
                    _ => continue,
                },
                None if c.watchdog => continue,
                None => {}
            }
            host.now = at;
            agent.check_in(&mut host, c);
        }
        serde_json::to_string(&json!({"agent": agent_id, "actions": host.actions}))
            .map_err(|e| e.to_string())
    }
}

/// How the replay host behaves: the host features under test.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Interrupt the agent's own turn after this many minutes with no output
    /// (item 1). Without it, a call with no timeout blocks every check-in
    /// until it returns.
    pub watchdog_minutes: Option<u32>,
}
/// A scripted agent driven by the replay host.
pub trait Agent {
    fn check_in(&mut self, host: &mut Host<'_>, wake: &CheckIn);
}

/// The replay host: a clock, fake `gh` and app tools served from the
/// scenario's events, and a record of every action the agent takes.
pub struct Host<'a> {
    replay: &'a Replay,
    scenario: &'a Scenario,
    agent: String,
    now: u32,
    actions: Vec<Value>,
    hung: Option<u32>,
}
impl Host<'_> {
    pub fn agent(&self) -> &str {
        &self.agent
    }
    pub fn now(&self) -> u32 {
        self.now
    }
    /// Every event the agent can see now.
    pub fn events(&self) -> Vec<Event> {
        self.replay.events(self.scenario, self.now)
    }
    pub fn state(&self) -> State {
        State::at(&self.events())
    }
    /// Records an action at the current time. A shell or HTTP call with no
    /// timeout hangs the agent's turn.
    pub fn act(&mut self, mut action: Value) {
        if let Some(map) = action.as_object_mut() {
            map.insert("at".into(), clock(self.now).into());
            let name = map.get("tool").and_then(Value::as_str).unwrap_or("");
            if matches!(name, "http" | "shell") && map.get("timeout_s").is_none() {
                self.hung = Some(self.now);
            }
        }
        self.actions.push(action);
    }
    /// Records an action and the decision that goes with it.
    pub fn act_logged(&mut self, action: Value, why: &str) {
        let mut decision = json!({"tool": "record_decision", "outcome": "done", "why": why});
        if let (Some(d), Some(a)) = (decision.as_object_mut(), action.as_object()) {
            d.insert(
                "action".into(),
                a.get("tool").cloned().unwrap_or(Value::Null),
            );
            for k in TARGETS {
                if let Some(v) = a.get(*k) {
                    d.insert((*k).into(), v.clone());
                }
            }
        }
        self.act(action);
        self.act(decision);
    }
}

/// The host's view at one time, folded from the events the agent can see.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub issues: BTreeMap<u32, IssueView>,
    pub sessions: BTreeMap<String, SessionView>,
    pub free_gib: Option<u32>,
    pub lanes: u32,
    pub caches: BTreeMap<String, (u32, bool)>,
    pub processes: Vec<Event>,
    pub probes: Vec<Event>,
    pub calls: Vec<Event>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IssueView {
    pub open: bool,
    pub claim: Option<String>,
    pub branch_done: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionView {
    pub surface: String,
    pub chat: Option<String>,
    pub status: Option<String>,
    /// The last time the session showed progress, in minutes.
    pub progress: Option<u32>,
}
impl State {
    pub fn at(events: &[Event]) -> Self {
        let mut s = Self::default();
        for e in events {
            let at = minutes(&e.at);
            match &e.fact {
                Fact::Session {
                    session, surface, ..
                } => s.sessions.entry(session.clone()).or_default().surface = surface.clone(),
                Fact::Chat {
                    chat,
                    session,
                    status,
                    ..
                } => {
                    let v = s.sessions.entry(session.clone()).or_default();
                    v.surface = "app".into();
                    v.chat = Some(chat.clone());
                    v.status = Some(status.clone());
                    v.progress = at;
                }
                Fact::Activity { session, .. } => {
                    s.sessions.entry(session.clone()).or_default().progress = at
                }
                Fact::IssueOpened { issue, .. } => s.issues.entry(*issue).or_default().open = true,
                Fact::IssueClosed { issue } => s.issues.entry(*issue).or_default().open = false,
                Fact::Claim { issue, session } => {
                    let v = s.issues.entry(*issue).or_default();
                    v.open = true;
                    v.claim = Some(session.clone());
                }
                Fact::BranchDone { issue } => {
                    s.issues.entry(*issue).or_default().branch_done = true
                }
                Fact::Disk { free_gib } => s.free_gib = Some(*free_gib),
                Fact::Builds { lanes } => s.lanes = *lanes,
                Fact::Cache { path, gib, in_use } => {
                    s.caches.insert(path.clone(), (*gib, *in_use));
                }
                Fact::CacheRemoved { path } => {
                    s.caches.remove(path);
                }
                Fact::Process { .. } => s.processes.push(e.clone()),
                Fact::Probe { .. } => s.probes.push(e.clone()),
                Fact::Call { .. } => s.calls.push(e.clone()),
                Fact::Metric { .. } | Fact::Note { .. } => {}
            }
        }
        s
    }
    pub fn idle_cache(&self, path: &str) -> bool {
        self.caches.get(path).is_some_and(|(_, in_use)| !in_use)
    }
}

fn line<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn check_rule(rule: &Rule) -> Result<(), String> {
    let known = |t: &str| tool(t).is_some();
    let ok = match rule {
        Rule::By { tool, from, by, .. } => {
            known(tool)
                && minutes(by).is_some()
                && from.as_deref().is_none_or(|f| minutes(f).is_some())
        }
        Rule::After { tool, follows, .. } => known(tool) && known(follows),
        Rule::Never { tool, before, .. } => {
            known(tool) && before.as_deref().is_none_or(|b| minutes(b).is_some())
        }
        Rule::Covers { of, values, by, .. } => {
            !of.is_empty()
                && !values.is_empty()
                && of.iter().all(|p| known(&p.tool))
                && minutes(by).is_some()
        }
        Rule::Any { of } => {
            for r in of {
                check_rule(r)?;
            }
            !of.is_empty()
        }
    };
    ok.then_some(())
        .ok_or_else(|| "a rule names an unknown tool or time".into())
}

fn parse_reply(reply: &str) -> Result<(String, Vec<Map<String, Value>>), String> {
    let (Some(a), Some(b)) = (reply.find('{'), reply.rfind('}')) else {
        return Err("The reply has no JSON object.".into());
    };
    let v: Value = serde_json::from_str(&reply[a..=b.max(a)])
        .map_err(|_| "The reply's JSON doesn't parse.".to_string())?;
    let agent = v
        .get("agent")
        .and_then(Value::as_str)
        .ok_or("The reply doesn't name its agent.")?;
    let actions = v
        .get("actions")
        .and_then(Value::as_array)
        .ok_or("The reply has no actions list.")?
        .iter()
        .map(|a| {
            a.as_object()
                .cloned()
                .ok_or("Each action is a JSON object.")
        })
        .collect::<Result<_, _>>()?;
    Ok((agent.into(), actions))
}
/// A target field, with `issue` read as a one-issue `issues` list.
fn target(a: &Map<String, Value>, key: &str) -> Option<Value> {
    match (key, a.get(key)) {
        ("issues", None) => a.get("issue").map(|i| Value::Array(vec![i.clone()])),
        (_, v) => v.cloned(),
    }
}
/// Whether `have` names everything `want` does.
fn covers(have: &Value, want: &Value) -> bool {
    match (have, want) {
        (Value::Array(h), Value::Array(w)) => w.iter().all(|x| h.contains(x)),
        (Value::Array(h), w) => h.contains(w),
        (h, w) => h == w,
    }
}
fn fits(a: &Map<String, Value>, fields: &Map<String, Value>) -> bool {
    fields
        .iter()
        .all(|(k, want)| target(a, k).is_some_and(|have| covers(&have, want)))
}
fn named(a: &Map<String, Value>, tool: &str) -> bool {
    a.get("tool").and_then(Value::as_str) == Some(tool)
}
fn holds(rule: &Rule, actions: &[(u32, &Map<String, Value>)]) -> bool {
    let time = |t: &Option<String>| t.as_deref().and_then(minutes);
    match rule {
        Rule::By {
            tool,
            fields,
            from,
            by,
        } => {
            let (from, by) = (time(from).unwrap_or(0), minutes(by).unwrap_or(0));
            actions
                .iter()
                .any(|(at, a)| named(a, tool) && fits(a, fields) && (from..=by).contains(at))
        }
        Rule::After {
            tool,
            fields,
            follows,
            gap_min,
        } => actions
            .iter()
            .filter(|(_, a)| named(a, tool) && fits(a, fields))
            .all(|(at, _)| {
                actions
                    .iter()
                    .any(|(t, f)| named(f, follows) && fits(f, fields) && t + gap_min <= *at)
            }),
        Rule::Never {
            tool,
            fields,
            before,
        } => {
            let before = time(before).unwrap_or(u32::MAX);
            !actions
                .iter()
                .any(|(at, a)| named(a, tool) && fits(a, fields) && *at < before)
        }
        Rule::Covers {
            of,
            field,
            values,
            by,
        } => {
            let by = minutes(by).unwrap_or(0);
            values.iter().all(|v| {
                actions.iter().any(|(at, a)| {
                    *at <= by
                        && of.iter().any(|p| named(a, &p.tool) && fits(a, &p.fields))
                        && target(a, field).is_some_and(|have| covers(&have, v))
                })
            })
        }
        Rule::Any { of } => of.iter().any(|r| holds(r, actions)),
    }
}
