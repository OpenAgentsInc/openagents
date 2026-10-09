//! The Gym in chat, through the Coder tab: every card from the worker's
//! recorded bodies, the sheets, runs on a hosted runner the test drives, a
//! computer's run, the first run's three taps and its relaunch, the
//! draft's round trip, credit from the ledger, and the words on screen.

use super::*;
use crate::basic_chats::BasicChats;
use crate::basic_coder::{Door, Reply, lock};
use crate::chats::Chats;
use crate::coder_tab::CoderTab;
use crate::eval_cards::{CardView, SheetView, jargon};
use crate::first_run::{FIRST_MESSAGE, MenuView};
use crate::router::Context;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// A NIP-CJ fixture the chat router writes (`crates/coder/fixtures/nip-cj`).
fn wire(name: &str) -> Value {
    let text = match name {
        "card-tool" => include_str!("../../../coder/fixtures/nip-cj/router-card-tool.json"),
        "card-result" => include_str!("../../../coder/fixtures/nip-cj/router-card-result.json"),
        "card-news" => include_str!("../../../coder/fixtures/nip-cj/router-card-news.json"),
        "card-check" => include_str!("../../../coder/fixtures/nip-cj/router-card-check.json"),
        "card-draft" => include_str!("../../../coder/fixtures/nip-cj/router-card-draft.json"),
        "card-credit" => include_str!("../../../coder/fixtures/nip-cj/router-card-credit.json"),
        "card-capability" => {
            include_str!("../../../coder/fixtures/nip-cj/router-card-capability.json")
        }
        "start-eval" => include_str!("../../../coder/fixtures/nip-cj/router-offer-start-eval.json"),
        "publish-eval" => {
            include_str!("../../../coder/fixtures/nip-cj/router-offer-publish-eval.json")
        }
        "open-gym-result" => {
            include_str!("../../../coder/fixtures/nip-cj/router-offer-open-gym-result.json")
        }
        "request-v2" => include_str!("../../../coder/fixtures/nip-cj/router-request-v2.json"),
        other => panic!("no fixture {other}"),
    };
    serde_json::from_str(text).expect("fixture JSON")
}

/// A one-run try of the chat's draft on the hosted runner.
fn try_offer() -> Value {
    json!({"v": 2, "requires": [], "type": "offer", "offer": "start_eval", "suite": "draft",
        "subject": "draft", "size": {"cases": 1, "runs": 1, "arms": 2}, "where": "hosted",
        "label": "Try it once"})
}

fn judgment(route: &str) -> Value {
    json!({"v": 2, "requires": [], "type": "judgment", "verdict": "respond", "line": "",
        "set": "chat-router-v2", "route": route, "tier": "gym"})
}

fn result_fields(route: &str) -> Value {
    json!({"v": 2, "type": "result", "model": "bank:chat-answers-v1", "tier": "gym",
        "route": route})
}

/// Each question the worker was asked: its turns, context, and reply.
type Asked = Arc<Mutex<Vec<(Vec<String>, Context, Arc<Mutex<Reply>>)>>>;
/// Each hosted run the runner was asked for, and where it reports.
type Started = Arc<Mutex<Vec<(HostedRun, Arc<Mutex<Live>>)>>>;
/// Each publish control: its request, report, and where it answers.
type Published = Arc<Mutex<Vec<(String, Value, Arc<Mutex<Live>>)>>>;

/// A chat worker the test answers by hand.
#[derive(Clone, Default)]
struct Worker {
    asked: Asked,
}

impl Door for Worker {
    fn ask(
        &self,
        turns: Vec<crate::basic_coder::Turn>,
        context: Context,
        reply: Arc<Mutex<Reply>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let texts = turns.into_iter().map(|t| t.text).collect();
        self.asked.lock().unwrap().push((texts, context, reply));
        Box::pin(async {})
    }
}

impl Worker {
    /// Answer the newest question with `text` and the router's feedback.
    fn answer(&self, text: &str, feedback: &[Value]) {
        let asked = self.asked.lock().unwrap();
        let (_, _, reply) = asked.last().expect("a question");
        let mut reply = reply.lock().unwrap();
        for body in feedback {
            match body["type"].as_str() {
                Some("judgment") => reply.meta.judged(body),
                Some("offer") => reply.meta.offered(body),
                Some("card") => reply.meta.carded(body),
                Some("result") => reply.meta.resulted(body),
                _ => {}
            }
        }
        reply.text = text.into();
        reply.done = true;
    }

    fn questions(&self) -> Vec<Vec<String>> {
        self.asked
            .lock()
            .unwrap()
            .iter()
            .map(|(texts, _, _)| texts.clone())
            .collect()
    }

    fn contexts(&self) -> Vec<Context> {
        self.asked
            .lock()
            .unwrap()
            .iter()
            .map(|(_, context, _)| context.clone())
            .collect()
    }
}

/// A hosted runner the test drives: each start, stop, and publish is
/// recorded, and the test writes what the runner said into `live`.
#[derive(Clone, Default)]
struct Runner {
    started: Started,
    resumed: Arc<Mutex<Vec<String>>>,
    stopped: Arc<Mutex<Vec<String>>>,
    published: Published,
}

impl Hosted for Runner {
    fn start(
        &self,
        _world: SecretKey,
        run: HostedRun,
        live: Arc<Mutex<Live>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let n = self.started.lock().unwrap().len();
        lock(&live).request = Some(format!("{n:0>64}"));
        lock(&live).event = Some(json!({"id": format!("{n:0>64}")}));
        self.started.lock().unwrap().push((run, live));
        Box::pin(async {})
    }
    fn resume(
        &self,
        _world: SecretKey,
        event: Value,
        live: Arc<Mutex<Live>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        self.resumed
            .lock()
            .unwrap()
            .push(event["id"].as_str().unwrap_or_default().to_owned());
        self.started.lock().unwrap().push((
            HostedRun {
                offer: Value::Null,
                draft: None,
                runs: 0,
                check: None,
            },
            live,
        ));
        Box::pin(async {})
    }
    fn stop(&self, _world: SecretKey, event: Value) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        self.stopped
            .lock()
            .unwrap()
            .push(event["id"].as_str().unwrap_or_default().to_owned());
        Box::pin(async {})
    }
    fn publish(
        &self,
        _world: SecretKey,
        request: String,
        report: Value,
        live: Arc<Mutex<Live>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        self.published.lock().unwrap().push((request, report, live));
        Box::pin(async {})
    }
}

impl Runner {
    fn live(&self, n: usize) -> Arc<Mutex<Live>> {
        self.started.lock().unwrap()[n].1.clone()
    }
    fn runs(&self) -> Vec<HostedRun> {
        self.started
            .lock()
            .unwrap()
            .iter()
            .map(|(run, _)| run.clone())
            .collect()
    }
}

/// A report the runner sealed: 1 of 2 tests without the tool, 2 of 2 with
/// it, Better, with each test's runs on each side.
fn report() -> String {
    use xp_ledger::eval::fixture;
    let author = fixture::signer("suite author");
    let suite = fixture::release(&author, "project-map-tests", 10);
    let tool = fixture::release(&author, "project-map", 11);
    let run = fixture::Run::better(&suite, &tool);
    let mut value: Value = serde_json::from_str(&fixture::report(&"ab".repeat(32), &run)).unwrap();
    let measurements = value["measurements"].as_array_mut().unwrap();
    for (arm, case, passed) in [
        ("subject", "map-repo", 3),
        ("baseline", "map-repo", 0),
        ("subject", "say-hello", 2),
        ("baseline", "say-hello", 3),
    ] {
        measurements.push(
            json!({"arm": arm, "metric": format!("case.{case}.runs_passed"),
            "value": passed, "denominator": 3, "unknown_count": 0, "uncertainty": null,
            "evidence": []}),
        );
    }
    value.to_string()
}

struct Phone {
    tab: CoderTab,
    chats: Chats,
    runtime: tokio::runtime::Runtime,
    dir: Arc<tempfile::TempDir>,
}

impl Phone {
    fn new(worker: &Worker, runner: Option<&Runner>) -> Self {
        Self::in_dir(worker, runner, Arc::new(tempfile::tempdir().unwrap()))
    }

    /// A phone whose stores live in `dir`, as after a relaunch.
    fn in_dir(worker: &Worker, runner: Option<&Runner>, dir: Arc<tempfile::TempDir>) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let secret = SecretKey::from_byte_array([0x31; 32]).unwrap();
        let basic = BasicChats::new(
            Some(runtime.handle().clone()),
            Some(Arc::new(worker.clone())),
            Cache::open(&dir.path().join("basic"), &secret).ok(),
        );
        let mut gym = Gym::new(
            Cache::open(&dir.path().join("gym"), &secret).ok(),
            runner.map(|r| Arc::new(r.clone()) as Arc<dyn Hosted>),
            Some(runtime.handle().clone()),
        );
        gym.set_world(SecretKey::from_byte_array([0x42; 32]).unwrap());
        let chats = Chats::new(runtime.handle().clone(), secret, Err("no store".into()));
        let tab = CoderTab::new("coder:gym".into())
            .with_basic(basic)
            .with_gym(gym)
            .with_app_build(Some("1.0.0 (21)".into()));
        Self {
            tab,
            chats,
            runtime,
            dir,
        }
    }

    /// Past the first run, at the main menu.
    fn returning(mut self) -> Self {
        self.tab.gym.set_start("done");
        self
    }

    fn render(&mut self) -> (Value, View) {
        let coder = self.tab.render(None, &mut self.chats).expect("coder view");
        let gym = self.tab.gym_view();
        (coder, gym)
    }

    fn gym(&mut self) -> View {
        self.render().1
    }

    /// Tap the Gym button `id` in the current view.
    fn tap(&mut self, id: &str) -> View {
        let view = self.gym();
        assert!(
            buttons(&view).iter().any(|b| b.id == id && b.enabled),
            "no {id} in {:?}",
            buttons(&view).iter().map(|b| &b.id).collect::<Vec<_>>()
        );
        self.tab.gym_tap(id, None, &mut self.chats);
        self.runtime.block_on(tokio::task::yield_now());
        self.gym()
    }

    /// Send `text` from the chat's composer.
    fn say(&mut self, text: &str) {
        let (coder, _) = self.render();
        let token = find(&coder, |n| n["element"]["kind"] == "composer")
            .expect("composer")["element"]["props"]["token"]
            .as_str()
            .unwrap()
            .to_owned();
        self.tab.submit(&token, text, None, &mut self.chats);
        self.runtime.block_on(tokio::task::yield_now());
    }

    /// Tap the chat's own node `key` (a header button, say) in the current
    /// view.
    fn press(&mut self, key: &str) {
        let (coder, _) = self.render();
        assert!(
            find(&coder, |n| n["key"] == key).is_some(),
            "no {key} in the chat view"
        );
        self.tab.activate(
            &rust_native::Activation {
                instance: coder["instance"].as_str().expect("instance").into(),
                revision: coder["revision"].as_u64().expect("revision"),
                node: key.into(),
            },
            None,
            &mut self.chats,
        );
        self.runtime.block_on(tokio::task::yield_now());
    }

    /// The card of `kind` the chat shows now.
    fn card(&mut self, kind: &str) -> CardView {
        let (coder, gym) = self.render();
        cards_in(&coder, &gym)
            .into_iter()
            .find(|card| card.kind == kind)
            .unwrap_or_else(|| panic!("no {kind} card in {:?}", gym.cards.keys()))
    }
}

