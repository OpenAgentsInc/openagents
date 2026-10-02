//! The route policy every surface reads (#10207; plan sections 3 to 6).
//!
//! The worker's router judges a message with Jev and its own policy table
//! and says so beside the reply: the judgment's route and tier, typed
//! offers, a proposed command, a dispatch plan ([`Meta`]). Before this
//! module each surface read those itself. Now one function reads them,
//! [`propose`], into the router contract's [`RouteResult`]; [`admit`]
//! builds the immutable [`AdmissionSnapshot`] for it; and every message the
//! terminal and `openagents chat` send leaves a [`RouteRecord`] in the
//! thread's route journal ([`Journal`]) that the task owner's dispositions,
//! cost, and wall time are copied into. The desktop, the phone, and the
//! future HTTP adapter read the same functions; [`crate::delegation::offered`]
//! is [`coder_offered`].
//!
//! Precedence, as the surfaces kept it (#10073, #10170):
//!
//! 1. **Coder** when [`coder_offered`]: an explicit `run_coder` offer, or
//!    the computer lane on a `work.dispatch` reply with no other typed
//!    action. A plan of several runs is a dispatch plan of N runs; a
//!    single run on a thread whose local task still works steers it, and
//!    one whose turn ended continues it (13.3).
//! 2. **Plugin creation** when the reply carries a step of making a
//!    plugin (#10177) that runs here.
//! 3. **Local command** when the reply proposed one: its effect is read
//!    from this computer's own command tree, never from the worker
//!    (13.4); a command the tree does not know is refused.
//! 4. A typed **screen**, deck, or Gym **program** offer.
//! 5. The judgment's route: `eval.author` is the plugin-creation flow,
//!    `capability.missing` a missing capability, `clarify` a question,
//!    `refuse` a refusal.
//! 6. Otherwise an **answer**: prepared, grounded, or the model's.
//!
//! Standing rules (13.9) have no reading on the wire yet (#10157): a
//! [`RouteResult::StandingRule`] goes through [`admit`] and the journal
//! like any other route when it comes.
//!
//! The policy reads exact enum words and typed fields only, never the
//! message's text (the text is digested, and cut into a brief).

use std::io::Write as _;
use std::path::{Path, PathBuf};

use route_contract::lifecycle::Lifecycle;
use route_contract::record::{RouteRecord, RunOutcome};
use route_contract::route::{
    AnswerSource, Chosen, ClarifyReason, Continuation, ContinueHow, DispatchPlan, Effect, FanOut,
    LocalAction, PlannedRun, PluginRoute, RefusalReason, Remedy, RouteResult, RunMode, Summary,
    TaskClass,
};
use route_contract::snapshot::{
    Access, AdmissionSnapshot, ByokMode, Caller, CallerKind, CapabilityPin, Capacity, CheckScope,
    CommandScope, ContentClass, DefaultApplied, DefaultKind, Deliverable, Disclosure, Effects,
    Evidence, Funding, GrantRef, GrantSource, Identity, Input, Money, Network, OsDenySet, Payer,
    PayerEntry, Placement, Publication, QuestionSet, ReadScope, Recipient, RecipientKind, Resource,
    Resources, Route, Surface, WorkspaceBinding, WriteScope,
};
use route_contract::{Digest, digest_of};

use crate::router::{Meta, Offer, Screen};

pub use route_contract::RouteFamily;

/// This policy's version, named in every snapshot.
pub const POLICY: &str = "route-policy-v1";

/// The delegate recipe every Coder route applies by default (13.10,
/// #10208): the Jev briefing, Jev-chosen knowledge, effort matched to the
/// task class, the lean tools, system prompt and five-minute cache where the
/// engine allows them, and frozen checks with an early stop
/// ([`route_contract::recipe`]). The route's adapter is the digest of the
/// rows its runs' engines use.
pub use route_contract::recipe::RECIPE_VERSION;

/// What this computer is called in a placement.
pub const THIS_COMPUTER: &str = "this-computer";

/// The longest brief a plugin-creation or missing-capability route keeps.
const MAX_BRIEF: usize = 400;

