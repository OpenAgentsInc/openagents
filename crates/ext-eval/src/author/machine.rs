//! The interview as a state machine: the model proposes, the machine owns
//! the steps, the gates, and the floor.
//!
//! A turn is two calls. [`Interview::accept`] takes the person's event and
//! says what the model must propose next ([`Need`]); only an explicit
//! [`Event::Approve`] at a gate moves past it, and anything else at a gate
//! is a change request. [`Interview::apply`] takes the model's proposal for
//! that need, keeps what the floor allows, and returns the [`Turn`] to show:
//! the words, the fixed line of the step it stops at, and at most one
//! offer. A proposal can't move the interview past a gate, and nothing the
//! model writes is kept without the floor.

use nostr::cj_conversation::{
    Draft, DraftCase, MAX_DRAFT_CASES, Offer as WireOffer, Size, SubjectSource, SuiteSource, Where,
    parse_draft,
};
use nostr::contracts::ArtifactRef;
use nostr::eval_ext::{CaseKind, HOSTED_MAX_CASES};

use super::catalog::{Catalog, Source, Tool};
use super::floor;
use super::proposal::{
    CaseChecks, ChecksProposal, FixProposal, SayProposal, TestProposal, TestsProposal, ToolProposal,
};
use super::render::{
    self, case_id, files_outcome_grader, outcome_grader, prompt_md_in, proposed_kind,
    workspace_template,
};
use super::runner::{FULL_RUNS, TRY_RUNS, Tried};
use super::stage::{Stage, Surface};

/// The most characters of the model's words a turn keeps.
pub const MAX_SAY_CHARS: usize = 700;
/// The most characters of a made tool's guidance.
pub const MAX_SKILL_CHARS: usize = 16 * 1024;
/// Every run the interview offers compares with and without the plugin.
pub const ARMS: u64 = 2;

/// What the person did this turn.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// An explicit approval: a tap on **Looks good**, or `y`.
    Approve,
    /// A change request, in their words.
    Change(String),
    /// An answer to a question, in their words.
    Answer(String),
    /// A result came back from a try or a full run.
    Tried(Tried),
}

/// Which tool the person means, as the driver decided at the start.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
    /// An existing tool.
    Existing(Tool),
    /// A new tool made in chat: a skill, maybe with catalog tools.
    Make,
    /// A new tool that needs new code, which is made with Coder on a
    /// connected computer.
    NeedsCode,
    /// Not clear yet.
    Unclear,
}

/// What the model must propose for this turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Need {
    /// Nothing: the machine's own words carry the turn.
    Nothing,
    /// Describe the existing tool, or propose the one we make.
    Tool { change: Option<String> },
    /// Propose the tests.
    Tests { change: Option<String> },
    /// Propose each test's checks.
    Checks { change: Option<String> },
    /// Read a try's result with the person.
    Read,
    /// Fix the tests or checks after a change request.
    Fix { change: String },
    /// Answer a message once the test set is ready.
    Say { message: String },
}

/// The model's proposal for a [`Need`].
#[derive(Clone, Debug, PartialEq)]
pub enum Proposal {
    /// For [`Need::Tool`].
    Tool(ToolProposal),
    /// For [`Need::Tests`].
    Tests(TestsProposal),
    /// For [`Need::Checks`].
    Checks(ChecksProposal),
    /// For [`Need::Fix`].
    Fix(FixProposal),
    /// For [`Need::Read`] and [`Need::Say`].
    Say(SayProposal),
}

/// An action the turn puts in front of the person. Chat renders it as a
/// tap; a terminal asks `y`.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum Planned {
    /// Try each test once with and once without the tool.
    Try(Size),
    /// The full run.
    Full(Size),
    /// Add the full run's result to the Gym.
    Publish(ArtifactRef),
    /// Make a tool with new code with Coder on a connected computer.
    RunCoder,
}

/// One turn to show.
#[derive(Clone, Debug, PartialEq)]
pub struct Turn {
    /// The step the interview stopped at.
    pub stage: Stage,
    /// The words for this step.
    pub say: String,
    /// What the floor changed, in plain words.
    pub notes: Vec<String>,
    /// The step's fixed line, which a gate ends with.
    pub line: Option<&'static str>,
    /// At most one action.
    pub offer: Option<Planned>,
}

