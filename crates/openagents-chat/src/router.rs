//! The phone's side of the chat router
//! (`docs/coder/design/2026-09-28-chat-router.md`, On the wire).
//!
//! A chat turn asks the worker for routing (`router`) and says what the
//! phone can do next (`context`): which surface it is, whether a computer
//! is ready, and the app's build. The context carries no credential, key,
//! host name, workspace, or amount.
//!
//! The worker answers with typed observations beside its text: the
//! judgment (which prepared answer, route, and tier), and offers (`offer`
//! feedback). An offer is never permission. The phone reads each one
//! against its own closed tables ([`Screen`], [`READ_ONLY`]) and shows it
//! as a control; nothing happens until the person taps it, and what the tap
//! does is decided here, never by the offer's own words. A label the worker
//! sends is ignored: the phone names every control itself.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The routing question set a turn asks for: `chat-router-v2`, with the
/// Gym and eval routes, their cards, and the eval offers.
pub const ROUTER: &str = "chat-router-v2";
/// The most cards one reply keeps.
const MAX_CARDS: usize = 4;
/// The most offers one reply keeps.
const MAX_OFFERS: usize = 4;
/// The most follow-up suggestions one reply keeps.
pub const MAX_FOLLOWUPS: usize = 3;
/// The longest follow-up suggestion, in characters.
const MAX_FOLLOWUP_CHARS: usize = 80;
/// The most words one proposed command has, and the longest word.
const MAX_ARGV: usize = 8;
const MAX_ARG_BYTES: usize = 200;
/// The most bytes of the worker's judgment the phone keeps, for a tester
/// who shares the chat.
const MAX_JUDGMENT_BYTES: usize = playtest::report::MAX_JUDGMENT_BYTES;

/// The native surface that sends this hosted conversation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Surface {
    #[default]
    Phone,
    Desktop,
}
impl Surface {
    pub fn word(self) -> &'static str {
        match self {
            Self::Phone => "phone",
            Self::Desktop => "desktop",
        }
    }
}

/// What a turn tells the worker about the phone.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub surface: Surface,
    /// A computer this device may operate is ready, so dispatching Coder
    /// is one tap.
    pub computer_ready: bool,
    /// The app's version and build, as `1.0.0 (19)`.
    pub app_build: Option<String>,
    /// The conversation's open test-set draft (`openagents.eval-draft.v1`),
    /// which the phone keeps and resends each turn: the request's `draft`,
    /// beside `context`, never inside it. Data, never an instruction.
    pub draft: Option<Value>,
    /// The result of a try or a full run of that draft, as the request's
    /// `tried` (`{runs, with, without, total, verdict, report, cases}`).
    pub tried: Option<Value>,
    /// Results a check must not be offered, as the request's `skip`: the
    /// trainer's own and the ones it already checked (public `3189` IDs).
    pub skip: Vec<String>,
}

impl Context {
    /// The request's `context` object: bounded, and without a key, host
    /// name, workspace, or amount.
    pub fn json(&self) -> Value {
        let mut context = json!({
            "surface": self.surface.word(),
            "computer_ready": self.computer_ready,
        });
        if let Some(build) = self.app_build.as_deref().filter(|build| build_like(build)) {
            context["app_build"] = json!(build);
        }
        context
    }
}

/// `1.0.0 (19)`: digits, dots, a space, and parentheses only.
fn build_like(text: &str) -> bool {
    (1..=24).contains(&text.len())
        && text
            .chars()
            .all(|ch| ch.is_ascii_digit() || " .()".contains(ch))
}

/// A screen an offer may open: the phone's own table, the same words as
/// the worker's `coder::router::Screen`. An offer naming any other screen is
/// set aside.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Screen {
    /// The Wallet tab.
    Wallet,
    /// Account > Computers.
    Computers,
    /// Account > Identity keys. The chat never shows a key itself.
    Keys,
    /// Account > Playtest.
    Playtest,
    /// Report a problem.
    Report,
    /// The Gym in the Verse, at its EVALS board: **See the board**.
    VerseGym,
    /// The person's own latest result (`SCR-05`), which only the phone
    /// holds: **See your result**.
    GymResult,
    /// Add to the Gym (`SCR-20`) for the person's latest result.
    GymPublish,
    /// The test set of the conversation's card or draft (`SCR-21`).
    GymTestSet,
}