/// What the worker said about one message.
#[derive(Clone, Copy, Debug)]
pub struct Reading<'a> {
    pub meta: Option<&'a Meta>,
    /// The worker judged the thread a computer's (`Snapshot::computer`).
    pub computer_lane: bool,
    /// The person's message.
    pub text: &'a str,
    /// The reply's text (a clarifying question is the reply).
    pub reply: &'a str,
}

/// The thread's Coder task on this computer, as the client found it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bound {
    pub task: String,
    /// Its turn still works: a message steers it. Otherwise the turn
    /// ended and a message is its next turn.
    pub working: bool,
    /// The task revision read, when the store said.
    pub revision: Option<u64>,
}

/// Where and for whom a message is routed: what code knows, never Jev.
#[derive(Clone, Debug)]
pub struct Situation {
    pub surface: Surface,
    /// The caller's identity (a device key's public half), or `local`.
    pub caller: String,
    pub request: String,
    pub thread: Option<String>,
    /// Where Coder runs: [`THIS_COMPUTER`], or another computer's label.
    pub computer: String,
    /// The project and checkout a run here works in.
    pub project: Option<WorkspaceBinding>,
    /// A run could start here now (an allowed agent signed in with
    /// capacity, in a project).
    pub ready: bool,
    pub bound: Option<Bound>,
    /// What a Coder run's check establishes on this computer.
    pub check: CheckScope,
}

/// Whether a reply offers Coder: the precedence every surface shares
/// (#10073). An explicit `run_coder` offer does; else a typed offer or
/// card for another action is what the router chose; else the computer
/// lane offers it, but only for a reply on `work.dispatch`, the router's
/// typed route for work a computer does (#10079). Opening Computers is
/// the dispatch's own "connect a computer" offer, not another action.
#[must_use]
pub fn coder_offered(meta: Option<&Meta>, computer_lane: bool) -> bool {
    if meta.is_some_and(|meta| meta.offers.contains(&Offer::RunCoder)) {
        return true;
    }
    // A step of making a plugin (#10177) starts Coder only with its own Run
    // Coder offer; every other step runs here, whatever the route read.
    if meta.is_some_and(|meta| meta.plugin.is_some()) {
        return false;
    }
    let other = meta.is_some_and(|meta| {
        !meta.cards.is_empty()
            || meta.offers.iter().any(|offer| {
                !matches!(
                    offer,
                    Offer::OpenScreen {
                        screen: Screen::Computers
                    }
                )
            })
    });
    computer_lane
        && !other
        && meta.is_some_and(|meta| meta.route.as_deref() == Some(crate::delegation::DISPATCH_ROUTE))
}

/// The route a reply calls for, by the precedence above. `effect` reads a
/// proposed command's effect from this computer's own command tree
/// (`None` when the tree does not know it).
#[must_use]
pub fn propose(
    reading: &Reading<'_>,
    situation: &Situation,
    effect: &dyn Fn(&[String]) -> Option<Effect>,
) -> RouteResult {
    let meta = reading.meta;
    if coder_offered(meta, reading.computer_lane) {
        return RouteResult::Coder {
            plan: dispatch_plan(meta, situation, reading.text),
        };
    }
    let Some(meta) = meta else {
        return RouteResult::Answer {
            source: AnswerSource::Model,
        };
    };
    // A step of making a plugin that runs here (#10177): the creation flow.
    if meta.plugin.is_some() {
        return RouteResult::Plugin {
            plugin: PluginRoute::Create {
                brief: brief(reading.text),
            },
        };
    }
    if let Some(argv) = &meta.command {
        return match effect(argv) {
            Some(effect) => RouteResult::LocalCommand {
                action: LocalAction::Command {
                    argv: argv.clone(),
                    effect,
                },
            },
            None => RouteResult::Refusal {
                reason: RefusalReason::RouteNotAllowed,
            },
        };
    }
    for offer in &meta.offers {
        match offer {
            Offer::OpenScreen { screen } => {
                return RouteResult::LocalCommand {
                    action: LocalAction::Screen {
                        screen: screen.word().to_owned(),
                        target: None,
                    },
                };
            }
            Offer::OpenPresentation { deck } => {
                return RouteResult::LocalCommand {
                    action: LocalAction::Screen {
                        screen: "presentation".to_owned(),
                        target: Some(deck.clone()),
                    },
                };
            }
            Offer::StartEval { body } | Offer::PublishEval { body } => {
                let id = match offer {
                    Offer::StartEval { .. } => "gym.start_eval",
                    _ => "gym.publish_eval",
                };
                return RouteResult::Plugin {
                    plugin: PluginRoute::Run {
                        capability: CapabilityPin {
                            id: id.to_owned(),
                            version: "nip-cj-2".to_owned(),
                            digest: digest_of(body),
                        },
                        arguments: body.clone(),
                    },
                };
            }
            Offer::RunCoder | Offer::Cli { .. } => {}
        }
    }
    match (meta.route.as_deref(), meta.tier.as_deref()) {
        (Some("eval.author"), _) | (_, Some("author")) => RouteResult::Plugin {
            plugin: PluginRoute::Create {
                brief: brief(reading.text),
            },
        },
        (Some("capability.missing"), _) => RouteResult::MissingCapability {
            need: brief(reading.text),
            remedy: Remedy::Build,
        },
        (Some("clarify"), _) => RouteResult::Clarification {
            question: brief(reading.reply),
            reason: ClarifyReason::AmbiguousIntent,
        },
        (Some("refuse"), _) | (_, Some("refuse")) => RouteResult::Refusal {
            reason: RefusalReason::Harmful,
        },
        _ => RouteResult::Answer {
            source: answer_source(meta),
        },
    }
}

