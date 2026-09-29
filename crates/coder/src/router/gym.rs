//! The Gym and eval routes: the verified records a reply may use, and
//! [`reply`], which decides what a `gym.*` or `eval.*` turn shows from
//! them.
//!
//! People use the Gym and extension evals through the one routed chat
//! (`docs/extensions/evaluation.md`, "Chat: the product path"). The route
//! is Jev's typed `route` reading and the tool is its typed `tool` reading;
//! nothing here reads the message. What a turn shows comes from three
//! places only:
//!
//! - **Records** the Gym seam ([`super::seams::GymKb`]) verified: published
//!   results and checks (`3189` with `oa:ext-eval:v1`, read by
//!   `nostr::eval_ext::parse_publication`), published test sets, adoptions
//!   (`coder-defaults` releases), the app's changelog, and our product
//!   notes. Every number in a card is a field of one of these records,
//!   copied, never computed from text.
//! - **The answer bank**, whose Gym entries are picked here from what the
//!   records hold ([`super::bank::Entry::records`]), never by the `answer`
//!   question, and whose `{slots}` are filled from the records.
//! - **The model**, for `gym.news` only, told to answer from the given
//!   items and cite them ([`instructions`]); a citation of anything else is
//!   reported as invented ([`check_reply`]).
//!
//! With no record for what was asked, the reply says so (CHK-13).

pub use nostr::contracts::{ArtifactRef, DefinitionRef};
pub use nostr::eval_ext::{EventPointer, Headline, Verdict};

use super::bank::{Bank, Entry, Facts};
use super::card::Card;
use super::{Offer, RouteId, Screen};
use nostr::cj_conversation::{Size, SubjectSource, SuiteSource, Where};
use nostr::eval_ext::{HOSTED_MAX_CASES, HOSTED_MAX_RUNS};

/// The most news items a reply and its card carry (`CARD-05.E01`).
pub const MAX_NEWS: usize = nostr::cj_conversation::MAX_NEWS;

/// The confirming checks at which a result is a candidate for adoption; a
/// result with fewer is offered for checking (`eval-check`).
pub const CHECKS_FOR_ADOPTION: u64 = 3;

/// Runs per arm a full run asks for (the engine's default, and the hosted
/// runner's most).
pub const FULL_RUNS: u64 = nostr::eval_ext::DEFAULT_RUNS;

/// The prefix of an item's citation id.
pub const CITE_PREFIX: &str = "gym:";

/// The first 8 hex digits of an event's id, for a citation id.
#[must_use]
pub fn short(event: &EventPointer) -> &str {
    &event.id[..event.id.len().min(8)]
}

/// Where a record comes from: a signed event, or a path in this public
/// repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Event(EventPointer),
    Path(String),
}

impl Source {
    /// How the model's reference names it.
    #[must_use]
    pub fn cite(&self) -> String {
        match self {
            Source::Event(event) => format!("event {} (kind {})", event.id, event.kind),
            Source::Path(path) => path.clone(),
        }
    }

    /// The card's source.
    #[must_use]
    pub fn card(&self) -> nostr::cj_conversation::Source {
        match self {
            Source::Event(event) => nostr::cj_conversation::Source::Event(event.clone()),
            Source::Path(path) => nostr::cj_conversation::Source::Path(path.clone()),
        }
    }
}

/// The verdict in the phone's words.
#[must_use]
pub fn plain(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Pass => "Better",
        Verdict::Inconclusive => "No clear change",
        Verdict::Fail => "Worse",
    }
}

/// A tool the chat can talk about: one of our product notes tagged as a
/// tool (`knowledge/openagents/openagents.tool-*.md`), whose title is its
/// name and whose summary is its plain line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tool {
    /// The note's id, `openagents.tool-project-map`; the `tool` question's
    /// option.
    pub id: String,
    /// Its name on screen, **Project map**.
    pub name: String,
    /// What it does, in one plain line.
    pub line: String,
    /// The note's path.
    pub source: String,
    /// The component slugs a published extension of this tool has
    /// (`repo-map`): the note's tags, which [`crate::gym_kb`] matches a
    /// subject's DefinitionRef against.
    pub slugs: Vec<String>,
}

/// Confirming and disputing checks of a result, from the checks' own
/// verified publications (`nostr::eval_ext::linkage`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Checks {
    pub confirmed: u64,
    pub disputed: u64,
}

/// A verified published result, or a check of one.
#[derive(Clone, Debug, PartialEq)]
pub struct ResultRecord {
    /// The publication.
    pub publication: EventPointer,
    /// The tool's id in our catalog ([`Tool::id`]), when it is one of ours.
    pub tool: Option<String>,
    /// The tool's name: the catalog's, else the subject's package.
    pub tool_name: String,
    /// The trainer it is credited to (a hosted run's requester, else the
    /// evaluator).
    pub trainer: String,
    /// The test set's release.
    pub suite: EventPointer,
    /// How many tests the test set has.
    pub cases: u64,
    pub subject: DefinitionRef,
    pub headline: Headline,
    pub verdict: Verdict,
    /// The report's ArtifactRef.
    pub report: ArtifactRef,
    /// The publication this one checks, when it is a check.
    pub checks: Option<String>,
    /// The checks of this result.
    pub checked: Checks,
    /// `created_at`, in Unix seconds.
    pub at: u64,
}

/// A published test set: its release, its tool, and its size. Read from a
/// verified `eval-suite` release, or from a verified result that ran it.
#[derive(Clone, Debug, PartialEq)]
pub struct SuiteRecord {
    pub release: EventPointer,
    pub tool: Option<String>,
    pub tool_name: String,
    /// The author: the release's signer.
    pub author: String,
    pub subject: DefinitionRef,
    /// Its tests.
    pub cases: u64,
    pub at: u64,
    /// The record it was read from.
    pub source: EventPointer,
}

/// A verified adoption: a `coder-defaults` release that depends on a
/// tool's release.
#[derive(Clone, Debug, PartialEq)]
pub struct AdoptionRecord {
    pub release: EventPointer,
    pub tool: Option<String>,
    pub tool_name: String,
    pub at: u64,
}

/// One build of the app, from its changelog
/// (`crates/openagents-mobile/src/account.rs`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub build: String,
    pub title: String,
    /// Its items' titles.
    pub items: Vec<String>,
    /// The changelog's path.
    pub source: String,
}

/// One of our product notes about the Gym (`knowledge/openagents/`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    /// `openagents.gym-news@1`.
    pub id: String,
    pub title: String,
    pub summary: String,
    /// The note's path.
    pub source: String,
}

