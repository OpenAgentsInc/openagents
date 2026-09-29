//! The benchmark: one in-process observer host over a temporary store, a
//! relay (the loopback fixture or a real `wss://` relay), and a loopback
//! direct listener, read by fresh phone clients.

use crate::dataset::{self, Scale};
use crate::phone;
use crate::stats::Phase;
use coder_connect::direct::{self, Change, Connection, HELLO, Welcome};
use coder_connect::host::Host;
use coder_connect::protocol::{Route, pubkey};
use coder_connect::transport::Link;
use coder_connect::{Client, ConnectionCode, Observation, Query, RelayPolicy, unix_time};
use coder_history::{CatalogRequest, Limits, TranscriptRequest};
use secp256k1::SecretKey;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncWriteExt, BufReader};

/// Where the history comes from.
#[derive(Clone, Debug)]
pub enum Source {
    /// Synthetic Coder task transcripts only: what a phone that shows only
    /// Coder chats reads.
    CoderFixture(usize),
    /// This machine's Coder task transcripts only.
    Coder,
    /// Synthetic Claude Code and Codex history.
    Fixture(Scale),
    /// This machine's own history, as `coder host serve` reads it.
    Real,
}

/// Which relay carries relay reads.
#[derive(Clone, Debug)]
pub enum Relay {
    /// The loopback NIP-42 fixture relay: no network.
    Fixture,
    /// A real relay, such as `wss://relay.openagents.com`, under the
    /// production relay policy. The host key is a fresh one in a temporary
    /// store, so no running host sees these reads.
    Url(String),
}

#[derive(Clone, Debug)]
pub struct Options {
    pub source: Source,
    pub relay: Relay,
    pub runs: usize,
    /// How many of the newest chats each run opens.
    pub chats: usize,
    pub use_relay: bool,
    pub use_direct: bool,
    /// Messages to send to the OpenAgents chat worker (the basic Coder),
    /// each from a new device key. Zero sends none.
    pub basic_coder: usize,
    /// Messages to send to a local `coder-worker` on its stub door, through
    /// the loopback relay: the protocol path with no model.
    pub basic_coder_local: usize,
    /// This benchmark's own executable, to measure a host process's first
    /// read in fresh processes.
    pub exe: Option<PathBuf>,
}

pub struct Report {
    pub dataset: String,
    pub facts: Vec<String>,
    pub phases: Vec<Phase>,
}

/// Every phase by name, created in first-seen order.
#[derive(Default)]
struct Phases(Vec<Phase>);
impl Phases {
    fn at(&mut self, group: &'static str, name: &str) -> &mut Phase {
        if let Some(index) = self
            .0
            .iter()
            .position(|p| p.group == group && p.name == name)
        {
            return &mut self.0[index];
        }
        self.0.push(Phase::new(group, name));
        self.0.last_mut().expect("just pushed")
    }
    fn add(&mut self, group: &'static str, name: &str, value: Duration) {
        self.at(group, name).samples.push(value);
    }
    fn note(&mut self, group: &'static str, name: &str, note: String) {
        self.at(group, name).note = note;
    }
}

const HOST: &str = "Host (in-process, no transport)";
const COLD: &str = "Host process cold start (fresh process per sample)";
const RELAY: &str = "Relay path";
const DIRECT: &str = "Direct (tailnet) path";
const PHONE: &str = "Phone-side CPU";
const TRANSCRIPT: &str = "Host transcript page cost by chat size";
const BASIC: &str = "Basic Coder send (NIP-CJ through relay.openagents.com)";

fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// A fresh device key paired for reading at `host`.
fn pair(
    host: &Host,
    relay: &str,
    config: &coder_history::Config,
) -> Result<(SecretKey, ConnectionCode), String> {
    let secret = SecretKey::new(&mut secp256k1::rand::rng());
    let now = unix_time().map_err(err)?;
    let code = host
        .pair(&pubkey(&secret), relay, config.clone(), now, now + 3600)
        .map_err(err)?;
    Ok((secret, code))
}

fn revoke(host: &Host, code: &ConnectionCode) {
    let _ = host.revoke(&code.grant, None, unix_time().unwrap_or_default());
}