/// The family [`propose`] gives a reply on its own, with no situation and
/// no command tree (a proposed command reads as refused): for a surface
/// handed a reply without its route.
#[must_use]
pub fn family(meta: Option<&Meta>, computer_lane: bool) -> RouteFamily {
    let situation = Situation {
        surface: Surface::Api,
        caller: String::new(),
        request: String::new(),
        thread: None,
        computer: THIS_COMPUTER.to_owned(),
        project: None,
        ready: false,
        bound: None,
        check: CheckScope::None,
    };
    let reading = Reading {
        meta,
        computer_lane,
        text: "",
        reply: "",
    };
    propose(&reading, &situation, &|_| None).family()
}

fn answer_source(meta: &Meta) -> AnswerSource {
    if meta.canned()
        && let Some(entry) = &meta.answer
    {
        return AnswerSource::Prepared {
            entry: entry.clone(),
        };
    }
    match (meta.tier.as_deref(), meta.route.as_deref()) {
        (Some("grounded"), Some(corpus)) => AnswerSource::Knowledge {
            corpus: corpus.to_owned(),
            citations: Vec::new(),
        },
        _ => AnswerSource::Model,
    }
}

fn brief(text: &str) -> String {
    let text = text.trim();
    match text.char_indices().nth(MAX_BRIEF) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.to_owned(),
    }
}

/// The dispatch plan a Coder reply carries: the worker's plan of N runs
/// (#10183), else one run on the engine the person named, the engine this
/// computer predicts, or whichever its settings choose; a single run on a
/// bound local task continues it.
fn dispatch_plan(meta: Option<&Meta>, situation: &Situation, text: &str) -> DispatchPlan {
    let input = Digest::of_bytes(text.as_bytes());
    let plan = meta.and_then(|meta| meta.plan.clone());
    let read_only = plan.as_ref().is_some_and(|plan| plan.read_only);
    let mode = if read_only {
        RunMode::ReadOnly
    } else {
        RunMode::Write
    };
    let class = if read_only {
        TaskClass::Exploration
    } else {
        TaskClass::RepositoryChange
    };
    let summary = if plan.as_ref().is_some_and(|plan| plan.summarize) {
        Summary::Compose
    } else {
        Summary::None
    };
    if let Some(plan) = plan.filter(|plan| !plan.is_single()) {
        return DispatchPlan {
            class,
            fan_out: FanOut::OnePerEngine,
            runs: plan
                .runs
                .iter()
                .map(|&engine| PlannedRun {
                    engine: crate::client::agent_word(engine).to_owned(),
                    chosen: Chosen::Default,
                    mode,
                    input: input.clone(),
                    continuation: None,
                })
                .collect(),
            summary,
        };
    }
    let named = meta.and_then(|meta| meta.engine);
    let predicted = meta
        .and_then(|meta| meta.runner.as_ref())
        .and_then(|runner| runner.provider());
    let (engine, chosen) = match (named, predicted) {
        (Some(engine), _) => (crate::client::agent_word(engine).to_owned(), Chosen::Named),
        (None, Some(provider)) => (provider.to_owned(), Chosen::Default),
        (None, None) => ("auto".to_owned(), Chosen::Default),
    };
    let continuation = situation.bound.as_ref().map(|bound| Continuation {
        task: bound.task.clone(),
        based_on: bound.revision.unwrap_or(0),
        how: if bound.working {
            ContinueHow::Steer
        } else {
            ContinueHow::NextTurn
        },
    });
    DispatchPlan {
        class,
        fan_out: FanOut::Single,
        runs: vec![PlannedRun {
            engine,
            chosen,
            mode,
            input,
            continuation,
        }],
        summary,
    }
}