impl Turn {
    /// The reply: the words, the floor's notes, and the fixed line.
    #[must_use]
    pub fn text(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if !self.say.trim().is_empty() {
            parts.push(self.say.trim().to_string());
        }
        if !self.notes.is_empty() {
            parts.push(self.notes.join(" "));
        }
        if let Some(line) = self.line {
            parts.push(line.to_string());
        }
        parts.join("\n\n")
    }
}

/// Why the machine refused an event or a proposal.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// The interview has no tool yet; [`Interview::start`] picks one.
    #[error("the interview has no plugin yet")]
    NoTool,
    /// An approval where no gate is waiting.
    #[error("there is no step waiting for approval at {0:?}")]
    NotAGate(Stage),
    /// A result the step doesn't read.
    #[error("a result with {runs} runs doesn't belong at {stage:?}")]
    Unexpected { stage: Stage, runs: u32 },
    /// A proposal for a different need.
    #[error("the proposal doesn't answer what this step needs")]
    WrongProposal,
}

/// The interview.
#[derive(Clone, Debug, PartialEq)]
pub struct Interview {
    /// Where it runs.
    pub surface: Surface,
    /// The tools chat may pick and turn on.
    pub catalog: Catalog,
    /// The step it is at.
    pub stage: Stage,
    /// The tool, once picked or proposed.
    pub tool: Option<Tool>,
    /// Whether we are making the tool.
    pub making: bool,
    /// The tests so far, holding the floor once the tests step is reached.
    pub cases: Vec<DraftCase>,
    /// What the person said a good and a failed run look like.
    pub quality: Option<String>,
    /// The latest try of the current tests.
    pub tried: Option<Tried>,
    /// The full run's result.
    pub full: Option<Tried>,
    /// The most tests a draft may hold here.
    pub max_cases: usize,
}

fn capped(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max).collect();
    match cut.rfind(['.', '?', '!']) {
        Some(end) if end > max / 2 => cut[..=end].to_string(),
        _ => format!("{}…", cut.trim_end()),
    }
}

impl Interview {
    /// A new interview at the start.
    #[must_use]
    pub fn new(surface: Surface, catalog: Catalog) -> Self {
        Self {
            surface,
            catalog,
            stage: Stage::Start,
            tool: None,
            making: false,
            cases: Vec::new(),
            quality: None,
            tried: None,
            full: None,
            max_cases: match surface {
                Surface::Chat => HOSTED_MAX_CASES as usize,
                Surface::Terminal => MAX_DRAFT_CASES,
            },
        }
    }

    /// The interview a chat turn continues: the tool and tests from the
    /// draft the phone resent, and the step from our last message's fixed
    /// line. A line the draft can't be at (tests named with no tests in the
    /// draft) falls back to the step the draft implies, which never skips a
    /// gate: it may only ask again.
    #[must_use]
    pub fn resume(
        surface: Surface,
        catalog: Catalog,
        draft: Option<&Draft>,
        last_ours: Option<&str>,
    ) -> Self {
        let mut interview = Self::new(surface, catalog);
        let Some(draft) = draft else {
            return interview;
        };
        let tool = interview.catalog.tool_of(&draft.tool);
        interview.making = tool.is_made();
        interview.tool = Some(tool);
        interview.cases.clone_from(&draft.cases);
        let implied = if draft.cases.is_empty() {
            Stage::Tool
        } else {
            Stage::Tests
        };
        let said = last_ours.and_then(|text| Stage::from_line(text, surface));
        interview.stage = match said {
            None | Some(Stage::Start) => implied,
            Some(stage @ (Stage::Tool | Stage::Quality)) => stage,
            Some(_) if draft.cases.is_empty() => implied,
            Some(stage) => stage,
        };
        interview
    }

