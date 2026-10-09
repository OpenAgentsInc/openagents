//! The authoring interview's driver: one step per turn, for the chat's
//! `eval.author` route and for `openagents ext eval init`.
//!
//! The steps, the gates, and the floor belong to the typed machine in
//! `ext_eval::author`. This module is what surrounds it: Jev's typed
//! questions decide which tool the person means and whether a reply at a
//! gate approves it ([`Author::pick`], [`Author::reply`]), the model door
//! writes each proposal from the specification's interview prompt
//! ([`Author::propose`]), and [`Author::step`] turns one chat request into
//! one reply, the revised draft, its card, and at most one offer. Nothing
//! here reads a message's words to decide a route or an approval; the only
//! deterministic reads are of our own fixed gate lines (an exact enum
//! value) and of bounded fields the model wrote.
//!
//! Chat keeps no state between turns: the phone resends the transcript and
//! the draft, and the step is recovered from them
//! ([`Interview::resume`]). A tap on **Looks good** reaches the worker as a
//! reply that Jev reads as an approval with at least [`APPROVE_AT`];
//! anything else at a gate is a change request, so a gate is never passed
//! on a guess. The interview never runs, spends, or publishes anything: a
//! try, the full run, and **Add to the Gym** are offers the phone renders
//! as taps.

use std::fmt;
use std::sync::Arc;

use ext_eval::author::machine::MAX_SAY_CHARS;
use ext_eval::author::proposal::{
    self, ChecksProposal, FixProposal, SayProposal, TestsProposal, ToolProposal,
};
use ext_eval::author::{
    Catalog, Event, Interview, Need, Pick, Proposal, Stage, Surface, Tried, Turn, prompt,
};
use indexmap::IndexMap;
use jev::{Choice, Entry, Questions, SystemOneRequest};
use nostr::cj_conversation::{Card, Offer, draft_value, parse_draft};
use serde_json::{Value, json};

use crate::generate::{Generate, Message, Role};
use crate::product_kb::Judge;

/// The probability of `approve` a reply at a gate needs to pass it.
pub const APPROVE_AT: f64 = 0.8;
/// The probability the tool choice needs; below it we ask which tool.
pub const PICK_AT: f64 = 0.5;
/// The probability of `code` (as Jev's choice) that sends a new tool to
/// Coder on a connected computer; anything less is a skill made in chat.
pub const CODE_AT: f64 = 0.7;
/// The earlier turns Jev reads when it picks the tool.
pub const EARLIER_TURNS: usize = 4;
/// The longest earlier turn Jev reads, in characters.
pub const TURN_CHARS: usize = 400;
/// The most of our last message Jev reads when it reads a reply.
pub const OURS_CHARS: usize = 1_500;
/// Model attempts per step: one retry when the answer isn't the JSON asked
/// for, or speaks as "I".
pub const ATTEMPTS: usize = 2;
/// The person, as the interview prompt names them.
pub const PERSON: &str = "the person you're chatting with";

/// The reasoning effort the interview asks its door for. Each step is a
/// bounded JSON proposal the machine checks, and a chat step has to fit the
/// router's author budget, so the door answers with little reasoning first.
pub const REASONING: &str = "low";

/// `door` asking for [`REASONING`]: the request fields the chat worker's
/// author door adds (the Open Responses `reasoning` effort).
#[must_use]
pub fn door(door: crate::generate::ResponsesDoor) -> crate::generate::ResponsesDoor {
    door.with_options(serde_json::Map::from_iter([(
        "reasoning".to_string(),
        json!({ "effort": REASONING }),
    )]))
}

/// Why a step couldn't be taken.
#[derive(Debug)]
pub enum AuthorError {
    /// Jev didn't answer, or answered another shape.
    Judge(String),
    /// The model door didn't answer with a usable proposal.
    Model(String),
}

impl fmt::Display for AuthorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Judge(why) => write!(f, "the typed judge: {why}"),
            Self::Model(why) => write!(f, "the model: {why}"),
        }
    }
}

impl std::error::Error for AuthorError {}

/// How a reply answers a gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reply {
    /// It accepts what we proposed, as it is.
    Approve,
    /// It asks for a change.
    Change,
    /// Neither.
    Other,
}