impl Screen {
    /// The screen an offer's exact `screen` value names.
    fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "wallet" => Screen::Wallet,
            "account.computers" => Screen::Computers,
            "account.keys" => Screen::Keys,
            "account.playtest" => Screen::Playtest,
            "account.report_problem" => Screen::Report,
            "verse.gym" => Screen::VerseGym,
            "gym.result" => Screen::GymResult,
            "gym.publish" => Screen::GymPublish,
            "gym.test_set" => Screen::GymTestSet,
            _ => return None,
        })
    }
}

/// Where a proposed command runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunsOn {
    /// The phone's own Rust core answers it.
    ThisDevice,
    /// The ready computer runs it.
    ConnectedComputer,
}

/// The `openagents` commands the phone chat may propose: read-only ones
/// only (the owner's decision for the phone), as `(group, subcommands)`.
/// Anything else, whatever the offer's `effect` says, is set aside.
pub const READ_ONLY: &[(&str, &[&str])] = &[
    ("computer", &["list", "show", "workspaces"]),
    ("verse", &["who", "quests", "board", "xp"]),
    ("kb", &["search"]),
    ("cap", &["list"]),
    ("prg", &["list"]),
    ("ext", &["list"]),
    ("session", &["list"]),
];

/// Whether `argv` (without `openagents`) is a read-only command in
/// [`READ_ONLY`], with bounded, printable words.
pub fn read_only(argv: &[String]) -> bool {
    (2..=MAX_ARGV).contains(&argv.len())
        && READ_ONLY
            .iter()
            .any(|(group, leaves)| argv[0] == *group && leaves.contains(&argv[1].as_str()))
        && argv.iter().all(|word| {
            !word.is_empty() && word.len() <= MAX_ARG_BYTES && !word.chars().any(char::is_control)
        })
}

/// What the worker offered beside a reply, as the phone read it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "offer", rename_all = "snake_case")]
pub enum Offer {
    /// Run Coder on the ready computer with the conversation, or, with
    /// none, connect one.
    RunCoder,
    /// Open one of the phone's screens.
    OpenScreen { screen: Screen },
    /// Run a read-only `openagents` command, after a tap.
    Cli { argv: Vec<String>, runs_on: RunsOn },
    /// Run a test set against a tool, after a tap on the card's button:
    /// the offer body as NIP-CJ's own parser accepted it.
    StartEval { body: Value },
    /// Add a result the phone holds to the Gym, after `SCR-20`'s button.
    PublishEval { body: Value },
}