/// Serve relay reads with the observer's real relay loop,
/// `coder_connect::cli::serve_observer`, as `coder host serve` runs it, and
/// wait until it answers.
async fn relay_host(
    state: PathBuf,
    relay: String,
    policy: RelayPolicy,
    config: &coder_history::Config,
) -> Result<(), String> {
    tokio::spawn(coder_connect::cli::serve_observer(
        Host::new(&state, policy),
        relay.clone(),
        policy,
    ));
    let host = Host::new(&state, policy);
    let (secret, code) = pair(&host, &relay, config)?;
    let client = Client::new_with_policy(code.clone(), secret, policy).map_err(err)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let read = tokio::time::timeout(
            Duration::from_secs(3),
            client.observe(Query::Catalog(CatalogRequest {
                cursor: None,
                limit: 1,
            })),
        )
        .await;
        if matches!(read, Ok(Ok(_))) {
            break;
        }
        if Instant::now() > deadline {
            return Err("the host's relay loop never answered".into());
        }
    }
    revoke(&host, &code);
    Ok(())
}

/// A loopback direct listener, as `coder_host::tailnet` serves it after its
/// tailnet admission checks.
async fn direct_host(state: PathBuf, policy: RelayPolicy) -> Result<SocketAddr, String> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(err)?;
    let address = listener.local_addr().map_err(err)?;
    let host = Arc::new(Host::new(&state, policy));
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let host = host.clone();
            tokio::spawn(async move {
                let _ = stream.set_nodelay(true);
                let (read, mut write) = stream.into_split();
                let mut reader = BufReader::new(read);
                let Ok(Some(hello)) = direct::line(&mut reader, direct::MAX_HELLO_BYTES).await
                else {
                    return;
                };
                if direct::Hello::parse(&hello).is_none() {
                    return;
                }
                let Ok(mut welcome) = serde_json::to_vec(&Welcome {
                    v: HELLO.into(),
                    refused: None,
                }) else {
                    return;
                };
                welcome.push(b'\n');
                if write.write_all(&welcome).await.is_err() {
                    return;
                }
                direct::serve(reader, write, host).await;
            });
        }
    });
    Ok(address)
}

/// Run the benchmark.
///
/// # Errors
/// When the dataset, host, relay, or a read fails.
pub async fn run(options: Options) -> Result<Report, String> {
    let temp = tempfile::tempdir().map_err(err)?;
    let config = match &options.source {
        Source::Fixture(scale) => {
            dataset::synthetic(&temp.path().join("home"), *scale).map_err(err)?
        }
        Source::Real => dataset::real().ok_or("this machine has no chat history to read")?,
        Source::CoderFixture(count) => {
            dataset::coder(&temp.path().join("home"), *count).map_err(err)?
        }
        Source::Coder => dataset::real_coder().ok_or("this machine has no Coder tasks")?,
    };
    let described = dataset::describe(&config);
    let mut phases = Phases::default();
    let mut facts = vec![];
    basic_coder(options.basic_coder, &mut phases, &mut facts).await;
    if options.basic_coder_local > 0 {
        local_basic_coder(&options, &mut phases, &mut facts).await?;
    }

    // Process-cold host reads, each in a fresh process.
    if let Some(exe) = &options.exe {
        for _ in 0..options.runs {
            cold_sample(exe, &config, &mut phases)?;
        }
    }

    // One relay, host store, and direct listener for the whole run.
    let _fixture;
    let (relay_url, policy) = match &options.relay {
        Relay::Fixture => {
            let (url, handle) = crate::relay::start().await;
            _fixture = Some(handle);
            (url, RelayPolicy::LoopbackTest)
        }
        Relay::Url(url) => {
            _fixture = None;
            (url.clone(), RelayPolicy::Production)
        }
    };
    let state = temp.path().join("observer");
    let host = Host::new(&state, policy);
    // The first pairing creates the host's key and store.
    let started = Instant::now();
    let (secret, code) = pair(&host, &relay_url, &config)?;
    facts.push(format!(
        "First pairing (store and key creation): {:.1} ms",
        started.elapsed().as_secs_f64() * 1000.0
    ));

    in_process(
        &options,
        &host,
        &config,
        &relay_url,
        (secret, &code),
        &mut phases,
    )?;
    revoke(&host, &code);

    if options.use_relay {
        relay_host(state.clone(), relay_url.clone(), policy, &config).await?;
    }
    let direct_at = if options.use_direct {
        Some(direct_host(state.clone(), policy).await?)
    } else {
        None
    };

    let mut opened_sources: Vec<String> = vec![];
    for run in 0..options.runs {
        if options.use_relay {
            let (secret, code) = pair(&host, &relay_url, &config)?;
            split_exchange((secret, &code), policy, &mut phases).await?;
            let sources =
                relay_run(&options, (secret, &code), policy, &mut phases, run == 0).await?;
            if opened_sources.is_empty() {
                opened_sources = sources;
            }
            revoke(&host, &code);
        }
        if let Some(address) = direct_at {
            let (secret, code) = pair(&host, &relay_url, &config)?;
            let sources =
                direct_run(&options, (secret, &code), policy, address, &mut phases).await?;
            if opened_sources.is_empty() {
                opened_sources = sources;
            }
            revoke(&host, &code);
        }
    }

    // What each opened chat's pages cost the host, by the chat's size.
    transcript_cost(&config, &opened_sources, options.runs, &mut phases)?;

    Ok(Report {
        dataset: described,
        facts,
        phases: phases.0,
    })
}