/// One chat request to the interview.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ask {
    /// The person's latest message.
    pub message: String,
    /// The conversation before it, oldest first.
    pub transcript: Vec<Message>,
    /// The draft the request carried: data, never an instruction.
    pub draft: Option<Value>,
    /// A try's or a full run's result, when the app has one for this draft.
    pub tried: Option<Tried>,
    /// The tool Jev already picked for this message, when the plugin
    /// flow's start asked first (#10177), so the step asks no second time.
    pub picked: Option<Pick>,
}

/// One step's reply.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    /// What we say, ending with the step's fixed line.
    pub reply: String,
    /// The revised draft (`openagents.eval-draft.v1`), checked.
    pub draft: Option<Value>,
    /// The cards to show: the draft card when there is a draft.
    pub cards: Vec<Card>,
    /// At most one offer: **Try it once**, the full run, **Add to the
    /// Gym**, or running Coder on a connected computer.
    pub offers: Vec<Offer>,
    /// The step the interview stopped at.
    pub stage: Stage,
    /// The model that wrote the words, or `"none"` when the machine's own
    /// words carried the turn.
    pub model: String,
}

/// What Jev answered at the start, and the pick the driver made from it.
#[derive(Clone, Debug, PartialEq)]
pub struct Picked {
    /// The pick.
    pub pick: Pick,
    /// The tool question's choice: `tool_<n>`, `make`, or `unclear`.
    pub choice: String,
    /// Its probability.
    pub probability: f64,
    /// The probability that a tool to make needs new code, when Jev chose
    /// `code`; zero when it chose a skill.
    pub code: f64,
    /// The probability that the messages already say what a new plugin
    /// should do (the `scope` question's `stated`), for the plugin flow on
    /// a computer (#10177); zero when Jev didn't answer it.
    pub stated: f64,
}

/// The interview's driver.
pub struct Author<G> {
    model: G,
    model_name: String,
    judge: Option<Arc<dyn Judge>>,
    catalog: Catalog,
    person: String,
}

fn singular(text: &str) -> bool {
    text.to_lowercase()
        .replace('\u{2019}', "'")
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .any(|word| {
            matches!(
                word,
                "i" | "i'll" | "i'm" | "i've" | "i'd" | "me" | "my" | "mine" | "myself"
            )
        })
}