    /// Leaves the start with the tool the person means.
    ///
    /// # Errors
    ///
    /// The turn to show instead when there is no tool to write tests for
    /// yet: a question for an unclear pick, and the offer to run Coder for a
    /// tool that needs new code.
    // A turn is only returned at the start, once per chat turn.
    #[allow(clippy::result_large_err)]
    pub fn start(&mut self, pick: Pick) -> Result<Need, Turn> {
        match pick {
            Pick::Existing(tool) => {
                self.tool = Some(tool);
                self.making = false;
                Ok(Need::Tool { change: None })
            }
            Pick::Make => {
                self.making = true;
                Ok(Need::Tool { change: None })
            }
            Pick::NeedsCode => Err(Turn {
                stage: Stage::Start,
                say: "That plugin needs new code, so we'd build it with Coder on your computer, then test it the same way. Run Coder on your connected computer?".into(),
                notes: Vec::new(),
                line: None,
                offer: Some(Planned::RunCoder),
            }),
            Pick::Unclear => {
                let names: Vec<&str> = self.catalog.tools.iter().map(|t| t.name.as_str()).collect();
                let listed = match names.as_slice() {
                    [] => String::new(),
                    [one] => format!("We can test {one}, or "),
                    [rest @ .., last] => format!("We can test {}, or {last}, or ", rest.join(", ")),
                };
                Err(Turn {
                    stage: Stage::Start,
                    say: format!(
                        "Which plugin should we write tests for? {listed}make a new plugin with you: tell us what it should help Coder do."
                    ),
                    notes: Vec::new(),
                    line: None,
                    offer: None,
                })
            }
        }
    }

    /// Takes the person's event and says what the model must propose.
    ///
    /// Only [`Event::Approve`] at a gate moves past it. Anything else at a
    /// gate is a change request, and the step is proposed again.
    ///
    /// # Errors
    ///
    /// [`Refused`] for an approval with no gate waiting, a result the step
    /// doesn't read, and an event before a tool is picked.
    pub fn accept(&mut self, event: Event) -> Result<Need, Refused> {
        if self.stage == Stage::Start {
            return Err(Refused::NoTool);
        }
        let stage = self.stage;
        let text = |event: &Event| match event {
            Event::Change(text) | Event::Answer(text) => text.clone(),
            _ => String::new(),
        };
        Ok(match (stage, event) {
            (Stage::Tool, Event::Approve) => {
                self.stage = Stage::Quality;
                Need::Nothing
            }
            (Stage::Tool, event @ (Event::Change(_) | Event::Answer(_))) => Need::Tool {
                change: Some(text(&event)),
            },
            (Stage::Quality, event @ (Event::Change(_) | Event::Answer(_))) => {
                self.quality = Some(text(&event));
                Need::Tests { change: None }
            }
            (Stage::Tests, Event::Approve) => Need::Checks { change: None },
            (Stage::Tests, event @ (Event::Change(_) | Event::Answer(_))) => Need::Tests {
                change: Some(text(&event)),
            },
            (Stage::Checks, Event::Approve) => {
                self.stage = Stage::Pilot;
                Need::Nothing
            }
            (Stage::Checks, event @ (Event::Change(_) | Event::Answer(_))) => Need::Checks {
                change: Some(text(&event)),
            },
            (Stage::Pilot, Event::Approve) => {
                self.stage = Stage::Size;
                Need::Nothing
            }
            (Stage::Pilot, Event::Tried(tried)) if tried.runs == TRY_RUNS => {
                self.tried = Some(tried);
                Need::Read
            }
            (Stage::Pilot | Stage::Size | Stage::Done, event @ Event::Change(_))
            | (Stage::Pilot | Stage::Size, event @ Event::Answer(_)) => Need::Fix {
                change: text(&event),
            },
            (Stage::Size, Event::Approve) => {
                self.stage = Stage::Done;
                Need::Nothing
            }
            (Stage::Done, Event::Tried(tried)) if tried.runs >= FULL_RUNS => {
                self.full = Some(tried);
                Need::Nothing
            }
            (Stage::Done, Event::Approve) => Need::Nothing,
            (Stage::Done, Event::Answer(message)) => Need::Say { message },
            (stage, Event::Tried(tried)) => {
                return Err(Refused::Unexpected {
                    stage,
                    runs: tried.runs,
                });
            }
            (stage, Event::Approve) => return Err(Refused::NotAGate(stage)),
            (Stage::Start, _) => return Err(Refused::NoTool),
        })
    }