fn find(view: &Value, matches: impl Fn(&Value) -> bool + Copy) -> Option<&Value> {
    let mut pending = vec![&view["root"]];
    while let Some(node) = pending.pop() {
        if matches(node) {
            return Some(node);
        }
        if let Some(children) = node["element"]["props"]["children"].as_array() {
            pending.extend(children.iter());
        }
    }
    None
}

/// The cards the chat page places, in its order, drawn from the packet.
fn cards_in(coder: &Value, gym: &View) -> Vec<CardView> {
    let mut out = vec![];
    let mut pending = vec![&coder["root"]];
    let mut order = vec![];
    while let Some(node) = pending.pop() {
        if node["element"]["kind"] == "surface" {
            order.push(
                node["element"]["props"]["resource"]
                    .as_str()
                    .unwrap()
                    .to_owned(),
            );
        }
        if let Some(children) = node["element"]["props"]["children"].as_array() {
            pending.extend(children.iter().rev());
        }
    }
    for resource in order {
        let id = resource.strip_prefix("gym-card:").expect("a Gym card");
        out.push(gym.cards.get(id).expect("the packet has its card").clone());
    }
    out
}

fn buttons(view: &View) -> Vec<Button> {
    let mut out = vec![];
    for card in view.cards.values() {
        out.extend(card.primary.clone());
        out.extend(card.secondary.clone());
        out.extend(card.chips.clone());
    }
    if let Some(sheet) = &view.sheet {
        out.extend(sheet.primary.clone());
        out.extend(sheet.secondary.clone());
        out.extend(sheet.close.clone());
    }
    if let Some(first) = &view.first_run {
        out.push(first.primary.clone());
        out.extend(first.secondary.clone());
    }
    out.push(view.menu.primary.clone());
    out.extend(view.menu.chips.clone());
    out.extend(view.menu.rows.iter().map(|row| row.button.clone()));
    out
}

/// Every word a view puts on screen.
fn words_of(view: &View) -> Vec<String> {
    let mut out = vec![];
    let card_words = |card: &CardView, out: &mut Vec<String>| {
        out.extend(card.step.clone());
        out.push(card.title.clone());
        out.extend(card.badge.clone());
        out.extend(card.lines.iter().map(|l| l.text.clone()));
        for item in &card.items {
            out.push(item.text.clone());
            out.extend(item.detail.clone());
            out.extend(item.trailing.clone());
        }
        out.extend(card.progress.iter().map(|p| p.label.clone()));
        out.extend(card.source.clone());
        if let Some(c) = &card.compare {
            out.push(c.with_label.clone());
            out.push(c.without_label.clone());
        }
    };
    for card in view.cards.values() {
        card_words(card, &mut out);
    }
    if let Some(sheet) = &view.sheet {
        out.extend(sheet_words(sheet));
    }
    if let Some(first) = &view.first_run {
        out.extend(first.indicator.clone());
        out.push(first.title.clone());
        out.extend(first.lines.clone());
        out.push(first.next.clone());
        if let Some((name, line)) = &first.agent {
            out.push(name.clone());
            out.push(line.clone());
        }
    }
    out.extend(menu_words(&view.menu));
    out.extend(buttons(view).into_iter().map(|b| b.label));
    out
}

fn sheet_words(sheet: &SheetView) -> Vec<String> {
    let mut out = vec![sheet.title.clone()];
    out.extend(sheet.headline.clone());
    out.extend(sheet.next.clone());
    out.extend(sheet.bar.as_ref().map(|b| b.label.clone()));
    for section in &sheet.sections {
        out.extend(section.heading.clone());
        out.extend(section.lines.iter().map(|l| l.text.clone()));
        for item in &section.items {
            out.push(item.text.clone());
            out.extend(item.detail.clone());
        }
    }
    out
}

fn menu_words(menu: &MenuView) -> Vec<String> {
    let mut out = vec![
        menu.player.name.clone(),
        menu.status.clone(),
        menu.next.clone(),
        menu.primary_subtitle.clone(),
        menu.footer.clone(),
    ];
    out.extend(menu.player.xp_label.clone());
    out.extend(menu.rows.iter().map(|row| row.subtitle.clone()));
    out
}

fn assert_plain(view: &View) {
    for text in words_of(view) {
        assert_eq!(jargon(&text), None, "a banned word in {text:?}");
    }
}

/// Opens a chat whose reply carries `feedback`.
fn asked(phone: &mut Phone, worker: &Worker, text: &str, reply: &str, feedback: &[Value]) {
    phone.say(text);
    worker.answer(reply, feedback);
    phone.render();
}

/// The missing-capability card (#9960): the worker's `capability` card
/// draws in the phone's words, names the closest capability from the card
/// and nothing of the message, and its button sends the message that
/// starts making one.
#[test]
fn a_missing_capability_card_offers_to_add_one() {
    let worker = Worker::default();
    let runner = Runner::default();
    let mut phone = Phone::new(&worker, Some(&runner)).returning();
    phone.tap("menu.chat");
    asked(
        &mut phone,
        &worker,
        "Book me a flight to Denver",
        "There's no plugin for that yet.",
        &[
            json!({"v": 2, "requires": [], "type": "judgment", "verdict": "respond",
                "line": "There's no plugin for that yet.", "set": "chat-router-v3",
                "route": "capability.missing", "tier": "canned"}),
            wire("card-capability"),
            json!({"v": 2, "type": "result", "model": "bank:chat-answers-v1", "tier": "canned",
                "route": "capability.missing", "answer": "capability.missing_near@2"}),
        ],
    );
    let card = phone.card("capability");
    assert_eq!(card.title, "NO PLUGIN FOR THAT YET");
    assert_eq!(card.lines[0].text, "There's no plugin for that yet.");
    assert_eq!(
        card.lines[1].text,
        "The closest one we have is Project map: What Project map does."
    );
    assert_eq!(card.primary.as_ref().unwrap().label, "ADD A PLUGIN");
    assert_plain(&phone.gym());
    assert!(
        !words_of(&phone.gym())
            .iter()
            .any(|word| word.contains("Denver") || word.contains("flight")),
        "the card carries nothing of the message"
    );
    // The button sends the make-a-capability message as the person's own.
    let add = card.primary.unwrap().id;
    phone.tap(&add);
    let sent = worker.asked.lock().unwrap();
    assert_eq!(
        sent.last().map(|(turns, _, _)| turns.last().cloned()),
        Some(Some(crate::eval_cards::ADD_CAPABILITY_MESSAGE.to_string()))
    );
}

#[test]
fn every_worker_card_renders_in_the_phones_words() {
    let worker = Worker::default();
    let runner = Runner::default();
    let mut phone = Phone::new(&worker, Some(&runner)).returning();
    phone.tap("menu.chat");

    // CARD-01: the tool, its latest verified result, and Start the test.
    asked(
        &mut phone,
        &worker,
        "Test Project map on Coder",
        "We'd try Project map.",
        &[
            judgment("eval.run"),
            wire("card-tool"),
            wire("start-eval"),
            result_fields("eval.run"),
        ],
    );
    let tool = phone.card("tool");
    assert_eq!(tool.title, "PROJECT MAP");
    assert_eq!(tool.icon, Some("map"));
    assert!(
        tool.lines
            .iter()
            .any(|l| l.text == "Latest: 5 of 8 → 7 of 8 tests · Better")
    );
    assert!(
        tool.lines
            .iter()
            .any(|l| l.text == "8 tests, with and without the plugin.")
    );
    assert_eq!(tool.primary.as_ref().unwrap().label, "START THE TEST");
    let others: Vec<&str> = tool.chips.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        others,
        [
            "Code finder",
            "Test reader",
            "Explain this error",
            "Release notes",
            "Dependency check"
        ]
    );
    assert_eq!(
        tool.source.as_deref(),
        Some("From a published result in the Gym.")
    );
    assert_plain(&phone.gym());
    // The worker's label is never shown.
    assert!(!words_of(&phone.gym()).iter().any(|w| w == "Start the test"));

    // CARD-06: a result waiting for a check.
    asked(
        &mut phone,
        &worker,
        "Find me a result to check",
        "Here's one.",
        &[
            judgment("eval.check"),
            wire("card-check"),
            wire("start-eval"),
            result_fields("eval.check"),
        ],
    );
    let check = phone.card("check");
    assert_eq!(check.title, "CHECK A RESULT");
    assert!(
        check.lines[0]
            .text
            .ends_with("says Project map made Coder pass 7 of 8 tests instead of 5.")
    );
    assert!(check.lines[0].text.starts_with("A trainer says"));
    assert_eq!(check.primary.as_ref().unwrap().label, "RUN THE CHECK");
    // No quest record states the checker's share yet: no number.
    assert_eq!(check.badge, None);

    // CARD-05: news, each item with its source line.
    asked(
        &mut phone,
        &worker,
        "What's new in the Gym?",
        "Here's what's new.",
        &[
            judgment("gym.news"),
            wire("card-news"),
            result_fields("gym.news"),
        ],
    );
    let news = phone.card("news");
    assert_eq!(news.items.len(), 3);
    assert_eq!(
        news.source.as_deref(),
        Some("From the Gym's records and our changelog.")
    );
    assert!(news.primary.is_none(), "no offer, no button");

    // CARD-04 for a published result.
    asked(
        &mut phone,
        &worker,
        "How did Project map do?",
        "Here's its latest result.",
        &[
            judgment("eval.result"),
            wire("card-result"),
            result_fields("eval.result"),
        ],
    );
    let result = phone.card("result");
    assert_eq!(result.title, "CODER GOT BETTER");
    let compare = result.compare.unwrap();
    assert_eq!(
        (compare.without.as_deref(), compare.with.as_str()),
        (Some("5 of 8"), "7 of 8")
    );

    // CARD-02: the draft, its tests, and Change it.
    asked(
        &mut phone,
        &worker,
        "Help me make a tool that tidies imports",
        "Here are the tests.\n\nAre these the right tests? Tap Looks good, or tell us what to change.",
        &[
            judgment("eval.author"),
            wire("card-draft"),
            result_fields("eval.author"),
        ],
    );
    let draft = phone.card("draft");
    assert_eq!(draft.title, "YOUR TEST SET · DRAFT");
    assert_eq!(draft.lines[0].text, "Plugin: Tidy imports (yours)");
    assert_eq!(draft.items[0].text, "1 Clean up main.rs.");
    assert_eq!(draft.primary.as_ref().unwrap().label, "LOOKS GOOD");
    let secondary: Vec<&str> = draft.secondary.iter().map(|b| b.label.as_str()).collect();
    assert_eq!(secondary, ["Change it", "See every test"]);

    // eval.credit: the phone draws CARD-07 from its own ledger, never the
    // worker's card.
    asked(
        &mut phone,
        &worker,
        "What have I earned?",
        "Here's your credit.",
        &[
            judgment("eval.credit"),
            wire("card-credit"),
            result_fields("eval.credit"),
        ],
    );
    let credit = phone.card("credit");
    assert_eq!(credit.title, "YOUR CREDIT");
    assert!(
        !words_of(&phone.gym())
            .iter()
            .any(|w| w.contains("Project map test set was checked"))
    );
    assert_plain(&phone.gym());
}