/// One news item: exactly one record. Items live for one turn, so the
/// variants are not boxed.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Item {
    Result(ResultRecord),
    TestSet(SuiteRecord),
    Adoption(AdoptionRecord),
    Build(Release),
    Note(Note),
}

fn cut(text: String, chars: usize) -> String {
    if text.chars().count() <= chars {
        return text;
    }
    let mut out: String = text.chars().take(chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Tests passed without and with the tool, in words; every number is the
/// headline's.
fn passes(headline: &Headline) -> String {
    match headline.baseline_passed {
        Some(without) => format!(
            "Without the tool Coder passed {without} of {total} tests; with it, {with} of {total}.",
            total = headline.total,
            with = headline.subject_passed
        ),
        None => format!(
            "With the tool Coder passed {} of {} tests; there was no run without it.",
            headline.subject_passed, headline.total
        ),
    }
}

impl Item {
    /// The kind: `result`, `check`, `test_set`, `adoption`, `build`, or
    /// `note`.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Item::Result(result) if result.checks.is_some() => "check",
            Item::Result(_) => "result",
            Item::TestSet(_) => "test_set",
            Item::Adoption(_) => "adoption",
            Item::Build(_) => "build",
            Item::Note(_) => "note",
        }
    }

    /// The citation id the model uses, `gym:result:1a2b3c4d`: stable for a
    /// record, and never a number the model could take for a count.
    #[must_use]
    pub fn id(&self) -> String {
        let tail = match self {
            Item::Result(result) => short(&result.publication).to_string(),
            Item::TestSet(suite) => short(&suite.release).to_string(),
            Item::Adoption(adoption) => short(&adoption.release).to_string(),
            Item::Build(release) => format!("build-{}", release.build),
            Item::Note(note) => note
                .id
                .split_once('@')
                .map_or(note.id.as_str(), |(id, _)| id)
                .trim_start_matches(knowledge::product::PREFIX)
                .to_string(),
        };
        format!("{CITE_PREFIX}{}:{tail}", self.kind())
    }

    /// Where it comes from.
    #[must_use]
    pub fn source(&self) -> Source {
        match self {
            Item::Result(result) => Source::Event(result.publication.clone()),
            Item::TestSet(suite) => Source::Event(suite.source.clone()),
            Item::Adoption(adoption) => Source::Event(adoption.release.clone()),
            Item::Build(release) => Source::Path(release.source.clone()),
            Item::Note(note) => Source::Path(note.source.clone()),
        }
    }

    /// When, in Unix seconds, for the records that say; builds and notes
    /// have no time.
    #[must_use]
    pub fn at(&self) -> Option<u64> {
        match self {
            Item::Result(result) => Some(result.at),
            Item::TestSet(suite) => Some(suite.at),
            Item::Adoption(adoption) => Some(adoption.at),
            Item::Build(_) | Item::Note(_) => None,
        }
    }

    /// The card's title and line, in the phone's plain words; every number
    /// is copied from the record.
    #[must_use]
    pub fn title_and_line(&self) -> (String, String) {
        let (title, line) = match self {
            Item::Result(result) if result.checks.is_some() => (
                format!("A check of a {} result", result.tool_name),
                format!(
                    "{} The check's verdict: {}.",
                    passes(&result.headline),
                    plain(result.verdict)
                ),
            ),
            Item::Result(result) => (
                format!("{}: {}", result.tool_name, plain(result.verdict)),
                format!(
                    "{} Confirmed by {} checks, disputed by {}.",
                    passes(&result.headline),
                    result.checked.confirmed,
                    result.checked.disputed
                ),
            ),
            Item::TestSet(suite) => (
                format!("A test set for {}", suite.tool_name),
                format!(
                    "{} tests, each run with the tool and without it.",
                    suite.cases
                ),
            ),
            Item::Adoption(adoption) => (
                format!("Coder now uses {}", adoption.tool_name),
                "Adopted into Coder's defaults, for everyone.".to_string(),
            ),
            Item::Build(release) => (
                format!("Build {}: {}", release.build, release.title),
                release.items.join("; "),
            ),
            Item::Note(note) => (note.title.clone(), note.summary.clone()),
        };
        (cut(title, 120), cut(line, 400))
    }

    /// What the relevance judgment and the grounded model read.
    #[must_use]
    pub fn text(&self) -> String {
        let (title, line) = self.title_and_line();
        let what = match self {
            Item::Result(_) => "A published result",
            Item::TestSet(_) => "A published test set",
            Item::Adoption(_) => "An adoption",
            Item::Build(release) => {
                return format!(
                    "App build {} (version {}): {}. {}",
                    release.build,
                    release.version,
                    release.title,
                    release.items.join("; ")
                );
            }
            Item::Note(_) => "Our note",
        };
        format!("{what}. {title}. {line}")
    }

    /// The card's item.
    #[must_use]
    pub fn card(&self) -> nostr::cj_conversation::NewsItem {
        let (title, line) = self.title_and_line();
        nostr::cj_conversation::NewsItem {
            title,
            line,
            source: self.source().card(),
        }
    }
}

/// Everything the Gym seam verified, as of one read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Records {
    /// Our tool catalog, in its order; the first is the default tool.
    pub tools: Vec<Tool>,
    /// Published results and checks, newest first.
    pub results: Vec<ResultRecord>,
    /// Published test sets, newest first.
    pub suites: Vec<SuiteRecord>,
    /// Adoptions, newest first.
    pub adoptions: Vec<AdoptionRecord>,
    /// App builds, newest first.
    pub releases: Vec<Release>,
    /// Gym product notes.
    pub notes: Vec<Note>,
}

impl Records {
    /// Every record as a news item: results, test sets, and adoptions
    /// newest first, then builds newest first, then notes.
    #[must_use]
    pub fn items(&self) -> Vec<Item> {
        let mut dated: Vec<Item> = self
            .results
            .iter()
            .cloned()
            .map(Item::Result)
            .chain(self.suites.iter().cloned().map(Item::TestSet))
            .chain(self.adoptions.iter().cloned().map(Item::Adoption))
            .collect();
        dated.sort_by(|a, b| b.at().cmp(&a.at()).then_with(|| a.id().cmp(&b.id())));
        dated
            .into_iter()
            .chain(self.releases.iter().cloned().map(Item::Build))
            .chain(self.notes.iter().cloned().map(Item::Note))
            .collect()
    }

    /// The tool with `id`.
    #[must_use]
    pub fn tool(&self, id: &str) -> Option<&Tool> {
        self.tools.iter().find(|tool| tool.id == id)
    }