/// `result`, a Coder route, as issue work on `number` (13.5): Jev reads
/// whether a coding message asks to work an issue only once the route is
/// Coder, so the class is refined before admission.
#[must_use]
pub fn issue_work(result: RouteResult, number: u64) -> RouteResult {
    match result {
        RouteResult::Coder { mut plan } => {
            // 0: pick an open issue nobody holds (#10206).
            plan.class = TaskClass::IssueWork {
                issue: (number != 0).then_some(number),
            };
            RouteResult::Coder { plan }
        }
        other => other,
    }
}

/// `record`, a Coder route, become issue work on `number` once Jev read
/// that the message asks for an issue: the same request, a new result, and
/// its snapshot with the issue flow's publication effects and gate, then
/// admitted. The issue flow starts at once today as it did before the
/// router; showing it as an offer first is phase 2's.
#[must_use]
pub fn reissue(record: &RouteRecord, number: u64, now_ms: u64) -> Option<RouteRecord> {
    let result = issue_work(record.result.clone(), number);
    let mut snapshot = record.snapshot.clone();
    snapshot.route.result = result.digest();
    snapshot.effects.publication = vec![
        Publication::IssueUpdate,
        Publication::Commit,
        Publication::Push,
    ];
    snapshot.evidence.deliverables = vec![Deliverable::Patch, Deliverable::LandedCommit];
    snapshot.evidence.check = CheckScope::IssueGate;
    let mut issue = RouteRecord::received(
        record.request.clone(),
        record.thread.clone(),
        result,
        snapshot,
        record.received_ms,
    )
    .ok()?;
    issue.step(Lifecycle::Admitted, "issue_work", now_ms).ok()?;
    Some(issue)
}

/// The provider a coding engine sends repository text to.
fn provider_of(engine: &str) -> &'static str {
    match engine {
        "codex" => "openai",
        "claude" => "anthropic",
        "grok" => "xai",
        "devin" => "cognition",
        "opencode" => "opencode",
        _ => "signed_in_engine",
    }
}