fn cut(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

impl<G: Generate> Author<G> {
    /// A driver writing through `model` (named `model_name` in replies) and
    /// asking `judge` its typed questions. A terminal driver passes no
    /// judge: its gates are `y`.
    pub fn new(
        model: G,
        model_name: impl Into<String>,
        judge: Option<Arc<dyn Judge>>,
        catalog: Catalog,
    ) -> Self {
        Self {
            model,
            model_name: model_name.into(),
            judge,
            catalog,
            person: PERSON.into(),
        }
    }

    /// The same driver naming the person differently in the prompt.
    #[must_use]
    pub fn for_person(mut self, person: impl Into<String>) -> Self {
        self.person = person.into();
        self
    }

    /// The catalog the driver picks from.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    async fn ask_judge(
        &self,
        request: SystemOneRequest,
    ) -> Result<jev::SystemOneResponse, AuthorError> {
        let judge = self
            .judge
            .as_ref()
            .ok_or_else(|| AuthorError::Judge("no typed judge is configured".into()))?;
        judge.judge(request).await.map_err(AuthorError::Judge)
    }

    /// Which tool the person means, from Jev's typed question over their
    /// message and the last few turns.
    ///
    /// # Errors
    ///
    /// [`AuthorError::Judge`] when Jev doesn't answer.
    pub async fn pick(&self, message: &str, transcript: &[Message]) -> Result<Pick, AuthorError> {
        Ok(self.read_pick(message, transcript).await?.pick)
    }

    /// The pick with what Jev answered: the tool question's choice and its
    /// probability, and the probability that a tool to make needs new code.
    ///
    /// A tool to make is a skill unless Jev chooses `code` with at least
    /// [`CODE_AT`]: a skill stays in the interview and reaches a draft,
    /// while new code sends the person to a computer they may not have.
    ///
    /// # Errors
    ///
    /// [`AuthorError::Judge`] when Jev doesn't answer.
    pub async fn read_pick(
        &self,
        message: &str,
        transcript: &[Message],
    ) -> Result<Picked, AuthorError> {
        let earlier: Vec<Value> = transcript
            .iter()
            .rev()
            .take(EARLIER_TURNS)
            .rev()
            .map(|m| {
                json!({
                    "from": if m.role == Role::User { "person" } else { "us" },
                    "text": cut(&m.text, TURN_CHARS),
                })
            })
            .collect();
        let mut options: IndexMap<String, Option<Entry>> = IndexMap::new();
        for (index, tool) in self.catalog.tools.iter().enumerate() {
            options.insert(
                format!("tool_{index}"),
                Some(Entry::from(rubric::catalog_tool(&tool.name, &tool.summary))),
            );
        }
        options.insert("make".into(), Some(Entry::from(rubric::make())));
        options.insert("unclear".into(), Some(Entry::from(rubric::unclear())));
        let catalog: Vec<Value> = self
            .catalog
            .tools
            .iter()
            .map(|t| json!({"name": t.name, "does": t.summary}))
            .collect();
        let questions = Questions::new()
            .with("tool", Choice::new(rubric::tool_instructions(), options))
            .with(
                "build",
                Choice::new(
                    rubric::build_instructions(),
                    IndexMap::from([
                        ("skill".to_string(), Some(Entry::from(rubric::skill()))),
                        ("code".to_string(), Some(Entry::from(rubric::code()))),
                    ]),
                ),
            )
            .with(
                "scope",
                Choice::new(
                    rubric::scope_instructions(),
                    IndexMap::from([
                        ("stated".to_string(), Some(Entry::from(rubric::stated()))),
                        ("missing".to_string(), Some(Entry::from(rubric::missing()))),
                    ]),
                ),
            );
        let state = json!({
            "message": cut(message, 1_200),
            "earlier": earlier,
            "catalog": catalog,
        });
        let response = self
            .ask_judge(SystemOneRequest::new(Entry::from(state), questions))
            .await?;
        let tool = response
            .choice("tool")
            .map_err(|e| AuthorError::Judge(e.to_string()))?;
        let p = tool.probabilities.get(&tool.choice).copied().unwrap_or(0.0);
        let code = response
            .choice("build")
            .ok()
            .filter(|b| b.choice == "code")
            .and_then(|b| b.probabilities.get("code").copied())
            .unwrap_or(0.0);
        let stated = response
            .choice("scope")
            .ok()
            .and_then(|scope| scope.probabilities.get("stated").copied())
            .unwrap_or(0.0);
        let pick = if p < PICK_AT {
            Pick::Unclear
        } else if let Some(index) = tool
            .choice
            .strip_prefix("tool_")
            .and_then(|n| n.parse::<usize>().ok())
        {
            self.catalog
                .tools
                .get(index)
                .cloned()
                .map_or(Pick::Unclear, Pick::Existing)
        } else if tool.choice == "make" {
            if code >= CODE_AT {
                Pick::NeedsCode
            } else {
                Pick::Make
            }
        } else {
            Pick::Unclear
        };
        Ok(Picked {
            pick,
            choice: tool.choice.clone(),
            probability: p,
            code,
            stated,
        })
    }

    /// How the person's reply answers what we last said, from Jev's typed
    /// question. [`Reply::Approve`] needs at least [`APPROVE_AT`].
    ///
    /// # Errors
    ///
    /// [`AuthorError::Judge`] when Jev doesn't answer.
    pub async fn reply(&self, ours: &str, message: &str) -> Result<Reply, AuthorError> {
        let questions = Questions::new().with(
            "reply",
            Choice::new(
                "We proposed something and asked the person to approve it or ask for a change. How does their reply answer us?",
                IndexMap::from([
                    (
                        "approve".to_string(),
                        Some(Entry::from(
                            "They accept what we proposed as it is: yes, looks good, go ahead, that's right, or the app's Looks good button. Nothing is asked to change.",
                        )),
                    ),
                    (
                        "change".to_string(),
                        Some(Entry::from(
                            "They ask for a change: to add, remove, reword, or fix something, or they say something is wrong, even if they also sound positive.",
                        )),
                    ),
                    (
                        "other".to_string(),
                        Some(Entry::from(
                            "They ask a question, or say something that neither accepts what we proposed nor asks for a change.",
                        )),
                    ),
                ]),
            ),
        );
        let state = json!({"we_said": cut(ours, OURS_CHARS), "they_replied": cut(message, 1_200)});
        let response = self
            .ask_judge(SystemOneRequest::new(Entry::from(state), questions))
            .await?;
        let answer = response
            .choice("reply")
            .map_err(|e| AuthorError::Judge(e.to_string()))?;
        let approve = answer.probabilities.get("approve").copied().unwrap_or(0.0);
        Ok(match answer.choice.as_str() {
            "approve" if approve >= APPROVE_AT => Reply::Approve,
            "change" => Reply::Change,
            _ => Reply::Other,
        })
    }

    fn fallback_say(interview: &Interview, need: &Need) -> String {
        match need {
            Need::Tool { .. } if interview.making => "Here's what we'd make.".into(),
            Need::Tool { .. } => interview
                .tool
                .as_ref()
                .map(|t| t.summary.clone())
                .unwrap_or_default(),
            Need::Tests { .. } => "Here are the tests we'd use.".into(),
            Need::Checks { .. } => "Here's how we'd check each test.".into(),
            Need::Fix { .. } => "We changed the tests.".into(),
            Need::Read => "Here's what the try showed.".into(),
            Need::Say { .. } | Need::Nothing => "The test set is ready.".into(),
        }
    }

    /// The model's proposal for `need`: the interview prompt, the step's
    /// task, and the state, over the conversation. `None` when the need
    /// takes no model.
    ///
    /// # Errors
    ///
    /// [`AuthorError::Model`] when the door fails, or twice doesn't answer
    /// with the JSON the step asks for.
    pub async fn propose(
        &self,
        interview: &Interview,
        need: &Need,
        input: &[Message],
    ) -> Result<Option<Proposal>, AuthorError> {
        Ok(self
            .propose_by(interview, need, input)
            .await?
            .map(|(proposal, _)| proposal))
    }

    /// [`Author::propose`], with the model that wrote the proposal: the
    /// one the door names ([`crate::generate::Meta::Model`]) when it names
    /// one, as a door with a fallback does, and this driver's model
    /// otherwise.
    async fn propose_by(
        &self,
        interview: &Interview,
        need: &Need,
        input: &[Message],
    ) -> Result<Option<(Proposal, String)>, AuthorError> {
        if *need == Need::Nothing {
            return Ok(None);
        }
        let mut wrote = self.model_name.clone();
        let base = prompt::instructions(interview, need, &self.person);
        let mut note = String::new();
        let mut last_error = String::new();
        let mut voiced: Option<Proposal> = None;
        for _ in 0..ATTEMPTS {
            let instructions = if note.is_empty() {
                base.clone()
            } else {
                format!("{base}\n\n## Note\n\n{note}")
            };
            let mut named = None;
            let (answer, _) = self
                .model
                .generate(&instructions, input, &mut |_| {}, &mut |meta| {
                    if let crate::generate::Meta::Model(model) = meta {
                        named = Some(model);
                    }
                })
                .await
                .map_err(|e| AuthorError::Model(e.to_string()))?;
            if let Some(named) = named {
                wrote = named;
            }
            let parsed: Result<Proposal, String> = match need {
                Need::Tool { .. } => proposal::parse::<ToolProposal>(&answer).map(Proposal::Tool),
                Need::Tests { .. } => {
                    proposal::parse::<TestsProposal>(&answer).map(Proposal::Tests)
                }
                Need::Checks { .. } => {
                    proposal::parse::<ChecksProposal>(&answer).map(Proposal::Checks)
                }
                Need::Fix { .. } => proposal::parse::<FixProposal>(&answer).map(Proposal::Fix),
                Need::Read | Need::Say { .. } => {
                    proposal::parse::<SayProposal>(&answer).map(Proposal::Say)
                }
                Need::Nothing => return Ok(None),
            };
            match parsed {
                Ok(proposal) if !singular(say_of(&proposal)) => {
                    return Ok(Some((proposal, wrote)));
                }
                Ok(proposal) => {
                    note = "Your last answer spoke as \"I\". Write `say` as OpenAgents: \"we\" and \"you\", never \"I\", \"me\", or \"my\".".into();
                    voiced = Some(proposal);
                }
                Err(error) => {
                    note = format!(
                        "Your last answer wasn't the JSON object this step asks for ({error}). Answer with that JSON object only."
                    );
                    last_error = error;
                }
            }
        }
        match voiced {
            // Keep the proposal, and say it in our own fixed words.
            Some(mut proposal) => {
                set_say(&mut proposal, Self::fallback_say(interview, need));
                Ok(Some((proposal, wrote)))
            }
            None => Err(AuthorError::Model(last_error)),
        }
    }

    /// Takes `event`, asks the model for what the step needs, and applies
    /// it. An event the machine refuses shows the step again.
    ///
    /// # Errors
    ///
    /// As [`Author::propose`].
    pub async fn advance(
        &self,
        interview: &mut Interview,
        event: Event,
        input: &[Message],
    ) -> Result<(Turn, Option<String>), AuthorError> {
        let Ok(need) = interview.accept(event) else {
            return Ok((interview.again(), None));
        };
        self.fulfil(interview, &need, input).await
    }

    /// Asks the model for `need` and applies it, with the model that wrote
    /// the words, or `None` when no model did.
    ///
    /// # Errors
    ///
    /// As [`Author::propose`].
    pub async fn fulfil(
        &self,
        interview: &mut Interview,
        need: &Need,
        input: &[Message],
    ) -> Result<(Turn, Option<String>), AuthorError> {
        let (proposal, wrote) = match self.propose_by(interview, need, input).await? {
            Some((proposal, wrote)) => (Some(proposal), Some(wrote)),
            None => (None, None),
        };
        match interview.apply(need, proposal) {
            Ok(turn) => Ok((turn, wrote)),
            Err(_) => Ok((interview.again(), None)),
        }
    }

    /// One chat turn of the interview.
    ///
    /// # Errors
    ///
    /// [`AuthorError`] when Jev or the model can't be reached; the router
    /// then answers from its bank.
    pub async fn step(&self, ask: &Ask) -> Result<Step, AuthorError> {
        let draft = ask.draft.as_ref().and_then(|value| parse_draft(value).ok());
        let ours = ask
            .transcript
            .iter()
            .rev()
            .find(|m| m.role == Role::Assistant)
            .map(|m| m.text.clone())
            .unwrap_or_default();
        let mut interview = Interview::resume(
            Surface::Chat,
            self.catalog.clone(),
            draft.as_ref(),
            (!ours.is_empty()).then_some(ours.as_str()),
        );
        let mut input = ask.transcript.clone();
        if input
            .last()
            .is_none_or(|m| m.role != Role::User || m.text != ask.message)
        {
            input.push(Message {
                role: Role::User,
                text: ask.message.clone(),
            });
        }
        let (turn, wrote) = if interview.stage == Stage::Start {
            let pick = match ask.picked.clone() {
                Some(pick) => pick,
                None => self.pick(&ask.message, &ask.transcript).await?,
            };
            match interview.start(pick) {
                Ok(need) => self.fulfil(&mut interview, &need, &input).await?,
                Err(turn) => (turn, None),
            }
        } else {
            let result = ask.tried.clone().filter(|tried| {
                (interview.stage == Stage::Pilot
                    && tried.runs == ext_eval::author::runner::TRY_RUNS)
                    || (interview.stage == Stage::Done
                        && tried.runs >= ext_eval::author::runner::FULL_RUNS)
            });
            let event = match result {
                Some(tried) => Event::Tried(tried),
                None if interview.stage.is_gate() || interview.stage == Stage::Done => {
                    match self.reply(&ours, &ask.message).await? {
                        Reply::Approve => Event::Approve,
                        Reply::Change => Event::Change(ask.message.clone()),
                        Reply::Other if interview.stage == Stage::Done => {
                            Event::Answer(ask.message.clone())
                        }
                        Reply::Other => Event::Change(ask.message.clone()),
                    }
                }
                None => Event::Answer(ask.message.clone()),
            };
            self.advance(&mut interview, event, &input).await?
        };
        Ok(self.shape(&interview, &turn, wrote))
    }

    /// A turn as the chat returns it.
    fn shape(&self, interview: &Interview, turn: &Turn, wrote: Option<String>) -> Step {
        let draft = interview.draft();
        let value = draft.as_ref().and_then(|d| draft_value(d).ok());
        let cards = draft.into_iter().map(Card::Draft).collect();
        let offers = turn
            .offer
            .as_ref()
            .and_then(|planned| interview.wire_offer(planned))
            .into_iter()
            .collect();
        Step {
            reply: turn.text(),
            draft: value,
            cards,
            offers,
            stage: turn.stage,
            model: wrote.unwrap_or_else(|| "none".into()),
        }
    }
}

