//! The hosted eval runner, as the phone and the desktop reach it (#9935,
//! #10060, `nostr::eval_ext::hosted`): a NIP-CJ execution worker on
//! `relay.openagents.com` whose one target is the `ext-eval` program.
//! Both apps hand [`HostedRelay`] to their [`crate::gym::Gym`]; only the
//! wake hook and the words a cancel carries differ.
//!
//! A tap on a card's **Start the test** (or **Run the check**, or **Try it
//! once**) sends a `25920` signed by the trainer's world key and encrypted
//! to the runner, whose `input` is `hosted::run_input` built from the
//! `start_eval` offer the card showed. The phone listens before it sends,
//! since the answers are ephemeral: `27020` `accepted` and `progress`, then
//! one `26920` result, each checked with `execution::bind_worker_event`
//! (signed by the runner, to this trainer, for this request). A finished
//! run names the report and the `3188` that seals it to the trainer; the
//! phone opens that envelope with the world key for each test's result,
//! and otherwise keeps the headline the result states. A connection lives
//! at most two minutes, so the phone reconnects and sends the same signed
//! request again, which the runner answers with what it recorded and never
//! runs twice. **Add to the Gym** sends a publish request for the report,
//! and **Stop** a cancel control.

use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nostr::domain::{Event, RelaySigner, Tag};
use nostr::eval_ext::hosted;
use nostr::execution::{self, Pending, Seal};
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::{Value, json};

use crate::basic_coder::lock;
use crate::gym::{Hosted, HostedRun, Live, Outcome};
use crate::router::Offer;

/// How long one relay connection lives: the transport's most.
const CONNECTION: Duration = Duration::from_secs(110);
/// How long a new request may go without any answer from the runner.
const CONTACT: u64 = 60;
/// How long the app waits before connecting again after a failure.
const RETRY: Duration = Duration::from_secs(3);

/// What a follower calls when a run's state moved, so the app draws it.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// The hosted runner on its relay.
pub struct HostedRelay {
    relay: String,
    runner: String,
    wake: Wake,
    /// The `reason` a cancel control carries.
    stopped: &'static str,
}

impl HostedRelay {
    /// The deployed runner, or, in a debug build, the relay and runner key
    /// a launch names. It rings the phone's [`crate::wake::ring`] and says
    /// a stop came from the phone, as it always has.
    pub fn new(relay: Option<&str>, runner: Option<&str>) -> Self {
        Self {
            relay: relay.unwrap_or(hosted::RELAY).to_owned(),
            runner: runner.unwrap_or(hosted::RUNNER).to_owned(),
            wake: Arc::new(crate::wake::ring),
            stopped: "The trainer stopped it from the phone.",
        }
    }

    /// The same runner, waking the desktop's event loop with `wake`, and
    /// saying a stop came from the desktop app.
    #[must_use]
    pub fn for_desktop(mut self, wake: Wake) -> Self {
        self.wake = wake;
        self.stopped = "The trainer stopped it from the desktop app.";
        self
    }
}