    /// The newest published result (not a check) for `tool`.
    #[must_use]
    pub fn latest(&self, tool: &str) -> Option<&ResultRecord> {
        self.results
            .iter()
            .filter(|result| result.checks.is_none() && result.tool.as_deref() == Some(tool))
            .max_by_key(|result| result.at)
    }

    /// The newest published test set for `tool`: a verified release, else
    /// the test set its newest result ran.
    #[must_use]
    pub fn suite(&self, tool: &str) -> Option<SuiteRecord> {
        self.suites
            .iter()
            .filter(|suite| suite.tool.as_deref() == Some(tool))
            .max_by_key(|suite| suite.at)
            .cloned()
            .or_else(|| {
                self.latest(tool).map(|result| SuiteRecord {
                    release: result.suite.clone(),
                    tool: result.tool.clone(),
                    tool_name: result.tool_name.clone(),
                    author: result.suite.pubkey.clone(),
                    subject: result.subject.clone(),
                    cases: result.cases,
                    at: result.at,
                    source: result.publication.clone(),
                })
            })
    }

    /// The newest published result (not a check) with fewer than
    /// [`CHECKS_FOR_ADOPTION`] confirming checks, for `tool` when given.
    #[must_use]
    pub fn checkable(&self, tool: Option<&str>) -> Option<&ResultRecord> {
        self.results
            .iter()
            .filter(|result| {
                result.checks.is_none()
                    && result.checked.confirmed < CHECKS_FOR_ADOPTION
                    && tool.is_none_or(|tool| result.tool.as_deref() == Some(tool))
            })
            .max_by_key(|result| result.at)
    }
}

/// What the Gym seam found for one turn.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Grounding {
    /// Every verified record.
    pub records: Records,
    /// For `gym.news`: the items Jev judged relevant to the message, with
    /// their relevance, most relevant first; empty for other routes.
    pub news: Vec<(Item, f64)>,
}

/// What a Gym or eval turn shows.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Reply {
    /// A bank entry, whole, with the card and the offer beside it. The
    /// model call is dropped.
    Bank {
        answer: Entry,
        text: String,
        card: Option<Card>,
        offer: Option<Offer>,
    },
    /// The model answers from `items` only ([`instructions`]), with the
    /// news card beside it.
    Grounded { items: Vec<Item>, card: Card },
    /// No record and no bank entry fits: the model, told we have no
    /// verified records ([`NO_RECORDS_NOTE`]).
    Model,
}

/// The instruction a model gets on a Gym or eval turn it answers without
/// records, so it never states a result it was not given.
pub const NO_RECORDS_NOTE: &str = "This message is about the OpenAgents Gym or testing tools on \
Coder, and we have no verified Gym records in front of us for it. Do not state any result, \
score, count of tests or trainers, XP amount, date, or tool name as a fact. Say plainly that we \
don't have that record, and answer only what the conversation already establishes.";

/// The offer that runs `suite` against its subject: on the hosted runner
/// when it fits the runner's bounds, else on a connected computer. A check
/// is the same offer beside a check card, whose publication the client
/// cites.
fn start(suite: &SuiteRecord, label: &str) -> Offer {
    let runs = FULL_RUNS.min(HOSTED_MAX_RUNS);
    Offer::StartEval {
        suite: SuiteSource::Published(suite.release.clone()),
        subject: SubjectSource::Definition(Box::new(suite.subject.clone())),
        size: Size {
            cases: suite.cases,
            runs,
            arms: 2,
        },
        at: if suite.cases <= HOSTED_MAX_CASES {
            Where::Hosted
        } else {
            Where::ConnectedComputer
        },
        label: label.to_string(),
    }
}

/// A bank entry rendered with `facts`, or `None` when it is missing or a
/// slot is unfilled.
fn bank_reply(
    bank: &Bank,
    facts: &Facts,
    id: &str,
    card: Option<Card>,
    offer: Option<Offer>,
) -> Option<Reply> {
    let entry = bank.entry(id)?;
    Some(Reply::Bank {
        text: entry.render(facts)?,
        answer: entry.clone(),
        card,
        offer,
    })
}

/// The tool a turn is about: the `tool` reading when it names a tool in
/// the catalog, else, for `eval.run` only, the catalog's default (the
/// first, Project map).
fn tool_of<'a>(records: &'a Records, tool: Option<&str>, route: RouteId) -> Option<&'a Tool> {
    match tool.and_then(|id| records.tool(id)) {
        Some(tool) => Some(tool),
        None if route == RouteId::EvalRun => records.tools.first(),
        None => None,
    }
}

/// What a `gym.*` or `eval.*` turn shows, from the verified `grounding`.
/// `tool` is the `tool` reading the policy found sure. See the module
/// documentation; `eval.author` and `eval.credit` are decided by the
/// policy and never reach here.
#[must_use]
pub fn reply(
    route: RouteId,
    tool: Option<&str>,
    grounding: &Grounding,
    bank: &Bank,
    facts: &Facts,
) -> Reply {
    let records = &grounding.records;
    let named = |tool: &Tool| facts.clone().set("gym.tool", tool.name.clone());
    let decided = match route {
        RouteId::GymNews => {
            let items: Vec<Item> = grounding
                .news
                .iter()
                .take(MAX_NEWS)
                .map(|(item, _)| item.clone())
                .collect();
            if items.is_empty() {
                bank_reply(bank, facts, "gym.news_empty", None, None)
            } else {
                Some(Reply::Grounded {
                    card: Card::News {
                        items: items.clone(),
                    },
                    items,
                })
            }
        }
        RouteId::EvalRun => tool_of(records, tool, route).and_then(|chosen| {
            let latest = records.latest(&chosen.id).cloned();
            let suite = records.suite(&chosen.id);
            let card = Card::Tool {
                tool: chosen.clone(),
                latest,
                subject: suite.as_ref().map(|suite| suite.subject.clone()),
            };
            match &suite {
                Some(suite) => bank_reply(
                    bank,
                    &named(chosen).set("gym.tests", suite.cases.to_string()),
                    "eval.run.offer",
                    Some(card),
                    Some(start(suite, "Start the test")),
                ),
                None => bank_reply(bank, &named(chosen), "eval.run.no_tests", Some(card), None),
            }
        }),
        RouteId::EvalCheck => {
            let wanted = tool.filter(|id| records.tool(id).is_some());
            match records.checkable(wanted) {
                Some(result) => {
                    let suite = SuiteRecord {
                        release: result.suite.clone(),
                        tool: result.tool.clone(),
                        tool_name: result.tool_name.clone(),
                        author: result.suite.pubkey.clone(),
                        subject: result.subject.clone(),
                        cases: result.cases,
                        at: result.at,
                        source: result.publication.clone(),
                    };
                    bank_reply(
                        bank,
                        facts,
                        "eval.check.one",
                        Some(Card::Check {
                            result: result.clone(),
                        }),
                        Some(start(&suite, "Run the check")),
                    )
                }
                None => bank_reply(bank, facts, "eval.check.none", None, None),
            }
        }
        RouteId::EvalResult => match tool_of(records, tool, route)
            .and_then(|chosen| Some((chosen, records.latest(&chosen.id)?)))
        {
            Some((chosen, result)) => bank_reply(
                bank,
                &named(chosen),
                "eval.result.tool",
                Some(Card::Result {
                    result: result.clone(),
                }),
                // A published result is on the Gym's EVALS board too.
                Some(Offer::OpenScreen {
                    screen: Screen::VerseGym,
                    label: "See the board".to_string(),
                }),
            ),
            // The person's own results stay on their phone until they add
            // one to the Gym; the phone opens its latest.
            None => bank_reply(
                bank,
                facts,
                "eval.result.mine",
                None,
                Some(Offer::OpenScreen {
                    screen: Screen::GymResult,
                    label: "See your result".to_string(),
                }),
            ),
        },
        _ => None,
    };
    decided.unwrap_or(Reply::Model)
}

