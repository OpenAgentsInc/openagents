//! The agent: a turn of conversation, Classify first, Generate on route.
//!
//! A turn runs in two phases so the terminal can draw the judgment inline
//! before the answer streams: [`Agent::classify`] reads the state and
//! returns a [`Verdict`] — the judgment plus the route it produced — and
//! [`Agent::reply`] runs Generate when the route says to. The transcript
//! folds each side in as it lands.

use std::collections::BTreeSet;
use std::env;
use std::path::Path;
use std::time::Instant;

use atif::Decision;
use jev::SystemOneRequest;
use serde_json::Value;

use crate::about::About;
use crate::capability;
use crate::classify::{
    Judgment, Route, ShellRoute, judgment_of, questions, questions_provenance, route,
    shell_provenance, shell_questions, shell_verdict_of, state_of,
};
use crate::generate::{Door, Generate, GenerateError, Message, Meta, Role, Usage};
use crate::permit::Permit;
use crate::program_authority::Grant;
use crate::repo::Repo;
use crate::runtime::{Inputs, Run, Runtime, Selected};
use crate::shell::{self, Outcome, Reply, ShellEvent};
use crate::survey::Survey;
use crate::trace::{Recorder, answers_value};

/// The instructions Generate hears for a plain answer.
pub const INSTRUCTIONS: &str = "You are Coder, an assistant that lives in a terminal. \
    Answer directly and tersely. Plain prose, short paragraphs, no headers. \
    You can run shell commands on the user's machine: when a request needs \
    the repository inspected, code searched, tests run, or anything checked \
    on disk, do not answer from memory — emit a command plan instead of \
    prose. A plan is the whole reply as one fenced code block holding one \
    JSON object, nothing before or after the fence: \
    ```json\n{\"v\":1,\"commands\":[{\"command\":\"git grep -rn foo .\",\"why\":\"find foo\"}]}\n```. \
    Write it as ordinary reply text inside the fence: this environment \
    declares no functions, so never emit a function call or tool call. \
    At most 10 commands; prefer read-only ones unless the task asks for a \
    change. After they run you receive their output; then plan again or \
    answer. The REPO CONTEXT block describes the repository the user is \
    working in: answer project questions from it and name real paths, and \
    if the context does not cover the question, say so rather than guessing. \
    The ABOUT THIS APPLICATION block describes Coder itself, the program you \
    are running as: answer questions about Coder, its terminal, or its \
    working directory from that block rather than from the repository.";

/// The instructions for the turn's last word: the loop is done, prose only.
const FINAL_SUFFIX: &str = " The command loop is finished — do not emit a \
    plan; answer with what you have.";

/// The instructions for a clarifying turn: the router marked the request
/// ambiguous, so the whole reply is the question.
const CLARIFY_SUFFIX: &str = " The router marked this turn ambiguous: ask \
    one short clarifying question and nothing else.";

/// The instructions after a round the judge read as `retry`: the output
/// did not settle the task, so the next plan must differ or the model
/// must answer with what it has.
const RETRY_SUFFIX: &str = " The judge read the last round's output as a \
    retry: those commands did not establish what the task needs. Do not \
    run the same commands again. Either plan different commands that get \
    at it another way, or answer in prose with what you have.";

/// How many rounds in a row the judge may read as `retry` before the turn
/// stops running commands. Past this the retry verdict is a refusal, not
/// another round.
pub const RETRIES_MAX: usize = 2;

/// Names the capability a program's `delegate` step hands work to, by
/// slug. Unset, the first capability that is a route on this machine does
/// the work; `devin-relay` sends it to the worker `CODER_WORKER` names.
pub const DELEGATE_VAR: &str = "CODER_DELEGATE";

/// The message the model reads after it answered a finished loop with
/// another plan: the one repair a turn allows before the host answers for
/// it.
const REPAIR: &str = "That reply was a command plan and none of it ran: the \
    command loop is finished. Answer in plain prose now, from the command \
    output above. State what you observed, and name what you could not \
    establish rather than proposing more commands.";

/// How many times a turn asks the model to repair a plan into prose after
/// the loop is finished. Past this the host writes the answer itself.
pub const REPAIRS_MAX: usize = 1;

/// How a turn ended, apart from what it said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ending {
    /// The model answered in prose.
    Answered,
    /// The model's last word was a plan the host would not run. The text
    /// the user reads is the host's refusal, not the plan; `why` is the
    /// host's reason and `proposal` the plan as the model wrote it.
    Refused {
        /// Why nothing in the plan ran.
        why: String,
        /// The reply the model wrote, kept for the record.
        proposal: String,
    },
}

/// Why a turn stopped running commands before the model answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exhausted {
    /// The turn ran every round its permit allowed.
    Rounds(usize),
    /// The judge read the last round and said to stop.
    Stopped,
    /// The judge read this many rounds in a row as `retry`, the most a
    /// turn allows.
    Retries(usize),
}

impl Exhausted {
    /// The sentence the trace and the user read.
    #[must_use]
    pub fn sentence(self) -> String {
        match self {
            Exhausted::Rounds(rounds) => {
                format!("the command loop ran its {rounds} permitted rounds")
            }
            Exhausted::Stopped => "the judge stopped the command loop".to_string(),
            Exhausted::Retries(retries) => {
                format!(
                    "the judge asked for a retry {retries} rounds in a row, the most a turn allows"
                )
            }
        }
    }
}

/// What one whole turn produced.
#[derive(Clone, Debug)]
pub struct Turned {
    /// What the user reads.
    pub text: String,
    /// What the turn cost across every generation, when the door says.
    pub usage: Option<Usage>,
    /// How the turn ended.
    pub ending: Ending,
    /// Why the loop stopped before the model answered, when it did. A turn
    /// whose model answered in prose on its own is `None` here.
    pub exhausted: Option<Exhausted>,
    /// How many commands ran on this turn, in all.
    pub commands: usize,
    /// What the turn cost in dollars, when the door says: the delegate
    /// door reports Jev and the executor together. `None` is unknown,
    /// never zero.
    pub cost_usd: Option<f64>,
}

/// The read Classify made and where it sent the turn, for the transcript's
/// inline display.
#[derive(Clone, Debug)]
pub struct Verdict {
    /// The typed answers, kept whole.
    pub judgment: Judgment,
    /// The route the table chose.
    pub route: Route,
}

/// What classify produced.
#[derive(Clone, Debug)]
pub enum Classified {
    /// Classify ran; the verdict carries the judgment and the route.
    Judged(Verdict),
    /// Classify did not run — no key or a failed call — and the note says
    /// why. The conversation still generates, unrouted.
    Skipped(String),
}

/// What an answered evidence selection hands the render: the order the
/// gate's distribution ranked the candidate paths into — `None` when it
/// abstained, because an abstention ranks nothing — and the manifest
/// record of the judgment itself.
struct Ranked {
    order: Option<Vec<String>>,
    record: crate::select::Record,
}