/// The public key of `secret`, as hex.
fn public_hex(secret: &SecretKey) -> String {
    secret
        .x_only_public_key(&secp256k1::Secp256k1::new())
        .0
        .to_string()
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn random_id() -> String {
    secp256k1::rand::random::<[u8; 16]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The run request's `input` for `run`, from the offer the card showed.
///
/// # Errors
///
/// When the offer isn't a `start_eval` or the input doesn't check.
pub fn run_input(run: &HostedRun) -> Result<Value, String> {
    let start = Offer::StartEval {
        body: run.offer.clone(),
    }
    .start_eval()
    .ok_or("the test's offer didn't read")?;
    let check = run
        .check
        .as_ref()
        .and_then(|pointer| pointer["id"].as_str())
        .map(str::to_owned);
    hosted::run_input(
        &start.suite,
        &start.subject,
        run.draft.as_ref(),
        run.runs,
        check.as_deref(),
    )
    .map_err(|e| e.to_string())
}

/// A signed request to `runner` with `input`, from `world`: the event and
/// its logical request ID.
fn request(world: &SecretKey, runner: &str, input: &Value) -> Result<(Event, String), String> {
    let now = unix_now();
    let logical = random_id();
    let body = hosted::request_body(runner, &logical, input, now).map_err(|e| e.to_string())?;
    let signer = RelaySigner::from_secret_hex(&world.display_secret().to_string())
        .map_err(|e| e.to_string())?;
    let peer = XOnlyPublicKey::from_str(runner).map_err(|e| e.to_string())?;
    let deadline = body["deadline"].as_u64().ok_or("the request's deadline")?;
    let event = Seal {
        signer: &signer,
        conversation: nostr::nip44::conversation_key(world, &peer),
        nonce: secp256k1::rand::random(),
        created_at: now,
    }
    .event(
        execution::REQUEST_KIND,
        hosted::request_tags(runner, deadline),
        &body,
    )
    .map_err(|e| format!("{e:?}"))?;
    Ok((event, logical))
}

/// The logical request ID a signed request carries, read back with the
/// world key.
fn logical_of(event: &Event, world: &SecretKey, runner: &str) -> Option<String> {
    let peer = XOnlyPublicKey::from_str(runner).ok()?;
    let key = nostr::nip44::conversation_key(world, &peer);
    let text = nostr::nip44::decrypt(&event.content, &key).ok()?;
    let body: Value = serde_json::from_str(&text).ok()?;
    body["request"].as_str().map(str::to_owned)
}

/// A refusal in the app's words, and whether it was on our side.
pub fn refused(code: &str) -> (String, bool) {
    match code {
        // An old runner's count: no surface names a limit (#10120).
        hosted::OVER_QUOTA => (
            "Our test computers can't take this run right now. Try again later.".into(),
            false,
        ),
        hosted::NOT_ADMITTED => (
            "Our test computers don't run this plugin or test set.".into(),
            false,
        ),
        hosted::TOO_LARGE => (
            "This test set is too big for our test computers.".into(),
            false,
        ),
        _ => (
            "Something went wrong on our side. This didn't use a run.".into(),
            true,
        ),
    }
}

/// What following a request is for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Follow {
    Run,
    Publish,
}

/// Listen for the runner's answers to `request`, sending it on each
/// connection, until a result lands in `live` or the request's deadline.
#[allow(clippy::too_many_arguments)]
async fn follow(
    relay: String,
    runner: String,
    world: SecretKey,
    request: Event,
    logical: String,
    live: Arc<Mutex<Live>>,
    mode: Follow,
    contact: Option<u64>,
    wake: Wake,
) {
    let me = public_hex(&world);
    let end = request.created_at + hosted::DEADLINE_SECONDS + 60;
    // A new request hears from the runner within a minute, or the card
    // says our computers didn't answer; a resumed one waits its deadline.
    let contact = contact.map(|seconds| unix_now() + seconds);
    while unix_now() < end {
        let heard = {
            let live = lock(&live);
            live.queued || live.planned.is_some()
        };
        let asking = connection(
            &relay, &runner, &world, &me, &request, &logical, &live, mode, &wake,
        );
        let answered = match contact.filter(|_| !heard) {
            Some(by) => {
                let wait = Duration::from_secs(by.saturating_sub(unix_now()).max(1));
                tokio::time::timeout(wait, asking).await.ok()
            }
            None => Some(asking.await),
        };
        match answered {
            Some(Ok(true)) => return,
            Some(Ok(false)) => {}
            Some(Err(_)) => tokio::time::sleep(RETRY).await,
            None => {}
        }
        let heard = {
            let live = lock(&live);
            live.queued || live.planned.is_some()
        };
        if let Some(by) = contact
            && !heard
            && unix_now() >= by
        {
            let mut live = lock(&live);
            match mode {
                Follow::Run => {
                    live.outcome = Some(Err((
                        "Our test computers didn't answer. This didn't use a run.".into(),
                        true,
                    )));
                }
                Follow::Publish => {
                    live.published = Some(Err("Our test computers didn't answer.".into()));
                }
            }
            return;
        }
    }
    let mut live = lock(&live);
    match mode {
        Follow::Run if live.outcome.is_none() => {
            live.outcome = Some(Err((
                "We lost touch with this test. Try it again.".into(),
                true,
            )));
        }
        Follow::Publish if live.published.is_none() => {
            live.published = Some(Err("We didn't hear back from our computers.".into()));
        }
        _ => {}
    }
}

/// One connection: subscribe, send the request, and read answers. `true`
/// once the result is in.
#[allow(clippy::too_many_arguments)]
async fn connection(
    relay: &str,
    runner: &str,
    world: &SecretKey,
    me: &str,
    request: &Event,
    logical: &str,
    live: &Arc<Mutex<Live>>,
    mode: Follow,
    wake: &Wake,
) -> Result<bool, String> {
    let mut socket = nostr_transport::Connection::connect(relay, world, CONNECTION)
        .await?
        .with_frame_budget(4_096);
    socket
        .send(json!(["REQ", "answers", {
            "kinds": [execution::RESULT_KIND, nostr::kinds::CJ_EXECUTION_FEEDBACK],
            "#p": [me],
            "#e": [request.id],
        }]))
        .await?;
    loop {
        let frame = socket.next().await?;
        if frame[0] == "EOSE" {
            break;
        }
    }
    socket.send(json!(["EVENT", request])).await?;
    let pending = Pending {
        execute_event: &request.id,
        worker: runner,
        customer: me,
        request: logical,
        attempt: 1,
    };
    loop {
        let frame = socket.next().await?;
        if frame[0] == "OK" && frame[1] == request.id.as_str() && frame[2] == false {
            return Err(frame[3].as_str().unwrap_or("refused").to_owned());
        }
        if frame[0] != "EVENT" {
            continue;
        }
        let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) else {
            continue;
        };
        let Ok(payload) = execution::bind_worker_event(&event, &pending, world) else {
            continue;
        };
        match payload["type"].as_str() {
            Some("accepted") => lock(live).queued = true,
            Some("progress") => {
                if let Ok(progress) = hosted::parse_progress(&payload) {
                    let mut live = lock(live);
                    live.done = Some(progress.completed);
                    live.planned = Some(progress.planned);
                }
            }
            Some("result") => {
                let done = result(&mut socket, world, &payload, live, mode).await;
                wake();
                return Ok(done);
            }
            _ => continue,
        }
        wake();
    }
}