/// Nothing runs, spends, or publishes without its tap: a card only draws;
/// its button starts the run; Add to the Gym only opens the sheet, and only
/// the sheet's button publishes.
#[test]
fn only_a_cards_button_sends_a_request() {
    let worker = Worker::default();
    let runner = Runner::default();
    let mut phone = Phone::new(&worker, Some(&runner)).returning();
    phone.tap("menu.chat");
    asked(
        &mut phone,
        &worker,
        "Test Project map on Coder",
        "We'd try Project map.",
        &[
            judgment("eval.run"),
            wire("card-tool"),
            wire("start-eval"),
            result_fields("eval.run"),
        ],
    );
    for _ in 0..3 {
        phone.render();
    }
    assert!(runner.runs().is_empty(), "drawing a card runs nothing");
    // An ID the view didn't mint does nothing.
    phone
        .tab
        .gym_tap("t00000000-1-0.start", None, &mut phone.chats);
    assert!(runner.runs().is_empty());

    let start = phone.card("tool").primary.unwrap().id;
    phone.tap(&start);
    let runs = runner.runs();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].offer["suite"]["kind"], 3184);
    assert_eq!(runs[0].runs, 3);
    // The run card replaced the tool card, and a second tap can't start
    // another.
    let card = phone.card("run");
    assert_eq!(card.title, "TESTING PROJECT MAP");
    assert!(card.busy && card.primary.is_none());
    phone.tab.gym_tap(&start, None, &mut phone.chats);
    assert_eq!(runner.runs().len(), 1);

    // The runner reports progress, then the result.
    {
        let live = runner.live(0);
        let mut live = lock(&live);
        live.planned = Some(6);
        live.done = Some(4);
    }
    let card = phone.card("run");
    assert_eq!(card.progress.len(), 1);
    assert_eq!(
        (card.progress[0].done, card.progress[0].total),
        (5, 8),
        "4 of 6 runs of 8 tests"
    );
    assert!(card.lines.iter().any(|l| l.text == "4 of 6 runs done"));
    assert!(
        card.lines
            .iter()
            .any(|l| l.text == "You can leave. We'll post the result here and on the menu.")
    );
    lock(&runner.live(0)).outcome = Some(Ok(Outcome::from_report(report().as_bytes()).unwrap()));
    let result = phone.card("result");
    assert_eq!(result.title, "CODER GOT BETTER");
    let compare = result.compare.clone().unwrap();
    assert_eq!(compare.without.as_deref(), Some("1 of 2"));
    assert_eq!(compare.with, "2 of 2");
    assert_eq!(compare.with_label, "with Project map");
    assert_eq!(result.primary.as_ref().unwrap().label, "ADD TO THE GYM");

    // Add to the Gym opens SCR-20; nothing is published yet.
    let view = phone.tap(&result.primary.unwrap().id);
    let sheet = view.sheet.expect("SCR-20");
    assert_eq!(sheet.kind, "publish");
    assert!(runner.published.lock().unwrap().is_empty());
    // SCR-20 lists exactly what becomes public.
    let public: Vec<String> = sheet.sections[0]
        .items
        .iter()
        .map(|i| i.text.clone())
        .collect();
    assert_eq!(public.len(), 3);
    assert_eq!(public[0], "the 8 tests you ran, and how they're checked");
    assert_eq!(public[1], "the result: 1 of 2 → 2 of 2");
    assert!(public[2].starts_with("your trainer name, Trainer "));
    assert_eq!(
        sheet.sections[1].lines[0].text,
        "Coder's full work on each test stays private."
    );
    assert_eq!(sheet.primary.as_ref().unwrap().label, "ADD TO THE GYM");
    assert_eq!(sheet.secondary[0].label, "Not now");

    // Not now closes it, and still nothing is published.
    let view = phone.tap("sheet.later");
    assert!(view.sheet.is_none());
    assert!(runner.published.lock().unwrap().is_empty());

    // The sheet's own button publishes, once.
    let add = phone.card("result").primary.unwrap().id;
    phone.tap(&add);
    let view = phone.tap("sheet.publish");
    assert!(view.sheet.unwrap().busy);
    assert_eq!(runner.published.lock().unwrap().len(), 1);
    let (_, report_ref, live) = runner.published.lock().unwrap()[0].clone();
    assert_eq!(report_ref["schema"], "openagents.eval-report.v1");
    lock(&live).published = Some(Ok(Some("cd".repeat(32))));
    let view = phone.gym();
    let sheet = view.sheet.unwrap();
    assert_eq!(
        sheet.sections[2].lines[0].text,
        "Added to the Gym. You'll earn XP when another trainer checks it."
    );
    assert_eq!(sheet.primary.as_ref().unwrap().label, "BACK TO CHAT");
    let card = phone.card("result");
    assert!(card.primary.is_none());
    assert!(card.lines.iter().any(|l| l.text == "✓ Added to the Gym"));
    assert_plain(&phone.gym());
}

#[test]
fn a_report_reads_each_test_by_the_majority_of_its_runs() {
    let outcome = Outcome::from_report(report().as_bytes()).unwrap();
    assert_eq!(outcome.claim.with, 2);
    assert_eq!(outcome.claim.without, Some(1));
    assert_eq!(outcome.claim.total, 2);
    assert_eq!(outcome.claim.verdict, Verdict3::Pass);
    assert_eq!(
        outcome.cases,
        [
            CaseOutcome {
                id: "map-repo".into(),
                kind: "should-fire".into(),
                with: Some(true),
                without: Some(false),
            },
            CaseOutcome {
                id: "say-hello".into(),
                kind: "should-not-fire".into(),
                with: Some(true),
                without: Some(true),
            },
        ]
    );
    let tried = outcome.tried(3);
    // The worker reads exactly this shape (`router::card::tried`).
    assert_eq!(
        tried["cases"][0],
        json!({"id": "map-repo", "kind": "should-fire", "with": true, "without": false,
            "failing": []})
    );
    assert_eq!(tried["runs"], 3);
    assert_eq!(tried["verdict"], "pass");
    assert!(Outcome::from_report(b"{}").is_err());
}

/// The draft is the person's: it stays on the phone, goes back with every
/// turn, and every interview step is a tap on Looks good or a change.
#[test]
fn the_draft_round_trips_through_the_phone_and_each_step_is_a_tap() {
    let worker = Worker::default();
    let runner = Runner::default();
    let mut phone = Phone::new(&worker, Some(&runner)).returning();
    phone.tap("menu.chat");
    asked(
        &mut phone,
        &worker,
        "Help me make a tool that tidies imports",
        "Here are the tests.\n\nAre these the right tests? Tap Looks good, or tell us what to change.",
        &[
            judgment("eval.author"),
            wire("card-draft"),
            result_fields("eval.author"),
        ],
    );
    // Looks good sends the approval with the draft.
    let good = phone.card("draft").primary.unwrap().id;
    phone.tap(&good);
    let questions = worker.questions();
    assert_eq!(questions.last().unwrap().last().unwrap(), LOOKS_GOOD);
    let context = worker.contexts().last().cloned().unwrap();
    let draft = wire("card-draft")["draft"].clone();
    assert_eq!(context.draft.as_ref(), Some(&draft));
    assert_eq!(context.tried, None);
    // The request is exactly what the worker's fixture reads.
    let body =
        crate::basic_coder::payload(&[crate::basic_coder::Turn::user("looks good")], &context);
    assert_eq!(body["router"], "chat-router-v2");
    assert_eq!(body["draft"], wire("request-v2")["draft"]);

    // Change it puts "Change: " in the composer, which takes the cursor.
    worker.answer(
        "Here are the checks.\n\nAre these the right checks? Tap Looks good, or tell us what to change.",
        &[judgment("eval.author"), wire("card-draft"), result_fields("eval.author")],
    );
    let change = phone.card("draft").secondary[0].id.clone();
    phone.tap(&change);
    let (coder, _) = phone.render();
    let composer = find(&coder, |n| n["element"]["kind"] == "composer").unwrap();
    assert_eq!(composer["element"]["props"]["draft"], "Change: ");
    assert_eq!(composer["element"]["props"]["focus"], true);

    // See every test opens SCR-21 with the draft, and its Looks good
    // approves the same step.
    let tests = phone.card("draft").secondary[1].id.clone();
    let view = phone.tap(&tests);
    let sheet = view.sheet.unwrap();
    assert_eq!(sheet.kind, "test_set");
    assert_eq!(sheet.title, "TIDY IMPORTS · 1 TESTS");
    assert_eq!(
        sheet.sections[0].items[0].detail.as_deref(),
        Some("Checked: Sorted.")
    );
    assert_eq!(sheet.primary.as_ref().unwrap().label, "LOOKS GOOD");

    // Try it once: a one-run try of the draft, on a tap.
    phone.tap("sheet.close");
    phone.say("Looks good");
    worker.answer(
        "Tap Try it once to run each test one time with and without the plugin, then tell us what to fix, or tap Looks good.",
        &[
            judgment("eval.author"),
            wire("card-draft"),
            try_offer(),
            result_fields("eval.author"),
        ],
    );
    let card = phone.card("draft");
    assert_eq!(card.primary.as_ref().unwrap().label, "TRY IT ONCE");
    assert_eq!(card.secondary[0].label, "Looks good");
    assert!(runner.runs().is_empty());
    phone.tap(&card.primary.unwrap().id);
    let runs = runner.runs();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].draft.as_ref(), Some(&draft));
    assert_eq!(runs[0].runs, 1);
    lock(&runner.live(0)).outcome = Some(Ok(Outcome::from_report(report().as_bytes()).unwrap()));
    let first = phone.card("result");
    assert_eq!(first.title, "FIRST TRY");
    assert_eq!(
        first.primary.as_ref().unwrap().label,
        "RUN THE FULL TEST SET"
    );
    // The next turn carries the try's result for the interview to read.
    phone.say("Test 2 looks too easy");
    let context = worker.contexts().last().cloned().unwrap();
    assert_eq!(context.tried.as_ref().unwrap()["runs"], 1);
    assert_eq!(context.tried.as_ref().unwrap()["with"], 2);
    worker.answer(
        "We'll change it.",
        &[judgment("eval.author"), result_fields("eval.author")],
    );
    // The full run is its own tap.
    phone.render();
    let full = phone.card("result").primary.unwrap().id;
    phone.tap(&full);
    assert_eq!(runner.runs().len(), 2);
    assert_eq!(runner.runs()[1].runs, eval_ext::DEFAULT_RUNS);
}