    /// The draft, when there is a tool to put in it.
    #[must_use]
    pub fn draft(&self) -> Option<Draft> {
        let tool = self.tool.as_ref()?;
        let draft = Draft {
            tool: tool.draft_tool(),
            cases: self.cases.clone(),
        };
        let value = nostr::cj_conversation::draft_value(&draft).ok()?;
        parse_draft(&value).ok()
    }

    /// The size of a run of the current tests.
    #[must_use]
    pub fn size(&self, runs: u32) -> Size {
        Size {
            cases: self.cases.len() as u64,
            runs: u64::from(runs),
            arms: ARMS,
        }
    }

    /// Where a run of the current tests goes from chat: our computers when
    /// it fits the hosted runner, the connected computer otherwise.
    #[must_use]
    pub fn place(&self) -> Where {
        if self.cases.len() as u64 <= HOSTED_MAX_CASES {
            Where::Hosted
        } else {
            Where::ConnectedComputer
        }
    }

    /// The wire offer for a planned action, for chat.
    #[must_use]
    pub fn wire_offer(&self, planned: &Planned) -> Option<WireOffer> {
        let subject = || match self.tool.as_ref().map(|t| &t.source) {
            Some(Source::Existing(definition)) => {
                Some(SubjectSource::Definition(Box::new(definition.clone())))
            }
            Some(Source::Made { .. }) => Some(SubjectSource::Draft),
            None => None,
        };
        Some(match planned {
            Planned::Try(size) | Planned::Full(size) => WireOffer::StartEval {
                suite: SuiteSource::Draft,
                subject: subject()?,
                size: *size,
                at: self.place(),
                label: if matches!(planned, Planned::Try(_)) {
                    "Try it once".into()
                } else {
                    "Run the full test set".into()
                },
            },
            Planned::Publish(report) => WireOffer::PublishEval {
                report: report.clone(),
                label: "Add to the Gym".into(),
            },
            Planned::RunCoder => WireOffer::RunCoder {
                label: "Run Coder on your computer".into(),
                engine: None,
            },
        })
    }

    fn turn(&self, say: String, notes: Vec<String>, offer: Option<Planned>) -> Turn {
        Turn {
            stage: self.stage,
            say: capped(&say, MAX_SAY_CHARS),
            notes,
            line: self.stage.line(self.surface),
            offer,
        }
    }

    fn operations(&self) -> Vec<String> {
        self.tool
            .as_ref()
            .map(|t| t.operations.clone())
            .unwrap_or_default()
    }

    fn enforce(&mut self, cases: Vec<DraftCase>) -> Vec<String> {
        let Some(tool) = self.tool.clone() else {
            return Vec::new();
        };
        let (cases, notes) = floor::enforce(&tool, &self.catalog, cases, self.max_cases);
        self.cases = cases;
        notes
    }

    /// Tests from proposals: each gets its prompt and the starting outcome
    /// check, and keeps the checks it had when its id and task are
    /// unchanged.
    fn tests_from(&self, tests: &[TestProposal]) -> Vec<DraftCase> {
        let mut cases: Vec<DraftCase> = Vec::new();
        for test in tests.iter().take(MAX_DRAFT_CASES) {
            if test.task.trim().is_empty() {
                continue;
            }
            let base = case_id(if test.id.trim().is_empty() {
                &test.task
            } else {
                &test.id
            });
            let mut id = base.clone();
            let mut n = 2;
            while cases.iter().any(|c| c.id == id) {
                id = format!("{base}-{n}");
                n += 1;
            }
            let kind = proposed_kind(&test.kind);
            let workspace = workspace_template(test.workspace.as_deref());
            let prompt = prompt_md_in(kind, &test.task, workspace.as_deref());
            let kept = self.cases.iter().find(|c| {
                c.id == id
                    && render::parse(c).is_ok_and(|parsed| {
                        parsed.prompt == test.task.trim()
                            && parsed.workspace.map(|w| w.template) == workspace
                    })
            });
            let starting = if workspace.is_some() {
                files_outcome_grader(test.good.as_deref())
            } else {
                outcome_grader(test.good.as_deref())
            };
            let (prompt, graders) = match kept {
                Some(existing) if render::wire_kind(kind) == existing.kind => {
                    (existing.prompt.clone(), existing.graders.clone())
                }
                _ => (prompt, vec![starting]),
            };
            cases.push(DraftCase {
                id,
                kind: render::wire_kind(kind),
                prompt,
                graders,
            });
        }
        cases
    }