fn say_of(proposal: &Proposal) -> &str {
    match proposal {
        Proposal::Tool(p) => &p.say,
        Proposal::Tests(p) => &p.say,
        Proposal::Checks(p) => &p.say,
        Proposal::Fix(p) => &p.say,
        Proposal::Say(p) => &p.say,
    }
}

fn set_say(proposal: &mut Proposal, say: String) {
    let say = cut(&say, MAX_SAY_CHARS);
    match proposal {
        Proposal::Tool(p) => p.say = say,
        Proposal::Tests(p) => p.say = say,
        Proposal::Checks(p) => p.say = say,
        Proposal::Fix(p) => p.say = say,
        Proposal::Say(p) => p.say = say,
    }
}

/// The `eval.author` seam: the router's ask as this driver's, and its step
/// as the router's. The router checks every step before it is shown
/// (`router::gym::check_step`).
impl<G: Generate + 'static> crate::router::seams::EvalAuthor for Author<G> {
    fn available(&self) -> bool {
        self.judge.is_some()
    }

    fn recipients(&self) -> Vec<String> {
        vec![
            "the chat model door".into(),
            "Jev (TypeSafe System One)".into(),
        ]
    }

    fn step<'a>(
        &'a self,
        ask: &'a crate::router::seams::AuthorAsk,
    ) -> futures_util::future::BoxFuture<
        'a,
        Result<crate::router::seams::AuthorStep, crate::router::seams::SeamError>,
    > {
        Box::pin(async move {
            let failed = |e: AuthorError| crate::router::seams::SeamError::Failed(e.to_string());
            // On a computer, a request for a new plugin, and every reply
            // while one is being made, is the plugin flow (#10177).
            let mut picked = None;
            if ask.here {
                if let Some(open) = plugin::open(&ask.transcript) {
                    return self.plugin_step(open, ask).await.map_err(failed);
                }
                if ask.draft.is_none() {
                    let read = self
                        .read_pick(&ask.message, &ask.transcript)
                        .await
                        .map_err(failed)?;
                    if matches!(read.pick, Pick::Make | Pick::NeedsCode) {
                        return Ok(plugin::start(read.stated));
                    }
                    picked = Some(read.pick);
                }
            }
            let step = self
                .step(&Ask {
                    message: ask.message.clone(),
                    transcript: ask.transcript.clone(),
                    draft: ask.draft.clone(),
                    tried: ask.tried.clone(),
                    picked,
                })
                .await
                .map_err(failed)?;
            Ok(crate::router::seams::AuthorStep {
                text: step.reply,
                draft: step.draft,
                offer: step.offers.into_iter().next().and_then(router_offer),
                model: step.model,
                plugin: None,
            })
        })
    }
}