const LOCAL: &str = "Basic Coder send, local worker on its stub door (loopback relay, no model)";

/// Run `coder-worker` beside this benchmark on the loopback relay, with no
/// door key in its environment so it answers from its stub, and time the
/// same legs.
async fn local_basic_coder(
    options: &Options,
    phases: &mut Phases,
    facts: &mut Vec<String>,
) -> Result<(), String> {
    let worker_bin = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("coder-worker")))
        .filter(|path| path.is_file())
        .ok_or("build coder-worker beside this binary: cargo build -p coder --bin coder-worker --release")?;
    let (url, _relay) = crate::relay::start().await;
    let secret = SecretKey::new(&mut secp256k1::rand::rng());
    let mut child = tokio::process::Command::new(worker_bin)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("CODER_WORKER_SECRET", secret.display_secret().to_string())
        .env("CODER_RELAY", &url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(err)?;
    let worker = pubkey(&secret);
    // Let the worker connect and subscribe; a job sent before that is lost,
    // as on the real relay.
    tokio::time::sleep(Duration::from_secs(2)).await;
    for _ in 0..options.basic_coder_local {
        let legs = crate::cj::ask(
            &url,
            &worker,
            "In two short sentences, what is a Nostr relay?",
        )
        .await?;
        record(LOCAL, &legs, phases);
    }
    let _ = child.kill().await;
    facts.push("Local worker: coder-worker with no door key answers from its stub door.".into());
    Ok(())
}

/// Every leg of one basic Coder job, under `group`.
fn record(group: &'static str, legs: &crate::cj::Legs, phases: &mut Phases) {
    if let Some(failure) = &legs.failure {
        let phase = phases.at(group, "FAILED jobs");
        phase.samples.push(legs.done.unwrap_or(legs.subscribed));
        phase.note = failure.clone();
        return;
    }
    phases.add(
        group,
        "seal: NIP-44 encrypt + sign the 25900 request",
        legs.sealed,
    );
    phases.add(
        group,
        "relay connect: TLS/WebSocket + NIP-42 AUTH (a new connection per message)",
        legs.connected - legs.sealed,
    );
    phases.add(
        group,
        "subscribe for the answer: REQ to EOSE",
        legs.subscribed - legs.connected,
    );
    if let Some(accepted) = legs.accepted {
        phases.add(
            group,
            "request to the relay's OK",
            accepted - legs.subscribed,
        );
    }
    // A reply with no partial shows whole with its result.
    if let Some(first) = legs.first_words.or(legs.done) {
        phases.add(
            group,
            "request to first words: 27000 partial, else the result (relay, worker, model)",
            first - legs.subscribed,
        );
        phases.add(group, "Send to first words (before the host's poll)", first);
        if let Some(done) = legs.done {
            phases.add(group, "first words to the 26900 result", done - first);
        }
    }
    if let Some(done) = legs.done {
        phases.add(group, "Send to reply done", done);
        phases.note(
            group,
            "Send to reply done",
            format!("{} partials in the last job", legs.partials),
        );
    }
}