/// The conversation: a transcript, a classifier that may be absent, and a
/// door to Generate.
pub struct Agent {
    classify: Option<jev::Client>,
    /// The profile the classifier resolved under — `None` for an agent
    /// built from a bare client, which cannot say what locality its
    /// door has and so decides no disclosure bound.
    decision_profile: Option<crate::profiles::Profile>,
    generate: Door,
    transcript: Vec<Message>,
    /// The repo the shell sits in, when it sits in one.
    repo: Option<Repo>,
    /// The ranking the current task's repository evidence was given.
    /// A new task clears it; an unchanged candidate set reuses it, and
    /// the reuse check — content, wording, policy, and artifact all
    /// identical — is what "unchanged" means.
    evidence_ranking: Option<crate::select::Ranking>,
    /// What the running application knows about itself and where it ran.
    about: About,
    /// The draft the current turn is classifying, kept until the reply
    /// lands.
    task: String,
    /// The session's trace, when this machine is recording one.
    trace: Option<Recorder>,
    /// Why there is no trace, when there should have been one.
    trace_error: Option<String>,
    /// What this machine can reach and what it could run, once something
    /// has asked.
    survey: Option<Survey>,
    /// The program slugs the command line granted this session, when it
    /// named any — merged with `CODER_PROGRAMS` each time a program runs,
    /// so a grant withdrawn between turns is not handed out anyway.
    program_grant: Option<String>,
    /// Why this session answers through the door it does, as the session
    /// header and the trace say it.
    door_reason: String,
}

impl Agent {
    /// An agent from the environment: the active decision profile
    /// builds the classifier, the door builds itself. An unconfigured
    /// profile degrades — the conversation still runs, without
    /// judgments. A malformed one is an error, not an absence.
    ///
    /// The session's trace opens here, so a conversation is recorded without
    /// anybody asking it to be. A trace that cannot be opened is reported
    /// through [`Agent::trace_error`] and costs the conversation nothing.
    ///
    /// # Errors
    ///
    /// Returns a sentence when the environment does not name one door. A
    /// door is not a thing to degrade over: a session that quietly picked
    /// between two configured doors would record which one it picked as if
    /// that had been asked for.
    pub fn from_env() -> Result<Self, String> {
        Self::opening(None)
    }

    /// An agent from the environment, recording to `path` rather than to
    /// the directory the environment names.
    ///
    /// A caller that names the file can read the trace back without
    /// watching a directory, which is what a script driving a turn needs.
    /// Naming a file is a request to record, so it outranks `CODER_TRACE`.
    ///
    /// # Errors
    ///
    /// Returns a sentence when the environment does not name one door.
    pub fn recording_to(path: &Path) -> Result<Self, String> {
        Self::opening(Some(path))
    }

    /// The shared opener: the door and the repository from the
    /// environment, the trace where `path` says or where the environment
    /// does.
    fn opening(path: Option<&Path>) -> Result<Self, String> {
        let (generate, door_reason) = crate::delegate_door::open()?;
        let working_directory = env::current_dir().unwrap_or_default();
        let repo = Repo::discover(&working_directory);
        let about = About::observe(&working_directory, repo.as_ref().map(|repo| repo.root()));
        let where_it_ran = repo
            .as_ref()
            .map(|repo| repo.root().display().to_string())
            .unwrap_or_default();
        let opened = match path {
            Some(path) => {
                Recorder::at(path, generate.model(), generate.name(), &where_it_ran).map(Some)
            }
            None => Recorder::start(generate.model(), generate.name(), &where_it_ran),
        };
        let (mut trace, trace_error) = match opened {
            Ok(recorder) => (recorder, None),
            Err(error) => (None, Some(error)),
        };
        if let Some(trace) = &mut trace {
            trace.door(generate.name(), generate.model(), &door_reason);
        }
        let decision_profile = crate::decision::profile_from_env()?;
        let classify = decision_profile
            .as_ref()
            .map(|profile| profile.client().map_err(|refusal| refusal.to_string()))
            .transpose()?;
        Ok(Self {
            classify,
            decision_profile,
            generate,
            transcript: Vec::new(),
            repo,
            evidence_ranking: None,
            about,
            task: String::new(),
            trace,
            trace_error,
            survey: None,
            program_grant: None,
            door_reason,
        })
    }

    /// An agent over explicit parts, for tests.
    pub fn new(classify: Option<jev::Client>, generate: Door) -> Self {
        Self {
            classify,
            decision_profile: None,
            generate,
            transcript: Vec::new(),
            repo: None,
            evidence_ranking: None,
            about: About::observe(&env::current_dir().unwrap_or_default(), None),
            task: String::new(),
            trace: None,
            trace_error: None,
            survey: None,
            program_grant: None,
            door_reason: String::new(),
        }
    }

    /// The repo the shell sits in, for the prompt's context block.
    pub fn with_repo(mut self, repo: Option<Repo>) -> Self {
        self.about.repository = repo.as_ref().map(|repo| repo.root().to_path_buf());
        self.repo = repo;
        self
    }

    /// What the application knows about itself: where it ran, and what
    /// its terminal shows.
    #[must_use]
    pub fn about(&self) -> &About {
        &self.about
    }

    /// The application block and the repo context block for one
    /// generation, in that order.
    #[must_use]
    pub fn context(&self) -> String {
        self.context_with_evidence().0
    }

    fn context_with_evidence(&self) -> (String, Option<crate::repo::RepositoryEvidence>) {
        let mut context = self.about.context();
        let mut evidence = None;
        if let Some(repo) = &self.repo {
            context.push_str("\n\n");
            let (rendered, captured) = repo.context_with_evidence(&self.task);
            context.push_str(&rendered);
            evidence = Some(captured);
        }
        (context, evidence)
    }

    /// The recorder this session writes to, for tests and for a caller that
    /// wants to name the directory.
    pub fn with_trace(mut self, trace: Option<Recorder>) -> Self {
        self.trace = trace;
        self
    }

    /// The survey this agent uses, for a caller that already read one.
    ///
    /// A test drives a machine it built rather than the one it runs on,
    /// and handing the survey over is how it says which machine that is.
    pub fn with_survey(mut self, survey: Survey) -> Self {
        self.survey = Some(survey);
        self
    }

    /// The program slugs this session may run, as `--programs` spells
    /// them — merged with `CODER_PROGRAMS` when a program is selected.
    ///
    /// With neither set, no program a turn selects may run: a selection
    /// is a proposal, and this is the grant it is proposed under. See
    /// `docs/coder/guides/program-authority.md`.
    pub fn with_program_grant(mut self, spec: Option<&str>) -> Self {
        self.program_grant = spec.map(str::to_string);
        self
    }