impl Offer {
    /// Reads one `offer` feedback payload. Exact enum values only; an
    /// unknown offer, screen, or command, or a command that is not
    /// read-only, is `None`.
    pub fn parse(payload: &Value) -> Option<Self> {
        match payload["offer"].as_str()? {
            "run_coder" => Some(Offer::RunCoder),
            "open_screen" => Screen::parse(payload["screen"].as_str()?)
                .map(|screen| Offer::OpenScreen { screen }),
            "cli" => {
                if payload["effect"].as_str() != Some("read_only") {
                    return None;
                }
                let mut argv: Vec<String> = payload["argv"]
                    .as_array()?
                    .iter()
                    .map(|word| word.as_str().map(str::to_owned))
                    .collect::<Option<_>>()?;
                // The command's own name is implied.
                if argv.first().is_some_and(|word| word == "openagents") {
                    argv.remove(0);
                }
                let runs_on = match payload["runs_on"].as_str() {
                    Some("this_device") => RunsOn::ThisDevice,
                    Some("connected_computer") => RunsOn::ConnectedComputer,
                    _ => return None,
                };
                read_only(&argv).then_some(Offer::Cli { argv, runs_on })
            }
            // The eval offers are read by NIP-CJ's own parser, whole: an
            // offer it refuses is set aside.
            "start_eval" => match nostr::cj_conversation::parse_offer(payload).ok()? {
                (_, nostr::cj_conversation::Offer::StartEval { .. }) => Some(Offer::StartEval {
                    body: bare(payload),
                }),
                _ => None,
            },
            "publish_eval" => match nostr::cj_conversation::parse_offer(payload).ok()? {
                (_, nostr::cj_conversation::Offer::PublishEval { .. }) => {
                    Some(Offer::PublishEval {
                        body: bare(payload),
                    })
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// A `start_eval` offer as NIP-CJ reads it.
    pub fn start_eval(&self) -> Option<StartEval> {
        let Offer::StartEval { body } = self else {
            return None;
        };
        match nostr::cj_conversation::parse_offer(body).ok()? {
            (
                _,
                nostr::cj_conversation::Offer::StartEval {
                    suite,
                    subject,
                    size,
                    at,
                    ..
                },
            ) => Some(StartEval {
                suite,
                subject,
                size,
                at,
            }),
            _ => None,
        }
    }

    /// The command as the person reads it.
    pub fn command_line(argv: &[String]) -> String {
        let words: Vec<String> = argv
            .iter()
            .map(|word| {
                if word
                    .chars()
                    .any(|ch| ch.is_whitespace() || "'\"$`\\".contains(ch))
                {
                    format!("'{}'", word.replace('\'', "'\\''"))
                } else {
                    word.clone()
                }
            })
            .collect();
        format!("openagents {}", words.join(" "))
    }
}

/// An offer body without the worker's label, which the phone never shows:
/// it names every control itself. The body still parses, with an empty
/// label replaced by the phone's own word.
fn bare(payload: &Value) -> Value {
    let mut body = payload.clone();
    if let Some(object) = body.as_object_mut() {
        object.insert("label".into(), json!("offer"));
    }
    body
}

/// A `start_eval` offer: which test set, which tool, how big, and where
/// the worker suggests it runs. The phone decides where it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartEval {
    pub suite: nostr::cj_conversation::SuiteSource,
    pub subject: nostr::cj_conversation::SubjectSource,
    pub size: nostr::cj_conversation::Size,
    pub at: nostr::cj_conversation::Where,
}

/// A suggested next question under a prepared answer: tapping it sends
/// its words as the person's message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Followup {
    /// The prepared answer it leads to, as the bank names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    pub label: String,
}

/// Reads a `followups` array: objects with a one-line `label` of at most
/// 80 characters and an optional bank `id` or `answer`. Anything else is
/// set aside.
fn followups(value: &Value) -> Vec<Followup> {
    let mut kept: Vec<Followup> = vec![];
    for entry in value.as_array().into_iter().flatten() {
        let Some(label) = entry["label"].as_str().map(str::trim) else {
            continue;
        };
        if label.is_empty()
            || label.chars().count() > MAX_FOLLOWUP_CHARS
            || label.chars().any(char::is_control)
            || kept.iter().any(|kept| kept.label == label)
        {
            continue;
        }
        let answer = entry["answer"]
            .as_str()
            .or_else(|| entry["id"].as_str())
            .filter(|id| tag_like(id))
            .map(str::to_owned);
        kept.push(Followup {
            answer,
            label: label.to_owned(),
        });
        if kept.len() == MAX_FOLLOWUPS {
            break;
        }
    }
    kept
}

/// A bank id, `id@version`, route, or tier word: short, lowercase ASCII.
fn tag_like(text: &str) -> bool {
    (1..=96).contains(&text.len())
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._@-:".contains(&b))
}

/// What the router said about one reply: kept with it, shown subtly, and
/// sent only when the person shares the chat or marks the answer wrong.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    /// What the worker decided to show first (`canned`, `opener`,
    /// `model`, or a later tier).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    /// The prepared answer that is the text, as `id@version`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bank: Option<String>,
    /// The judgment feedback as it arrived, bounded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub judgment: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offers: Vec<Offer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub followups: Vec<Followup>,
    /// The Gym's cards (NIP-CJ `card` feedback), each body as NIP-CJ's own
    /// parser accepted it; a newer card of one kind replaces the older.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cards: Vec<Value>,
}

impl Meta {
    /// The reply is a prepared answer from the bank, not model text.
    pub fn canned(&self) -> bool {
        self.tier.as_deref() == Some("canned") && self.answer.is_some()
    }

    /// Nothing to keep.
    pub fn is_empty(&self) -> bool {
        *self == Meta::default()
    }

    /// Takes a `judgment` feedback payload.
    pub fn judged(&mut self, payload: &Value) {
        let text = payload.to_string();
        if text.len() <= MAX_JUDGMENT_BYTES {
            self.judgment = Some(text);
        }
        self.take_words(payload);
        self.take_followups(payload);
    }

    /// Takes an `offer` feedback payload.
    pub fn offered(&mut self, payload: &Value) {
        if let Some(offer) = Offer::parse(payload)
            && self.offers.len() < MAX_OFFERS
            && !self.offers.contains(&offer)
        {
            self.offers.push(offer);
        }
    }