/// The grounded model's instructions for `items`: answer only from them,
/// in the plural voice and the phone's plain words, citing each item used
/// by its id. The citations are for [`check_reply`]; [`Tidy`] takes them
/// out before the phone sees the reply, whose news card already names
/// each item's source.
#[must_use]
pub fn instructions(items: &[Item]) -> String {
    let mut out = String::from(
        "We are OpenAgents, answering in the OpenAgents app's chat about what's new in the Gym, \
where people test tools on Coder. Always speak as \"we\", never \"I\". Answer the user's latest \
message using only the Gym records below, which we verified. After each sentence that uses a \
record, cite it by its id in square brackets, such as [gym:build:build-20]; we take the brackets \
out before the reply is shown, so never write an id, a file path, an event, or a \"source\" line \
anywhere else. Every result, count, verdict, tool name, and build you mention must come from a \
record below; state no other result, score, count, XP, or date. Use the app's plain words: a \
tool, a test, a test set, with and without the tool, Better, No clear change, Worse, results, \
attempts. Never use these words, even when a record does: benchmark, Terminal-Bench, trace, \
eval, evaluation, suite, case, grader, rubric, judge, baseline, arm, harness, extension, plugin, \
Wasm, relay, Nostr, key, host, workspace. If the records don't answer what the user asked, say \
we don't have a record of that yet. Keep it short for a phone screen.\n",
    );
    for item in items {
        out.push_str(&format!(
            "\n<record id=\"{}\" source=\"{}\">\n{}\n</record>\n",
            item.id(),
            item.source().cite().replace('"', "'"),
            item.text()
        ));
    }
    out
}

/// The longest interview step's text, in characters.
pub const MAX_STEP_CHARS: usize = 1_200;

/// Why an interview step was not shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BadStep {
    Empty,
    TooLong,
    FirstPersonSingular,
}

/// An interview step as it may be shown: its text is non-empty, at most
/// [`MAX_STEP_CHARS`], and plural; a draft that fails
/// [`super::card::draft`] is dropped, and so is any offer but `start_eval`
/// on the draft, `publish_eval`, `run_coder` (the handoff when a tool needs
/// new code), or opening the test set or Add to the Gym.
/// The seam proposes; this decides what reaches the phone.
///
/// # Errors
///
/// Why the text may not be shown; the router then answers from the bank.
pub fn check_step(step: &super::seams::AuthorStep) -> Result<super::seams::AuthorStep, BadStep> {
    let text = step.text.trim();
    if text.is_empty() {
        return Err(BadStep::Empty);
    }
    if text.chars().count() > MAX_STEP_CHARS {
        return Err(BadStep::TooLong);
    }
    let singular = text
        .to_lowercase()
        .replace('\u{2019}', "'")
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .any(|word| {
            matches!(
                word,
                "i" | "i'll" | "i'm" | "i've" | "i'd" | "me" | "my" | "mine" | "myself"
            )
        });
    if singular {
        return Err(BadStep::FirstPersonSingular);
    }
    let offer = step.offer.clone().filter(|offer| match offer {
        Offer::StartEval { suite, .. } => *suite == SuiteSource::Draft,
        Offer::PublishEval { .. } => true,
        Offer::OpenScreen { screen, .. } => {
            matches!(screen, Screen::GymTestSet | Screen::GymPublish)
        }
        // A tool that needs new code is made with Coder on a connected
        // computer: the interview hands off with Run Coder (the phone shows
        // Connect a computer when none is ready).
        Offer::RunCoder { .. } => true,
        Offer::Cli { .. } => false,
    });
    Ok(super::seams::AuthorStep {
        text: text.to_string(),
        draft: step
            .draft
            .as_ref()
            .and_then(|draft| super::card::draft(draft).ok()),
        offer,
        model: step.model.clone(),
    })
}

/// Which records a grounded reply cited.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cited {
    /// Cited ids that name an item the reply was given, in order, once
    /// each.
    pub known: Vec<String>,
    /// Cited `gym:` ids that name no item it was given: invented.
    pub invented: Vec<String>,
}

/// The `[gym:…]` citations in `reply`, sorted into the ones that name one
/// of `items` and the invented ones. This reads the model's own output
/// for a bounded shape after the route was chosen; it routes nothing.
#[must_use]
pub fn check_reply(reply: &str, items: &[Item]) -> Cited {
    let given: Vec<String> = items.iter().map(Item::id).collect();
    let mut cited = Cited::default();
    for piece in reply.split('[').skip(1) {
        let Some((inside, _)) = piece.split_once(']') else {
            continue;
        };
        for id in inside.split([',', ';']).map(str::trim) {
            if !id.starts_with(CITE_PREFIX)
                || id.len() > 96
                || !id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '-' | '_' | '.'))
            {
                continue;
            }
            let list = if given.iter().any(|known| known == id) {
                &mut cited.known
            } else {
                &mut cited.invented
            };
            if !list.iter().any(|seen| seen == id) {
                list.push(id.to_string());
            }
        }
    }
    cited
}