    /// What this machine can reach and what it could run, read once and
    /// recorded to the trace the first time anything asks.
    ///
    /// Read on demand rather than at startup because a probe spawns a
    /// process for every declared capability, and a conversation that never
    /// delegates should not pay for asking. The first caller is whatever
    /// needs an option set: a host builds one from the capabilities the
    /// probe found available, so an absent executor is a route that was
    /// never offered rather than one that fails when it is taken.
    pub fn survey(&mut self) -> &Survey {
        if self.survey.is_none() {
            let workspace = env::current_dir().unwrap_or_default();
            let root = self.repo.as_ref().map(|repo| repo.root().to_path_buf());
            let survey = Survey::read(root.as_deref(), &workspace);
            if let Some(trace) = &mut self.trace {
                survey.record(trace, None);
            }
            self.survey = Some(survey);
        }
        self.survey
            .as_ref()
            .expect("the survey was read a moment ago")
    }

    /// Whether this turn asks for a program to run, and what running it
    /// did.
    ///
    /// `None` is the ordinary turn, and it is nearly every turn: the
    /// selection question answered `none`, or there was no way to ask it.
    /// The caller then does what it has always done, and nothing about the
    /// turn changes.
    ///
    /// The door is the classifier's, so a machine with no decision door
    /// asks nothing, probes nothing, and answers exactly as before.
    /// `selected` hears the program's slug before it
    /// runs, because a fan-out takes minutes and a terminal that said
    /// nothing until it finished would look wedged.
    ///
    /// A refusal from selection is a note in the trace rather than a
    /// failure. A door that would not answer should cost a turn its
    /// program, not its reply.
    pub async fn program(&mut self, selected: &mut (dyn FnMut(&str) + Send)) -> Option<Run> {
        let door = self.classify.clone()?;
        let repository = self.repo.as_ref().map(|repo| repo.root().to_path_buf());
        let survey = self.survey().clone();
        let mut runtime = Runtime::using(survey, repository.as_deref()).asking(Some(door));
        if let Some(dir) = crate::runstate::directory() {
            runtime = runtime.with_runstate(dir);
        }
        let slug = match runtime.select(&self.task, self.trace.as_mut()).await {
            Ok(Selected::Program(slug)) => slug,
            Ok(Selected::None) => return None,
            Err(refused) => {
                if let Some(trace) = &mut self.trace {
                    trace.note(&format!("no program selected: {refused}"));
                }
                return None;
            }
        };
        let program = runtime.survey().programs.get(&slug)?.clone();
        selected(&slug);
        // The grant is the operator's, read fresh from this session's
        // settings on every run: a selection is a proposal, and the grant
        // is the authority it is proposed under. Neither the selection
        // nor any judgment the program records widens it.
        let grant = Grant::operator(self.program_grant.as_deref());
        // A relay capability is probed only now, once a program is going
        // to run, because the probe is a round trip to a worker and an
        // ordinary turn should not pay for it — and neither should a
        // program the grant does not cover, since a probe spends the
        // network the session never allowed.
        if grant.authorizes(&slug) && grant.effects().network {
            runtime.probe_relays().await;
            if let Some(trace) = &mut self.trace {
                for found in &runtime.survey().capabilities {
                    if found.manifest.transport == capability::RELAY {
                        trace.check(&found.message(), found.call());
                    }
                }
            }
        }
        // Which executor does the work is the program's when a `delegate`
        // step names one: `review-runs` hands its question to Coder One's
        // ask mode and nothing else answers it. Otherwise it is the
        // operator's choice when `CODER_DELEGATE` names one, and otherwise
        // the survey's answer: the first capability that is a route here.
        // A machine with none still runs the program, and the `delegate`
        // step refuses by name rather than by silence.
        let survey = runtime.survey();
        // The same variable turns the delegate door on or off; a mode
        // word names no capability.
        let executor = program
            .executor()
            .map(str::to_string)
            .or_else(|| {
                env::var(DELEGATE_VAR).ok().filter(|slug| {
                    !slug.is_empty() && crate::delegate_door::Mode::parse(slug).is_none()
                })
            })
            .unwrap_or_else(|| {
                // A capability some program names as its own executor is
                // that program's, and a capability that isn't an executor
                // at all, such as a read-only adapter, takes no work.
                let dedicated: Vec<&str> = survey
                    .programs
                    .programs()
                    .iter()
                    .filter_map(crate::program::Program::executor)
                    .collect();
                let general = |found: &&crate::capability::Found| {
                    found.manifest.profile == "executor" && !dedicated.contains(&found.capability())
                };
                survey
                    .options()
                    .into_iter()
                    .find(general)
                    .or_else(|| survey.capabilities.iter().find(general))
                    .map(|found| found.capability().to_string())
                    .unwrap_or_default()
            });
        let inputs = Inputs::read(&self.task, &executor);
        Some(
            runtime
                .run(&program, &inputs, &grant, self.trace.as_mut())
                .await,
        )
    }

    /// Where this session is being recorded, when it is.
    pub fn trace_path(&self) -> Option<&Path> {
        self.trace.as_ref().map(Recorder::path)
    }

    /// Why this session is not being recorded, or why its recording
    /// stopped.
    pub fn trace_error(&self) -> Option<&str> {
        self.trace_error
            .as_deref()
            .or_else(|| self.trace.as_ref().and_then(Recorder::failure))
    }

    /// Closes the session's trace, so the document says the session ended
    /// rather than that it was interrupted.
    pub fn finish_trace(&mut self) {
        if let Some(trace) = &mut self.trace {
            trace.finish(atif::log::ENDED);
        }
    }

    /// Records why a turn did not finish, so the trace says what the exit
    /// code says.
    ///
    /// A failed turn writes no reply, and a reader holding only the trace
    /// would find a session that stops mid-turn with no reason given: a
    /// relay that would not take the job, a worker that never answered,
    /// and a worker that declined all read as the same missing line.
    pub fn record_failure(&mut self, cause: &str, reason: &str) {
        if let Some(trace) = &mut self.trace {
            trace.note(&format!("the turn did not finish ({cause}): {reason}"));
        }
    }

    /// Records a reply the terminal produced without generating one — the
    /// canned answers an `End` or a `Halt` route gives.
    pub fn record_reply(&mut self, text: &str) {
        if let Some(trace) = &mut self.trace {
            trace.answer(text, None, 0, None);
        }
    }

    /// The model the door serves, which the trace's session header holds.
    pub fn model(&self) -> &str {
        self.generate.model()
    }

    /// What the composer's location rail shows for this door.
    pub fn label(&self) -> &str {
        self.generate.label()
    }

    /// Which kind of door answers this session's turns, such as
    /// `delegate` or `live`.
    pub fn door(&self) -> &str {
        self.generate.name()
    }