/// With no hosted runner and no computer, a run can't start: the card says
/// why in plain words, with Connect a computer.
#[test]
fn without_a_runner_the_card_says_why_it_cant_run() {
    let worker = Worker::default();
    let mut phone = Phone::new(&worker, None).returning();
    phone.tap("menu.chat");
    asked(
        &mut phone,
        &worker,
        "Test Project map on Coder",
        "We'd try Project map.",
        &[judgment("eval.run"), wire("card-tool"), wire("start-eval")],
    );
    let start = phone.card("tool").primary.unwrap().id;
    phone.tap(&start);
    let card = phone.card("run");
    assert_eq!(
        card.lines[0].text,
        "Our computers can't take these tests right now. Connect a computer and we'll run them there with Coder."
    );
    assert_eq!(card.chips[0].label, "Connect a computer");
    assert_eq!(card.primary.as_ref().unwrap().label, "TRY AGAIN");
    // A check and a draft wait for our computers.
    let run = phone.tab.gym.runs()[0].id.clone();
    assert!(matches!(
        phone.tab.gym.run(&run).unwrap().state,
        RunState::Refused { connect: true, .. }
    ));
    assert_plain(&phone.gym());
    phone.tap(&card.chips[0].id);
    assert_eq!(phone.tab.take_go(), Some(crate::coder_tab::Go::Connect));
}

/// A computer run is a Coder task that runs `openagents ext eval run`; its
/// card follows the task and never states a result it didn't read.
#[test]
fn a_computer_run_follows_its_coder_task() {
    let mut gym = Gym::empty();
    let offer = wire("start-eval");
    let effect = gym.start(
        "talk",
        1,
        &offer,
        "Project map",
        Purpose::Test,
        None,
        Some("Studio Mac"),
        None,
    );
    let Effect::Computer { run, prompt } = effect else {
        panic!("{effect:?}")
    };
    assert!(prompt.contains("openagents ext eval run DIR --trust --grant write --runs 3"));
    assert!(prompt.contains("Don't publish anything"));
    gym.on_computer(
        &run,
        Ok(("host".into(), "Studio Mac".into(), "task".into())),
    );
    use nostr::activity_summary::Phase;
    gym.settle(&|_, _| Some(Phase::Running));
    assert!(matches!(
        gym.run(&run).unwrap().state,
        RunState::Running { .. }
    ));
    let card = gym.result_card_or_run("r", &run, &Here::default());
    assert_eq!(card.secondary[0].label, "Open Coder on Studio Mac");
    gym.settle(&|_, _| Some(Phase::Completed));
    let card = gym.result_card("r", &run);
    assert_eq!(card.title, "DONE ON STUDIO MAC");
    assert!(card.compare.is_none(), "no numbers it didn't read");
    // Add to the Gym, after SCR-20, asks Coder there to publish.
    match gym.publish(&run) {
        Effect::Command {
            host,
            task,
            text,
            stop,
        } => {
            assert_eq!(
                (host.as_str(), task.as_str(), stop),
                ("host", "task", false)
            );
            assert!(text.contains("openagents ext eval publish"));
        }
        other => panic!("{other:?}"),
    }
    // A check waits for our computers: `check` publishes as it ends.
    let effect = gym.start(
        "talk",
        2,
        &offer,
        "Project map",
        Purpose::Check {
            publication: json!({"id": "0a".repeat(32), "pubkey": "6e".repeat(32), "kind": 3189}),
            trainer: "Trainer 7KQ".into(),
            claim: Claim {
                with: 7,
                without: Some(5),
                total: 8,
                verdict: Verdict3::Pass,
            },
        },
        None,
        Some("Studio Mac"),
        None,
    );
    assert_eq!(effect, Effect::None);
}

/// Chat first: a fresh install opens on the chat with the tab bar, and
/// nothing of the Gym is volunteered (no intro, no card, no Gym starter,
/// no menu) until **Train Coder**. Profile and the previous chats stay in
/// the chat's header.
#[test]
fn a_fresh_install_opens_on_the_chat_with_nothing_of_the_gym() {
    let worker = Worker::default();
    let runner = Runner::default();
    let mut phone = Phone::new(&worker, Some(&runner));
    let dir = phone.dir.clone();
    let (coder, view) = phone.render();
    assert_eq!(view.screen, "chat");
    assert!(view.first_run.is_none());
    assert!(view.cards.is_empty(), "{:?}", view.cards.keys());
    assert!(view.sheet.is_none());
    assert!(!phone.tab.gym.opted_in());
    // The chat's header: the previous chats only; no Profile, no target
    // pill, no Menu.
    assert!(find(&coder, |n| n["key"] == "coder-profile").is_none());
    assert!(find(&coder, |n| n["key"] == "coder-target").is_none());
    assert!(find(&coder, |n| n["key"] == "coder-welcome").is_none());
    assert!(find(&coder, |n| n["key"] == "coder-menu").is_some());
    assert!(find(&coder, |n| n["key"] == "coder-back").is_none());
    // Suggested questions, as on every new chat: questions to send, no
    // card and no menu.
    assert!(find(&coder, |n| n["key"] == "coder-suggest-meta.who").is_some());
    // Profile opens from Account, as a sheet on the Chat tab.
    phone.tab.show_profile();
    assert_eq!(
        phone.gym().sheet.map(|sheet| sheet.kind),
        Some("profile"),
        "Profile opens as a sheet"
    );
    phone.tab.gym.sheet = None;
    // A relaunch is the chat again.
    drop(phone);
    let mut phone = Phone::in_dir(&worker, Some(&runner), dir);
    let (coder, view) = phone.render();
    assert_eq!(view.screen, "chat");
    assert!(find(&coder, |n| n["key"] == "coder-suggest-meta.who").is_some());

    // Train Coder: the intro at step 1, with Not now back to the chat.
    phone.tab.train_coder();
    assert_eq!(phone.tab.take_go(), Some(crate::coder_tab::Go::Chat));
    let view = phone.gym();
    assert_eq!(view.screen, "first_run");
    assert_eq!(view.first_run.as_ref().unwrap().step, "choose");
    assert_plain(&view);
    let view = phone.tap("first.later");
    assert_eq!(view.screen, "chat");
    assert!(view.first_run.is_none());
    // Opted in and past the intro, the header's Menu leads to the Gym
    // menu; a new chat's suggestions are the same as before.
    phone.tab.gym.set_start("done");
    let (coder, view) = phone.render();
    assert_eq!(view.screen, "chat");
    assert!(find(&coder, |n| n["key"] == "coder-suggest-meta.tools").is_some());
    phone.press("coder-back");
    assert_eq!(phone.gym().screen, "menu");
}