/// Words the app never shows on a card, a label, or a Gym reply: the
/// wireframe's banned list (`docs/product/2026-09-28-app-wireframe.md`,
/// Words on screen; `CHK-02`), lowercased. The phone keeps the same list
/// (`crates/openagents-mobile/src/eval_cards.rs`).
pub const BANNED: &[&str] = &[
    "npub",
    "nsec",
    "key",
    "relay",
    "nostr",
    "nip",
    "atif",
    "tailnet",
    "tailscale",
    "wasm",
    "plugin",
    "extension",
    "benchmark",
    "terminal-bench",
    "tb",
    "eval",
    "evaluation",
    "suite",
    "case",
    "grader",
    "rubric",
    "judge",
    "baseline",
    "arm",
    "harness",
    "stand-in",
    "mock",
    "pilot",
    "jev",
    "luna",
    "microcoder",
    "verifier",
    "trace",
    "recipe",
    "grant",
    "sats",
    "btc",
    "₿",
    "lightning",
    "invoice",
    "host",
    "workspace",
    "pubkey",
    "hex",
];

/// The banned words in `text`, each once, in order: whole words or their
/// plurals ("traces"), in any case. This checks words we show, after the
/// route was chosen; it routes nothing.
#[must_use]
pub fn jargon_all(text: &str) -> Vec<&'static str> {
    let lower = text.to_lowercase();
    let mut found: Vec<&'static str> = Vec::new();
    for word in lower
        .split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '₿'))
        .filter(|word| !word.is_empty())
    {
        let hit = BANNED.iter().copied().find(|banned| {
            word == *banned
                || word
                    .strip_suffix('s')
                    .is_some_and(|stem| stem == *banned || stem.strip_suffix('e') == Some(banned))
        });
        if let Some(hit) = hit
            && !found.contains(&hit)
        {
            found.push(hit);
        }
    }
    found
}

/// The first banned word in `text` ([`jargon_all`]).
#[must_use]
pub fn jargon(text: &str) -> Option<&'static str> {
    jargon_all(text).first().copied()
}

/// The raw identifiers in `text` a person should never see: a `gym:`
/// citation id, a note id (`openagents.…`), a repository path, a digest,
/// a key, or a run of eight or more hex digits (an event id or its
/// start). Bounded shapes of our own output, read after routing.
#[must_use]
pub fn raw_ids(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for token in text.split(|c: char| {
        c.is_whitespace() || matches!(c, '[' | ']' | '(' | ')' | ',' | ';' | '"' | '`' | '*')
    }) {
        let token = token.trim_matches(|c: char| matches!(c, '.' | ':' | '!' | '?' | '\''));
        if token.is_empty() {
            continue;
        }
        let lower = token.to_lowercase();
        let hex_run = lower.split(|c: char| !c.is_ascii_hexdigit()).any(|run| {
            run.len() >= 8
                && run.chars().any(|c| c.is_ascii_digit())
                && run.chars().any(|c| c.is_ascii_alphabetic())
        });
        let raw = lower.starts_with(CITE_PREFIX)
            || lower.contains(":gym:")
            || lower.starts_with("openagents.")
            || lower.starts_with("knowledge/")
            || lower.starts_with("crates/")
            || lower.starts_with("sha256:")
            || lower.starts_with("npub1")
            || lower.starts_with("nsec1")
            || lower.starts_with("event ")
            || hex_run;
        if raw && !found.iter().any(|seen: &String| seen == token) {
            found.push(token.to_string());
        }
    }
    found
}

/// What a Gym reply must not show, once tidied: banned words and raw ids.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shown {
    pub banned: Vec<&'static str>,
    pub raw: Vec<String>,
}

impl Shown {
    /// Whether the reply is fit to show.
    #[must_use]
    pub fn clean(&self) -> bool {
        self.banned.is_empty() && self.raw.is_empty()
    }
}

/// Checks a reply as the phone will draw it: every banned word and raw id
/// in it.
#[must_use]
pub fn post_check(text: &str) -> Shown {
    Shown {
        banned: jargon_all(text),
        raw: raw_ids(text),
    }
}

/// The longest bracket [`Tidy`] holds back before it shows it as text.
const HOLD: usize = 200;

/// Takes a grounded Gym reply's `[gym:…]` citations out as it streams, so
/// the phone never shows an id; the news card names each item's source.
/// A bracket that is not only citations is shown as written. A space
/// before a citation is dropped when punctuation follows it
/// ("credit [gym:note:gym-news]." reads "credit.").
#[derive(Clone, Debug, Default)]
pub struct Tidy {
    /// Whitespace not yet shown.
    space: String,
    /// An open bracket and what followed it.
    bracket: String,
    /// A citation was just taken out.
    after: bool,
}

/// Whether `inside` (a bracket's text) is only citation ids.
fn citations_only(inside: &str) -> bool {
    let ids: Vec<&str> = inside.split([',', ';']).map(str::trim).collect();
    !ids.is_empty()
        && ids.iter().all(|id| {
            id.starts_with(CITE_PREFIX)
                && id.len() <= 96
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '-' | '_' | '.' | '@'))
        })
}

/// Whether `open` (a bracket so far) could still become a citation.
fn could_cite(open: &str) -> bool {
    let inside = &open[1..];
    let last = inside.rsplit([',', ';']).next().unwrap_or("").trim_start();
    let prefix_ok = |id: &str| {
        if id.len() < CITE_PREFIX.len() {
            CITE_PREFIX.starts_with(id)
        } else {
            id.starts_with(CITE_PREFIX)
        }
    };
    open.len() <= HOLD
        && !inside.contains('\n')
        && inside
            .split([',', ';'])
            .map(str::trim)
            .all(|id| id.is_empty() || prefix_ok(id))
        && prefix_ok(last)
}

impl Tidy {
    /// The text of `delta` that may be shown now.
    pub fn push(&mut self, delta: &str) -> String {
        let mut out = String::new();
        for c in delta.chars() {
            if !self.bracket.is_empty() {
                self.bracket.push(c);
                let close = if self.bracket.starts_with('[') {
                    ']'
                } else {
                    ')'
                };
                if c == close {
                    let inside = &self.bracket[1..self.bracket.len() - 1];
                    if citations_only(inside) {
                        self.after = true;
                    } else {
                        out.push_str(&self.space);
                        out.push_str(&self.bracket);
                        self.space.clear();
                        self.after = false;
                    }
                    self.bracket.clear();
                } else if !could_cite(&self.bracket) {
                    out.push_str(&self.space);
                    out.push_str(&self.bracket);
                    self.space.clear();
                    self.bracket.clear();
                    self.after = false;
                }
                continue;
            }
            if c == '[' || c == '(' {
                self.bracket.push(c);
                continue;
            }
            if c.is_whitespace() {
                self.space.push(c);
                continue;
            }
            if self.after {
                // "credit [gym:…]." reads "credit.", and a citation that
                // ended a line leaves no space before the break.
                if let Some(at) = self.space.find('\n') {
                    self.space.drain(..at);
                } else if matches!(c, '.' | ',' | ';' | ':' | '!' | '?') {
                    self.space.clear();
                }
            }
            out.push_str(&self.space);
            self.space.clear();
            self.after = false;
            out.push(c);
        }
        out
    }