    /// Why this session answers through the door it does. Empty for an
    /// agent built over explicit parts.
    pub fn door_reason(&self) -> &str {
        &self.door_reason
    }

    /// Whether a classifier is configured.
    pub fn classifies(&self) -> bool {
        self.classify.is_some()
    }

    /// The transcript so far.
    pub fn transcript(&self) -> &[Message] {
        &self.transcript
    }

    /// Starts a turn: the draft folds into the transcript as the user side
    /// and becomes the state Classify reads.
    pub fn push_user(&mut self, draft: &str) {
        self.task = draft.to_string();
        // A new task is a different judgment input; the last turn's
        // ranking is not this one's answer.
        self.evidence_ranking = None;
        self.transcript.push(Message {
            role: Role::User,
            text: draft.to_string(),
        });
        if let Some(trace) = &mut self.trace {
            trace.user(draft);
        }
    }

    /// Classifies the current state: the questions over the task and the
    /// bounded transcript.
    ///
    /// The call is recorded whether it answers or not, with the state's
    /// digest, the question set, the typed answers, and the route the table
    /// made of them. That is the record the Gym's rows cannot hold: a row
    /// says what one door answered, and this says what the agent did next.
    pub async fn classify(&mut self) -> Classified {
        let Some(classify) = self.classify.clone() else {
            let note = "no decision door is configured — generating unrouted".to_string();
            if let Some(trace) = &mut self.trace {
                trace.note(&note);
            }
            return Classified::Skipped(note);
        };
        let members: &[String] = self.repo.as_ref().map_or(&[], |repo| repo.members());
        let state = state_of(&self.task, &self.transcript, members);
        let request = SystemOneRequest::new(state, questions());
        // The body is what goes on the wire; reading it here is what a
        // recorded exchange means.
        let asked = request
            .body(classify.default_model())
            .map_or(Value::Null, Value::Object);
        let started = Instant::now();
        let answered = classify.system_one(request).await;
        let milliseconds = started.elapsed().as_millis() as u64;
        match answered {
            Ok(response) => {
                let judgment = judgment_of(&response);
                let route = route(&judgment);
                self.record_decision(
                    questions_provenance(),
                    Decision {
                        id: String::new(),
                        name: "classify".to_string(),
                        door: classify.base_url().to_string(),
                        model: response.model.clone(),
                        request: asked,
                        answers: answers_value(&response.answers),
                        route: Some(route.word().to_string()),
                        error: None,
                        attempts: Vec::new(),
                        review: None,
                        milliseconds,
                    },
                );
                Classified::Judged(Verdict { route, judgment })
            }
            Err(error) => {
                self.record_decision(
                    questions_provenance(),
                    Decision {
                        id: String::new(),
                        name: "classify".to_string(),
                        door: classify.base_url().to_string(),
                        model: classify.default_model().to_string(),
                        request: asked,
                        answers: Value::Null,
                        route: None,
                        error: Some(error.to_string()),
                        attempts: Vec::new(),
                        review: None,
                        milliseconds,
                    },
                );
                Classified::Skipped(format!("classify failed ({error}) — generating unrouted"))
            }
        }
    }

    /// Asks the door which of the sniff's observed candidates the task
    /// wants — once per distinct evidence set, then reused while
    /// content, wording, policy, and artifact all stay identical. A
    /// door that will not answer, or an answer that reads as no
    /// ranking, costs the turn its ordering rather than its context:
    /// the refusal lands in the trace and the sniff renders in its own
    /// order. A set naming one path asks nothing — the deterministic
    /// order already names its own most relevant, whichever of its
    /// observations a pick would choose.
    async fn select_evidence(&mut self, sniff: &crate::repo::Sniff) -> Option<Ranked> {
        let classify = self.classify.clone()?;
        let candidates = sniff.candidates();
        let paths: BTreeSet<&str> = candidates
            .candidates
            .iter()
            .map(|candidate| candidate.path.as_str())
            .collect();
        if paths.len() < 2 {
            return None;
        }
        let reused = self.evidence_ranking.as_ref().is_some_and(|ranking| {
            ranking.reusable_for(
                candidates,
                classify.default_model(),
                crate::select::Select::set(),
            )
        });
        let ranking = if reused {
            self.evidence_ranking
                .clone()
                .expect("the reuse check passed")
        } else {
            let request = crate::select::Select::request(&self.task, candidates);
            let asked = request
                .body(classify.default_model())
                .map_or(Value::Null, Value::Object);
            let started = Instant::now();
            let answered = classify.system_one(request).await;
            let milliseconds = started.elapsed().as_millis() as u64;
            let ((ranking, fault), answers, model, error) = match &answered {
                Ok(response) => (
                    match crate::select::Select::ranking(response, candidates) {
                        Ok(ranking) => (Some(ranking), None),
                        Err(fault) => (None, Some(fault.to_string())),
                    },
                    answers_value(&response.answers),
                    response.model.clone(),
                    None,
                ),
                Err(error) => (
                    (None, None),
                    Value::Null,
                    classify.default_model().to_string(),
                    Some(error.to_string()),
                ),
            };
            self.record_decision(
                crate::select::Select::provenance(),
                Decision {
                    id: String::new(),
                    name: "repository/select".to_string(),
                    door: classify.base_url().to_string(),
                    model,
                    request: asked,
                    answers,
                    route: ranking
                        .as_ref()
                        .map(|ranking| ranking.verdict.word().to_string()),
                    error: error.or(fault),
                    attempts: Vec::new(),
                    review: None,
                    milliseconds,
                },
            );
            ranking?
        };
        self.evidence_ranking = Some(ranking.clone());
        let disclosure = self.decision_profile.as_ref().map(|profile| {
            crate::select::Select::disclosure(profile, sniff.read_set(), candidates, &ranking)
        });
        Some(Ranked {
            order: ranking.order(candidates),
            record: crate::select::Record::of(&ranking, candidates, disclosure.as_ref()),
        })
    }

    /// Puts one decision call in the trace, when there is one, with the
    /// wording record beside it — the same fields a file-defined
    /// question set's provenance carries on a program's `decide` step.
    fn record_decision(&mut self, provenance: Value, decision: Decision) {
        if let Some(trace) = &mut self.trace {
            let mut call = decision.call();
            if let Some(fields) = provenance.as_object() {
                call.extra.extend(fields.clone());
            }
            trace.decision_call(call);
        }
    }