/// FLOW-01: three taps from **Train Coder** to a test starting, and a
/// relaunch at every step reopens the furthest one.
#[test]
fn the_gym_intro_starts_a_test_in_three_taps_and_resumes() {
    let worker = Worker::default();
    let runner = Runner::default();
    let mut phone = Phone::new(&worker, Some(&runner));
    let dir = phone.dir.clone();
    phone.tab.train_coder();
    let view = phone.gym();
    assert_eq!(view.screen, "first_run");
    assert_eq!(view.first_run.as_ref().unwrap().step, "choose");
    assert_plain(&view);

    // Tap 1.
    let view = phone.tap("first.choose");
    assert_eq!(view.first_run.as_ref().unwrap().step, "end_card");
    // A relaunch reopens the end card.
    let mut phone = Phone::in_dir(&worker, Some(&runner), dir.clone());
    assert_eq!(phone.gym().first_run.unwrap().step, "end_card");

    // Tap 2: the intro's chat asks for Project map's card.
    let view = phone.tap("first.go");
    assert_eq!(view.screen, "chat");
    assert_eq!(
        worker.questions().last().unwrap(),
        &vec![FIRST_MESSAGE.to_owned()]
    );
    worker.answer(
        "We'd try Project map.",
        &[
            judgment("eval.run"),
            wire("card-tool"),
            wire("start-eval"),
            result_fields("eval.run"),
        ],
    );
    let card = phone.card("tool");
    assert_eq!(card.step.as_deref(), Some("STEP 2 OF 3"));
    // No way back to a menu until the first result.
    let (coder, _) = phone.render();
    assert!(find(&coder, |n| n["key"] == "coder-back").is_none());

    // A relaunch reopens the first-run chat with its card on top.
    drop(phone);
    let mut phone = Phone::in_dir(&worker, Some(&runner), dir.clone());
    assert_eq!(phone.gym().screen, "chat");
    let card = phone.card("tool");
    assert_eq!(card.step.as_deref(), Some("STEP 2 OF 3"));

    // Tap 3: the test starts.
    phone.tap(&card.primary.unwrap().id);
    assert_eq!(runner.runs().len(), 1);
    let run = phone.card("run");
    assert_eq!(run.step.as_deref(), Some("STEP 3 OF 3"));

    // The result ends the guided path: Not now on SCR-20 goes to the menu.
    lock(&runner.live(0)).outcome = Some(Ok(Outcome::from_report(report().as_bytes()).unwrap()));
    let add = phone.card("result").primary.unwrap().id;
    phone.tap(&add);
    let view = phone.tap("sheet.later");
    assert_eq!(view.screen, "menu");
    assert_eq!(phone.tab.gym.first_run(), FirstRun::Done);
    assert_eq!(view.menu.next, "Next: add your result to the Gym.");
    assert_plain(&view);

    // A relaunch opens on the chat (chat first, even opted in); the
    // header's Menu opens the Gym menu, and its CHAT goes back to the
    // result the next step names.
    drop(phone);
    let mut phone = Phone::in_dir(&worker, Some(&runner), dir);
    assert_eq!(phone.gym().screen, "chat");
    phone.press("coder-back");
    assert_eq!(phone.gym().screen, "menu");
    let view = phone.tap("menu.chat");
    assert_eq!(view.screen, "chat");
    assert!(phone.card("result").primary.is_some());
}

/// A phone upgraded from a build whose guided first run stopped in the
/// intro's chat (first run at `Chat`, the intro's talk saved, no Gym
/// opt-in) opens on a fresh chat, and New chat stays a new chat: the
/// intro's chat is history in the drawer, not the screen (build 25 on
/// the owner's phone reopened "Test Project map on Coder" on every
/// frame, so New chat did nothing).
#[test]
fn an_upgraded_phone_with_an_unfinished_intro_opens_on_a_fresh_chat() {
    let worker = Worker::default();
    let runner = Runner::default();
    let mut phone = Phone::new(&worker, Some(&runner));
    let dir = phone.dir.clone();
    phone.tab.train_coder();
    phone.tap("first.choose");
    phone.tap("first.go");
    worker.answer(
        "We'd try Project map.",
        &[
            judgment("eval.run"),
            wire("card-tool"),
            wire("start-eval"),
            result_fields("eval.run"),
        ],
    );
    assert_eq!(phone.card("tool").step.as_deref(), Some("STEP 2 OF 3"));
    assert_eq!(phone.tab.gym.first_run(), FirstRun::Chat);
    assert!(phone.tab.gym.first_talk().is_some());
    // The upgrade: the saved intro state stays, and the opt-in this build
    // introduced is unset.
    phone.tab.gym.saved.gym = false;
    phone.tab.gym.save();
    drop(phone);

    let mentions_intro = |node: &serde_json::Value| {
        ["text", "label"]
            .iter()
            .any(|k| node[k].as_str().is_some_and(|t| t.contains(FIRST_MESSAGE)))
    };
    let mut phone = Phone::in_dir(&worker, Some(&runner), dir);
    let (coder, view) = phone.render();
    assert_eq!(view.screen, "chat");
    assert!(view.cards.is_empty(), "{:?}", view.cards.keys());
    assert!(
        find(&coder, mentions_intro).is_none(),
        "the intro's chat is open"
    );
    // The intro's chat is in the drawer; New chat is a new chat.
    phone.press("coder-menu");
    let (drawer, _) = phone.render();
    assert!(find(&drawer, |n| n["key"] == "coder-new").is_some());
    let listed = find(&drawer, |n| {
        n["key"].as_str().is_some_and(|k| k.starts_with("talk-"))
    });
    assert!(listed.is_some(), "the intro's chat is history");
    phone.press("coder-new");
    let (coder, view) = phone.render();
    assert!(view.cards.is_empty(), "{:?}", view.cards.keys());
    assert!(
        find(&coder, mentions_intro).is_none(),
        "New chat reopened the intro"
    );
}

/// Adding the first result ends the guided path at the menu, so the added
/// sheet's button says where it goes (#9941).
#[test]
fn adding_the_first_result_names_the_menu() {
    let worker = Worker::default();
    let runner = Runner::default();
    let mut phone = Phone::new(&worker, Some(&runner));
    phone.tab.train_coder();
    phone.tap("first.choose");
    phone.tap("first.go");
    worker.answer(
        "We'd try Project map.",
        &[
            judgment("eval.run"),
            wire("card-tool"),
            wire("start-eval"),
            result_fields("eval.run"),
        ],
    );
    let start = phone.card("tool").primary.unwrap().id;
    phone.tap(&start);
    lock(&runner.live(0)).outcome = Some(Ok(Outcome::from_report(report().as_bytes()).unwrap()));
    let add = phone.card("result").primary.unwrap().id;
    phone.tap(&add);
    phone.tap("sheet.publish");
    let (_, _, live) = runner.published.lock().unwrap()[0].clone();
    lock(&live).published = Some(Ok(Some("cd".repeat(32))));
    let sheet = phone.gym().sheet.unwrap();
    assert_eq!(sheet.primary.as_ref().unwrap().label, "TO THE MENU");
    let view = phone.tap("sheet.done");
    assert_eq!(view.screen, "menu");
    assert_eq!(phone.tab.gym.first_run(), FirstRun::Done);
    assert_plain(&view);
    // A check is never offered this trainer's own result.
    assert_eq!(phone.tab.gym.skip(), vec!["cd".repeat(32)]);
}