/// An offer the interview makes, as the router carries it: a run, **Add
/// to the Gym**, or running Coder. The interview makes no other offer.
#[must_use]
pub fn router_offer(offer: Offer) -> Option<crate::router::Offer> {
    use crate::router::Offer as Routed;
    match offer {
        Offer::RunCoder {
            label,
            engine,
            plan,
        } => Some(Routed::RunCoder {
            label,
            engine,
            plan,
        }),
        Offer::StartEval {
            suite,
            subject,
            size,
            at,
            label,
        } => Some(Routed::StartEval {
            suite,
            subject,
            size,
            at,
            label,
        }),
        Offer::PublishEval { report, label } => Some(Routed::PublishEval { report, label }),
        Offer::OpenScreen { .. } | Offer::Cli { .. } | Offer::OpenPresentation { .. } => None,
    }
}

/// The `eval.author` seam for the chat worker: the worker's own door with
/// [`REASONING`] and its judge, over an empty catalog: the hosted runner's
/// sample plugins are its test fixtures, never offered. Without a live
/// door or a judge there is no interview, and the route answers from the
/// bank.
#[must_use]
pub fn seam(
    door: &crate::generate::Door,
    judge: Option<Arc<dyn Judge>>,
) -> Arc<dyn crate::router::seams::EvalAuthor> {
    match (door, judge) {
        (crate::generate::Door::Live(live), Some(judge)) => {
            let model = live.model.clone();
            Arc::new(Author::new(
                self::door(live.clone()),
                model,
                Some(judge),
                Catalog::default(),
            ))
        }
        // The worker's primary first, its fallback after, both asking for
        // [`REASONING`] (#10109); each step names the one that wrote it.
        (crate::generate::Door::Fallback(ordered), Some(judge)) => {
            let model = ordered.primary.model.clone();
            Arc::new(Author::new(
                ordered.before(self::door(ordered.fallback.clone())),
                model,
                Some(judge),
                Catalog::default(),
            ))
        }
        _ => Arc::new(crate::router::seams::NoAuthor),
    }
}

pub mod fake;
pub mod plugin;
pub mod rubric;

#[cfg(test)]
mod tests;