    /// Generates the reply the route asks for and folds it into the
    /// transcript as the assistant side. `clarify` swaps the instructions
    /// for the one-question variant. `sink` receives text deltas as they
    /// stream. One generation, no shell loop — [`Agent::turn`] is the
    /// full round.
    pub async fn reply(
        &mut self,
        clarify: bool,
        sink: &mut (dyn FnMut(&str) + Send),
        meta: &mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        let (instructions, evidence) = self.selected_instructions(clarify, false, false).await;
        if let Some(trace) = &mut self.trace {
            trace.instructions_with_repository(&instructions, evidence.as_ref());
        }
        let started = Instant::now();
        let answered = Answered::default();
        let (text, usage) = self
            .generate
            .generate(
                &instructions,
                &self.transcript,
                sink,
                &mut answered.watching(meta),
            )
            .await?;
        let milliseconds = started.elapsed().as_millis() as u64;
        if let Some(trace) = &mut self.trace {
            trace.answer(&text, usage, milliseconds, answered.model().as_deref());
        }
        self.transcript.push(Message {
            role: Role::Assistant,
            text: text.clone(),
        });
        Ok((text, usage))
    }

    /// A whole turn: generate, run a plan when this turn is permitted to
    /// and the reply carries a supported one, judge the round, and go
    /// again until the model answers in prose or the permit runs out.
    /// `shell` hears each proposal, outcome, and verdict as it happens so
    /// the terminal can draw the loop.
    ///
    /// `clarify` shapes the prompt and `permit` says what the host
    /// allows. They are two arguments because they are two decisions: a
    /// clarifying turn asks one question, and the reason it runs nothing
    /// is the permit, not the wording. A caller that asks for
    /// clarification and hands over an executing permit does not get one
    /// — this withdraws it — but [`crate::permit::Permit::for_route`] is
    /// where the host makes that call.
    pub async fn turn(
        &mut self,
        clarify: bool,
        permit: Permit,
        sink: &mut (dyn FnMut(&str) + Send),
        meta: &mut (dyn FnMut(Meta) + Send),
        shell: &mut (dyn FnMut(ShellEvent) + Send),
    ) -> Result<Turned, GenerateError> {
        let mut total: Option<Usage> = None;
        let mut rounds = 0usize;
        let mut repairs = 0usize;
        let mut retries = 0usize;
        let mut exhausted: Option<Exhausted> = None;
        let mut ran: Vec<Outcome> = Vec::new();
        let mut permit = match clarify {
            true => permit.withdrawn(),
            false => permit,
        };
        if let Door::Delegate(door) = &self.generate {
            let door = std::sync::Arc::clone(door);
            return self
                .delegated(&door, clarify, permit, sink, meta, shell)
                .await;
        }
        loop {
            // A turn that runs nothing is on its last word, so it is told
            // so — except while clarifying, where the one question it is
            // asking for is the whole instruction.
            let final_only = !permit.executes() && !clarify;
            let (instructions, evidence) = self
                .selected_instructions(clarify, final_only, retries > 0)
                .await;
            if let Some(trace) = &mut self.trace {
                trace.instructions_with_repository(&instructions, evidence.as_ref());
            }
            let started = Instant::now();
            let answered = Answered::default();
            let (text, usage) = self
                .generate
                .generate(
                    &instructions,
                    &self.transcript,
                    sink,
                    &mut answered.watching(meta),
                )
                .await?;
            let milliseconds = started.elapsed().as_millis() as u64;
            if let Some(trace) = &mut self.trace {
                trace.answer(&text, usage, milliseconds, answered.model().as_deref());
            }
            if let Some(usage) = usage {
                let entry = total.get_or_insert(Usage {
                    input_tokens: 0,
                    output_tokens: 0,
                });
                entry.input_tokens += usage.input_tokens;
                entry.output_tokens += usage.output_tokens;
            }
            // What the reply is, is the host's read of it under this
            // turn's permit. Prose is the answer. A plan the host will not
            // run is refused: the trace keeps the plan and the reason, the
            // model gets one chance to answer in prose when the loop had
            // run, and past that the host answers with what was observed.
            let plan = match Reply::read(&text, permit) {
                Reply::Plan(plan) => plan,
                Reply::Answer(text) => {
                    self.transcript.push(Message {
                        role: Role::Assistant,
                        text: text.clone(),
                    });
                    return Ok(Turned {
                        text,
                        usage: total,
                        ending: Ending::Answered,
                        exhausted,
                        commands: ran.len(),
                        cost_usd: None,
                    });
                }
                Reply::Refused { text, why } => {
                    if let Some(trace) = &mut self.trace {
                        trace.note(&format!("the host ran none of this reply: {why}"));
                    }
                    self.transcript.push(Message {
                        role: Role::Assistant,
                        text: text.clone(),
                    });
                    if exhausted.is_some() && repairs < REPAIRS_MAX {
                        repairs += 1;
                        self.transcript.push(Message {
                            role: Role::User,
                            text: REPAIR.to_string(),
                        });
                        continue;
                    }
                    let shown = shell::refusal_text(&why, exhausted.map(Exhausted::sentence), &ran);
                    if let Some(trace) = &mut self.trace {
                        trace.answer(&shown, None, 0, None);
                    }
                    return Ok(Turned {
                        text: shown,
                        usage: total,
                        ending: Ending::Refused {
                            why,
                            proposal: text,
                        },
                        exhausted,
                        commands: ran.len(),
                        cost_usd: None,
                    });
                }
            };
            rounds += 1;
            self.transcript.push(Message {
                role: Role::Assistant,
                text,
            });
            let mut outcomes: Vec<Outcome> = Vec::new();
            for proposal in plan.proposals {
                shell(ShellEvent::Proposed(proposal.clone()));
                let outcome = shell::run(&proposal, permit).await;
                if let Some(trace) = &mut self.trace {
                    trace.command(&outcome);
                }
                shell(ShellEvent::Ran(outcome.clone()));
                outcomes.push(outcome);
            }
            let route = self.judge(&outcomes, shell).await;
            self.transcript.push(Message {
                role: Role::User,
                text: shell::transcript_of(&outcomes),
            });
            ran.extend(outcomes);
            retries = match route {
                ShellRoute::Retry => retries + 1,
                ShellRoute::Pass | ShellRoute::Stop => 0,
            };
            let spent = match (route, rounds >= permit.rounds()) {
                (ShellRoute::Stop, _) => Some(Exhausted::Stopped),
                (_, true) => Some(Exhausted::Rounds(rounds)),
                (ShellRoute::Retry, false) if retries >= RETRIES_MAX => {
                    Some(Exhausted::Retries(retries))
                }
                _ => None,
            };
            if let Some(spent) = spent {
                if let Some(trace) = &mut self.trace {
                    trace.note(&format!(
                        "execution withdrawn: {}; {} commands ran",
                        spent.sentence(),
                        ran.len()
                    ));
                }
                exhausted = Some(spent);
                permit = permit.withdrawn();
            }
        }
    }