    /// Takes a `card` feedback payload: kept only when NIP-CJ's parser
    /// reads it, so the phone never draws a card it can't show exactly.
    pub fn carded(&mut self, payload: &Value) {
        let Ok((_, card)) = nostr::cj_conversation::parse_card(payload) else {
            return;
        };
        let word = card.word();
        if let Some(at) = self
            .cards
            .iter()
            .position(|kept| kept["card"].as_str() == Some(word))
        {
            self.cards[at] = payload.clone();
        } else if self.cards.len() < MAX_CARDS {
            self.cards.push(payload.clone());
        }
    }

    /// The cards, read again.
    pub fn parsed_cards(&self) -> Vec<nostr::cj_conversation::Card> {
        self.cards
            .iter()
            .filter_map(|card| nostr::cj_conversation::parse_card(card).ok())
            .map(|(_, card)| card)
            .collect()
    }

    /// Takes a result's router fields; they outrank the judgment's.
    pub fn resulted(&mut self, payload: &Value) {
        // A result whose model is the bank is a prepared answer, even from
        // a worker that names no tier.
        if payload["model"]
            .as_str()
            .is_some_and(|model| model.starts_with("bank:"))
        {
            self.tier = Some("canned".into());
        }
        self.take_words(payload);
        self.take_followups(payload);
    }

    fn take_words(&mut self, payload: &Value) {
        let word = |field: &str| {
            payload[field]
                .as_str()
                .filter(|w| tag_like(w))
                .map(str::to_owned)
        };
        for (slot, field) in [
            (&mut self.tier, "tier"),
            (&mut self.answer, "answer"),
            (&mut self.route, "route"),
            (&mut self.bank, "bank"),
        ] {
            if let Some(value) = word(field) {
                *slot = Some(value);
            }
        }
        // A judgment's `answer` is the argmax even when the reply is not
        // that answer; only a canned tier keeps it as the text's source.
        if self.tier.as_deref() != Some("canned") {
            self.answer = None;
        }
    }