    /// What is left once the reply ends.
    pub fn finish(&mut self) -> String {
        let mut out = String::new();
        if !self.bracket.is_empty() {
            out.push_str(&self.space);
            out.push_str(&self.bracket);
        } else if !self.after {
            out.push_str(&self.space);
        } else if let Some(at) = self.space.find('\n') {
            out.push_str(&self.space[at..]);
        }
        *self = Tidy::default();
        out
    }
}

/// `text` with its citations taken out, as [`Tidy`] streams it.
#[must_use]
pub fn tidy(text: &str) -> String {
    let mut tidy = Tidy::default();
    let mut out = tidy.push(text);
    out.push_str(&tidy.finish());
    out
}

#[cfg(test)]
pub(crate) mod fixtures {
    //! Records for tests: every number is distinct, so a test can tell
    //! which field a card copied.
    use super::*;

    pub fn event(n: u8, kind: u16) -> EventPointer {
        EventPointer {
            id: format!("{n:02x}").repeat(32),
            pubkey: format!("{:02x}", n.wrapping_add(100)).repeat(32),
            kind,
        }
    }

    pub fn artifact(n: u8, schema: Option<&str>) -> ArtifactRef {
        ArtifactRef {
            digest: format!("sha256:{}", format!("{n:02x}").repeat(32)),
            size: 4096,
            media_type: "application/json".into(),
            schema: schema.map(str::to_string),
            event: None,
            sources: Vec::new(),
        }
    }

    pub fn definition(n: u8, slug: &str) -> DefinitionRef {
        DefinitionRef {
            id: format!("{}:{slug}/{slug}", format!("{n:02x}").repeat(32)),
            artifact: artifact(n, None),
            event: None,
        }
    }

    pub fn tool(id: &str, name: &str) -> Tool {
        Tool {
            id: format!("openagents.tool-{id}"),
            name: name.to_string(),
            line: format!("What {name} does."),
            source: format!("knowledge/openagents/openagents.tool-{id}.md"),
            slugs: vec![id.to_string()],
        }
    }

    pub fn result(n: u8, tool: &str, confirmed: u64) -> ResultRecord {
        ResultRecord {
            publication: event(n, 3189),
            tool: Some(format!("openagents.tool-{tool}")),
            tool_name: tool.replace('-', " "),
            trainer: format!("{:02x}", n.wrapping_add(50)).repeat(32),
            suite: event(n.wrapping_add(1), 3184),
            cases: 8,
            subject: definition(n, tool),
            headline: Headline {
                subject_passed: 7,
                baseline_passed: Some(5),
                total: 8,
            },
            verdict: Verdict::Pass,
            report: artifact(n, Some(nostr::kb::REPORT_SCHEMA)),
            checks: None,
            checked: Checks {
                confirmed,
                disputed: 1,
            },
            at: 1_790_000_000 + u64::from(n),
        }
    }

    pub fn suite(n: u8, tool: &str, cases: u64) -> SuiteRecord {
        SuiteRecord {
            release: event(n, 3184),
            tool: Some(format!("openagents.tool-{tool}")),
            tool_name: tool.replace('-', " "),
            author: format!("{:02x}", n.wrapping_add(60)).repeat(32),
            subject: definition(n, tool),
            cases,
            at: 1_790_000_000 + u64::from(n),
            source: event(n, 3184),
        }
    }