/// The immutable admission snapshot for `result` in `situation`, under
/// `parent` when it continues a run (13.3). `meta` names the question set
/// the worker judged with.
#[must_use]
pub fn admit(
    result: &RouteResult,
    situation: &Situation,
    meta: Option<&Meta>,
    text: &str,
    parent: Option<&AdmissionSnapshot>,
) -> AdmissionSnapshot {
    let family = result.family();
    let here = situation.computer == THIS_COMPUTER;
    let mut defaults = Vec::new();
    let mut runs: Vec<&PlannedRun> = Vec::new();
    let mut capability = None;
    let (effects, evidence, placement) = match result {
        RouteResult::Coder { plan } => {
            runs = plan.runs.iter().collect();
            let read_only = plan.runs.iter().all(|run| run.mode == RunMode::ReadOnly);
            let issue = matches!(plan.class, TaskClass::IssueWork { .. });
            if plan.runs.iter().any(|run| run.chosen == Chosen::Default) {
                defaults.push(DefaultApplied {
                    default: DefaultKind::SignedInEngines,
                    value: plan
                        .runs
                        .iter()
                        .map(|run| run.engine.as_str())
                        .collect::<Vec<_>>()
                        .join(","),
                });
            }
            if here {
                defaults.push(DefaultApplied {
                    default: DefaultKind::ThisComputer,
                    value: THIS_COMPUTER.to_owned(),
                });
            }
            if let Some(project) = &situation.project {
                defaults.push(DefaultApplied {
                    default: DefaultKind::CurrentProject,
                    value: project.project.clone(),
                });
            }
            if plan.runs.iter().any(|run| run.continuation.is_some()) {
                defaults.push(DefaultApplied {
                    default: DefaultKind::CurrentSession,
                    value: situation
                        .bound
                        .as_ref()
                        .map_or_else(String::new, |bound| bound.task.clone()),
                });
            }
            defaults.push(DefaultApplied {
                default: DefaultKind::DelegateSettings,
                value: RECIPE_VERSION.to_owned(),
            });
            let effects = Effects {
                reads: vec![ReadScope::Workspace, ReadScope::Toolchains],
                writes: if read_only {
                    WriteScope::None
                } else {
                    WriteScope::IsolatedWorktree
                },
                network: Network::Open,
                commands: CommandScope::EngineTools,
                publication: if issue {
                    vec![
                        Publication::IssueUpdate,
                        Publication::Commit,
                        Publication::Push,
                    ]
                } else {
                    Vec::new()
                },
                // The local settings' default access is full (#10104):
                // visible here, never implied.
                access: Access::Full,
                os_deny: OsDenySet::macos(),
            };
            let deliverables = if read_only || plan.runs.len() > 1 {
                vec![Deliverable::RunResults, Deliverable::RetainedArtifacts]
            } else if issue {
                vec![Deliverable::Patch, Deliverable::LandedCommit]
            } else {
                vec![Deliverable::Patch, Deliverable::RetainedArtifacts]
            };
            let evidence = Evidence {
                deliverables,
                check: if issue {
                    CheckScope::IssueGate
                } else {
                    situation.check
                },
                checker: None,
                retention_days: None,
            };
            let continues = plan.runs.iter().any(|run| run.continuation.is_some());
            let placement = Placement {
                computer: Some(situation.computer.clone()),
                workspace: situation.project.clone(),
                grant: Some(GrantRef {
                    id: format!("{}:coder", situation.computer),
                    epoch: 0,
                    source: if continues {
                        GrantSource::Continuation
                    } else {
                        GrantSource::Autostart
                    },
                }),
            };
            (effects, evidence, placement)
        }
        RouteResult::LocalCommand { action } => {
            let (commands, writes, deliverable) = match action {
                LocalAction::Command { effect, .. } => (
                    if *effect == Effect::ReadOnly {
                        CommandScope::ReadOnlyTree
                    } else {
                        CommandScope::Bounded
                    },
                    if *effect == Effect::ReadOnly {
                        WriteScope::None
                    } else {
                        WriteScope::Workspace
                    },
                    Deliverable::CommandOutput,
                ),
                LocalAction::Screen { .. } => {
                    (CommandScope::None, WriteScope::None, Deliverable::Answer)
                }
            };
            let effects = Effects {
                reads: vec![ReadScope::SystemDirectories],
                writes,
                network: Network::Open,
                commands,
                publication: Vec::new(),
                access: Access::Boundary,
                os_deny: OsDenySet::macos(),
            };
            let evidence = Evidence {
                deliverables: vec![deliverable],
                check: CheckScope::ExecutorExit,
                checker: None,
                retention_days: None,
            };
            let placement = Placement {
                computer: Some(situation.computer.clone()),
                workspace: None,
                grant: None,
            };
            (effects, evidence, placement)
        }
        RouteResult::Plugin { plugin } => {
            let deliverable = match plugin {
                PluginRoute::Run {
                    capability: pin, ..
                } => {
                    capability = Some(pin.clone());
                    Deliverable::PluginOutput
                }
                PluginRoute::Create { .. } => Deliverable::PluginPackage,
            };
            let evidence = Evidence {
                deliverables: vec![deliverable],
                check: CheckScope::ModelJudgment,
                checker: None,
                retention_days: None,
            };
            (Effects::none(), evidence, Placement::empty())
        }
        RouteResult::StandingRule { .. } => {
            let mut effects = Effects::none();
            effects.publication = vec![Publication::Install];
            let evidence = Evidence {
                deliverables: vec![Deliverable::Rule],
                check: CheckScope::None,
                checker: None,
                retention_days: None,
            };
            let placement = Placement {
                computer: Some(situation.computer.clone()),
                workspace: None,
                grant: None,
            };
            (effects, evidence, placement)
        }
        RouteResult::Answer { .. }
        | RouteResult::MissingCapability { .. }
        | RouteResult::Clarification { .. }
        | RouteResult::Refusal { .. } => (
            Effects::none(),
            Evidence {
                deliverables: vec![Deliverable::Answer],
                check: CheckScope::None,
                checker: None,
                retention_days: None,
            },
            Placement::empty(),
        ),
    };
    // Every message went to the chat worker and its decision provider.
    let mut recipients = vec![
        Recipient {
            kind: RecipientKind::OpenAgents,
            id: "openagents".to_owned(),
        },
        Recipient {
            kind: RecipientKind::DecisionProvider,
            id: "typesafe".to_owned(),
        },
    ];
    let mut context = vec![ContentClass::Message, ContentClass::Thread];
    let mut artifacts = Vec::new();
    let mut payers = vec![
        PayerEntry {
            resource: Resource::Routing,
            payer: Payer::OpenAgents,
        },
        PayerEntry {
            resource: Resource::Decision,
            payer: Payer::OpenAgents,
        },
        PayerEntry {
            resource: Resource::ChatModel,
            payer: Payer::OpenAgents,
        },
    ];
    for run in &runs {
        let recipient = Recipient {
            kind: RecipientKind::ModelProvider,
            id: provider_of(&run.engine).to_owned(),
        };
        if !recipients.contains(&recipient) {
            recipients.push(recipient);
        }
        payers.push(PayerEntry {
            resource: Resource::Executor,
            payer: Payer::CallerLogin {
                engine: run.engine.clone(),
            },
        });
    }
    if !runs.is_empty() {
        context.extend([ContentClass::RepositorySource, ContentClass::CommandOutput]);
        artifacts.extend([ContentClass::Patch, ContentClass::RunSummary]);
    }
    let set = meta
        .and_then(|meta| meta.judgment.as_deref())
        .and_then(|judgment| serde_json::from_str::<serde_json::Value>(judgment).ok())
        .and_then(|judgment| judgment["set"].as_str().map(str::to_owned))
        .unwrap_or_else(|| crate::router::ROUTER.to_owned());
    let set_id = set.split('@').next().unwrap_or(&set).to_owned();
    AdmissionSnapshot {
        schema: route_contract::SNAPSHOT_SCHEMA.to_owned(),
        identity: Identity {
            caller: Caller {
                kind: CallerKind::AppUser,
                id: situation.caller.clone(),
            },
            surface: situation.surface,
            workspace: None,
            request: situation.request.clone(),
            thread: situation.thread.clone(),
            task: situation
                .bound
                .as_ref()
                .filter(|_| runs.iter().any(|run| run.continuation.is_some()))
                .map(|bound| bound.task.clone()),
            attempt: None,
        },
        input: Input {
            request: Digest::of_bytes(text.as_bytes()),
            source: None,
            instructions: Vec::new(),
            attachments: Vec::new(),
        },
        route: Route {
            family,
            result: result.digest(),
            explicit: runs.iter().any(|run| run.chosen == Chosen::Named),
            capability,
            adapter: (!runs.is_empty()).then(|| {
                let engines: Vec<&str> = runs.iter().map(|run| run.engine.as_str()).collect();
                route_contract::recipe::adapter_digest(&engines)
            }),
            executor_revision: None,
            model: None,
            policy: POLICY.to_owned(),
            question_set: QuestionSet {
                id: set_id,
                digest: Digest::of_bytes(set.as_bytes()),
            },
        },
        placement,
        effects,
        disclosure: Disclosure {
            recipients,
            context,
            artifacts,
        },
        resources: Resources {
            wall_secs: None,
            memory_bytes: None,
            max_parallel: (!runs.is_empty()).then(|| u32::try_from(runs.len()).unwrap_or(u32::MAX)),
            capacity: if runs.is_empty() {
                Capacity::NotNeeded
            } else if situation.ready {
                Capacity::Available
            } else {
                Capacity::Unknown
            },
            caller_limits: Vec::new(),
        },
        money: Money {
            byok: ByokMode::Ours,
            payers,
            funding: Funding::None,
            price_book: None,
            quote: None,
            fees: Vec::new(),
            reservation: None,
            settlement: None,
            // Our own apps record costs and never show them (13.6).
            shown: false,
        },
        evidence,
        defaults_applied: defaults,
        inherits: parent.map(AdmissionSnapshot::digest),
    }
}