/// Takes a `26920` result: a finished run's report (opened from its sealed
/// envelope when the relay has it), a publish's result, or a refusal.
async fn result(
    socket: &mut nostr_transport::Connection,
    world: &SecretKey,
    payload: &Value,
    live: &Arc<Mutex<Live>>,
    mode: Follow,
) -> bool {
    let outcome = payload["outcome"].as_str().unwrap_or_default();
    if outcome != "completed" {
        let code = payload["code"].as_str().unwrap_or(outcome);
        let (why, ours) = if outcome == "cancelled" {
            ("You stopped this test.".to_owned(), false)
        } else {
            refused(code)
        };
        let mut live = lock(live);
        match mode {
            Follow::Run => live.outcome = Some(Err((why, ours))),
            Follow::Publish => live.published = Some(Err(why)),
        }
        return true;
    }
    match mode {
        Follow::Publish => {
            let published = hosted::parse_publish_output(&payload["output"])
                .map(|output| Some(output.result.id))
                .map_err(|_| "We couldn't read our computers' answer.".to_owned());
            lock(live).published = Some(published);
        }
        Follow::Run => {
            let outcome = match hosted::parse_run_output(&payload["output"]) {
                Ok(output) => {
                    let opened = sealed(socket, world, &output.sealed.id).await;
                    Ok(opened.unwrap_or_else(|| Outcome::from_output(&output)))
                }
                Err(_) => Err(("We couldn't read our computers' answer.".to_owned(), true)),
            };
            lock(live).outcome = Some(outcome);
        }
    }
    true
}