    pub fn records() -> Records {
        Records {
            tools: vec![
                tool("project-map", "Project map"),
                tool("code-finder", "Code finder"),
            ],
            results: vec![result(10, "project-map", 1)],
            suites: vec![suite(20, "project-map", 8)],
            adoptions: Vec::new(),
            releases: vec![Release {
                version: "1.0.0".into(),
                build: "20".into(),
                title: "Smarter chat and a simpler Wallet".into(),
                items: vec!["Instant answers".into()],
                source: "crates/openagents-mobile/src/account.rs".into(),
            }],
            notes: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;
    use serde_json::json;

    fn facts() -> Facts {
        crate::router::worker_facts(
            crate::generate::Lane::Gemini.model(),
            Some(crate::generate::DEFAULT_DOOR_URL),
            Some((6, 40)),
            &crate::router::Seams::default(),
        )
    }

    fn grounded(records: Records) -> Grounding {
        Grounding {
            records,
            news: Vec::new(),
        }
    }

    /// `eval.run` with a published test set: the bank's sentence names the
    /// tool and its tests from the records, the tool card carries the
    /// latest result, and the offer starts that test set, hosted.
    #[test]
    fn a_tool_with_a_test_set_is_offered_to_run() {
        let reply = reply(
            RouteId::EvalRun,
            Some("openagents.tool-project-map"),
            &grounded(records()),
            Bank::builtin(),
            &facts(),
        );
        let Reply::Bank {
            answer,
            text,
            card: Some(Card::Tool { tool, latest, .. }),
            offer: Some(Offer::StartEval {
                suite, size, at, ..
            }),
        } = &reply
        else {
            panic!("{reply:?}");
        };
        assert_eq!(answer.id, "eval.run.offer");
        assert!(text.contains("Project map") && text.contains('8'), "{text}");
        assert_eq!(tool.name, "Project map");
        assert_eq!(
            latest.as_ref().map(|r| r.publication.clone()),
            Some(event(10, 3189))
        );
        assert_eq!(suite, &SuiteSource::Published(event(20, 3184)));
        assert_eq!(
            *size,
            Size {
                cases: 8,
                runs: FULL_RUNS,
                arms: 2
            }
        );
        assert_eq!(*at, Where::Hosted);
    }

    /// With no suite release admitted, the test set a verified result ran
    /// is still offered; a larger one runs on a connected computer.
    #[test]
    fn a_results_test_set_is_runnable_and_size_picks_the_place() {
        let mut records = records();
        records.suites.clear();
        let suite = records.suite("openagents.tool-project-map").unwrap();
        assert_eq!(suite.release, event(11, 3184));
        assert_eq!(suite.source, event(10, 3189));
        records.results[0].cases = 12;
        let reply = reply(
            RouteId::EvalRun,
            Some("openagents.tool-project-map"),
            &grounded(records),
            Bank::builtin(),
            &facts(),
        );
        assert!(matches!(
            reply,
            Reply::Bank {
                offer: Some(Offer::StartEval {
                    at: Where::ConnectedComputer,
                    ..
                }),
                ..
            }
        ));
    }

    /// No tool named: the default tool. No test set: no offer, and the
    /// bank says so.
    #[test]
    fn a_tool_without_a_test_set_offers_nothing_to_start() {
        let reply = reply(
            RouteId::EvalRun,
            Some("openagents.tool-code-finder"),
            &grounded(records()),
            Bank::builtin(),
            &facts(),
        );
        let Reply::Bank {
            answer,
            text,
            offer,
            ..
        } = &reply
        else {
            panic!("{reply:?}");
        };
        assert_eq!(answer.id, "eval.run.no_tests");
        assert!(text.contains("Code finder"), "{text}");
        assert_eq!(offer, &None);
        let default = super::reply(
            RouteId::EvalRun,
            None,
            &grounded(records()),
            Bank::builtin(),
            &facts(),
        );
        assert!(
            matches!(&default, Reply::Bank { card: Some(Card::Tool { tool, .. }), .. } if tool.name == "Project map")
        );
        assert_eq!(
            super::reply(
                RouteId::EvalRun,
                None,
                &Grounding::default(),
                Bank::builtin(),
                &facts()
            ),
            Reply::Model
        );
    }

    /// `eval.check`: the newest result with fewer than three confirming
    /// checks, as a check card and the offer that reruns its test set;
    /// none: the bank says so.
    #[test]
    fn a_result_waiting_for_checks_is_offered_as_a_check() {
        let reply = reply(
            RouteId::EvalCheck,
            None,
            &grounded(records()),
            Bank::builtin(),
            &facts(),
        );
        let Reply::Bank {
            card: Some(Card::Check { result }),
            offer: Some(Offer::StartEval { suite, size, .. }),
            ..
        } = &reply
        else {
            panic!("{reply:?}");
        };
        assert_eq!(result.publication, event(10, 3189));
        assert_eq!(suite, &SuiteSource::Published(result.suite.clone()));
        assert_eq!(size.cases, 8);

        let mut done = records();
        done.results[0].checked.confirmed = CHECKS_FOR_ADOPTION;
        let reply = super::reply(
            RouteId::EvalCheck,
            None,
            &grounded(done),
            Bank::builtin(),
            &facts(),
        );
        assert!(
            matches!(&reply, Reply::Bank { answer, card: None, offer: None, .. } if answer.id == "eval.check.none")
        );
    }

    /// `eval.result`: a named tool's published result as a card; otherwise
    /// the person's own result, which only their phone holds.
    #[test]
    fn results_are_published_records_or_the_phones_own() {
        let reply = reply(
            RouteId::EvalResult,
            Some("openagents.tool-project-map"),
            &grounded(records()),
            Bank::builtin(),
            &facts(),
        );
        assert!(
            matches!(&reply, Reply::Bank { answer, card: Some(Card::Result { result }), .. }
                if answer.id == "eval.result.tool" && result.publication == event(10, 3189))
        );
        let mine = super::reply(
            RouteId::EvalResult,
            None,
            &grounded(records()),
            Bank::builtin(),
            &facts(),
        );
        assert!(matches!(&mine, Reply::Bank {
            answer,
            card: None,
            offer: Some(Offer::OpenScreen { screen: Screen::GymResult, .. }),
            ..
        } if answer.id == "eval.result.mine"));
    }

    /// `gym.news`: at most five items and the news card built from them;
    /// nothing relevant: the bank.
    #[test]
    fn news_is_the_kept_items() {
        let records = records();
        let mut items: Vec<(Item, f64)> = records
            .items()
            .into_iter()
            .map(|item| (item, 0.9))
            .collect();
        for n in 0..6 {
            items.push((Item::Result(result(40 + n, "code-finder", 0)), 0.8));
        }
        let reply = reply(
            RouteId::GymNews,
            None,
            &Grounding {
                records,
                news: items,
            },
            Bank::builtin(),
            &facts(),
        );
        let Reply::Grounded { items, card } = &reply else {
            panic!("{reply:?}");
        };
        assert_eq!(items.len(), MAX_NEWS);
        assert_eq!(
            card,
            &Card::News {
                items: items.clone()
            }
        );
        let empty = super::reply(
            RouteId::GymNews,
            None,
            &grounded(Records::default()),
            Bank::builtin(),
            &facts(),
        );
        assert!(
            matches!(&empty, Reply::Bank { answer, card: None, offer: None, .. } if answer.id == "gym.news_empty")
        );
    }

    /// A grounded reply may cite only the items it was given; any other
    /// `gym:` id is reported as invented.
    #[test]
    fn a_grounded_reply_may_cite_only_its_items() {
        let items = records().items();
        let ids: Vec<String> = items.iter().map(Item::id).collect();
        assert!(ids.contains(&"gym:build:build-20".to_string()), "{ids:?}");
        let prompt = instructions(&items);
        for id in &ids {
            assert!(prompt.contains(&format!("id=\"{id}\"")), "{id}");
        }
        let reply = format!(
            "Build 20 made chat smarter [{}]. Project map did better [{}, gym:result:ffffffff]. \
             See [gym:note:made-up]; not a citation [1].",
            ids[ids.len() - 1],
            ids[0]
        );
        let cited = check_reply(&reply, &items);
        assert_eq!(
            cited.known,
            vec![ids[ids.len() - 1].clone(), ids[0].clone()]
        );
        assert_eq!(
            cited.invented,
            vec!["gym:result:ffffffff", "gym:note:made-up"]
        );
    }

    /// The reply the phone showed in #9944, and replies shaped like the
    /// model's: tidied, they carry no citation id, and the check finds
    /// nothing; streamed in any split, the tidy is the same.
    #[test]
    fn a_news_reply_shows_no_ids_and_no_banned_words() {
        let items = records().items();
        let ids: Vec<String> = items.iter().map(Item::id).collect();
        let replies = [
            (
                "In the Gym you can test tools on Coder, check results, and see your credit \
                 [gym:note:gym-news]."
                    .to_string(),
                "In the Gym you can test tools on Coder, check results, and see your credit.",
            ),
            (
                format!(
                    "Build 20 made chat smarter [{}]. Project map did Better, 7 of 8 tests \
                     [{}, {}].\n\nWant to try it?",
                    ids[ids.len() - 1],
                    ids[0],
                    ids[1]
                ),
                "Build 20 made chat smarter. Project map did Better, 7 of 8 tests.\n\nWant to \
                 try it?",
            ),
            (
                format!("A new test set for Project map [{}]", ids[1]),
                "A new test set for Project map",
            ),
            (
                "We measure with and without the tool (see the card) [1] and [a link](x).".into(),
                "We measure with and without the tool (see the card) [1] and [a link](x).",
            ),
            (
                "Two items:\n- Better [gym:result:0a0a0a0a]\n- Build 20 [gym:build:build-20]\n"
                    .into(),
                "Two items:\n- Better\n- Build 20\n",
            ),
        ];
        for (reply, want) in &replies {
            let tidied = tidy(reply);
            assert_eq!(&tidied, want);
            assert!(
                post_check(&tidied).clean(),
                "{tidied}: {:?}",
                post_check(&tidied)
            );
            // The model's own citations are still read before tidying.
            assert!(check_reply(reply, &items).invented.len() <= 1);
            for size in 1..=7 {
                let mut stream = Tidy::default();
                let chars: Vec<char> = reply.chars().collect();
                let mut out = String::new();
                for chunk in chars.chunks(size) {
                    out.push_str(&stream.push(&chunk.iter().collect::<String>()));
                }
                out.push_str(&stream.finish());
                assert_eq!(&out, want, "split every {size}");
            }
        }
        // Untidied, the check sees the ids; banned words are whole words
        // or their plurals.
        let raw = post_check(&replies[0].0);
        assert_eq!(raw.raw, vec!["gym:note:gym-news".to_string()]);
        let worded = post_check("It shows Terminal-Bench results and traces for each case.");
        assert_eq!(worded.banned, vec!["terminal-bench", "trace", "case"]);
        assert!(post_check("Your keyboard and monkey business").clean());
        assert_eq!(
            raw_ids("event 1a2b3c4d5e from openagents.gym-news in knowledge/openagents/x.md"),
            vec![
                "1a2b3c4d5e",
                "openagents.gym-news",
                "knowledge/openagents/x.md"
            ]
        );
        assert!(raw_ids("Build 20, 7 of 8 tests, version 1.0.0").is_empty());
    }

    /// Every news item a card can carry, from the real catalog, Gym notes,
    /// and changelog, and from every kind of published record, reads in
    /// the phone's words: no banned word and no raw id in its title or
    /// line (CHK-02).
    #[test]
    fn every_news_items_words_are_the_phones() {
        let root = knowledge::product::repository();
        let corpus =
            knowledge::product::Corpus::load(&knowledge::product::default_dir(), Some(&root))
                .expect("the corpus loads");
        let mut records = records();
        records.tools = crate::gym_kb::tools(&corpus);
        records.notes = crate::gym_kb::notes(&corpus);
        records.releases = crate::gym_kb::changelog()
            .into_iter()
            .take(crate::gym_kb::BUILDS)
            .collect();
        let mut check = result(12, "project-map", 0);
        check.checks = Some(event(10, 3189).id);
        records.results.push(check);
        records.adoptions.push(AdoptionRecord {
            release: event(30, 3184),
            tool: Some("openagents.tool-project-map".into()),
            tool_name: "Project map".into(),
            at: 1_790_000_030,
        });
        let items = records.items();
        assert!(items.iter().any(|item| item.kind() == "note"));
        assert!(items.iter().any(|item| item.kind() == "build"));
        for item in &items {
            let (title, line) = item.title_and_line();
            let shown = post_check(&format!("{title}\n{line}"));
            assert!(shown.clean(), "{}: {title} / {line}: {shown:?}", item.id());
        }
    }

    /// Every number an item's line carries is its record's.
    #[test]
    fn an_items_numbers_are_its_records() {
        let mut result = result(7, "project-map", 2);
        result.headline = Headline {
            subject_passed: 6,
            baseline_passed: Some(4),
            total: 9,
        };
        let (title, line) = Item::Result(result.clone()).title_and_line();
        assert_eq!(title, "project map: Better");
        assert_eq!(
            line,
            "Without the tool Coder passed 4 of 9 tests; with it, 6 of 9. Confirmed by 2 checks, \
             disputed by 1."
        );
        result.headline.baseline_passed = None;
        let (_, line) = Item::Result(result).title_and_line();
        assert!(line.starts_with("With the tool Coder passed 6 of 9 tests; there was no run"));
    }

    /// An interview step is shown only in the plural and bounded; a bad
    /// draft or an offer the interview may not make is dropped.
    #[test]
    fn an_interview_step_is_checked_before_it_is_shown() {
        use crate::router::seams::AuthorStep;
        let draft: serde_json::Value = serde_json::from_str(include_str!(
            "../../../nostr/fixtures/eval-ext/eval-draft/valid/chat-made-tool.json"
        ))
        .unwrap();
        let step = AuthorStep {
            text: "What should a good run of your tool look like?".into(),
            draft: Some(draft.clone()),
            offer: Some(Offer::StartEval {
                suite: SuiteSource::Draft,
                subject: SubjectSource::Draft,
                size: Size {
                    cases: 5,
                    runs: 1,
                    arms: 2,
                },
                at: Where::Hosted,
                label: "Try it once".into(),
            }),
            model: "test".into(),
        };
        let checked = check_step(&step).unwrap();
        assert_eq!(checked.draft, Some(draft));
        assert!(checked.offer.is_some());
        let sneaky = AuthorStep {
            draft: Some(json!({ "v": "other" })),
            offer: Some(Offer::Cli {
                argv: vec!["computer".into(), "list".into()],
                effect: crate::router::Effect::ReadOnly,
                runs_on: crate::router::RunsOn::ThisDevice,
            }),
            ..step.clone()
        };
        let checked = check_step(&sneaky).unwrap();
        assert_eq!((checked.draft, checked.offer), (None, None));
        let handoff = AuthorStep {
            offer: Some(Offer::RunCoder {
                label: "Run Coder".into(),
            }),
            ..step.clone()
        };
        assert!(matches!(
            check_step(&handoff).unwrap().offer,
            Some(Offer::RunCoder { .. })
        ));
        for (text, why) in [
            ("  ", BadStep::Empty),
            ("I'll write the tests.", BadStep::FirstPersonSingular),
            (&"x".repeat(MAX_STEP_CHARS + 1), BadStep::TooLong),
        ] {
            let bad = AuthorStep {
                text: text.to_string(),
                ..step.clone()
            };
            assert_eq!(check_step(&bad), Err(why), "{text}");
        }
    }
}