trait EmptyPlacement {
    fn empty() -> Self;
}

impl EmptyPlacement for Placement {
    fn empty() -> Self {
        Placement {
            computer: None,
            workspace: None,
            grant: None,
        }
    }
}

/// Unix milliseconds now.
#[must_use]
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Each thread's route records on this computer, one JSON line per write
/// in `<dir>/<thread>.jsonl`: the latest line of a request is its record.
/// Kept beside the task store (`~/.openagents/routes` for
/// `~/.openagents/tasks`), private to this computer: the snapshot names
/// the checkout's path.
#[derive(Clone, Debug)]
pub struct Journal {
    dir: PathBuf,
}

/// The longest journal line read back; a longer one is set aside.
const MAX_LINE: usize = 1024 * 1024;

impl Journal {
    #[must_use]
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The journal beside the task store `store`.
    #[must_use]
    pub fn beside(store: &Path) -> Self {
        Self::at(store.with_file_name("routes"))
    }

    fn path(&self, thread: &str) -> Option<PathBuf> {
        let safe = !thread.is_empty()
            && thread.len() <= 128
            && thread
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        safe.then(|| self.dir.join(format!("{thread}.jsonl")))
    }

    /// Append `record`'s current state to its thread's journal.
    ///
    /// # Errors
    ///
    /// A record with no thread or an unsafe thread name, or a failed write.
    pub fn write(&self, record: &RouteRecord) -> std::io::Result<()> {
        let path = record
            .thread
            .as_deref()
            .and_then(|thread| self.path(thread))
            .ok_or_else(|| std::io::Error::other("a route record names no thread"))?;
        std::fs::create_dir_all(&self.dir)?;
        let mut line = serde_json::to_vec(record).map_err(std::io::Error::other)?;
        line.push(b'\n');
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        file.write_all(&line)?;
        file.sync_data()
    }