    /// A turn the delegate door answers: Coder One's probes, Jev's
    /// judgments, a briefing, and an executor, streamed as this turn's own
    /// events and recorded to this session's trace.
    ///
    /// The permit decides the boundary. A turn that runs no commands runs
    /// the executor read-only, and nothing the executor says widens that.
    async fn delegated(
        &mut self,
        door: &crate::delegate_door::DelegateDoor,
        clarify: bool,
        permit: Permit,
        sink: &mut (dyn FnMut(&str) + Send),
        meta: &mut (dyn FnMut(Meta) + Send),
        shell: &mut (dyn FnMut(ShellEvent) + Send),
    ) -> Result<Turned, GenerateError> {
        use crate::delegate_door::Update;
        let read_only = !permit.executes();
        let earlier = crate::delegate_door::earlier(&self.transcript);
        let request = self.task.clone();
        if let Some(trace) = &mut self.trace {
            trace.note(&format!(
                "delegating to {} in a {} boundary{}",
                door.label(),
                if read_only {
                    "read-only"
                } else {
                    "workspace-writable"
                },
                door.session()
                    .map(|id| format!(", resuming session {id}"))
                    .unwrap_or_default()
            ));
        }
        let started = Instant::now();
        let mut said = false;
        let done = door
            .answer(
                &request,
                &earlier,
                read_only,
                clarify,
                &mut |update| match update {
                    Update::Line(line) => meta(Meta::Judgment(line)),
                    Update::Text(text) => {
                        if said {
                            sink("\n\n");
                        }
                        said = true;
                        sink(&text);
                    }
                    Update::Proposed(proposal) => shell(ShellEvent::Proposed(proposal)),
                    Update::Ran(outcome) => shell(ShellEvent::Ran(outcome)),
                },
            )
            .await?;
        let milliseconds = started.elapsed().as_millis() as u64;
        let turn = self
            .transcript
            .iter()
            .filter(|message| message.role == Role::User)
            .count();
        if let Some(trace) = &mut self.trace {
            trace.external(done.steps.clone(), &format!("turn-{turn}/"));
            trace.delegated(&done.summary);
        }
        if let Some(failure) = done.failure {
            return Err(failure);
        }
        meta(Meta::Model(done.model.clone()));
        if let Some(trace) = &mut self.trace {
            trace.answer(&done.text, done.usage, milliseconds, Some(&done.model));
        }
        self.transcript.push(Message {
            role: Role::Assistant,
            text: done.text.clone(),
        });
        Ok(Turned {
            text: done.text,
            usage: done.usage,
            ending: Ending::Answered,
            exhausted: None,
            commands: done.commands,
            cost_usd: done.cost_usd,
        })
    }

    /// The instructions for one generation: the base text, the clarify,
    /// retry, or final suffix, and the repo context block — the
    /// deterministic path, exercised directly by tests; `reply` and
    /// `turn` take the selected path.
    #[cfg(test)]
    fn instructions(
        &self,
        clarify: bool,
        final_only: bool,
        retrying: bool,
    ) -> (String, Option<crate::repo::RepositoryEvidence>) {
        self.instructions_from(clarify, final_only, retrying, None, None, None)
    }

    /// The instructions a generation round takes: the text `instructions`
    /// composes, except the repo block renders from an observation
    /// already made — in the order a ranking gave it, when a decision
    /// door answered, with the manifest recording the judgment beside
    /// the evidence it ranked.
    async fn selected_instructions(
        &mut self,
        clarify: bool,
        final_only: bool,
        retrying: bool,
    ) -> (String, Option<crate::repo::RepositoryEvidence>) {
        let sniff = self.repo.as_ref().map(|repo| repo.observe(&self.task));
        let ranked = match &sniff {
            Some(sniff) => self.select_evidence(sniff).await,
            None => None,
        };
        let (order, record) = match ranked {
            Some(ranked) => (ranked.order, Some(ranked.record)),
            None => (None, None),
        };
        self.instructions_from(clarify, final_only, retrying, sniff, order, record)
    }

    fn instructions_from(
        &self,
        clarify: bool,
        final_only: bool,
        retrying: bool,
        sniff: Option<crate::repo::Sniff>,
        order: Option<Vec<String>>,
        record: Option<crate::select::Record>,
    ) -> (String, Option<crate::repo::RepositoryEvidence>) {
        let mut instructions = if clarify {
            format!("{INSTRUCTIONS}{CLARIFY_SUFFIX}")
        } else {
            INSTRUCTIONS.to_string()
        };
        if final_only {
            instructions.push_str(FINAL_SUFFIX);
        } else if retrying {
            instructions.push_str(RETRY_SUFFIX);
        }
        instructions.push_str("\n\n");
        let mut context = self.about.context();
        let mut evidence = None;
        if let Some(repo) = &self.repo {
            context.push_str("\n\n");
            let (rendered, mut captured) = match &sniff {
                Some(sniff) => repo.render(sniff, order.as_deref()),
                None => repo.context_with_evidence(&self.task),
            };
            if let Some(record) = record {
                captured.set_selection(record);
            }
            context.push_str(&rendered);
            evidence = Some(captured);
        }
        instructions.push_str(&context);
        (instructions, evidence)
    }

    /// The judge's read on a round of outcomes: `Pass` without a
    /// classifier, the verdict's route otherwise, with the display line
    /// reported to `shell` either way.
    async fn judge(
        &mut self,
        outcomes: &[Outcome],
        shell: &mut (dyn FnMut(ShellEvent) + Send),
    ) -> ShellRoute {
        let Some(classify) = self.classify.clone() else {
            return ShellRoute::Pass;
        };
        let state = shell::state_of(&self.task, outcomes);
        let request = SystemOneRequest::new(state, shell_questions());
        let asked = request
            .body(classify.default_model())
            .map_or(Value::Null, Value::Object);
        let started = Instant::now();
        let answered = classify.system_one(request).await;
        let milliseconds = started.elapsed().as_millis() as u64;
        match answered {
            Ok(response) => {
                let verdict = shell_verdict_of(&response);
                let route = verdict.route();
                self.record_decision(
                    shell_provenance(),
                    Decision {
                        id: String::new(),
                        name: "shell_judge".to_string(),
                        door: classify.base_url().to_string(),
                        model: response.model.clone(),
                        request: asked,
                        answers: answers_value(&response.answers),
                        route: Some(route.word().to_string()),
                        error: None,
                        attempts: Vec::new(),
                        review: None,
                        milliseconds,
                    },
                );
                shell(ShellEvent::Verdict(verdict.line()));
                route
            }
            Err(error) => {
                self.record_decision(
                    shell_provenance(),
                    Decision {
                        id: String::new(),
                        name: "shell_judge".to_string(),
                        door: classify.base_url().to_string(),
                        model: classify.default_model().to_string(),
                        request: asked,
                        answers: Value::Null,
                        route: None,
                        error: Some(error.to_string()),
                        attempts: Vec::new(),
                        review: None,
                        milliseconds,
                    },
                );
                ShellRoute::Pass
            }
        }
    }
}