/// Send to the basic Coder and time each leg.
async fn basic_coder(count: usize, phases: &mut Phases, facts: &mut Vec<String>) {
    let mut models = std::collections::BTreeSet::new();
    for _ in 0..count {
        let legs = match crate::cj::ask(
            crate::cj::RELAY,
            crate::cj::WORKER,
            "In two short sentences, what is a Nostr relay?",
        )
        .await
        {
            Ok(legs) => legs,
            Err(error) => {
                facts.push(format!("Basic Coder: {error}"));
                continue;
            }
        };
        record(BASIC, &legs, phases);
        if let Some(model) = legs.model.clone() {
            models.insert(model);
        }
    }
    if !models.is_empty() {
        facts.push(format!(
            "Basic Coder model: {}",
            models.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
}

/// Sealing, answering, and opening one request with no transport, so each
/// cost stands alone.
fn in_process(
    options: &Options,
    host: &Host,
    config: &coder_history::Config,
    relay: &str,
    (secret, code): (SecretKey, &ConnectionCode),
    phases: &mut Phases,
) -> Result<(), String> {
    let started = Instant::now();
    let client = Client::new_with_policy(
        code.clone(),
        secret,
        if code.relay.starts_with("ws:") {
            RelayPolicy::LoopbackTest
        } else {
            RelayPolicy::Production
        },
    )
    .map_err(err)?;
    phases.add(
        PHONE,
        "Client::new (verify the pairing code)",
        started.elapsed(),
    );
    for index in 0..=options.runs {
        for route in [Route::Relay, Route::Direct] {
            let query = Query::Catalog(CatalogRequest {
                cursor: None,
                limit: route.limits().catalog_page,
            });
            let now = unix_time().map_err(err)?;
            let started = Instant::now();
            let pending = client.prepare_for(query, now, route).map_err(err)?;
            let sealed = started.elapsed();
            let started = Instant::now();
            let observation = match route {
                Route::Relay => {
                    let reply = host.handle(&pending.event, relay, now).map_err(err)?;
                    let answered = started.elapsed();
                    let started = Instant::now();
                    let observation = client.verify_reply(&pending, &reply, now).map_err(err)?;
                    (answered, started.elapsed(), observation)
                }
                Route::Direct => {
                    let handled = host.handle_direct(&pending.event).map_err(err)?;
                    let answered = started.elapsed();
                    let started = Instant::now();
                    let observation = client
                        .verify_detached(
                            &pending,
                            &handled.reply,
                            handled.payload.as_deref().unwrap_or_default(),
                            unix_time().map_err(err)?,
                        )
                        .map_err(err)?;
                    (answered, started.elapsed(), observation)
                }
            };
            let (answered, opened, observation) = observation;
            let Observation::Catalog(page) = observation else {
                return Err("a catalog read answered with a transcript page".into());
            };
            let label = match route {
                Route::Relay => "relay bounds, 32 chats",
                Route::Direct => "direct bounds, 256 chats",
            };
            if index == 0 {
                // The first read in this process: the memos start empty
                // unless the cold samples above ran here.
                phases.add(
                    HOST,
                    &format!("host answers catalog page 1, first in process ({label})"),
                    answered,
                );
                continue;
            }
            phases.add(
                HOST,
                &format!("phone seals request: sign + NIP-44 encrypt ({label})"),
                sealed,
            );
            phases.add(
                HOST,
                &format!("host answers catalog page 1, warm ({label})"),
                answered,
            );
            phases.add(
                HOST,
                &format!("phone opens reply: NIP-44 decrypt + verify ({label})"),
                opened,
            );
            phases.note(
                HOST,
                &format!("host answers catalog page 1, warm ({label})"),
                format!("{} entries", page.entries.len()),
            );
            // The history read alone, without the book, keys, or sealing.
            let started = Instant::now();
            let history = coder_history::History::open(config.clone()).map_err(err)?;
            history
                .catalog_within(
                    CatalogRequest {
                        cursor: None,
                        limit: route.limits().catalog_page,
                    },
                    route.limits(),
                )
                .map_err(err)?;
            phases.add(
                HOST,
                &format!("history scan + page only, warm ({label})"),
                started.elapsed(),
            );
            if route == Route::Relay {
                let list = phone::list(&page.entries);
                let started = Instant::now();
                let view = rust_native::View::new("chats:bench", 1, list)
                    .validate()
                    .map_err(|e| format!("{e:?}"))?;
                let json = serde_json::to_vec(view.view()).map_err(err)?;
                phases.add(
                    PHONE,
                    "chat list view: validate + JSON (one page of rows)",
                    started.elapsed(),
                );
                phases.note(
                    PHONE,
                    "chat list view: validate + JSON (one page of rows)",
                    format!("{} rows, {} KB", page.entries.len(), json.len() / 1024),
                );
            }
        }
    }
    // The phone's later catalog pages, straight from the history reader:
    // each is a full scan again.
    let history = coder_history::History::open(config.clone()).map_err(err)?;
    for _ in 0..options.runs {
        let mut cursor = None;
        for page in 1..=phone::CATALOG_PAGES {
            let started = Instant::now();
            let read = history.catalog_within(
                CatalogRequest {
                    cursor: cursor.take(),
                    limit: Limits::RELAY.catalog_page,
                },
                Limits::RELAY,
            );
            let name = format!("history catalog page {page} of a relay load (cursor), warm");
            match read {
                Ok(read) => {
                    phases.add(HOST, &name, started.elapsed());
                    match read.next {
                        Some(next) => cursor = Some(next),
                        None => break,
                    }
                }
                Err(error) => {
                    failed(phases, HOST, &name, started, &format!("{error:?}"));
                    break;
                }
            }
        }
    }
    Ok(())
}

const LEGS: &str = "Relay exchange, leg by leg";

/// Catalog exchanges by hand on one relay connection with one standing
/// reply subscription, as `coder_connect::transport::Link` makes them, with
/// each leg stamped.
async fn split_exchange(
    (secret, code): (SecretKey, &ConnectionCode),
    policy: RelayPolicy,
    phases: &mut Phases,
) -> Result<(), String> {
    use serde_json::json;
    let client = Client::new_with_policy(code.clone(), secret, policy).map_err(err)?;
    let started = Instant::now();
    let mut socket =
        nostr_transport::Connection::connect(&code.relay, &secret, Duration::from_secs(60))
            .await?
            .with_frame_budget(4096);
    phases.add(
        LEGS,
        "connect: TLS/WebSocket + NIP-42 AUTH",
        started.elapsed(),
    );
    let subscription = "bench-replies";
    let started = Instant::now();
    socket
        .send(json!(["REQ", subscription, {"kinds":[3188],"authors":[code.host],"#p":[code.client],"limit":0}]))
        .await?;
    loop {
        let frame = socket.next().await?;
        if frame[0] == "EOSE" && frame[1] == subscription {
            break;
        }
    }
    phases.add(
        LEGS,
        "standing reply subscription: REQ to EOSE, once per link",
        started.elapsed(),
    );
    for _ in 0..3 {
        let pending = client
            .prepare_for(
                Query::Catalog(CatalogRequest {
                    cursor: None,
                    limit: Route::Relay.limits().catalog_page,
                }),
                unix_time().map_err(err)?,
                Route::Relay,
            )
            .map_err(err)?;
        let sent = Instant::now();
        socket.send(json!(["EVENT", pending.event])).await?;
        let (mut ok, mut reply) = (None, None);
        while ok.is_none() || reply.is_none() {
            let frame = socket.next().await?;
            if frame[0] == "OK" && frame[1] == pending.event.id.as_str() {
                ok = Some(sent.elapsed());
            }
            if frame[0] == "EVENT"
                && frame[1] == subscription
                && frame[2]["tags"].as_array().is_some_and(|tags| {
                    tags.iter()
                        .any(|t| t[0] == "h" && t[1] == pending.request.request.as_str())
                })
            {
                reply = Some(sent.elapsed());
            }
        }
        phases.add(
            LEGS,
            "request EVENT to the relay's OK",
            ok.unwrap_or_default(),
        );
        phases.add(
            LEGS,
            "request EVENT to its reply (relay in, host answers, relay out)",
            reply.unwrap_or_default(),
        );
    }
    Ok(())
}

/// One relay run: the relay link's own cost, then the phone's list and chat
/// loads with a new client and again with the same one.
async fn relay_run(
    options: &Options,
    (secret, code): (SecretKey, &ConnectionCode),
    policy: RelayPolicy,
    phases: &mut Phases,
    first: bool,
) -> Result<Vec<String>, String> {
    // The client's relay link by hand, so its opening and each exchange on
    // it are timed apart.
    let client = Client::new_with_policy(code.clone(), secret, policy).map_err(err)?;
    let started = Instant::now();
    let link = Link::connect(&code.relay, &secret, policy, &code.host)
        .await
        .map_err(err)?;
    phases.add(
        RELAY,
        "relay link: connect + NIP-42 AUTH + standing subscription",
        started.elapsed(),
    );
    for name in [
        "catalog page 1 exchange on the link",
        "catalog page 1 exchange on the link again",
    ] {
        let pending = client
            .prepare_for(
                Query::Catalog(CatalogRequest {
                    cursor: None,
                    limit: Route::Relay.limits().catalog_page,
                }),
                unix_time().map_err(err)?,
                Route::Relay,
            )
            .map_err(err)?;
        let started = Instant::now();
        let reply = link.exchange(&pending).await.map_err(err)?;
        phases.add(RELAY, name, started.elapsed());
        client
            .verify_reply(&pending, &reply, unix_time().map_err(err)?)
            .map_err(err)?;
    }
    drop(link);

    // The phone's list load, as the app does it at launch: a new client.
    let client = Client::new_with_policy(code.clone(), secret, policy).map_err(err)?;
    let load = phone::catalog(&client).await?;
    phases.add(
        RELAY,
        "list: first page shows (new client, link opened by the first read)",
        load.first_page,
    );
    phases.add(
        RELAY,
        "list: load done (new client, link opened by the first read)",
        load.total,
    );
    let shown = load.chats.iter().filter(|c| phone::shown(c)).count();
    phases.note(
        RELAY,
        "list: load done (new client, link opened by the first read)",
        format!(
            "{} pages, {} chats, {shown} shown",
            load.pages,
            load.chats.len()
        ),
    );
    // A refresh on the same client reuses its link.
    let again = phone::catalog(&client).await?;
    phases.add(
        RELAY,
        "list: load done again (same client and link)",
        again.total,
    );
    // The app warms a computer's client when it comes to the foreground.
    let warmed = Client::new_with_policy(code.clone(), secret, policy).map_err(err)?;
    let started = Instant::now();
    warmed.warm().await;
    phases.add(
        RELAY,
        "Client::warm (link opened ahead of the first read)",
        started.elapsed(),
    );
    let load_warm = phone::catalog(&warmed).await?;
    phases.add(RELAY, "list: load done after Client::warm", load_warm.total);

    let sources = newest_sources(&load.chats, options.chats);
    for (index, source) in sources.iter().enumerate() {
        let started = Instant::now();
        let opened = match phone::open(&client, source).await {
            Ok(opened) => opened,
            Err(error) => {
                failed(
                    phases,
                    RELAY,
                    "chat open: FAILED (time until the error)",
                    started,
                    &error,
                );
                continue;
            }
        };
        let name = format!("chat #{index}: relay open, done");
        phases.add(TRANSCRIPT, &name, opened.total);
        phases.note(
            TRANSCRIPT,
            &name,
            format!(
                "{} pages, then {} in the background, {} KB read",
                opened.pages,
                opened.fill_pages,
                opened.bytes / 1024
            ),
        );
        phases.add(
            RELAY,
            "chat open: first page's rows show",
            opened.first_page,
        );
        phases.add(
            RELAY,
            "chat open: done (a page with rows; loading ends)",
            opened.total,
        );
        phases.add(
            RELAY,
            "chat open: earlier rows filled in the background (12 rows)",
            opened.filled,
        );
        let pages = phases.at(RELAY, "chat open: pages until done");
        pages
            .samples
            .push(Duration::from_millis(opened.pages as u64));
        pages.note = "values are page counts, not ms".into();
        if first {
            layout(index, &opened.rows, phases)?;
        }
    }
    Ok(sources)
}

/// One direct run: the connection's own cost, then the same loads.
async fn direct_run(
    options: &Options,
    (secret, code): (SecretKey, &ConnectionCode),
    policy: RelayPolicy,
    address: SocketAddr,
    phases: &mut Phases,
) -> Result<Vec<String>, String> {
    let client = Client::new_with_policy(code.clone(), secret, policy).map_err(err)?;
    let (changes, _) = tokio::sync::broadcast::channel::<Change>(64);
    let started = Instant::now();
    let connection = Connection::open(address, changes).await.map_err(err)?;
    phases.add(
        DIRECT,
        "direct connect: TCP + hello/welcome",
        started.elapsed(),
    );
    for name in [
        "catalog page 1 exchange (256 chats)",
        "catalog page 1 exchange again",
    ] {
        let now = unix_time().map_err(err)?;
        let pending = client
            .prepare_for(
                Query::Catalog(CatalogRequest {
                    cursor: None,
                    limit: Route::Direct.limits().catalog_page,
                }),
                now,
                Route::Direct,
            )
            .map_err(err)?;
        let started = Instant::now();
        let (reply, payload) = connection
            .exchange(&pending.event, Duration::from_secs(8))
            .await
            .map_err(err)?;
        let exchange = started.elapsed();
        client
            .verify_detached(
                &pending,
                &reply,
                payload.as_deref().unwrap_or_default(),
                unix_time().map_err(err)?,
            )
            .map_err(err)?;
        phases.add(DIRECT, name, exchange);
    }
    drop(connection);

    let client = Client::new_with_policy(code.clone(), secret, policy).map_err(err)?;
    client.set_direct(Some(address));
    let load = phone::catalog(&client).await?;
    phases.add(
        DIRECT,
        "list: first page shows (new client)",
        load.first_page,
    );
    phases.add(DIRECT, "list: load done (new client)", load.total);
    phases.note(
        DIRECT,
        "list: load done (new client)",
        format!("{} pages, {} chats", load.pages, load.chats.len()),
    );
    let again = phone::catalog(&client).await?;
    phases.add(
        DIRECT,
        "list: load done again (same connection)",
        again.total,
    );
    let sources = newest_sources(&load.chats, options.chats);
    for (index, source) in sources.iter().enumerate() {
        let started = Instant::now();
        let opened = match phone::open(&client, source).await {
            Ok(opened) => opened,
            Err(error) => {
                failed(
                    phases,
                    DIRECT,
                    "chat open: FAILED (time until the error)",
                    started,
                    &error,
                );
                continue;
            }
        };
        let name = format!("chat #{index}: direct open, done");
        phases.add(TRANSCRIPT, &name, opened.total);
        phases.note(
            TRANSCRIPT,
            &name,
            format!(
                "{} pages, then {} in the background, {} KB read",
                opened.pages,
                opened.fill_pages,
                opened.bytes / 1024
            ),
        );
        phases.add(
            DIRECT,
            "chat open: first page's rows show",
            opened.first_page,
        );
        phases.add(DIRECT, "chat open: done", opened.total);
        phases.add(
            DIRECT,
            "chat open: earlier rows filled in the background",
            opened.filled,
        );
        let pages = phases.at(DIRECT, "chat open: pages until done");
        pages
            .samples
            .push(Duration::from_millis(opened.pages as u64));
        pages.note = "values are page counts, not ms".into();
    }
    Ok(sources)
}

/// Count a failed load and how long it took to fail.
fn failed(phases: &mut Phases, group: &'static str, name: &str, started: Instant, error: &str) {
    phases.add(group, name, started.elapsed());
    let phase = phases.at(group, name);
    if !phase.note.contains(error) {
        if !phase.note.is_empty() {
            phase.note.push_str("; ");
        }
        phase.note.push_str(error);
    }
}

/// The source IDs of the newest shown chats, as the list orders them.
fn newest_sources(chats: &[coder_history::Chat], count: usize) -> Vec<String> {
    let mut shown: Vec<&coder_history::Chat> = chats.iter().filter(|c| phone::shown(c)).collect();
    shown.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    shown
        .into_iter()
        .filter_map(|c| c.source_id.clone())
        .take(count)
        .collect()
}

/// Lay out one opened chat's rows as the phone's transcript layout does:
/// first with a new layout and shaper, then again from its cache.
fn layout(index: usize, rows: &[phone::Row], phases: &mut Phases) -> Result<(), String> {
    use rust_native::layout::{TranscriptLayout, Update, shape::ShapingMeasurer};
    let node = phone::transcript(rows);
    let started = Instant::now();
    let mut measurer = ShapingMeasurer::new();
    phases.add(
        PHONE,
        "layout: ShapingMeasurer::new (load bundled fonts)",
        started.elapsed(),
    );
    let mut layout = TranscriptLayout::new();
    let update = Update::from_transcript(&node, 390.0, 1.0).ok_or("not a transcript")?;
    let started = Instant::now();
    let summary = layout
        .update(update.clone(), &mut measurer)
        .map_err(|e| format!("{e:?}"))?;
    phases.add(
        PHONE,
        "layout: first layout of an opened chat",
        started.elapsed(),
    );
    let name = "layout: first layout of an opened chat";
    let note = phases.at(PHONE, name);
    if !note.note.is_empty() {
        note.note.push_str(", ");
    }
    note.note
        .push_str(&format!("#{index}: {} rows", summary.count));
    let started = Instant::now();
    let mut again = TranscriptLayout::new();
    // The same rows at a new width: measured again, with the shaper warm.
    let mut wider = update;
    wider.width = 430.0;
    again
        .update(wider, &mut measurer)
        .map_err(|e| format!("{e:?}"))?;
    phases.add(
        PHONE,
        "layout: same chat, warm shaper, new width",
        started.elapsed(),
    );
    Ok(())
}

/// Each opened chat's first two backward pages at relay bounds, straight
/// from the history reader: its cost grows with the chat's size.
fn transcript_cost(
    config: &coder_history::Config,
    sources: &[String],
    runs: usize,
    phases: &mut Phases,
) -> Result<(), String> {
    let history = coder_history::History::open(config.clone()).map_err(err)?;
    for (index, source) in sources.iter().enumerate() {
        let size = history.source_length(source).map_or(0, |(_, len)| len);
        let name = format!(
            "chat #{index} ({:.1} MB): backward page 1 and 2",
            size as f64 / 1e6
        );
        for _ in 0..runs.max(1) {
            let started = Instant::now();
            let first = history
                .transcript_within(
                    TranscriptRequest {
                        source_id: source.clone(),
                        cursor: None,
                        max_bytes: phone::page_bytes(Route::Relay),
                        end: Some(coder_history::NEWEST),
                    },
                    Limits::RELAY,
                )
                .map_err(err)?;
            if let Some(earlier) = first.previous {
                history
                    .transcript_within(
                        TranscriptRequest {
                            source_id: source.clone(),
                            cursor: None,
                            max_bytes: phone::page_bytes(Route::Relay),
                            end: Some(earlier),
                        },
                        Limits::RELAY,
                    )
                    .map_err(err)?;
            }
            phases.add(TRANSCRIPT, &name, started.elapsed());
        }
    }
    Ok(())
}

/// Time a host process's first reads in a fresh process.
fn cold_sample(
    exe: &Path,
    config: &coder_history::Config,
    phases: &mut Phases,
) -> Result<(), String> {
    let mut command = std::process::Command::new(exe);
    command.arg("internal-cold");
    for (flag, path) in [
        ("--codex", &config.codex),
        ("--claude", &config.claude),
        ("--coder", &config.coder),
        ("--opencode", &config.opencode),
        ("--devin", &config.devin),
    ] {
        if let Some(path) = path {
            command.arg(flag).arg(path);
        }
    }
    let output = command.output().map_err(err)?;
    if !output.status.success() {
        return Err(format!(
            "the cold sample failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let sample: ColdSample = serde_json::from_slice(&output.stdout).map_err(err)?;
    let us = Duration::from_micros;
    phases.add(COLD, "History::open (open roots)", us(sample.open_us));
    phases.add(
        COLD,
        "catalog page 1, first read (scan + every head)",
        us(sample.first_us),
    );
    phases.add(
        COLD,
        "catalog page 1, second read (memos warm)",
        us(sample.second_us),
    );
    phases.add(
        COLD,
        "catalog page 2, warm (a full scan again)",
        us(sample.page2_us),
    );
    phases.note(
        COLD,
        "catalog page 1, first read (scan + every head)",
        format!("a 256-chat page held {}", sample.entries),
    );
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ColdSample {
    pub open_us: u64,
    pub first_us: u64,
    pub second_us: u64,
    pub page2_us: u64,
    pub entries: usize,
}

/// The `internal-cold` subcommand: a host process's first catalog reads.
///
/// # Errors
/// When the history cannot be read.
pub fn cold(config: coder_history::Config) -> Result<ColdSample, String> {
    let micros = |d: Duration| u64::try_from(d.as_micros()).unwrap_or(u64::MAX);
    let started = Instant::now();
    let history = coder_history::History::open(config).map_err(err)?;
    let open_us = micros(started.elapsed());
    let request = CatalogRequest {
        cursor: None,
        limit: Limits::RELAY.catalog_page,
    };
    let started = Instant::now();
    let first = history
        .catalog_within(request.clone(), Limits::RELAY)
        .map_err(err)?;
    let first_us = micros(started.elapsed());
    let started = Instant::now();
    history
        .catalog_within(request, Limits::RELAY)
        .map_err(err)?;
    let second_us = micros(started.elapsed());
    let started = Instant::now();
    if let Some(next) = first.next.clone() {
        history
            .catalog_within(
                CatalogRequest {
                    cursor: Some(next),
                    limit: Limits::RELAY.catalog_page,
                },
                Limits::RELAY,
            )
            .map_err(err)?;
    }
    let page2_us = micros(started.elapsed());
    // Count everything a scan lists: the direct-sized page's entries.
    let all = history
        .catalog_within(
            CatalogRequest {
                cursor: None,
                limit: Limits::DIRECT.catalog_page,
            },
            Limits::DIRECT,
        )
        .map_err(err)?;
    Ok(ColdSample {
        open_us,
        first_us,
        second_us,
        page2_us,
        entries: all.entries.len(),
    })
}