    fn take_followups(&mut self, payload: &Value) {
        let read = followups(&payload["followups"]);
        if !read.is_empty() {
            self.followups = read;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_context_names_no_host_and_bounds_the_build() {
        let context = Context {
            computer_ready: true,
            app_build: Some("1.0.0 (19)".into()),
            ..Context::default()
        };
        assert_eq!(
            context.json(),
            json!({"surface": "phone", "computer_ready": true, "app_build": "1.0.0 (19)"})
        );
        let odd = Context {
            computer_ready: false,
            app_build: Some("Studio Mac".into()),
            ..Context::default()
        };
        assert_eq!(
            odd.json(),
            json!({"surface": "phone", "computer_ready": false})
        );
    }

    /// Offers are read against the phone's own tables: a screen it does
    /// not know, a command that is not read-only, or an effect other than
    /// `read_only` is set aside, and the worker's label is ignored.
    #[test]
    fn offers_pass_only_the_phones_own_tables() {
        let read = |value: Value| Offer::parse(&value);
        assert_eq!(
            read(json!({"offer": "run_coder", "target": "connected_computer", "label": "x"})),
            Some(Offer::RunCoder)
        );
        assert_eq!(
            read(
                json!({"offer": "open_screen", "screen": "account.computers",
                "label": "Delete everything"})
            ),
            Some(Offer::OpenScreen {
                screen: Screen::Computers
            })
        );
        assert_eq!(
            read(json!({"offer": "open_screen", "screen": "wallet"})),
            Some(Offer::OpenScreen {
                screen: Screen::Wallet
            })
        );
        // `crates/coder/src/router.rs` names exactly these screens.
        for word in [
            "account.computers",
            "account.keys",
            "account.playtest",
            "account.report_problem",
            "wallet",
            "verse.gym",
        ] {
            assert!(Screen::parse(word).is_some(), "{word}");
        }
        // NIP-CJ's `verse.gym`: See the board.
        assert_eq!(
            read(json!({"offer": "open_screen", "screen": "verse.gym", "label": "See the board"})),
            Some(Offer::OpenScreen {
                screen: Screen::VerseGym
            })
        );
        assert_eq!(
            read(json!({"offer": "open_screen", "screen": "settings.danger"})),
            None
        );
        assert_eq!(
            read(json!({"offer": "cli", "argv": ["computer", "list"],
                "effect": "read_only", "runs_on": "this_device", "confirm": true})),
            Some(Offer::Cli {
                argv: vec!["computer".into(), "list".into()],
                runs_on: RunsOn::ThisDevice
            })
        );
        // The worker calls it read-only; the phone's table does not.
        for argv in [
            json!(["wallet", "pay", "lnbc1"]),
            json!(["computer", "approve", "host"]),
            json!(["wallet", "export"]),
            json!(["computer"]),
            json!(["computer", "list\u{7}"]),
        ] {
            assert_eq!(
                read(json!({"offer": "cli", "argv": argv, "effect": "read_only",
                    "runs_on": "this_device"})),
                None,
                "{argv}"
            );
        }
        assert_eq!(
            read(
                json!({"offer": "cli", "argv": ["computer", "list"], "effect": "publishes",
                "runs_on": "this_device"})
            ),
            None
        );
        assert_eq!(read(json!({"offer": "pay", "amount": 5000})), None);
        assert_eq!(
            Offer::command_line(&["kb".into(), "search".into(), "docker cp".into()]),
            "openagents kb search 'docker cp'"
        );
    }

    #[test]
    fn a_canned_result_keeps_its_answer_and_followups() {
        let mut meta = Meta::default();
        meta.judged(&json!({"v": 2, "type": "judgment", "verdict": "respond",
            "tier": "model", "answer": "meta.model@1", "answer_p": 0.4}));
        // The argmax answer is not the text of a model reply.
        assert_eq!(meta.answer, None);
        assert!(!meta.canned());
        meta.resulted(
            &json!({"v": 2, "type": "result", "text": "Our chat runs on …",
            "model": "bank:chat-answers-v1", "tier": "canned", "answer": "meta.model@1",
            "route": "meta", "bank": "chat-answers-v1@9f2c",
            "followups": [{"id": "meta.privacy", "label": "Is this chat private?"},
                {"label": ""}, {"label": "x".repeat(81)}, "meta.pricing",
                {"id": "meta.pricing", "label": "What does it cost?"},
                {"label": "Is this chat private?"}]}),
        );
        assert!(meta.canned());
        assert_eq!(meta.answer.as_deref(), Some("meta.model@1"));
        assert_eq!(meta.route.as_deref(), Some("meta"));
        let labels: Vec<&str> = meta.followups.iter().map(|f| f.label.as_str()).collect();
        assert_eq!(labels, ["Is this chat private?", "What does it cost?"]);
        assert!(meta.judgment.as_deref().unwrap().contains("\"judgment\""));
        // Today's worker names a bank answer by its model alone.
        let mut older = Meta::default();
        older.resulted(
            &json!({"type": "result", "text": "Hi!", "model": "bank:chat-answers-v1",
            "answer": "smalltalk.hello@1"}),
        );
        assert!(older.canned());
    }

    /// The phone's read-only list is the owner's list the worker's CLI
    /// route offers from (`coder::cli_route::gate::PHONE_COMMANDS`), which
    /// this crate cannot depend on: read it from its source.
    #[test]
    fn the_read_only_list_matches_the_worker_phone_list() {
        let gate = include_str!("../../coder/src/cli_route/gate.rs");
        let start = gate
            .find("pub const PHONE_COMMANDS")
            .expect("the worker's phone list");
        let body = &gate[start..start + gate[start..].find("];").expect("its end")];
        let mut worker: Vec<String> = body
            .lines()
            .filter_map(|line| {
                line.trim()
                    .strip_prefix('"')?
                    .strip_suffix("\",")
                    .map(str::to_owned)
            })
            .collect();
        worker.sort();
        let mut phone: Vec<String> = READ_ONLY
            .iter()
            .flat_map(|(group, leaves)| leaves.iter().map(move |leaf| format!("{group} {leaf}")))
            .collect();
        phone.sort();
        assert!(!worker.is_empty());
        assert_eq!(phone, worker);
    }
}

#[cfg(test)]
mod desktop_context_tests {
    #[test]
    fn desktop_requests_identify_the_surface_without_claiming_computer_authority() {
        let context = super::Context {
            surface: super::Surface::Desktop,
            ..Default::default()
        };
        let request =
            crate::basic_coder::payload(&[crate::basic_coder::Turn::user("Hello")], &context);
        assert_eq!(request["context"]["surface"], "desktop");
        assert_eq!(request["client"], "openagents-desktop");
        assert_eq!(request["context"]["computer_ready"], false);
        assert!(
            !request["instructions"]
                .as_str()
                .unwrap()
                .contains("on their phone")
        );
    }
}