    /// Applies checks to the tests they name; checks the engine refuses
    /// leave the test's checks as they were.
    fn with_checks(&self, mut cases: Vec<DraftCase>, checks: &[CaseChecks]) -> Vec<DraftCase> {
        let operations = self.operations();
        for case in &mut cases {
            let Some(proposed) = checks.iter().find(|c| case_id(&c.test) == case.id) else {
                continue;
            };
            let mut graders: Vec<(String, String)> = Vec::new();
            for grader in proposed.typed() {
                if let Some((name, text)) = render::grader(&grader, &operations) {
                    let mut unique = name.clone();
                    let mut n = 2;
                    while graders.iter().any(|(existing, _)| *existing == unique) {
                        unique = format!("{name}-{n}");
                        n += 1;
                    }
                    graders.push((unique, text));
                }
            }
            if graders.is_empty() {
                continue;
            }
            graders.sort_by(|a, b| a.0.cmp(&b.0));
            let candidate = DraftCase {
                graders,
                ..case.clone()
            };
            if render::parse(&candidate).is_ok() {
                case.graders = candidate.graders;
            }
        }
        cases
    }

    /// Applies the model's proposal for `need` and returns the turn to show.
    ///
    /// # Errors
    ///
    /// [`Refused::WrongProposal`] for a proposal that doesn't answer
    /// `need`, and [`Refused::NoTool`] when the step needs a tool.
    #[allow(clippy::too_many_lines)]
    pub fn apply(&mut self, need: &Need, proposal: Option<Proposal>) -> Result<Turn, Refused> {
        match (need, proposal) {
            (Need::Nothing, _) => Ok(self.fixed_turn()),
            (Need::Tool { .. }, Some(Proposal::Tool(proposal))) => {
                if !self.making {
                    if self.tool.is_none() {
                        return Err(Refused::NoTool);
                    }
                    self.stage = Stage::Tool;
                    return Ok(self.turn(proposal.say, Vec::new(), None));
                }
                let skill = proposal
                    .skill
                    .as_deref()
                    .map(|s| s.trim().chars().take(MAX_SKILL_CHARS).collect::<String>())
                    .filter(|s| !s.is_empty());
                let name = proposal
                    .name
                    .as_deref()
                    .map(|n| n.trim().chars().take(80).collect::<String>())
                    .filter(|n| !n.is_empty());
                match (proposal.asking, name, skill) {
                    (false, Some(name), Some(skill)) => {
                        let uses = self.catalog.uses(&proposal.uses);
                        let operations = uses
                            .iter()
                            .filter_map(|id| self.catalog.by_id(id))
                            .flat_map(|t| t.operations.iter().cloned())
                            .collect();
                        let summary: String = proposal
                            .summary
                            .unwrap_or_default()
                            .trim()
                            .chars()
                            .take(400)
                            .collect();
                        self.tool = Some(Tool {
                            name,
                            summary,
                            words: skill.clone(),
                            source: Source::Made { skill, uses },
                            operations,
                        });
                        self.stage = Stage::Tool;
                        let notes = if self.cases.is_empty() {
                            Vec::new()
                        } else {
                            let cases = std::mem::take(&mut self.cases);
                            self.enforce(cases)
                        };
                        Ok(self.turn(proposal.say, notes, None))
                    }
                    _ => {
                        // Not enough to propose yet: one question, and the
                        // interview stays at the start (or at the tool it had).
                        let mut turn = self.turn(proposal.say, Vec::new(), None);
                        if self.tool.is_none() {
                            self.stage = Stage::Start;
                            turn.stage = Stage::Start;
                            turn.line = None;
                        }
                        Ok(turn)
                    }
                }
            }
            (Need::Tests { .. }, Some(Proposal::Tests(proposal))) => {
                if self.tool.is_none() {
                    return Err(Refused::NoTool);
                }
                let cases = self.tests_from(&proposal.tests);
                let notes = self.enforce(cases);
                self.stage = Stage::Tests;
                self.tried = None;
                Ok(self.turn(proposal.say, notes, None))
            }
            (Need::Checks { .. }, Some(Proposal::Checks(proposal))) => {
                if self.tool.is_none() {
                    return Err(Refused::NoTool);
                }
                let cases = self.with_checks(self.cases.clone(), &proposal.checks);
                let notes = self.enforce(cases);
                self.stage = Stage::Checks;
                self.tried = None;
                Ok(self.turn(proposal.say, notes, None))
            }
            (Need::Fix { .. }, Some(Proposal::Fix(proposal))) => {
                if self.tool.is_none() {
                    return Err(Refused::NoTool);
                }
                let mut cases = match &proposal.tests {
                    Some(tests) => self.tests_from(tests),
                    None => self.cases.clone(),
                };
                if let Some(checks) = &proposal.checks {
                    cases = self.with_checks(cases, checks);
                }
                let notes = self.enforce(cases);
                self.stage = Stage::Pilot;
                self.tried = None;
                self.full = None;
                let offer = Some(Planned::Try(self.size(TRY_RUNS)));
                Ok(self.turn(proposal.say, notes, offer))
            }
            (Need::Read, Some(Proposal::Say(proposal))) => {
                self.stage = Stage::Pilot;
                let offer = Some(Planned::Try(self.size(TRY_RUNS)));
                Ok(self.turn(proposal.say, Vec::new(), offer))
            }
            (Need::Say { .. }, Some(Proposal::Say(proposal))) => {
                self.stage = Stage::Done;
                let offer = self.done_offer();
                Ok(self.turn(proposal.say, Vec::new(), offer))
            }
            _ => Err(Refused::WrongProposal),
        }
    }