/// The report in the `3188` the runner sealed to this trainer, read with
/// NIP-EVAL's parser.
async fn sealed(
    socket: &mut nostr_transport::Connection,
    world: &SecretKey,
    id: &str,
) -> Option<Outcome> {
    socket
        .send(json!(["REQ", "sealed", {"ids": [id], "limit": 1}]))
        .await
        .ok()?;
    for _ in 0..16 {
        let frame = socket.next().await.ok()?;
        if frame[0] == "EOSE" && frame[1] == "sealed" {
            return None;
        }
        if frame[0] == "EVENT" && frame[1] == "sealed" {
            let event = serde_json::from_value::<Event>(frame[2].clone()).ok()?;
            let opened = nostr::private_artifact::open(&event, world).ok()?;
            return Outcome::from_report(opened.inline_bytes()?).ok();
        }
    }
    None
}

/// The signed cancel control for the run `event` requested, saying
/// `reason`.
fn cancel(world: &SecretKey, runner: &str, event: Value, reason: &str) -> Option<Event> {
    let event = serde_json::from_value::<Event>(event).ok()?;
    let logical = logical_of(&event, world, runner)?;
    let signer = RelaySigner::from_secret_hex(&world.display_secret().to_string()).ok()?;
    let peer = XOnlyPublicKey::from_str(runner).ok()?;
    let body = json!({"v": execution::SCHEMA, "requires": [], "type": "cancel",
        "request": logical, "attempt": 1, "run": logical,
        "reason": reason});
    (Seal {
        signer: &signer,
        conversation: nostr::nip44::conversation_key(world, &peer),
        nonce: secp256k1::rand::random(),
        created_at: unix_now(),
    })
    .event(
        execution::REQUEST_KIND,
        vec![
            Tag::new(vec!["e".into(), event.id.clone()]),
            Tag::new(vec!["p".into(), runner.to_owned()]),
        ],
        &body,
    )
    .ok()
}

impl Hosted for HostedRelay {
    fn start(
        &self,
        world: SecretKey,
        run: HostedRun,
        live: Arc<Mutex<Live>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let (relay, runner, wake) = (self.relay.clone(), self.runner.clone(), self.wake.clone());
        Box::pin(async move {
            let sent = run_input(&run).and_then(|input| request(&world, &runner, &input));
            let (event, logical) = match sent {
                Ok(sent) => sent,
                Err(_) => {
                    lock(&live).outcome = Some(Err((
                        "We couldn't ask our computers for this test.".into(),
                        true,
                    )));
                    return;
                }
            };
            {
                let mut live = lock(&live);
                live.request = Some(event.id.clone());
                live.event = serde_json::to_value(&event).ok();
            }
            wake();
            follow(
                relay,
                runner,
                world,
                event,
                logical,
                live,
                Follow::Run,
                Some(CONTACT),
                wake,
            )
            .await;
        })
    }

    fn resume(
        &self,
        world: SecretKey,
        event: Value,
        live: Arc<Mutex<Live>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let (relay, runner, wake) = (self.relay.clone(), self.runner.clone(), self.wake.clone());
        Box::pin(async move {
            let Ok(event) = serde_json::from_value::<Event>(event) else {
                lock(&live).outcome = Some(Err(("We lost touch with this test.".into(), true)));
                return;
            };
            let Some(logical) = logical_of(&event, &world, &runner) else {
                lock(&live).outcome = Some(Err(("We lost touch with this test.".into(), true)));
                return;
            };
            follow(
                relay,
                runner,
                world,
                event,
                logical,
                live,
                Follow::Run,
                None,
                wake,
            )
            .await;
        })
    }