/// A check's Add to the Gym says what a check publishes and when its XP
/// comes, in a sentence's own case (#9941).
#[test]
fn a_checks_add_to_the_gym_speaks_of_the_check() {
    let worker = Worker::default();
    let runner = Runner::default();
    let mut phone = Phone::new(&worker, Some(&runner)).returning();
    phone.tap("menu.chat");
    asked(
        &mut phone,
        &worker,
        "Find me a result to check",
        "Here's one.",
        &[
            judgment("eval.check"),
            wire("card-check"),
            wire("start-eval"),
            result_fields("eval.check"),
        ],
    );
    let check = phone.card("check").primary.unwrap().id;
    phone.tap(&check);
    assert_eq!(runner.runs().len(), 1);
    lock(&runner.live(0)).outcome = Some(Ok(Outcome::from_report(report().as_bytes()).unwrap()));
    let add = phone.card("result").primary.unwrap().id;
    let view = phone.tap(&add);
    let sheet = view.sheet.unwrap();
    assert_eq!(
        sheet.sections[0].items[0].text,
        "your check of a trainer's result"
    );
    assert_eq!(
        sheet.sections[1].lines[1].text,
        "You and the trainer who added the result earn XP whether your check confirms it or not."
    );
    phone.tap("sheet.publish");
    let (_, _, live) = runner.published.lock().unwrap()[0].clone();
    lock(&live).published = Some(Ok(Some("cd".repeat(32))));
    let sheet = phone.gym().sheet.unwrap();
    assert_eq!(
        sheet.sections[2].lines[0].text,
        "Added to the Gym. XP comes once our referee signs your check, whichever way it went."
    );
    assert_eq!(sheet.primary.as_ref().unwrap().label, "BACK TO CHAT");
    assert_plain(&phone.gym());

    // The next turn names the checked result, so the worker, which has no
    // trainer key, doesn't offer it again (#9941). A check is never offered
    // for checking, so the phone's own check isn't named.
    phone.tap("sheet.done");
    let checked = wire("card-check")["publication"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    asked(
        &mut phone,
        &worker,
        "Find me a result to check",
        "Here's one.",
        &[judgment("eval.check"), result_fields("eval.check")],
    );
    let skip = worker.contexts().last().unwrap().skip.clone();
    assert_eq!(skip, vec![checked]);
}

/// A run survives a relaunch: its follower picks the request up again.
#[test]
fn a_hosted_run_is_followed_again_after_a_relaunch() {
    let worker = Worker::default();
    let runner = Runner::default();
    let mut phone = Phone::new(&worker, Some(&runner)).returning();
    let dir = phone.dir.clone();
    phone.tap("menu.chat");
    asked(
        &mut phone,
        &worker,
        "Test Project map on Coder",
        "We'd try Project map.",
        &[judgment("eval.run"), wire("card-tool"), wire("start-eval")],
    );
    let start = phone.card("tool").primary.unwrap().id;
    phone.tap(&start);
    lock(&runner.live(0)).queued = true;
    phone.render();
    drop(phone);
    let mut phone = Phone::in_dir(&worker, Some(&runner), dir);
    assert_eq!(runner.resumed.lock().unwrap().len(), 1);
    assert_eq!(runner.resumed.lock().unwrap()[0], format!("{:0>64}", 0));
    let view = phone.gym();
    assert!(view.live);
    assert_eq!(
        view.menu.next,
        "Next: your test is running. We'll post the result in chat."
    );
    // The menu's primary opens the chat with the run's card.
    phone.tap("menu.chat");
    assert_eq!(phone.card("run").title, "TESTING PROJECT MAP");
    // Stop asks first.
    let stop = phone.card("run").secondary.last().unwrap().id.clone();
    let view = phone.tap(&stop);
    assert_eq!(view.sheet.as_ref().unwrap().kind, "stop");
    assert!(runner.stopped.lock().unwrap().is_empty());
    phone.tap("sheet.stop");
    assert_eq!(runner.stopped.lock().unwrap().len(), 1);
    assert_eq!(phone.card("run").lines[0].text, "You stopped this test.");
}

/// CARD-07 and the Profile's "What you made" come from the phone's ledger;
/// with nothing, they say how to earn.
#[test]
fn credit_comes_from_the_phones_own_ledger() {
    let mut gym = Gym::empty();
    let card = gym.credit_card("c");
    assert_eq!(card.lines[0].text, "Reading your XP…");
    gym.standing = Standing {
        name: "Trainer 7KQ".into(),
        read: true,
        xp: 140,
        level: 2,
        level_at: 100,
        next_at: 283,
        ..Standing::default()
    };
    let card = gym.credit_card("c");
    assert_eq!(
        card.lines[0].text,
        "Nothing yet. When another trainer checks a result you added, you earn XP here."
    );
    assert!(card.chips.is_empty());
    gym.standing.results = vec![
        MadeRow {
            id: "a".into(),
            verdict: "pass".into(),
            confirmed_by: 2,
            standing: "awarded".into(),
            xp: 25,
            check: false,
            suite: String::new(),
        },
        MadeRow {
            id: "b".into(),
            verdict: "pass".into(),
            confirmed_by: 0,
            standing: "waiting".into(),
            xp: 0,
            check: false,
            suite: String::new(),
        },
    ];
    gym.standing.adoptions = vec![("Changelog helper".into(), 200)];
    let card = gym.credit_card("c");
    let rows: Vec<(&str, Option<&str>)> = card
        .items
        .iter()
        .map(|i| (i.text.as_str(), i.trailing.as_deref()))
        .collect();
    assert_eq!(
        rows,
        [
            ("Your result: checked by 2 trainers", Some("+25 XP")),
            ("Your result: waiting for a check", None),
            ("Coder uses it now: Changelog helper", Some("+200 XP")),
            ("Level 2 · 143 XP to level 3", None),
        ]
    );
    assert_eq!(card.primary.as_ref().unwrap().label, "SHARE WHAT YOU MADE");
    // Share opens the system sheet with the ledger's numbers.
    let share = card.primary.unwrap().id;
    let Some(crate::eval_cards::Action::Share { text }) = gym.actions.get(&share).cloned() else {
        panic!()
    };
    assert!(text.contains("Trainer 7KQ") && text.contains("level 2, 140 XP"));
    // A quest record's share names the XP a result earns.
    gym.standing.evaluator_xp = Some(25);
    let run = Run {
        id: "r".into(),
        talk: "t".into(),
        turn: 0,
        tool: "Project map".into(),
        purpose: Purpose::Test,
        offer: Value::Null,
        draft: None,
        cases: 8,
        runs: 3,
        arms: 2,
        place: None,
        state: RunState::Done,
        started_at: 0,
        outcome: None,
        publish: PublishState::None,
        first: false,
    };
    assert_eq!(gym.xp_line(&run), "+25 XP when another trainer checks it.");
    // The profile lists what they made.
    gym.sheet = Some(Sheet::Profile);
    let sheet = gym.sheet_view(None).unwrap();
    assert_eq!(sheet.title, "TRAINER 7KQ");
    assert_eq!(
        sheet.headline.as_deref(),
        Some("Level 2 · 143 XP to level 3")
    );
    // With no run kept on this phone (a reinstall), the ledger's results
    // stand alone: no "No results yet" over them.
    assert_eq!(sheet.sections[0].heading.as_deref(), Some("WHAT YOU MADE"));
    assert_eq!(sheet.sections[0].items.len(), 3);
    assert!(
        sheet
            .sections
            .iter()
            .all(|section| section.heading.as_deref() != Some("YOUR RESULTS"))
    );
}

#[test]
fn a_new_level_shows_once() {
    let mut gym = Gym::empty();
    gym.standing = Standing {
        read: true,
        level: 2,
        xp: 140,
        level_at: 100,
        next_at: 283,
        ..Standing::default()
    };
    gym.level_up();
    assert_eq!(gym.sheet, None, "the first reading only records it");
    gym.standing.level = 3;
    gym.standing.xp = 290;
    gym.standing.titles = vec!["playtester".into()];
    gym.level_up();
    assert_eq!(gym.sheet, Some(Sheet::LevelUp { level: 3 }));
    let sheet = gym.sheet_view(None).unwrap();
    assert_eq!(sheet.big.as_deref(), Some("3"));
    assert_eq!(sheet.primary.as_ref().unwrap().label, "NICE");
    gym.sheet = None;
    gym.level_up();
    assert_eq!(gym.sheet, None);
}

/// `eval.result` with `open_screen gym.result`: the phone's own newest
/// result, or a line that there is none yet.
#[test]
fn see_your_result_shows_the_phones_newest_result() {
    let worker = Worker::default();
    let mut phone = Phone::new(&worker, None).returning();
    phone.tap("menu.chat");
    asked(
        &mut phone,
        &worker,
        "How did my test do?",
        "Here's your latest result.",
        &[
            judgment("eval.result"),
            wire("open-gym-result"),
            result_fields("eval.result"),
        ],
    );
    let card = phone.card("result");
    assert_eq!(card.title, "NO RESULT YET");
    let (coder, _) = phone.render();
    assert!(
        find(&coder, |n| n["element"]["props"]["label"]
            == "See your result")
        .is_none()
    );
}

/// The interview's gate lines are its own, word for word.
#[test]
fn the_gate_lines_are_the_interviews() {
    let stage = include_str!("../../../ext-eval/src/author/stage.rs");
    for line in GATE_LINES {
        assert!(stage.contains(&format!("\"{line}\"")), "{line}");
    }
}

/// Every label the menu, the first run, the cards, and the sheets can show
/// is plain: no banned word (`CHK-02`).
#[test]
fn no_label_uses_a_banned_word() {
    let mut gym = Gym::empty();
    gym.opt_in();
    let view = crate::first_run::first_run(&mut gym).unwrap();
    for text in [view.title, view.next, view.primary.label]
        .into_iter()
        .chain(view.secondary.map(|b| b.label))
        .chain(view.lines)
    {
        assert_eq!(jargon(&text), None, "{text}");
    }
    gym.set_first_run(FirstRun::EndCard);
    let view = crate::first_run::first_run(&mut gym).unwrap();
    for text in [view.title, view.next, view.primary.label]
        .into_iter()
        .chain(view.lines)
    {
        assert_eq!(jargon(&text), None, "{text}");
    }
    let menu = crate::first_run::menu(&mut gym, Some("1.0.0 (21)"));
    for text in menu_words(&menu)
        .into_iter()
        .chain([menu.primary.label])
        .chain(menu.chips.into_iter().map(|c| c.label))
        .chain(menu.rows.into_iter().map(|r| r.button.label))
    {
        assert_eq!(jargon(&text), None, "{text}");
    }
    // Every refusal and state line of a run card.
    for state in [
        RunState::Starting,
        RunState::Queued,
        RunState::Running {
            done: Some(1),
            planned: Some(6),
        },
        RunState::Failed {
            why: "Coder couldn't finish the tests on your computer.".into(),
            ours: true,
        },
        RunState::Stopped,
    ] {
        let run = Run {
            id: "r".into(),
            talk: "t".into(),
            turn: 0,
            tool: "Project map".into(),
            purpose: Purpose::Test,
            offer: Value::Null,
            draft: None,
            cases: 8,
            runs: 3,
            arms: 2,
            place: Some(Place::Computer {
                host: "h".into(),
                label: "Studio Mac".into(),
                task: "t".into(),
            }),
            state,
            started_at: 0,
            outcome: None,
            publish: PublishState::None,
            first: true,
        };
        let card = gym.progress_card("r", &run, &Here::default());
        for text in card
            .lines
            .iter()
            .map(|l| l.text.clone())
            .chain([card.title.clone()])
            .chain(card.secondary.iter().map(|b| b.label.clone()))
        {
            assert_eq!(jargon(&text), None, "{text}");
        }
    }
    for why in [
        "Our computers can't take this check right now. Try again in a moment.",
        "This check is more than our computers run: at most 8 tests and 3 runs.",
        "These tests are more than our computers run: at most 8 tests and 3 runs. We'll keep your draft here.",
        "This phone no longer has these tests' draft. Ask in the chat and we'll make them again.",
        "Our computers can't take these tests right now. We'll keep your draft here.",
        "Our computers can't take these tests right now. Connect a computer and we'll run them there with Coder.",
        "These tests are more than our computers run: at most 8 tests and 3 runs. Connect a computer and we'll run them there with Coder.",
    ] {
        assert_eq!(jargon(why), None, "{why}");
    }
}

/// Writes `fixtures/gym-report.json`, the report the debug-only screenshot
/// runner returns, when `GYM_FIXTURE_WRITE=1`; always checks it parses.
#[test]
fn the_fixture_report_parses() {
    if std::env::var("GYM_FIXTURE_WRITE").as_deref() == Ok("1") {
        // Eight tests: five pass without the tool, seven with it.
        let mut value: Value = serde_json::from_str(&report()).unwrap();
        let cases = [
            ("find-login-handler", "should-fire", 3, 3),
            ("add-date-parser-test", "should-fire", 3, 0),
            ("explain-the-build", "should-fire", 3, 1),
            ("rename-a-module", "should-fire", 3, 3),
            ("fix-a-failing-test", "should-fire", 1, 1),
            ("list-the-crates", "should-fire", 3, 3),
            ("leave-a-typo-fix-alone", "should-not-fire", 3, 3),
            ("answer-a-question", "should-not-fire", 3, 3),
        ];
        value["meta"]["ext_eval"]["cases"] = json!(
            cases
                .iter()
                .map(|(id, kind, _, _)| json!({"id": id, "kind": kind}))
                .collect::<Vec<_>>()
        );
        value["meta"]["ext_eval"]["headline"] =
            json!({"subject_passed": 7, "baseline_passed": 5, "total": 8});
        let mut measurements = vec![json!({"arm": "subject", "metric": "cases_passed",
            "value": 7, "denominator": 8, "unknown_count": 0, "uncertainty": null,
            "evidence": []})];
        for (id, _, with, without) in cases {
            for (arm, passed) in [("subject", with), ("baseline", without)] {
                measurements.push(
                    json!({"arm": arm, "metric": format!("case.{id}.runs_passed"),
                    "value": passed, "denominator": 3, "unknown_count": 0,
                    "uncertainty": null, "evidence": []}),
                );
            }
        }
        value["measurements"] = json!(measurements);
        std::fs::write(
            concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/gym-report.json"),
            serde_json::to_string_pretty(&value).unwrap() + "\n",
        )
        .unwrap();
    }
    let outcome = Outcome::from_report(crate::gym_fixture::REPORT.as_bytes()).unwrap();
    assert_eq!((outcome.claim.without, outcome.claim.with), (Some(5), 7));
    assert_eq!(outcome.claim.total, 8);
}

/// The referee pays a checker once per test set version, so a check of a
/// test set whose checker award the trainer already holds promises no XP,
/// on the check card or on its result (#9948).
#[test]
fn a_second_check_of_a_test_set_promises_no_xp() {
    let worker = Worker::default();
    let runner = Runner::default();
    let mut phone = Phone::new(&worker, Some(&runner)).returning();
    phone.tap("menu.chat");
    asked(
        &mut phone,
        &worker,
        "Find me a result to check",
        "Here's one.",
        &[
            judgment("eval.check"),
            wire("card-check"),
            wire("start-eval"),
            result_fields("eval.check"),
        ],
    );
    phone.tab.gym.standing.read = true;
    phone.tab.gym.standing.checker_xp = Some(50);
    assert_eq!(phone.card("check").badge.as_deref(), Some("+50 XP"));
    // A checker award on another test set doesn't use this one's.
    let award = |suite: &str| MadeRow {
        id: "e1".repeat(32),
        verdict: "pass".into(),
        confirmed_by: 0,
        standing: "awarded".into(),
        xp: 50,
        check: true,
        suite: suite.to_owned(),
    };
    phone.tab.gym.standing.results = vec![award(&"15".repeat(32))];
    assert_eq!(phone.card("check").badge.as_deref(), Some("+50 XP"));
    // The fixture's offer runs test set 1414…: already paid for.
    phone.tab.gym.standing.results = vec![award(&"14".repeat(32))];
    let check = phone.card("check");
    assert_eq!(check.badge, None);
    assert!(check.lines.iter().any(|l| l.text == CHECKED_ALREADY));
    assert_eq!(check.primary.as_ref().unwrap().label, "RUN THE CHECK");
    assert_plain(&phone.gym());
    // Its result says so too, rather than "+50 XP once…".
    let start = check.primary.unwrap().id;
    phone.tap(&start);
    lock(&runner.live(0)).outcome = Some(Ok(Outcome::from_report(report().as_bytes()).unwrap()));
    let result = phone.card("result");
    assert!(result.lines.iter().any(|l| l.text == CHECKED_ALREADY));
    assert!(!result.lines.iter().any(|l| l.text.contains("XP once")));
    // Without that award, the result promises the quest's share.
    phone.tab.gym.standing.results.clear();
    let result = phone.card("result");
    assert!(
        result
            .lines
            .iter()
            .any(|l| l.text == "+50 XP once our referee signs your check, whichever way it went.")
    );
    // A check's own award doesn't count against it.
    let run = phone.tab.gym.saved.runs[0].clone();
    phone.tab.gym.saved.runs[0].publish = PublishState::Published {
        event: Some("e1".repeat(32)),
    };
    phone.tab.gym.standing.results = vec![award(&"14".repeat(32))];
    assert!(!phone.tab.gym.check_credited(&phone.tab.gym.saved.runs[0]));
    assert!(phone.tab.gym.check_credited(&run));
}

/// Profile's Your results lists full runs only: not a draft's try, which
/// can't be added to the Gym, and not a check, which shows under What you
/// made (#9949).
#[test]
fn your_results_lists_full_runs_only() {
    let mut gym = Gym::empty();
    gym.standing = Standing {
        name: "Trainer DJF".into(),
        read: true,
        ..Standing::default()
    };
    let outcome = Outcome::from_report(report().as_bytes()).unwrap();
    let run = |id: &str, tool: &str, purpose: Purpose| Run {
        id: id.into(),
        talk: "t".into(),
        turn: 0,
        tool: tool.into(),
        purpose,
        offer: Value::Null,
        draft: None,
        cases: 8,
        runs: 3,
        arms: 2,
        place: None,
        state: RunState::Done,
        started_at: 0,
        outcome: Some(outcome.clone()),
        publish: PublishState::None,
        first: false,
    };
    let claim = outcome.claim;
    gym.saved.runs = vec![
        run("try", "Changelog helper", Purpose::Try),
        run("full", "Changelog helper", Purpose::Test),
        run(
            "check",
            "Project map",
            Purpose::Check {
                publication: Value::Null,
                trainer: "A trainer".into(),
                claim,
            },
        ),
    ];
    // A draft's one-run try, however it started.
    let mut pilot = run("pilot", "Code finder", Purpose::Test);
    pilot.runs = 1;
    pilot.draft = Some(json!({}));
    gym.saved.runs.push(pilot);
    gym.sheet = Some(Sheet::Profile);
    let sheet = gym.sheet_view(None).unwrap();
    let results = sheet
        .sections
        .iter()
        .find(|s| s.heading.as_deref() == Some("YOUR RESULTS"))
        .unwrap();
    let tools: Vec<&str> = results.items.iter().map(|i| i.text.as_str()).collect();
    assert_eq!(tools, ["Changelog helper"]);

    // With only a try and a check on the phone and only a check in the
    // ledger, there are no results yet, and the section says so.
    gym.saved.runs.remove(1);
    gym.standing.results = vec![MadeRow {
        id: "c".into(),
        verdict: "pass".into(),
        confirmed_by: 0,
        standing: "awarded".into(),
        xp: 50,
        check: true,
        suite: "14".repeat(32),
    }];
    let sheet = gym.sheet_view(None).unwrap();
    let results = sheet
        .sections
        .iter()
        .find(|s| s.heading.as_deref() == Some("YOUR RESULTS"))
        .unwrap();
    assert!(results.items.is_empty());
    assert_eq!(
        results.lines[0].text,
        "No results yet. Your first test takes a few minutes."
    );
}

#[test]
fn desktop_and_phone_show_identical_worker_card_values() {
    fn strip_ids(value: &mut Value) {
        match value {
            Value::Object(fields) => {
                fields.remove("id");
                for value in fields.values_mut() {
                    strip_ids(value);
                }
            }
            Value::Array(values) => {
                for value in values {
                    strip_ids(value);
                }
            }
            _ => {}
        }
    }
    for (name, route) in [
        ("card-tool", "eval.run"),
        ("card-result", "eval.result"),
        ("card-news", "gym.news"),
        ("card-check", "eval.check"),
        ("card-draft", "eval.make"),
        ("card-credit", "eval.credit"),
        ("card-capability", "eval.find"),
    ] {
        let worker = Worker::default();
        let mut phone = Phone::new(&worker, None);
        phone.tab.gym = Gym::empty();
        let feedback = vec![judgment(route), wire(name), result_fields(route)];
        asked(
            &mut phone,
            &worker,
            "Show this card",
            "An answer",
            &feedback,
        );
        let (_, view) = phone.render();
        let mut meta = crate::router::Meta::default();
        meta.judged(&feedback[0]);
        meta.carded(&feedback[1]);
        meta.resulted(&feedback[2]);
        let mut desktop = crate::cards::Cards::default();
        let snapshot = openagents_chat::service::Snapshot {
            chat: Some("a".repeat(32)),
            turns: vec![
                crate::basic_coder::Turn::user("Show this card"),
                crate::basic_coder::Turn::assistant("An answer", Some(meta)),
            ],
            total: 2,
            ..Default::default()
        };
        let rows = desktop.rows(&snapshot);
        assert!(!rows.is_empty(), "{name}");
        let mut a = serde_json::to_value(view.cards.values().collect::<Vec<_>>()).unwrap();
        let mut b = serde_json::to_value(desktop.gym.cards().values().collect::<Vec<_>>()).unwrap();
        strip_ids(&mut a);
        strip_ids(&mut b);
        assert_eq!(a, b, "{name}");
        // The desktop semantic rows retain every visible card title and label.
        let encoded = serde_json::to_string(&rows).unwrap();
        for card in desktop.gym.cards().values() {
            assert!(encoded.contains(&card.title), "{name}: {}", card.title);
            for button in card
                .primary
                .iter()
                .chain(&card.secondary)
                .chain(&card.chips)
            {
                assert!(encoded.contains(&button.label));
                assert!(desktop.action(&button.id).is_some());
            }
        }
    }
}

/// Desktop parity (#10020): the desktop's chat mounts these cards through
/// [`crate::session::Session`] and its buttons go through the same
/// [`Gym::tap`] as the phone's Coder tab.
mod desktop {
    use super::*;
    use crate::cards::Effect as DesktopEffect;
    use crate::session::Session;
    use openagents_chat::service::Snapshot;
    use std::time::Instant;

    /// Card and sheet values without their minted IDs, which name each
    /// surface's own conversation.
    fn plain(value: impl Serialize) -> Value {
        fn strip(value: &mut Value) {
            match value {
                Value::Object(fields) => {
                    fields.remove("id");
                    fields.values_mut().for_each(strip);
                }
                Value::Array(values) => values.iter_mut().for_each(strip),
                _ => {}
            }
        }
        let mut value = serde_json::to_value(value).unwrap();
        strip(&mut value);
        value
    }

    /// What each of a card's buttons does, by label, as a variant name.
    fn does(gym: &Gym, card: &CardView) -> Vec<(String, String)> {
        card.primary
            .iter()
            .chain(&card.secondary)
            .chain(&card.chips)
            .map(|button| {
                let action = gym.actions.get(&button.id).expect("a minted action");
                let name = format!("{action:?}");
                let name = name.split([' ', '{', '(']).next().unwrap().to_owned();
                (button.label.clone(), name)
            })
            .collect()
    }

    struct Desktop {
        session: Session,
        chat: String,
    }

    impl Desktop {
        /// A desktop chat whose newest reply carries `feedback`.
        fn new(feedback: &[Value], computer: Option<&str>) -> Self {
            let mut meta = crate::router::Meta::default();
            for body in feedback {
                match body["type"].as_str() {
                    Some("judgment") => meta.judged(body),
                    Some("result") => meta.resulted(body),
                    Some("offer") => {
                        meta.offers.extend(crate::router::Offer::parse(body));
                    }
                    _ => meta.carded(body),
                }
            }
            let chat = "a".repeat(32);
            let mut session = Session::new(Instant::now());
            session.select(&chat);
            session.states.insert(
                chat.clone(),
                Snapshot {
                    chat: Some(chat.clone()),
                    turns: vec![
                        crate::basic_coder::Turn::user("Ask"),
                        crate::basic_coder::Turn::assistant("An answer", Some(meta)),
                    ],
                    total: 2,
                    ready_computer: computer.map(Into::into),
                    ..Default::default()
                },
            );
            let mut desktop = Self { session, chat };
            desktop.rows();
            desktop
        }

        /// The chat's rows, as the desktop mounts them each pass.
        fn rows(&mut self) -> String {
            let snapshot = self.session.states[&self.chat].clone();
            let rows = self.session.cards.rows_with(&snapshot, false, None);
            serde_json::to_string(&rows).unwrap()
        }

        fn card(&mut self, kind: &str) -> CardView {
            self.rows();
            self.session
                .cards
                .gym
                .cards()
                .values()
                .find(|card| card.kind == kind)
                .cloned()
                .unwrap_or_else(|| panic!("no {kind} card"))
        }

        fn tap(&mut self, id: &str) -> DesktopEffect {
            let effect = self.session.card_action(id);
            self.rows();
            effect
        }
    }

    /// Run a test set from a tool card: both surfaces show the same card
    /// with the same buttons doing the same things, and the tap leaves the
    /// same run card behind.
    #[test]
    fn starting_a_test_from_chat_matches_the_phone() {
        let feedback = [judgment("eval.run"), wire("card-tool"), wire("start-eval")];
        let worker = Worker::default();
        let mut phone = Phone::new(&worker, None).returning();
        phone.tap("menu.chat");
        asked(&mut phone, &worker, "Ask", "An answer", &feedback);
        let mut desktop = Desktop::new(&feedback, None);

        let (phone_tool, desktop_tool) = (phone.card("tool"), desktop.card("tool"));
        assert_eq!(plain(&phone_tool), plain(&desktop_tool));
        assert_eq!(
            does(&phone.tab.gym, &phone_tool),
            does(&desktop.session.cards.gym, &desktop_tool)
        );

        phone.tap(&phone_tool.primary.unwrap().id);
        let effect = desktop.tap(&desktop_tool.primary.unwrap().id);
        assert!(matches!(effect, DesktopEffect::None), "{effect:?}");
        let (phone_run, desktop_run) = (phone.card("run"), desktop.card("run"));
        assert_eq!(plain(&phone_run), plain(&desktop_run));
        assert_eq!(
            does(&phone.tab.gym, &phone_run),
            does(&desktop.session.cards.gym, &desktop_run)
        );
        assert_eq!(desktop_run.chips[0].label, "Connect a computer");
        // Connect a computer goes to the computers screen on each.
        phone.tap(&phone_run.chips[0].id);
        assert_eq!(phone.tab.take_go(), Some(crate::coder_tab::Go::Connect));
        assert!(matches!(
            desktop.tap(&desktop_run.chips[0].id),
            DesktopEffect::Navigate(crate::router::Screen::Computers)
        ));
        // A button the last pass didn't mint does nothing.
        assert!(matches!(desktop.tap("invented"), DesktopEffect::None));
    }

    /// With a ready computer the run goes to Coder there, with the phone's
    /// own prompt; once started, the card follows that Coder task.
    #[test]
    fn a_run_on_the_ready_computer_hands_coder_the_phones_prompt() {
        let feedback = [judgment("eval.run"), wire("card-tool"), wire("start-eval")];
        let mut desktop = Desktop::new(&feedback, Some("Studio Mac"));
        let start = desktop.card("tool").primary.unwrap().id;
        let DesktopEffect::GymCoder { run, prompt } = desktop.tap(&start) else {
            panic!("a Coder run")
        };
        let mut phone = Gym::empty();
        let Effect::Computer { prompt: phones, .. } = phone.start(
            "talk",
            1,
            &wire("start-eval"),
            "Project map",
            Purpose::Test,
            None,
            Some("Studio Mac"),
            None,
        ) else {
            panic!("the phone's computer run")
        };
        assert_eq!(
            prompt.replace(&run, "RUN"),
            phones.replace(&phone.runs()[0].id, "RUN")
        );
        desktop.session.cards.gym.on_computer(
            &run,
            Ok((
                crate::coder_run::LOCAL.into(),
                "Studio Mac".into(),
                desktop.chat.clone(),
            )),
        );
        let card = desktop.card("run");
        assert_eq!(card.secondary[0].label, "Open Coder on Studio Mac");
        assert!(matches!(
            desktop.tap(&card.secondary[0].id),
            DesktopEffect::OpenCoder { host, task }
                if host == crate::coder_run::LOCAL && task == desktop.chat
        ));
        let stop = card
            .secondary
            .iter()
            .find(|button| button.label == "Stop")
            .expect("stop")
            .id
            .clone();
        desktop.rows();
        desktop.tap(&stop);
        // Stop asks first, in the same sheet the phone shows.
        let rows = desktop.rows();
        let sheet = desktop.session.cards.gym.sheet_view(None).expect("a sheet");
        assert_eq!(sheet.kind, "stop");
        assert!(rows.contains(&sheet.title));
        let confirm = sheet.primary.unwrap().id;
        desktop.rows();
        match desktop.tap(&confirm) {
            DesktopEffect::GymCommand {
                host, task, stop, ..
            } => {
                assert_eq!(host, crate::coder_run::LOCAL);
                assert_eq!(task, desktop.chat);
                assert!(stop);
            }
            other => panic!("{other:?}"),
        }
    }

    /// The draft's sheet (`SCR-21`) and its buttons: the desktop mounts
    /// the phone's own sheet under the cards and its buttons do the same.
    #[test]
    fn the_test_set_sheet_matches_the_phone() {
        let feedback = [
            judgment("eval.author"),
            wire("card-draft"),
            result_fields("eval.author"),
        ];
        let reply = "Here are the tests.\n\nAre these the right tests? Tap Looks good, or tell us what to change.";
        let worker = Worker::default();
        let mut phone = Phone::new(&worker, None).returning();
        phone.tap("menu.chat");
        asked(&mut phone, &worker, "Ask", reply, &feedback);
        let mut desktop = Desktop::new(&feedback, None);
        if let Some(turn) = desktop
            .session
            .states
            .get_mut(&desktop.chat)
            .and_then(|state| state.turns.last_mut())
        {
            turn.text = reply.into();
        }
        let (phone_draft, desktop_draft) = (phone.card("draft"), desktop.card("draft"));
        assert_eq!(plain(&phone_draft), plain(&desktop_draft));
        assert_eq!(
            does(&phone.tab.gym, &phone_draft),
            does(&desktop.session.cards.gym, &desktop_draft)
        );

        let phone_sheet = phone
            .tap(&phone_draft.secondary[1].id)
            .sheet
            .expect("the phone's sheet");
        desktop.tap(&desktop_draft.secondary[1].id);
        let rows = desktop.rows();
        let snapshot = desktop.session.states[&desktop.chat].clone();
        let desktop_sheet = desktop
            .session
            .cards
            .gym
            .sheet_view(Gym::draft_of(&snapshot.turns))
            .expect("the desktop's sheet");
        assert_eq!(plain(&phone_sheet), plain(&desktop_sheet));
        assert_eq!(desktop_sheet.kind, "test_set");
        for button in desktop_sheet
            .primary
            .iter()
            .chain(&desktop_sheet.secondary)
            .chain(&desktop_sheet.close)
        {
            assert!(rows.contains(&button.label), "{}", button.label);
        }
        assert!(rows.contains(&desktop_sheet.title));
        desktop.rows();
        // Looks good sends the approval in this chat, as on the phone.
        let good = desktop_sheet.primary.unwrap().id;
        let DesktopEffect::Requests(requests) = desktop.tap(&good) else {
            panic!("a send")
        };
        assert!(matches!(
            &requests[..],
            [(_, openagents_chat::service::Command::Send { text, .. })] if text == LOOKS_GOOD
        ));
        assert!(desktop.session.cards.gym.sheet.is_none());
    }
}

/// A Gym that reads its trainer key only when a hosted run needs it
/// (#10096, the desktop): building it asks for nothing, a hosted start
/// waits and asks once, the key starts the waiting run and opens the store
/// with this session's run kept; a denied key refuses in one plain line
/// and nothing asks again.
#[test]
fn a_lazy_trainer_key_is_asked_for_once_by_a_hosted_start() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let draft = Some(json!({"tests": []}));
    let lazy = |runner: &Runner| {
        let mut gym = Gym::new(
            None,
            Some(Arc::new(runner.clone()) as Arc<dyn Hosted>),
            Some(runtime.handle().clone()),
        );
        gym.wait_for_world();
        gym
    };

    // The key arrives: the waiting run starts on the runner and is kept.
    let runner = Runner::default();
    let mut gym = lazy(&runner);
    assert!(!gym.wants_world(), "nothing asks at launch");
    let effect = gym.start(
        "t",
        1,
        &try_offer(),
        "map",
        Purpose::Try,
        draft.clone(),
        None,
        None,
    );
    assert_eq!(effect, Effect::None);
    assert!(gym.wants_world());
    assert!(gym.runs()[0].running());
    runtime.block_on(tokio::task::yield_now());
    assert!(runner.runs().is_empty(), "nothing is sent without the key");
    let dir = tempfile::tempdir().unwrap();
    let world = SecretKey::from_byte_array([0x42; 32]).unwrap();
    gym.attach_store(Cache::open(&dir.path().join("gym"), &world).unwrap());
    gym.set_world(world);
    assert!(!gym.wants_world());
    runtime.block_on(tokio::task::yield_now());
    assert_eq!(runner.runs().len(), 1);
    assert_eq!(gym.runs().len(), 1);
    let reopened = Gym::new(
        Cache::open(&dir.path().join("gym"), &world).ok(),
        None,
        None,
    );
    assert_eq!(reopened.runs().len(), 1, "the session's run was saved");

    // The key is denied: the run refuses plainly, and a second start
    // refuses at once without asking.
    let runner = Runner::default();
    let mut gym = lazy(&runner);
    gym.start(
        "t",
        1,
        &try_offer(),
        "map",
        Purpose::Try,
        draft.clone(),
        None,
        None,
    );
    gym.world_unavailable("No trainer key: the keychain said no.");
    assert!(!gym.wants_world());
    assert!(matches!(&gym.runs()[0].state,
        RunState::Refused { why, .. } if why == "No trainer key: the keychain said no."));
    gym.start("t", 2, &try_offer(), "map", Purpose::Try, draft, None, None);
    assert!(!gym.wants_world(), "a denied key is never asked for again");
    assert!(matches!(&gym.runs()[1].state, RunState::Refused { .. }));
    assert!(runner.runs().is_empty());
}