    /// The thread's records, the latest of each request, in the order the
    /// requests were first routed.
    #[must_use]
    pub fn records(&self, thread: &str) -> Vec<RouteRecord> {
        let Some(text) = self
            .path(thread)
            .and_then(|path| std::fs::read_to_string(path).ok())
        else {
            return Vec::new();
        };
        let mut out: Vec<RouteRecord> = Vec::new();
        for line in text.lines().filter(|line| line.len() <= MAX_LINE) {
            let Ok(record) = serde_json::from_str::<RouteRecord>(line) else {
                continue;
            };
            match out.iter_mut().find(|kept| kept.request == record.request) {
                Some(kept) => *kept = record,
                None => out.push(record),
            }
        }
        out
    }

    /// The latest record of `request` in `thread`.
    #[must_use]
    pub fn latest(&self, thread: &str, request: &str) -> Option<RouteRecord> {
        self.records(thread)
            .into_iter()
            .find(|record| record.request == request)
    }

    /// The latest record whose runs include `task`: what a continuation
    /// inherits from.
    #[must_use]
    pub fn of_task(&self, thread: &str, task: &str) -> Option<RouteRecord> {
        self.records(thread)
            .into_iter()
            .rev()
            .find(|record| record.runs.iter().any(|run: &RunOutcome| run.task == task))
    }
}

/// Whether a record waits for the person (an offer, or a command to
/// confirm) rather than having run.
#[must_use]
pub fn waiting(record: &RouteRecord) -> bool {
    record.state == Lifecycle::Proposed
}

#[cfg(test)]
mod tests;