    fn done_offer(&self) -> Option<Planned> {
        match &self.full {
            Some(full) => full.report.clone().map(Planned::Publish),
            None => Some(Planned::Full(self.size(FULL_RUNS))),
        }
    }

    /// The turn when the machine's own words carry it.
    fn fixed_turn(&self) -> Turn {
        match self.stage {
            Stage::Quality => self.turn("Good.".into(), Vec::new(), None),
            Stage::Pilot => {
                let say = "Next, we try it once: each test runs one time with the plugin and one time without, so we can fix the tests before the full run.";
                self.turn(
                    say.into(),
                    Vec::new(),
                    Some(Planned::Try(self.size(TRY_RUNS))),
                )
            }
            Stage::Size => {
                let size = self.size(FULL_RUNS);
                let say = format!(
                    "The full run is {} tests, {} runs each with the plugin and {} without: {} runs in all.",
                    size.cases,
                    size.runs,
                    size.runs,
                    size.cases * size.runs * size.arms
                );
                self.turn(say, Vec::new(), None)
            }
            Stage::Done => match &self.full {
                Some(full) => {
                    let say = format!(
                        "{} That's {}. Nothing is public until you add it to the Gym.",
                        full.headline(),
                        full.verdict.plain()
                    );
                    self.turn(say, Vec::new(), self.done_offer())
                }
                None => self.turn(
                    "Done. Every test runs three times with the plugin and three times without."
                        .into(),
                    Vec::new(),
                    self.done_offer(),
                ),
            },
            _ => self.turn(String::new(), Vec::new(), None),
        }
    }

    /// The turn that shows the current step again, with no model: its
    /// fixed words where it has them, and its line. A driver shows it when
    /// an event can't be taken, so the person is asked again rather than
    /// moved on.
    #[must_use]
    pub fn again(&self) -> Turn {
        match self.stage {
            Stage::Quality | Stage::Pilot | Stage::Size | Stage::Done => self.fixed_turn(),
            _ => self.turn(String::new(), Vec::new(), None),
        }
    }

    /// Whether the current tests hold the floor.
    #[must_use]
    pub fn holds_floor(&self) -> bool {
        match &self.tool {
            Some(tool) if !self.cases.is_empty() => {
                floor::check(tool, &self.catalog, &self.cases, self.max_cases).is_empty()
            }
            _ => true,
        }
    }

    /// Whether the interview has tests where the tool should stay out of
    /// the way.
    #[must_use]
    pub fn has_quiet_test(&self) -> bool {
        self.cases.iter().any(|c| c.kind == CaseKind::ShouldNotFire)
    }
}