/// The model that answered one generation, as the door reported it.
///
/// A door that forwards a turn somewhere else names its model in the
/// answer rather than in its configuration, and it says so through
/// [`Meta::Model`] as the answer lands. This keeps that name on its way
/// past, so the answer step records what produced it, and passes every
/// sideband item through to the caller unchanged.
#[derive(Default)]
struct Answered(std::sync::Mutex<Option<String>>);

impl Answered {
    /// The caller's sideband sink, watched for a model name.
    fn watching<'a>(
        &'a self,
        meta: &'a mut (dyn FnMut(Meta) + Send),
    ) -> impl FnMut(Meta) + Send + 'a {
        move |item| {
            if let Meta::Model(name) = &item
                && let Ok(mut named) = self.0.lock()
            {
                *named = Some(name.clone());
            }
            meta(item);
        }
    }

    /// The model, when the door named one.
    fn model(&self) -> Option<String> {
        self.0.lock().ok().and_then(|named| named.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::StubGenerate;

    /// An agent whose door answers with `line`, every time.
    fn saying(line: String) -> Agent {
        Agent::new(None, Door::Stub(StubGenerate::saying(line)))
    }

    /// An agent whose door plays `script` once and then says `line`.
    fn scripted(script: Vec<String>, line: &str) -> Agent {
        Agent::new(None, Door::Stub(StubGenerate::scripted(script, line)))
    }

    /// A read-only plan of `count` commands, none of which change anything.
    fn plan_reading(count: usize) -> String {
        serde_json::json!({
            "v": 1,
            "commands": (0..count)
                .map(|index| serde_json::json!({
                    "command": format!("printf 'observed {index}'"),
                    "why": "look",
                }))
                .collect::<Vec<_>>(),
        })
        .to_string()
    }

    /// A plan that writes `marker`, which is how a test tells whether
    /// anything ran.
    fn plan_writing(marker: &Path) -> String {
        serde_json::json!({
            "v": 1,
            "commands": [{
                "command": format!("printf harmless > '{}'", marker.display()),
                "why": "write a marker",
            }],
        })
        .to_string()
    }

    /// The audit's A01 case. The router asked for a clarifying question,
    /// the door answered with a valid plan anyway, and the file it would
    /// have written is not there. The caller even handed over a permit
    /// that runs commands; clarification withdraws it.
    #[tokio::test]
    async fn a_clarifying_turn_runs_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("clarify");
        let plan = plan_writing(&marker);
        let mut agent = saying(plan.clone());
        agent.push_user("Ask a clarifying question");
        let turned = agent
            .turn(
                true,
                Permit::executing(),
                &mut |_| {},
                &mut |_| {},
                &mut |_| {},
            )
            .await
            .unwrap();
        assert!(!marker.exists(), "a clarifying turn ran a command");
        assert!(
            matches!(&turned.ending, Ending::Refused { proposal, .. } if *proposal == plan),
            "{:?}",
            turned.ending
        );
        assert!(
            turned.text.contains("none of them ran") && !turned.text.contains("\"commands\""),
            "the user reads a refusal, not the plan: {}",
            turned.text
        );
        assert_eq!(turned.commands, 0);
        assert_eq!(turned.exhausted, None);
    }

    /// The operator's permit is the host's, so a turn that carries no
    /// execution runs nothing however it was routed.
    #[tokio::test]
    async fn an_answering_permit_runs_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("answering");
        let mut agent = saying(plan_writing(&marker));
        agent.push_user("read the repository");
        agent
            .turn(
                false,
                Permit::answering(),
                &mut |_| {},
                &mut |_| {},
                &mut |_| {},
            )
            .await
            .unwrap();
        assert!(!marker.exists(), "a turn with no permit ran a command");
    }

    /// The audit's other A01 case, at the turn level: a reply that quotes
    /// a plan as an example runs nothing, on a turn that would have run a
    /// real one.
    #[tokio::test]
    async fn a_quoted_example_runs_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("example");
        let example = format!(
            "Here is an example; do not run it.\n```json\n{}\n```\nThat is the format.",
            plan_writing(&marker)
        );
        let mut agent = saying(example.clone());
        agent.push_user("what does a plan look like");
        let Turned { text: reply, .. } = agent
            .turn(
                false,
                Permit::executing(),
                &mut |_| {},
                &mut |_| {},
                &mut |_| {},
            )
            .await
            .unwrap();
        assert!(!marker.exists(), "a quoted example ran");
        assert_eq!(reply, example);
    }

    /// Closing the door on examples does not close it on work: a
    /// supported plan on a permitted turn still runs, and the loop still
    /// stops when the permit's rounds are spent.
    #[tokio::test]
    async fn a_permitted_plan_runs() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("permitted");
        let mut agent = saying(plan_writing(&marker));
        agent.push_user("write the marker");
        let mut ran = 0usize;
        agent
            .turn(
                false,
                Permit::executing(),
                &mut |_| {},
                &mut |_| {},
                &mut |event| {
                    if matches!(event, ShellEvent::Ran(_)) {
                        ran += 1;
                    }
                },
            )
            .await
            .unwrap();
        assert!(marker.exists(), "a permitted plan did not run");
        assert_eq!(ran, Permit::executing().rounds());
    }

    /// The audit's exhaustion case: three valid rounds, then the model
    /// answers the finished loop with a fourth plan, and again when asked
    /// to repair it. Exactly nine commands ran, the fourth plan did not,
    /// the turn ended as a refusal rather than as the plan's JSON, and what
    /// the nine commands observed is in the reply.
    #[tokio::test]
    async fn an_exhausted_loop_ends_as_a_refusal_with_what_it_saw() {
        let rounds = Permit::executing().rounds();
        let mut agent = scripted(vec![plan_reading(3); rounds], &plan_reading(2));
        agent.push_user("what does the bottom-right rail mean");
        let mut proposed = 0usize;
        let mut ran = 0usize;
        let turned = agent
            .turn(
                false,
                Permit::executing(),
                &mut |_| {},
                &mut |_| {},
                &mut |event| match event {
                    ShellEvent::Proposed(_) => proposed += 1,
                    ShellEvent::Ran(_) => ran += 1,
                    ShellEvent::Verdict(_) => {}
                },
            )
            .await
            .unwrap();
        assert_eq!(proposed, 3 * rounds);
        assert_eq!(ran, 3 * rounds, "the fourth plan ran");
        assert_eq!(turned.commands, 3 * rounds);
        assert_eq!(turned.exhausted, Some(Exhausted::Rounds(rounds)));
        assert!(
            matches!(&turned.ending, Ending::Refused { proposal, .. } if *proposal == plan_reading(2)),
            "{:?}",
            turned.ending
        );
        assert!(turned.text.contains("none of them ran"), "{}", turned.text);
        assert!(turned.text.contains("permitted rounds"), "{}", turned.text);
        assert!(
            turned.text.contains("observed 0") && turned.text.contains("exit 0"),
            "what ran is not in the reply: {}",
            turned.text
        );
        assert!(
            !turned.text.contains("\"commands\""),
            "the reply is the plan's JSON: {}",
            turned.text
        );
        // Three plans, one refused plan, one repair asked and refused: the
        // model heard the repair request once and no more.
        let repairs = agent
            .transcript()
            .iter()
            .filter(|message| message.role == Role::User && message.text == REPAIR)
            .count();
        assert_eq!(repairs, REPAIRS_MAX);
    }

    /// A model that takes the repair answers: the fourth plan is refused,
    /// the repair request lands, and the prose it produces is the answer.
    #[tokio::test]
    async fn a_repaired_exhaustion_answers_in_prose() {
        let rounds = Permit::executing().rounds();
        let mut script = vec![plan_reading(1); rounds];
        script.push(plan_reading(1));
        let mut agent = scripted(script, "The rail shows tokens for the last turn.");
        agent.push_user("what does the rail mean");
        let mut ran = 0usize;
        let turned = agent
            .turn(
                false,
                Permit::executing(),
                &mut |_| {},
                &mut |_| {},
                &mut |event| {
                    if matches!(event, ShellEvent::Ran(_)) {
                        ran += 1;
                    }
                },
            )
            .await
            .unwrap();
        assert_eq!(ran, rounds);
        assert_eq!(turned.ending, Ending::Answered);
        assert_eq!(turned.exhausted, Some(Exhausted::Rounds(rounds)));
        assert_eq!(turned.text, "The rail shows tokens for the last turn.");
    }

    /// A model that answers in prose after the rounds are spent needs no
    /// repair, and the turn says the loop was exhausted all the same.
    #[tokio::test]
    async fn prose_after_exhaustion_is_the_answer() {
        let rounds = Permit::executing().rounds();
        let mut agent = scripted(vec![plan_reading(1); rounds], "nine crates.");
        agent.push_user("count the crates");
        let turned = agent
            .turn(
                false,
                Permit::executing(),
                &mut |_| {},
                &mut |_| {},
                &mut |_| {},
            )
            .await
            .unwrap();
        assert_eq!(turned.ending, Ending::Answered);
        assert_eq!(turned.text, "nine crates.");
        assert_eq!(turned.exhausted, Some(Exhausted::Rounds(rounds)));
        assert_eq!(turned.commands, rounds);
    }

    /// A retry verdict reaches the next generation as an instruction; a
    /// finished loop's final suffix takes its place, since a turn that runs
    /// nothing more has nothing to retry.
    #[test]
    fn a_retry_changes_the_next_instructions() {
        let agent = saying("hi".to_string());
        let plain = agent.instructions(false, false, false).0;
        assert!(!plain.contains(RETRY_SUFFIX), "{plain}");
        let retrying = agent.instructions(false, false, true).0;
        assert!(retrying.contains(RETRY_SUFFIX), "{retrying}");
        let last = agent.instructions(false, true, true).0;
        assert!(
            last.contains(FINAL_SUFFIX) && !last.contains(RETRY_SUFFIX),
            "{last}"
        );
    }

    /// A plan the host cannot read on a permitted turn is refused before
    /// anything runs, with no repair: the loop never started.
    #[tokio::test]
    async fn an_unsupported_plan_is_refused_without_a_repair() {
        let plan = r#"{"v":2,"commands":[{"command":"ls","why":"look"}]}"#;
        let mut agent = saying(plan.to_string());
        agent.push_user("list the files");
        let mut proposed = 0usize;
        let turned = agent
            .turn(
                false,
                Permit::executing(),
                &mut |_| {},
                &mut |_| {},
                &mut |event| {
                    if matches!(event, ShellEvent::Proposed(_)) {
                        proposed += 1;
                    }
                },
            )
            .await
            .unwrap();
        assert_eq!(proposed, 0);
        assert!(
            matches!(turned.ending, Ending::Refused { .. }),
            "{:?}",
            turned.ending
        );
        assert_eq!(turned.exhausted, None);
        assert!(
            turned.text.contains("No command ran on this turn"),
            "{}",
            turned.text
        );
        assert_eq!(agent.transcript().len(), 2, "a repair was asked for");
    }

    /// The prompt carries both blocks: the application's, always, and
    /// the repository's when there is one. An agent in an unrelated
    /// repository with a decoy terminal directory still names its real
    /// working directory and says Coder's source is not there, while the
    /// repo context still describes that repository for project questions.
    #[test]
    fn the_instructions_tell_the_application_from_the_workspace() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("crates/coder-terminal/src")).unwrap();
        std::fs::write(
            dir.path().join("crates/coder-terminal/src/rail.rs"),
            "// decoy\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("README.md"), "# decoy project\n").unwrap();
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(dir.path())
                .output()
                .unwrap()
        };
        git(&["init", "-q"]);
        let outside = saying("hi".to_string());
        let none = outside.instructions(false, false, false).0;
        assert!(none.contains("about this application:"), "{none}");
        assert!(none.contains("not available on this machine"), "{none}");

        let repo = Repo::discover(dir.path());
        assert!(repo.is_some(), "the decoy is a repository");
        let inside = saying("hi".to_string()).with_repo(repo);
        let both = inside.instructions(false, false, false).0;
        assert!(
            both.contains(&format!("workspace repository: {}", dir.path().display()))
                || both.contains(&format!(
                    "workspace repository: {}",
                    dir.path().canonicalize().unwrap().display()
                )),
            "{both}"
        );
        assert!(both.contains("not available on this machine"), "{both}");
        assert!(both.contains("repo context:"), "{both}");
        assert!(
            both.find("about this application:") < both.find("repo context:"),
            "the application block comes first"
        );
        assert!(!inside.about().in_own_source());
    }

    #[tokio::test]
    async fn classify_without_a_key_skips_with_a_note() {
        let mut agent = Agent::new(None, Door::Stub(StubGenerate::default()));
        let Classified::Skipped(note) = agent.classify().await else {
            panic!("expected a skipped classify");
        };
        assert!(note.contains("decision door"));
    }

    #[tokio::test]
    async fn a_reply_streams_and_joins_the_transcript() {
        let mut agent = Agent::new(None, Door::Stub(StubGenerate::default()));
        agent.push_user("hello");
        let mut seen = String::new();
        let (text, _) = agent
            .reply(false, &mut |d| seen.push_str(d), &mut |_| {})
            .await
            .unwrap();
        assert_eq!(text, seen);
        assert_eq!(agent.transcript().len(), 2);
        assert_eq!(agent.transcript()[1].role, Role::Assistant);
    }
}