    fn stop(&self, world: SecretKey, event: Value) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let (relay, runner, stopped) = (self.relay.clone(), self.runner.clone(), self.stopped);
        Box::pin(async move {
            let Some(cancel) = cancel(&world, &runner, event, stopped) else {
                return;
            };
            if let Ok(mut socket) =
                nostr_transport::Connection::connect(&relay, &world, Duration::from_secs(20)).await
            {
                let _ = socket.send(json!(["EVENT", cancel])).await;
                let _ = socket.next().await;
            }
        })
    }

    fn publish(
        &self,
        world: SecretKey,
        _request: String,
        report: Value,
        live: Arc<Mutex<Live>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let (relay, runner, wake) = (self.relay.clone(), self.runner.clone(), self.wake.clone());
        Box::pin(async move {
            let input = nostr::contracts::parse_artifact(&report)
                .map(|report| hosted::publish_input(&report))
                .map_err(|e| e.to_string());
            let Ok((event, logical)) = input.and_then(|input| request(&world, &runner, &input))
            else {
                lock(&live).published =
                    Some(Err("We couldn't ask our computers to add it.".into()));
                return;
            };
            follow(
                relay,
                runner,
                world,
                event,
                logical,
                live,
                Follow::Publish,
                Some(CONTACT),
                wake,
            )
            .await;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> SecretKey {
        SecretKey::from_byte_array([0x42; 32]).unwrap()
    }

    fn runner() -> (SecretKey, String) {
        let secret = SecretKey::from_byte_array([0x51; 32]).unwrap();
        (secret, public_hex(&secret))
    }

    /// The request a card's tap sends is exactly the hosted runner's: a
    /// `25920` from the trainer's world key, one `p` (the runner), and an
    /// `expiration` equal to its deadline, which the runner's own opener
    /// reads, with the offer's suite, tool, and runs.
    #[test]
    fn a_tap_sends_the_runners_own_request() {
        let (runner_secret, runner_key) = runner();
        let offer: Value = serde_json::from_str(include_str!(
            "../../coder/fixtures/nip-cj/router-offer-start-eval.json"
        ))
        .unwrap();
        let run = HostedRun {
            offer,
            draft: None,
            runs: 3,
            check: None,
        };
        let input = run_input(&run).unwrap();
        let (event, logical) = request(&world(), &runner_key, &input).unwrap();
        assert_eq!(event.kind, 25_920);
        assert_eq!(event.pubkey, public_hex(&world()));
        assert_eq!(
            event.tag_values("p").collect::<Vec<_>>(),
            [runner_key.as_str()]
        );
        let opened = execution::open_request(
            &event,
            &runner_key,
            &runner_secret,
            unix_now(),
            execution::Window::DEFAULT,
        )
        .unwrap();
        let execution::Body::Execute(execute) = &opened.body else {
            panic!("an execute request")
        };
        assert_eq!(execute.request, logical);
        let parsed = hosted::parse_input(&execute.input).unwrap();
        let hosted::Input::Run(run) = parsed else {
            panic!("a run")
        };
        assert_eq!(run.runs, 3);
        assert!(run.check.is_none());
        assert_eq!(logical_of(&event, &world(), &runner_key), Some(logical));
        // A check names the result it checks.
        let check = HostedRun {
            check: Some(json!({"id": "0a".repeat(32), "pubkey": "6e".repeat(32), "kind": 3189})),
            ..HostedRun {
                offer: serde_json::from_str(include_str!(
                    "../../coder/fixtures/nip-cj/router-offer-start-eval.json"
                ))
                .unwrap(),
                draft: None,
                runs: 3,
                check: None,
            }
        };
        assert_eq!(run_input(&check).unwrap()["check"], "0a".repeat(32));
    }

    /// The desktop's runner is the phone's: the same request, only its own
    /// wake, and a stop that says it came from the desktop app. A stop is
    /// a cancel of that same request, signed by the trainer to the runner.
    #[test]
    fn the_desktop_runner_is_the_phones_with_its_own_wake() {
        let (runner_secret, runner_key) = runner();
        let rung = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = rung.clone();
        let desktop = HostedRelay::new(None, Some(&runner_key)).for_desktop(Arc::new(move || {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }));
        let phone = HostedRelay::new(None, Some(&runner_key));
        assert_eq!(
            (desktop.relay.as_str(), phone.relay.as_str()),
            (hosted::RELAY, hosted::RELAY)
        );
        assert_eq!(desktop.runner, phone.runner);
        (desktop.wake)();
        assert_eq!(rung.load(std::sync::atomic::Ordering::SeqCst), 1);
        let run = HostedRun {
            offer: serde_json::from_str(include_str!(
                "../../coder/fixtures/nip-cj/router-offer-start-eval.json"
            ))
            .unwrap(),
            draft: None,
            runs: 1,
            check: None,
        };
        let (event, logical) = request(&world(), &runner_key, &run_input(&run).unwrap()).unwrap();
        for (relay, words) in [
            (&phone, "The trainer stopped it from the phone."),
            (&desktop, "The trainer stopped it from the desktop app."),
        ] {
            let stop = cancel(
                &world(),
                &runner_key,
                serde_json::to_value(&event).unwrap(),
                relay.stopped,
            )
            .unwrap();
            assert_eq!(stop.pubkey, public_hex(&world()));
            assert_eq!(
                stop.tag_values("e").collect::<Vec<_>>(),
                [event.id.as_str()]
            );
            let key = nostr::nip44::conversation_key(
                &runner_secret,
                &XOnlyPublicKey::from_str(&stop.pubkey).unwrap(),
            );
            let body: Value =
                serde_json::from_str(&nostr::nip44::decrypt(&stop.content, &key).unwrap()).unwrap();
            assert_eq!(body["type"], "cancel");
            assert_eq!(body["request"], logical.as_str());
            assert_eq!(body["reason"], words);
        }
    }

    /// A refusal reads in plain words; only a failure on our side says so.
    #[test]
    fn refusals_read_plainly() {
        assert!(!refused(hosted::OVER_QUOTA).1);
        let over = refused(hosted::OVER_QUOTA).0.to_lowercase();
        assert!(!over.contains("limit") && !over.contains("today"), "{over}");
        assert!(refused("failed").1);
        for code in [
            hosted::OVER_QUOTA,
            hosted::NOT_ADMITTED,
            hosted::TOO_LARGE,
            "x",
        ] {
            assert_eq!(crate::eval_cards::jargon(&refused(code).0), None, "{code}");
        }
    }
}

/// The apps' hosted path against the deployed runner, from a fresh key:
/// a request for a test set the runner doesn't run, which it refuses
/// before anything runs (so it spends no quota). It shows the request is
/// signed, sent, answered, and bound the way the runner speaks. Run it
/// with the runner up:
///
/// ```sh
/// cargo test -p openagents-chat-app \
///   live_the_runner_answers_the_phone -- --ignored --nocapture
/// ```
#[cfg(test)]
#[test]
#[ignore = "network: needs the hosted eval runner on its relay"]
fn live_the_runner_answers_the_phone() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let world = SecretKey::new(&mut secp256k1::rand::rng());
    let live = Arc::new(Mutex::new(Live::default()));
    let offer: Value = serde_json::from_str(include_str!(
        "../../coder/fixtures/nip-cj/router-offer-start-eval.json"
    ))
    .unwrap();
    let started = std::time::Instant::now();
    runtime.block_on(HostedRelay::new(None, None).start(
        world,
        HostedRun {
            offer,
            draft: None,
            runs: 1,
            check: None,
        },
        live.clone(),
    ));
    let live = lock(&live).clone();
    eprintln!(
        "after {:?}: request {:?}, queued {}, outcome {:?}",
        started.elapsed(),
        live.request,
        live.queued,
        live.outcome.as_ref().map(|o| o.as_ref().err())
    );
    let (why, ours) = live.outcome.expect("an answer").expect_err("a refusal");
    assert!(!ours, "{why}");
    assert_eq!(why, refused(hosted::NOT_ADMITTED).0);
}
