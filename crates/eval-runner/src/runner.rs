//! The service: one signed request in, admission, the run or the publish,
//! and every answer back to the requester.
//!
//! [`Runner::handle`] takes each `25920` the relay delivers. A request
//! that doesn't open (a bad signature, another worker's, stale, or not
//! execution v1) is logged and dropped: nothing proves who sent it, so
//! there is no one to answer. An opened request is admitted or refused
//! with a typed reason before anything is reserved; an admitted one is
//! claimed in the ledger, persisted, acknowledged with `27020 accepted`,
//! and run. A retransmission gets the recorded answer and never a second
//! run.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::sync::{Arc, Mutex};

use coder::relay::liveness::{self, Liveness, Renewal, Successor};
use coder::relay::{Identity, Socket, send};
use ext_eval::arms::{AgentPin, Subject};
use ext_eval::artifact::{ArtifactRef, JSON};
use ext_eval::case::{Grant, LoadOptions};
use ext_eval::discover::Suite;
use ext_eval::run::{self, Author, Options, Progress as RunProgress, Setup};
use ext_eval::signal::Cancel;
use futures_util::StreamExt as _;
use nostr::cj_conversation::{SubjectSource, SuiteSource};
use nostr::contracts::{digest_bytes, jcs};
use nostr::domain::{Event, Tag};
use nostr::eval_ext::hosted::{self, Input, PublishOutput, RunInput, RunOutput};
use nostr::eval_ext::{self, EventPointer, Headline, Verdict};
use nostr::execution::{self, Admission, Body, Execute, Opened, Report, Seal, Service, Window};
use secp256k1::XOnlyPublicKey;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite;

use crate::catalog::{self, Catalog};
use crate::config::Config;
use crate::quota::{Quota, Ticket};
use crate::store::{Job, Store};
use crate::usage::{self, Record};
use crate::wire::{Blobs, Wire};
use crate::{Refusal, unix_now};

/// The largest `EVENT` message the runner sends, in bytes: under the
/// relay's 128 KiB message limit.
pub const MAX_MESSAGE_BYTES: usize = 125 * 1024;
/// Claims the ledger holds a slot for at once: running and queued suites
/// and publishes. Suites beyond [`crate::config::Limits::jobs`] wait their
/// turn, reported as `queued`.
pub const CLAIMS: u32 = 16;
/// Request IDs remembered to drop a relay's duplicate delivery.
const SEEN: usize = 1_024;

/// Mutable state, behind one lock.
struct State {
    service: Service,
    quota: Quota,
    /// A live run's stop switch, by claim key.
    running: BTreeMap<String, Cancel>,
    seen: VecDeque<String>,
    /// Counts ledger snapshots, so writes land in the order taken.
    ledger_generation: u64,
}

impl State {
    /// The ledger as of now and its generation, for
    /// [`Runner::persist_ledger`] outside the lock.
    fn ledger_snapshot(&mut self) -> (u64, Service) {
        self.ledger_generation += 1;
        (self.ledger_generation, self.service.clone())
    }
}

/// The hosted runner.
pub struct Runner {
    config: Config,
    identity: Arc<Identity>,
    catalog: Catalog,
    agent: AgentPin,
    wire: Arc<dyn Wire>,
    blobs: Arc<dyn Blobs>,
    store: Store,
    /// Every job, one line each (#10121).
    usage: usage::Log,
    state: Mutex<State>,
    /// Orders `service.json` writes (see [`crate::store::LedgerWriter`]).
    ledger: crate::store::LedgerWriter,
    suites: tokio::sync::Semaphore,
    /// Each catalog tool's NIP-EXT release, by its definition ID, once
    /// known.
    tool_releases: tokio::sync::Mutex<BTreeMap<String, Value>>,
    /// The defaults release last logged, so a change is logged once;
    /// `None` until the first read, which is always logged.
    defaults_seen: Mutex<Option<Option<String>>>,
}

/// What an admitted run will run.
struct Plan {
    _staging: tempfile::TempDir,
    suite: Suite,
    subject: Subject,
    author: String,
    package: String,
    component: String,
    suite_release: Option<Value>,
    runs: u32,
    check: Option<String>,
    validates: Option<String>,
    turns: u64,
    /// Coder's defaults at admission, admitted in both arms.
    defaults: Option<ext_eval::arms::Defaults>,
}

enum Admitted {
    Run(Box<Plan>),
    Publish(Box<Job>),
}

fn log(line: &str) {
    eprintln!("{line}");
}

fn short(id: &str) -> &str {
    &id[..id.len().min(12)]
}

fn random_hex() -> String {
    secp256k1::rand::random::<[u8; 32]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl Runner {
    /// A runner for `config`, as `identity`, over `wire` and `blobs`.
    ///
    /// # Errors
    ///
    /// Names what doesn't load: the catalog, the agent, or the state
    /// directory.
    pub fn new(
        config: Config,
        identity: Identity,
        wire: Arc<dyn Wire>,
        blobs: Arc<dyn Blobs>,
    ) -> Result<Arc<Self>, String> {
        let catalog = Catalog::load(&config.catalog)?;
        let agent = AgentPin::of(config.coder.clone())?.with_questions(&config.questions)?;
        let store = Store::open(&config.state).map_err(|error| error.to_string())?;
        let now = unix_now();
        let mut service = store.service(identity.pubkey(), CLAIMS, horizon(now));
        // A run that was dispatched when the runner stopped has an unknown
        // outcome: say so to a retransmission, and never run it again.
        let interrupted: Vec<String> = service
            .claims
            .values()
            .filter(|claim| claim.dispatch_intent && claim.outcome.is_none())
            .map(|claim| claim.key.clone())
            .collect();
        for key in &interrupted {
            let _ = service.crash(key, nostr::run::Boundary::Effect);
            if let Some(claim) = service.claims.get_mut(key) {
                claim.holds_slot = false;
            }
        }
        service.active = service
            .claims
            .values()
            .filter(|claim| claim.holds_slot)
            .count() as u32;
        store
            .save_service(&service)
            .map_err(|error| error.to_string())?;
        for mut job in store.jobs() {
            if job.status == "running" {
                job.status = "unknown".into();
                let _ = store.save_job(&job);
            }
        }
        let quota = Quota::open(config.state.join("quota.json"));
        let jobs = config.limits.jobs;
        let usage = usage::Log::new(config.usage_dir());
        Ok(Arc::new(Self {
            config,
            identity: Arc::new(identity),
            catalog,
            agent,
            wire,
            blobs,
            store,
            usage,
            state: Mutex::new(State {
                service,
                quota,
                running: BTreeMap::new(),
                seen: VecDeque::with_capacity(SEEN),
                ledger_generation: 0,
            }),
            ledger: crate::store::LedgerWriter::default(),
            suites: tokio::sync::Semaphore::new(jobs),
            tool_releases: tokio::sync::Mutex::new(BTreeMap::new()),
            defaults_seen: Mutex::new(None),
        }))
    }

    /// Coder's defaults as of now, read from the relay under the ledger's
    /// checks and resolved against the catalog
    /// ([`crate::defaults::read`]). A relay that can't be read, or a
    /// package with no release, means nothing admitted, and the run says
    /// so by naming no defaults. A change in the release is logged once.
    pub async fn defaults(&self) -> crate::defaults::Read {
        let releases = self.tool_releases.lock().await.clone();
        let read = match crate::defaults::read(
            self.wire.as_ref(),
            &self.config.defaults_root,
            self.config.defaults_documents.as_deref(),
            &self.catalog,
            &releases,
            unix_now(),
        )
        .await
        {
            Ok(read) => read,
            Err(why) => {
                log(&format!("defaults not read: {why}"));
                return crate::defaults::Read::default();
            }
        };
        let id = read.defaults.as_ref().map(|d| d.release.id.clone());
        let changed = {
            let mut seen = self.defaults_seen.lock().expect("the defaults log");
            let changed = seen.as_ref() != Some(&id);
            *seen = Some(id);
            changed
        };
        if changed {
            let names: BTreeMap<String, String> = self
                .catalog
                .tools
                .iter()
                .filter_map(|tool| {
                    releases
                        .get(&tool.definition.id)
                        .and_then(|r| r["id"].as_str())
                        .map(|id| (id.to_string(), tool.name.clone()))
                })
                .collect();
            log(&read.line(&names));
        }
        read
    }

    /// The runner's public key.
    #[must_use]
    pub fn pubkey(&self) -> &str {
        self.identity.pubkey()
    }

    /// The catalog it admits.
    #[must_use]
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// The agent both arms run.
    #[must_use]
    pub fn agent(&self) -> &AgentPin {
        &self.agent
    }

    /// The relay filter that delivers this runner's requests.
    #[must_use]
    pub fn filter(&self) -> Value {
        json!({"kinds": [execution::REQUEST_KIND], "#p": [self.pubkey()], "since": unix_now().saturating_sub(60)})
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Writes a ledger snapshot taken with [`State::ledger_snapshot`],
    /// in generation order.
    fn persist_ledger(&self, (generation, service): (u64, Service)) -> std::io::Result<()> {
        self.ledger.persist(&self.store, generation, &service)
    }

    fn save_service(&self, snapshot: (u64, Service)) {
        if let Err(error) = self.persist_ledger(snapshot) {
            log(&format!("ledger: {error}"));
        }
    }

    /// Seals `payload` to `to`, binds it to `event`, and publishes it.
    async fn answer(&self, to: &str, event: &str, kind: u16, payload: &Value) {
        let Ok(recipient) = XOnlyPublicKey::from_str(to) else {
            return;
        };
        let conversation = nostr::nip44::conversation_key(self.identity.secret(), &recipient);
        let seal = Seal {
            signer: self.identity.signer(),
            conversation,
            nonce: secp256k1::rand::random(),
            created_at: unix_now(),
        };
        let tags = vec![
            Tag::new(vec!["p".into(), to.into()]),
            Tag::new(vec!["e".into(), event.into()]),
        ];
        match seal.event(kind, tags, payload) {
            Ok(signed) => {
                if let Err(error) = self.wire.publish(signed).await {
                    log(&format!("answer to {}: {error}", short(event)));
                }
            }
            Err(error) => log(&format!("answer to {}: {error:?}", short(event))),
        }
    }

    /// Appends `record` to the usage log; a write that fails is logged
    /// and never stops a job.
    fn record(&self, record: &Record) {
        if let Err(error) = self.usage.append(record) {
            log(&format!("usage log: {error}"));
        }
    }

    async fn refuse(
        &self,
        arrived: Arrived,
        opened: &Opened,
        execute: &Execute,
        refusal: &Refusal,
    ) {
        log(&format!(
            "refused {} from {}: {refusal}",
            short(&opened.event_id),
            short(&opened.principal)
        ));
        let mut record = arrived.record(opened, "refused");
        described(&mut record, &execute.input);
        record.code = Some(refusal.code.clone());
        self.record(&record);
        let payload = execution::refusal_result(
            &execute.request,
            execute.attempt,
            &execute.run,
            &refusal.code,
            &refusal.message,
        );
        self.answer(
            &opened.principal,
            &opened.event_id,
            execution::RESULT_KIND,
            &payload,
        )
        .await;
    }

    /// Handles one event the relay delivered.
    pub async fn handle(self: &Arc<Self>, event: Event) {
        {
            let mut state = self.state();
            if state.seen.contains(&event.id) {
                return;
            }
            if state.seen.len() >= SEEN {
                state.seen.pop_front();
            }
            state.seen.push_back(event.id.clone());
        }
        let opened = match execution::open_request(
            &event,
            self.pubkey(),
            self.identity.secret(),
            unix_now(),
            Window::DEFAULT,
        ) {
            Ok(opened) => opened,
            Err(error) => {
                log(&format!("dropped {}: {error:?}", short(&event.id)));
                return;
            }
        };
        match opened.body.clone() {
            Body::Control { .. } => self.control(&opened).await,
            Body::Execute(execute) => self.execute(event, opened, *execute).await,
        }
    }

    async fn control(&self, opened: &Opened) {
        let Body::Control {
            request,
            attempt,
            control,
            ..
        } = &opened.body
        else {
            return;
        };
        let payload = {
            let mut state = self.state();
            let answer = state.service.control(opened);
            if matches!(control, execution::Control::Cancel { .. }) && answer.is_ok() {
                let key = state.service.key(&opened.principal, request, *attempt);
                if let Some(cancel) = state.running.get(&key) {
                    cancel.cancel();
                    log(&format!("cancelling {}", crate::store::key_name(&key)));
                }
            }
            let snapshot = state.ledger_snapshot();
            drop(state);
            self.save_service(snapshot);
            match answer {
                Ok(payload) => payload,
                Err(error) => execution::refusal_result(
                    request,
                    *attempt,
                    "",
                    error.code().unwrap_or("malformed"),
                    &format!("{error:?}"),
                ),
            }
        };
        self.answer(
            &opened.principal,
            &opened.event_id,
            execution::RESULT_KIND,
            &payload,
        )
        .await;
    }

    async fn execute(self: &Arc<Self>, event: Event, opened: Opened, execute: Execute) {
        let arrived = Arrived {
            unix_ms: usage::unix_ms(),
            bytes: event.content.len(),
        };
        // A retransmission: the recorded answer, bound to this event.
        let known = {
            let mut state = self.state();
            let key = state
                .service
                .key(&opened.principal, &execute.request, execute.attempt);
            state.service.claims.get_mut(&key).map(|claim| {
                if claim.fingerprint != execute.fingerprint {
                    Err(Refusal::new(
                        "idempotency_conflict",
                        "this request ID was used for a different request",
                    ))
                } else {
                    if !claim.aliases.contains(&opened.event_id) {
                        claim.aliases.push(opened.event_id.clone());
                    }
                    Ok(claim.result.clone())
                }
            })
        };
        match known {
            Some(Err(refusal)) => return self.refuse(arrived, &opened, &execute, &refusal).await,
            Some(Ok(Some(result))) => {
                return self
                    .answer(
                        &opened.principal,
                        &opened.event_id,
                        execution::RESULT_KIND,
                        &result,
                    )
                    .await;
            }
            // Still running: the result goes to every alias when it ends.
            Some(Ok(None)) => return,
            None => {}
        }
        let admitted = match self.admit(&opened, &execute).await {
            Ok(admitted) => admitted,
            Err(refusal) => return self.refuse(arrived, &opened, &execute, &refusal).await,
        };
        let now = unix_now();
        let mut ticket: Option<Ticket> = None;
        if let Admitted::Run(plan) = &admitted {
            let taken = self.state().quota.take(
                &opened.principal,
                plan.turns,
                plan.check.is_some(),
                now,
                &self.config.limits,
            );
            match taken {
                Ok(taken) => ticket = Some(taken),
                Err(refusal) => return self.refuse(arrived, &opened, &execute, &refusal).await,
            }
        }
        let mailbox = random_hex();
        let reserved = {
            let mut state = self.state();
            state.service.horizon = horizon(now);
            state.service.prepare(&opened, now, &mailbox)
        };
        let (claim, root) = match reserved {
            Ok(Admission::Reserved { claim, root }) => (claim, root),
            Ok(Admission::Retransmission(_)) => return,
            Err(error) => {
                if let Some(ticket) = &ticket {
                    self.state().quota.give_back(ticket);
                }
                let refusal =
                    Refusal::new(error.code().unwrap_or("malformed"), format!("{error:?}"));
                return self.refuse(arrived, &opened, &execute, &refusal).await;
            }
        };
        let key = claim.key.clone();
        let accepted = self
            .store
            .save_root(&key, &root)
            .map_err(|e| e.to_string())
            .and_then(|()| {
                let mut state = self.state();
                let accepted = state
                    .service
                    .acknowledge(&key, &root)
                    .map_err(|e| format!("{e:?}"));
                let intended = accepted.is_ok() && state.service.intend(&key).is_ok();
                let snapshot = state.ledger_snapshot();
                drop(state);
                self.persist_ledger(snapshot)
                    .map_err(|error| error.to_string())?;
                if intended {
                    accepted
                } else {
                    Err("the claim couldn't be dispatched".into())
                }
            });
        let accepted = match accepted {
            Ok(accepted) => accepted,
            Err(why) => {
                self.state().service.rollback(&key);
                if let Some(ticket) = &ticket {
                    self.state().quota.give_back(ticket);
                }
                return self
                    .refuse(
                        arrived,
                        &opened,
                        &execute,
                        &Refusal::new("unavailable", why),
                    )
                    .await;
            }
        };
        self.answer(
            &opened.principal,
            &opened.event_id,
            execution::FEEDBACK_KIND,
            &accepted,
        )
        .await;
        let this = Arc::clone(self);
        tokio::spawn(async move {
            match admitted {
                Admitted::Run(plan) => {
                    this.run(event, opened, key, *plan, ticket, arrived).await;
                }
                Admitted::Publish(job) => this.publish(event, opened, key, *job, arrived).await,
            }
        });
    }

    async fn admit(&self, opened: &Opened, execute: &Execute) -> Result<Admitted, Refusal> {
        hosted::check_target(&execute.target, self.pubkey()).map_err(|e| Refusal::contract(&e))?;
        hosted::check_requirements(&execute.requirements).map_err(|e| Refusal::contract(&e))?;
        let input = hosted::parse_input(&execute.input).map_err(|e| Refusal::contract(&e))?;
        if self.config.closed() {
            return Err(Refusal::not_admitted(
                "the hosted runner isn't taking new runs right now; try again later",
            ));
        }
        match input {
            Input::Publish { report } => self
                .store
                .run_with_report(&opened.principal, &report.digest)
                .map(|job| Admitted::Publish(Box::new(job)))
                .ok_or_else(|| Refusal::not_admitted("no finished run of yours has that report")),
            Input::Run(run) => self
                .plan(&opened.principal, *run)
                .await
                .map(|plan| Admitted::Run(Box::new(plan))),
        }
    }

    async fn plan(&self, requester: &str, run: RunInput) -> Result<Plan, Refusal> {
        let mut subject = match &run.subject {
            SubjectSource::Definition(definition) => self
                .catalog
                .find(definition)
                .map(|tool| tool.subject.clone())
                .ok_or_else(|| {
                    Refusal::not_admitted(
                        "the hosted runner tests catalog tools and tools made in chat only",
                    )
                })?,
            SubjectSource::Draft => {
                let draft = run
                    .draft
                    .as_ref()
                    .ok_or_else(|| Refusal::new("malformed", "the draft is missing"))?;
                self.catalog.draft_subject(&draft.tool, requester)?
            }
        };
        // A catalog tool is named by its release, so credit can find it.
        if let Some(tool) = self
            .catalog
            .tools
            .iter()
            .find(|tool| tool.subject.definition == subject.definition)
        {
            match self.tool_release(tool).await {
                Ok(event) => subject.definition["event"] = event,
                Err(why) => log(&format!("{} has no release: {why}", tool.name)),
            }
        }
        let staging = tempfile::Builder::new()
            .prefix("eval-runner-suite-")
            .tempdir_in(&self.config.temp_root)
            .map_err(|error| Refusal::new("unavailable", error.to_string()))?;
        let (eval_dir, author, package, component, suite_release) = match &run.suite {
            SuiteSource::Published(release) => {
                let found = self
                    .wire
                    .query(json!({"ids": [release.id], "kinds": [nostr::kinds::EXT_RELEASE]}))
                    .await
                    .map_err(|error| Refusal::new("unavailable", error))?;
                let event = found
                    .into_iter()
                    .find(|event| event.id == release.id && event.pubkey == release.pubkey)
                    .ok_or_else(|| {
                        Refusal::not_admitted("that test set's release isn't on the relay")
                    })?;
                let blobs = Arc::clone(&self.blobs);
                let into = staging.path().to_path_buf();
                let materialized = tokio::task::spawn_blocking(move || {
                    ext_eval::check::materialize(&event, &|digest| blobs.fetch(digest), &into)
                })
                .await
                .map_err(|error| Refusal::new("unavailable", error.to_string()))?
                .map_err(|error| {
                    Refusal::not_admitted(format!("that test set doesn't check: {error}"))
                })?;
                let loaded = Suite::load(&materialized.eval_dir, LoadOptions::default())
                    .map_err(|error| Refusal::new("malformed", error.to_string()))?;
                let (suite_bytes, _) = ext_eval::evaluate::suite_documents(
                    &loaded,
                    &materialized.author,
                    &materialized.package,
                    &materialized.component,
                )
                .map_err(|error| Refusal::new("unavailable", error.to_string()))?;
                if digest_bytes(&suite_bytes) != materialized.suite_digest {
                    return Err(Refusal::not_admitted(
                        "that test set was released under another version of the Gym's rules, so this runner can't reproduce it byte for byte",
                    ));
                }
                (
                    materialized.eval_dir,
                    materialized.author,
                    materialized.package,
                    materialized.component,
                    Some(materialized.release),
                )
            }
            SuiteSource::Draft => {
                let draft = run
                    .draft
                    .as_ref()
                    .ok_or_else(|| Refusal::new("malformed", "the draft is missing"))?;
                let evals = staging.path().join("evals");
                ext_eval::author::files::write(&evals, &draft.cases)
                    .map_err(|error| Refusal::new("malformed", error.to_string()))?;
                (
                    evals,
                    self.pubkey().to_string(),
                    format!("{}-tests", catalog::made_slug(&draft.tool.name)),
                    ext_eval::publish::SUITE_COMPONENT.to_string(),
                    None,
                )
            }
        };
        let suite = Suite::load(&eval_dir, LoadOptions::default())
            .map_err(|error| Refusal::new("malformed", error.to_string()))?;
        if suite.cases.is_empty() {
            return Err(Refusal::new("malformed", "the test set has no tests"));
        }
        if suite.cases.len() as u64 > eval_ext::HOSTED_MAX_CASES {
            return Err(Refusal::too_large(format!(
                "the hosted runner runs at most {} tests, and this set has {}",
                eval_ext::HOSTED_MAX_CASES,
                suite.cases.len()
            )));
        }
        let allowed = BTreeSet::from([Grant::Read, Grant::Write]);
        if let Some(case) = suite
            .cases
            .iter()
            .find(|case| !case.run.allowed_operations.is_subset(&allowed))
        {
            return Err(Refusal::not_admitted(format!(
                "the test {} asks to run commands or reach the network, which the hosted runner never allows",
                case.name
            )));
        }
        if let Some(check) = &run.check {
            let SuiteSource::Published(release) = &run.suite else {
                return Err(Refusal::not_admitted("a check reruns a published test set"));
            };
            self.checked(check, release).await?;
        }
        if let Some(validates) = &run.validates {
            let SuiteSource::Published(release) = &run.suite else {
                return Err(Refusal::not_admitted(
                    "a validation runs a published second test set",
                ));
            };
            self.validated(validates, release, &subject).await?;
        }
        let runs = u32::try_from(run.runs).unwrap_or(1);
        let turns = suite.cases.len() as u64 * u64::from(runs) * eval_ext::HOSTED_ARMS;
        // The defaults both arms admit, read at admission so a run holds
        // what was current when it was admitted.
        let defaults = self.defaults().await.arms;
        Ok(Plan {
            _staging: staging,
            suite,
            subject,
            author,
            package,
            component,
            suite_release,
            runs,
            check: run.check,
            validates: run.validates,
            turns,
            defaults,
        })
    }

    /// Checks that `validates` is a published result on the same tool
    /// and another test set than `release`: what a validation is. Whether
    /// the second suite is independent (another signer, released after
    /// the tool) is the reader's to decide from the two releases.
    async fn validated(
        &self,
        validates: &str,
        release: &EventPointer,
        subject: &Subject,
    ) -> Result<(), Refusal> {
        let found = self
            .wire
            .query(json!({"ids": [validates], "kinds": [nostr::kb::EVIDENCE_KIND]}))
            .await
            .map_err(|error| Refusal::new("unavailable", error))?;
        let original = found
            .iter()
            .find(|event| event.id == validates)
            .ok_or_else(|| Refusal::not_admitted("the result to validate isn't on the relay"))?;
        let publication = eval_ext::parse_publication(original).map_err(|error| {
            Refusal::not_admitted(format!("the result to validate doesn't check: {error}"))
        })?;
        if publication.suite_release.id == release.id {
            return Err(Refusal::not_admitted(
                "a validation runs a second test set; rerunning the result's own is a check",
            ));
        }
        if publication.report.subject.definition.id != subject.definition["id"] {
            return Err(Refusal::not_admitted(
                "the result to validate tested another tool",
            ));
        }
        Ok(())
    }

    /// Checks that `check` is a published result of the suite `release`.
    async fn checked(&self, check: &str, release: &EventPointer) -> Result<(), Refusal> {
        let found = self
            .wire
            .query(json!({"ids": [check], "kinds": [nostr::kb::EVIDENCE_KIND]}))
            .await
            .map_err(|error| Refusal::new("unavailable", error))?;
        let original = found
            .iter()
            .find(|event| event.id == check)
            .ok_or_else(|| Refusal::not_admitted("the result to check isn't on the relay"))?;
        let publication = eval_ext::parse_publication(original).map_err(|error| {
            Refusal::not_admitted(format!("the result to check doesn't check: {error}"))
        })?;
        if publication.suite_release.id != release.id {
            return Err(Refusal::not_admitted(
                "the result to check ran another test set",
            ));
        }
        Ok(())
    }

    /// Sends the terminal `report` for claim `key` to every event the
    /// requester sent for it.
    async fn finish(&self, principal: &str, key: &str, report: Report) {
        let (payload, aliases) = {
            let mut state = self.state();
            state.running.remove(key);
            let payload = state.service.observe(key, report);
            let aliases = state
                .service
                .claims
                .get(key)
                .map(|claim| claim.aliases.clone())
                .unwrap_or_default();
            let snapshot = state.ledger_snapshot();
            drop(state);
            self.save_service(snapshot);
            (payload, aliases)
        };
        match payload {
            Ok(payload) => {
                for alias in &aliases {
                    self.answer(principal, alias, execution::RESULT_KIND, &payload)
                        .await;
                }
            }
            Err(error) => log(&format!(
                "result for {}: {error:?}",
                crate::store::key_name(key)
            )),
        }
    }

    async fn progress(
        &self,
        principal: &str,
        key: &str,
        completed: u64,
        planned: u64,
        status: &str,
    ) {
        let (payload, event) = {
            let mut state = self.state();
            let seq = state
                .service
                .claims
                .get(key)
                .map_or(0, |claim| claim.progress.len() as u64);
            let payload = state.service.progress(key, seq, status);
            let event = state
                .service
                .claims
                .get(key)
                .map(|claim| claim.execute_event.clone());
            (payload, event)
        };
        let (Ok(mut payload), Some(event)) = (payload, event) else {
            return;
        };
        payload["meta"] = hosted::progress_meta(hosted::Progress { completed, planned });
        self.answer(principal, &event, execution::FEEDBACK_KIND, &payload)
            .await;
    }

    async fn run(
        self: Arc<Self>,
        event: Event,
        opened: Opened,
        key: String,
        plan: Plan,
        ticket: Option<Ticket>,
        arrived: Arrived,
    ) {
        let principal = opened.principal.clone();
        let mut record = arrived.record(&opened, "failed");
        record.action = action(plan.check.is_some(), plan.validates.is_some()).into();
        record.subject = plan.subject.definition["id"].as_str().map(str::to_string);
        record.suite = Some(
            plan.suite_release
                .as_ref()
                .and_then(|release| release["id"].as_str())
                .unwrap_or("draft")
                .to_string(),
        );
        record.tests = Some(plan.suite.cases.len() as u64);
        record.runs = Some(plan.runs);
        record.turns = Some(plan.turns);
        let cancel = Cancel::new();
        self.state().running.insert(key.clone(), cancel.clone());
        let mut job = Job {
            key: key.clone(),
            action: "run".into(),
            principal: principal.clone(),
            request: event.clone(),
            status: "running".into(),
            check: plan.check.clone(),
            validates: plan.validates.clone(),
            results: None,
            report: None,
            sealed: None,
            published: None,
        };
        let _ = self.store.save_job(&job);
        let planned = plan.turns;
        self.progress(&principal, &key, 0, planned, "queued").await;
        let Ok(_slot) = self.suites.acquire().await else {
            return;
        };
        log(&format!(
            "running {} for {}: {} tests, {} runs per arm{}",
            crate::store::key_name(&key),
            short(&principal),
            plan.suite.cases.len(),
            plan.runs,
            match (&plan.check, &plan.validates, &plan.defaults) {
                (Some(_), _, _) => ", a check",
                (None, Some(_), _) => ", a validation",
                (None, None, Some(_)) => ", marginal over the defaults",
                (None, None, None) => "",
            }
        ));
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<u64>();
        let forwarder = {
            let this = Arc::clone(&self);
            let (principal, key) = (principal.clone(), key.clone());
            tokio::spawn(async move {
                this.progress(&principal, &key, 0, planned, "running").await;
                while let Some(done) = receiver.recv().await {
                    this.progress(&principal, &key, done, planned, "running")
                        .await;
                }
            })
        };
        let results_base = self.store.job_dir(&key).join("results");
        let requester = json!({"id": event.id, "pubkey": event.pubkey, "kind": event.kind});
        let this = Arc::clone(&self);
        let stop = cancel.clone();
        let ran = tokio::task::spawn_blocking(move || {
            this.run_blocking(&plan, requester, &results_base, &stop, sender)
        })
        .await
        .unwrap_or_else(|error| Err(format!("the run's thread ended: {error}")));
        let _ = forwarder.await;
        let report = match ran {
            Ok(finished) if cancel.cancelled() => {
                job.status = "cancelled".into();
                job.results = Some(finished.results);
                Report {
                    outcome: "cancelled",
                    dispatched: true,
                    output: None,
                    artifacts: Vec::new(),
                    receipts: Vec::new(),
                    spend: None,
                    verification: Some("not_run"),
                    integration: Some("not_requested"),
                    code: None,
                    message: Some("the requester stopped the run".into()),
                }
            }
            Ok(finished) => match self.seal(&principal, &key, &finished).await {
                Ok(sealed) => {
                    let output = RunOutput {
                        report: nostr::contracts::parse_artifact(&finished.report_ref)
                            .expect("the runner's own report reference"),
                        sealed: sealed.clone(),
                        headline: finished.headline,
                        verdict: finished.verdict,
                        notes: finished.notes.clone(),
                    };
                    job.status = "completed".into();
                    job.results = Some(finished.results.clone());
                    job.report = Some(finished.report_ref.clone());
                    job.sealed = Some(sealed.to_value());
                    record.verdict = Some(finished.verdict.word().to_string());
                    record.subject_passed = Some(finished.headline.subject_passed);
                    record.baseline_passed = finished.headline.baseline_passed;
                    log(&format!(
                        "finished {}: {} of {} with the tool, {:?} without, {}",
                        crate::store::key_name(&key),
                        finished.headline.subject_passed,
                        finished.headline.total,
                        finished.headline.baseline_passed,
                        finished.verdict.word()
                    ));
                    Report {
                        outcome: "completed",
                        dispatched: true,
                        output: Some(hosted::run_output(&output)),
                        artifacts: vec![finished.report_ref.clone()],
                        receipts: Vec::new(),
                        spend: None,
                        verification: Some("not_run"),
                        integration: Some("not_requested"),
                        code: None,
                        message: None,
                    }
                }
                Err(why) => {
                    job.status = "failed".into();
                    failed(&why)
                }
            },
            Err(why) => {
                log(&format!(
                    "run {} failed: {why}",
                    crate::store::key_name(&key)
                ));
                if let Some(ticket) = &ticket {
                    self.state().quota.give_back(ticket);
                }
                job.status = "failed".into();
                failed(&why)
            }
        };
        let _ = self.store.save_job(&job);
        record.outcome = report.outcome.to_string();
        record.code.clone_from(&report.code);
        record.total_ms = usage::unix_ms().saturating_sub(arrived.unix_ms);
        self.record(&record);
        self.finish(&principal, &key, report).await;
    }

    fn run_blocking(
        &self,
        plan: &Plan,
        requester: Value,
        results_base: &Path,
        cancel: &Cancel,
        progress: tokio::sync::mpsc::UnboundedSender<u64>,
    ) -> Result<Finished, String> {
        let options = Options {
            runs: Some(plan.runs),
            baseline: true,
            concurrency: self.config.limits.concurrency,
            keep_temp: false,
            grants: BTreeSet::from([Grant::Read, Grant::Write]),
            temp_root: self.config.temp_root.clone(),
            backend: None,
            gate: None,
            defaults: plan.defaults.clone(),
        };
        let setup = Setup {
            suite: &plan.suite,
            subject: &plan.subject,
            agent: &self.agent,
            door: &self.config.door,
            decision: self.config.decision.as_ref(),
            options: &options,
        };
        let author = Author {
            author: plan.author.clone(),
            package: plan.package.clone(),
            component: plan.component.clone(),
            evaluator: self.pubkey().to_string(),
            suite_release: plan.suite_release.clone(),
            requester: Some(requester),
        };
        let jev = self
            .config
            .decision
            .as_ref()
            .and_then(|pin| pin.jev_door(None).ok());
        let done = std::sync::atomic::AtomicU64::new(0);
        let on_progress = |event: RunProgress| {
            if let RunProgress::Finished { .. } = event {
                let count = done.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                let _ = progress.send(count);
            }
        };
        let outcome = run::run_suite(
            &setup,
            &author,
            results_base,
            jev.as_ref().map(|door| door as &dyn ext_eval::DecisionDoor),
            cancel,
            &on_progress,
        )
        .map_err(|error| error.to_string())?;
        drop(progress);
        // The report the trainer holds and publishes is the report's
        // canonical bytes, which its sealed copy digests too.
        let path = outcome.results.join("report.json");
        let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        let canonical = jcs(&value).map_err(|error| error.to_string())?;
        std::fs::write(&path, &canonical).map_err(|error| error.to_string())?;
        let parsed = eval_ext::parse_report(&canonical).map_err(|error| error.to_string())?;
        let mut notes = ext_eval::notes(&outcome.evaluation.scores);
        notes.truncate(4);
        notes.retain(|note| note.chars().count() <= 200);
        Ok(Finished {
            results: outcome.results,
            report_ref: ArtifactRef::of(&canonical, JSON, Some(nostr::kb::REPORT_SCHEMA)).value(),
            report: value,
            headline: parsed.profile.headline,
            verdict: parsed.verdict,
            notes,
        })
    }

    /// Seals the finished report to the requester as a `3188` and
    /// publishes it.
    async fn seal(
        &self,
        principal: &str,
        key: &str,
        finished: &Finished,
    ) -> Result<EventPointer, String> {
        let recipient =
            XOnlyPublicKey::from_str(principal).map_err(|_| "the requester's key".to_string())?;
        let (mailbox, retain_until) = {
            let state = self.state();
            let claim = state.service.claims.get(key).ok_or("the claim is gone")?;
            (claim.mailbox.clone(), claim.retain_until)
        };
        let now = unix_now();
        let body = json!({
            "v": "openagents.artifact-envelope.v1",
            "requires": [],
            "artifact": finished.report_ref,
            "inline": finished.report,
            "issued_at": now,
            "retain_until": retain_until.max(now + 1),
        });
        let event = nostr::private_artifact::seal(
            &body,
            self.identity.secret(),
            &recipient,
            &mailbox,
            now,
            secp256k1::rand::random(),
        )
        .map_err(|error| format!("sealing the report: {error}"))?;
        self.wire
            .publish(event.clone())
            .await
            .map_err(|error| format!("publishing the sealed report: {error}"))?;
        Ok(EventPointer {
            id: event.id,
            pubkey: event.pubkey,
            kind: event.kind,
        })
    }

    async fn publish(
        self: Arc<Self>,
        _event: Event,
        opened: Opened,
        key: String,
        mut run: Job,
        arrived: Arrived,
    ) {
        let principal = opened.principal.clone();
        let mut record = arrived.record(&opened, "failed");
        record.action = "publish".into();
        let report = match self.publish_run(&mut run).await {
            Ok(output) => {
                record.result = Some(output.result.id.clone());
                log(&format!(
                    "published {} for {}: result {}",
                    crate::store::key_name(&run.key),
                    short(&principal),
                    short(&output.result.id)
                ));
                Report {
                    outcome: "completed",
                    dispatched: true,
                    output: Some(hosted::publish_output(&output)),
                    artifacts: Vec::new(),
                    receipts: Vec::new(),
                    spend: None,
                    verification: Some("not_run"),
                    integration: Some("not_requested"),
                    code: None,
                    message: None,
                }
            }
            Err(refusal) => {
                log(&format!(
                    "publish {} failed: {refusal}",
                    crate::store::key_name(&run.key)
                ));
                Report {
                    outcome: "failed",
                    dispatched: true,
                    output: None,
                    artifacts: Vec::new(),
                    receipts: Vec::new(),
                    spend: None,
                    verification: Some("not_run"),
                    integration: Some("not_requested"),
                    code: Some(refusal.code),
                    message: Some(refusal.message),
                }
            }
        };
        record.outcome = report.outcome.to_string();
        record.code.clone_from(&report.code);
        record.total_ms = usage::unix_ms().saturating_sub(arrived.unix_ms);
        self.record(&record);
        self.finish(&principal, &key, report).await;
    }

    /// Publishes a finished run: its suite's release, once, and the `3189`,
    /// once, carrying the trainer's signed request.
    async fn publish_run(&self, run: &mut Job) -> Result<PublishOutput, Refusal> {
        if let Some(published) = &run.published {
            return hosted::parse_publish_output(&hosted::publish_output(&PublishOutput {
                suite_release: pointer(&published["suite_release"])?,
                result: pointer(&published["result"])?,
            }))
            .map_err(|error| Refusal::contract(&error));
        }
        let results_dir = run
            .results
            .clone()
            .ok_or_else(|| Refusal::not_admitted("the run left no results"))?;
        let results = ext_eval::publish::Results::open(&results_dir)
            .map_err(|error| Refusal::new("unavailable", error.to_string()))?;
        let unavailable = |error: String| Refusal::new("unavailable", error);
        let (suite_release, report_bytes) = match results.suite_release() {
            Some(release) => (release, results.report.clone()),
            None => {
                let release = self.release(&results).await.map_err(unavailable)?;
                let bytes = ext_eval::publish::with_suite_release(&results.value, &release)
                    .map_err(|error| unavailable(error.to_string()))?;
                std::fs::write(results_dir.join("report.json"), &bytes)
                    .map_err(|error| unavailable(error.to_string()))?;
                (release, bytes)
            }
        };
        let text =
            String::from_utf8(report_bytes).map_err(|error| unavailable(error.to_string()))?;
        let cites = match (&run.check, &run.validates) {
            (Some(check), _) => Some(eval_ext::Cites::Check(check)),
            (None, Some(validates)) => Some(eval_ext::Cites::Validates(validates)),
            (None, None) => None,
        };
        let unsigned = eval_ext::hosted_publication_citing(&text, cites, &run.request)
            .map_err(|error| Refusal::contract(&error))?;
        let digest = digest_bytes(text.as_bytes());
        let existing = self
            .wire
            .query(json!({"kinds": [unsigned.kind], "authors": [self.pubkey()], "#x": [digest.trim_start_matches("sha256:")]}))
            .await
            .map_err(unavailable)?
            .into_iter()
            .find(|event| event.content == unsigned.content);
        let result = match existing {
            Some(event) => event,
            None => {
                let event = self.identity.signer().sign(
                    unix_now(),
                    unsigned.kind,
                    unsigned.tags,
                    unsigned.content,
                );
                let message = serde_json::to_string(&json!(["EVENT", event]))
                    .map_or(usize::MAX, |text| text.len());
                if message > MAX_MESSAGE_BYTES {
                    return Err(Refusal::too_large(
                        "the result is too large for the relay to hold",
                    ));
                }
                self.wire
                    .publish(event.clone())
                    .await
                    .map_err(unavailable)?;
                event
            }
        };
        eval_ext::parse_publication(&result).map_err(|error| Refusal::contract(&error))?;
        let output = PublishOutput {
            suite_release: pointer(&suite_release)?,
            result: EventPointer {
                id: result.id.clone(),
                pubkey: result.pubkey.clone(),
                kind: result.kind,
            },
        };
        run.published = Some(json!({
            "suite_release": output.suite_release.to_value(),
            "result": output.result.to_value(),
        }));
        let _ = self.store.save_job(run);
        Ok(output)
    }

    /// Releases a suite the runner authored (a chat draft's) as NIP-EXT
    /// `3184`: its files to the blob store, then the release, once.
    async fn release(&self, results: &ext_eval::publish::Results) -> Result<Value, String> {
        let release =
            ext_eval::publish::suite_release(results).map_err(|error| error.to_string())?;
        self.release_files(&release).await
    }

    /// Uploads a suite release's files and publishes the release, reusing
    /// one the relay already holds with the same content. Returns its
    /// `{id, pubkey, kind}`.
    ///
    /// # Errors
    ///
    /// The blob store's or the relay's refusal.
    pub async fn release_files(
        &self,
        release: &ext_eval::publish::SuiteRelease,
    ) -> Result<Value, String> {
        let blobs = Arc::clone(&self.blobs);
        let signer = self.identity.signer().clone();
        let files = release.blobs();
        tokio::task::spawn_blocking(move || {
            for (bytes, media) in files {
                blobs.upload(&signer, &bytes, &media)?;
            }
            Ok::<(), String>(())
        })
        .await
        .map_err(|error| error.to_string())??;
        let unsigned = release.event();
        let existing = self
            .wire
            .query(json!({"kinds": [unsigned.kind], "authors": [self.pubkey()], "#t": ["oa:ext:release:v1"]}))
            .await?
            .into_iter()
            .find(|event| event.content == unsigned.content);
        let event = match existing {
            Some(event) => event,
            None => {
                let event = self.identity.signer().sign(
                    unix_now(),
                    unsigned.kind,
                    unsigned.tags,
                    unsigned.content,
                );
                self.wire.publish(event.clone()).await?;
                event
            }
        };
        Ok(json!({"id": event.id, "pubkey": event.pubkey, "kind": event.kind}))
    }

    /// The NIP-EXT release of a catalog tool the runner publishes: the one
    /// on the relay with the same content, or a new one. A tool whose
    /// package names another publisher isn't the runner's to release.
    ///
    /// # Errors
    ///
    /// Why the release can't be found or sent.
    pub async fn tool_release(&self, tool: &catalog::Tool) -> Result<Value, String> {
        let mut known = self.tool_releases.lock().await;
        if let Some(event) = known.get(&tool.definition.id) {
            return Ok(event.clone());
        }
        if tool.package.publisher != self.pubkey() {
            return Err(format!(
                "its package names the publisher {}, not this runner",
                tool.package.publisher
            ));
        }
        let release = ext_eval::publish::extension_release(
            self.pubkey(),
            &tool.subject,
            &tool.package.version,
            &tool.record,
        )
        .map_err(|error| error.to_string())?;
        let event = self.release_files(&release).await?;
        known.insert(tool.definition.id.clone(), event.clone());
        Ok(event)
    }

    /// Releases every catalog tool this runner publishes, once, and
    /// returns each tool's name and release.
    ///
    /// # Errors
    ///
    /// The first release that can't be sent.
    pub async fn release_tools(&self) -> Result<Vec<(String, Value)>, String> {
        let mut out = Vec::new();
        for tool in &self.catalog.tools {
            if tool.package.publisher == self.pubkey() {
                out.push((tool.name.clone(), self.tool_release(tool).await?));
            }
        }
        Ok(out)
    }

    /// Releases the suite of a catalog extension directory as the runner
    /// (a starter test set): computes its `suite.json` and `cases.json` as a
    /// run writes them, and releases them under `<runner>:<package>/suite`.
    ///
    /// # Errors
    ///
    /// When the suite doesn't load or the release can't be sent.
    pub async fn release_extension_suite(
        &self,
        root: &Path,
        package: &str,
    ) -> Result<Value, String> {
        let suite = Suite::load(&root.join("evals"), LoadOptions::default())
            .map_err(|error| error.to_string())?;
        let (suite_bytes, cases_bytes) = ext_eval::evaluate::suite_documents(
            &suite,
            self.pubkey(),
            package,
            ext_eval::publish::SUITE_COMPONENT,
        )
        .map_err(|error| error.to_string())?;
        let results = ext_eval::publish::Results::of_suite(suite, suite_bytes, cases_bytes);
        let release =
            ext_eval::publish::suite_release(&results).map_err(|error| error.to_string())?;
        self.release_files(&release).await
    }
}

/// A finished run's report and what the result names.
/// When a request arrived and how long its ciphertext was, for its usage
/// record.
#[derive(Clone, Copy)]
struct Arrived {
    unix_ms: u64,
    bytes: usize,
}

impl Arrived {
    fn record(self, opened: &Opened, outcome: &str) -> Record {
        let mut record = Record::new(
            &opened.principal,
            &opened.event_id,
            self.bytes,
            self.unix_ms,
            outcome,
        );
        record.total_ms = usage::unix_ms().saturating_sub(self.unix_ms);
        record
    }
}

/// The usage log's word for a run: a check, a validation, or a run.
fn action(check: bool, validates: bool) -> &'static str {
    match (check, validates) {
        (true, _) => "check",
        (false, true) => "validation",
        (false, false) => "run",
    }
}

/// Fills in what a request asked, from its input, for a refusal's record.
fn described(record: &mut Record, input: &Value) {
    match hosted::parse_input(input) {
        Ok(Input::Publish { .. }) => record.action = "publish".into(),
        Ok(Input::Run(run)) => {
            record.action = action(run.check.is_some(), run.validates.is_some()).into();
            record.suite = Some(match &run.suite {
                SuiteSource::Published(release) => release.id.clone(),
                SuiteSource::Draft => "draft".into(),
            });
            record.subject = match &run.subject {
                SubjectSource::Definition(definition) => Some(definition.id.clone()),
                SubjectSource::Draft => None,
            };
            record.runs = u32::try_from(run.runs).ok();
        }
        Err(_) => {}
    }
}

struct Finished {
    results: PathBuf,
    report_ref: Value,
    report: Value,
    headline: Headline,
    verdict: Verdict,
    notes: Vec<String>,
}

fn failed(why: &str) -> Report {
    Report {
        outcome: "failed",
        dispatched: true,
        output: None,
        artifacts: Vec::new(),
        receipts: Vec::new(),
        spend: None,
        verification: Some("not_run"),
        integration: Some("not_requested"),
        code: Some("failed".into()),
        message: Some(why.chars().take(400).collect()),
    }
}

fn pointer(value: &Value) -> Result<EventPointer, Refusal> {
    Ok(EventPointer {
        id: value["id"].as_str().unwrap_or_default().to_string(),
        pubkey: value["pubkey"].as_str().unwrap_or_default().to_string(),
        kind: value["kind"]
            .as_u64()
            .and_then(|kind| u16::try_from(kind).ok())
            .ok_or_else(|| Refusal::new("malformed", "an event reference"))?,
    })
}

/// The latest `retain_until` the runner promises, as of `now`.
fn horizon(now: u64) -> u64 {
    now + hosted::DEADLINE_SECONDS + hosted::RETAIN_SECONDS + 86_400
}

/// The subscription that carries the runner's requests.
pub const JOBS: &str = "jobs";

/// Overrides the probe period (`coder::relay::liveness::PROBE_EVERY`), in
/// milliseconds.
pub const PROBE_VAR: &str = "EVAL_RUNNER_PROBE_MS";

/// Overrides the renewal period (`coder::relay::liveness::RENEW_EVERY`),
/// in milliseconds.
pub const RENEW_VAR: &str = "EVAL_RUNNER_RENEW_MS";

/// Why a connection ended.
#[derive(Debug)]
pub struct Ended {
    /// Whether the relay had confirmed the subscription on it, so a
    /// reconnect starts from the shortest wait again.
    pub subscribed: bool,
    /// What happened.
    pub why: String,
}

/// Hands one `EVENT` frame on the jobs subscription to the runner, from
/// whichever connection delivered it. [`Runner::handle`] drops an event it
/// has seen, and the ledger answers a retransmission with its recorded
/// answer, so a request delivered on two connections runs once.
fn deliver(runner: &Arc<Runner>, value: &Value) {
    if let Ok(event) = serde_json::from_value::<Event>(value[2].clone()) {
        let runner = Arc::clone(runner);
        tokio::spawn(async move { runner.handle(event).await });
    }
}

/// One connection: subscribe to this runner's requests and hand each to
/// the runner, until the connection fails.
///
/// The runner never trusts a quiet socket (#9946). `relay.openagents.com`
/// is a Cloud Run domain mapping, and when the relay instance restarts,
/// Google's front end can keep this connection established while nothing
/// reaches it. So every [`Liveness::probe`] the runner sends a probe (a
/// `REQ` with `limit` 0 that the relay answers with `EOSE`), and a probe
/// still unanswered at the next one ends the connection. Every
/// [`Liveness::renew`], before the relay's one-hour request timeout, a
/// successor connection subscribes first, then this one is closed and
/// read for [`liveness::DRAIN`] longer, so a request is never published
/// to neither. Each answered probe tells systemd's watchdog the runner is
/// live.
///
/// It returns only when the connection ends, saying why.
pub async fn listen(
    url: &str,
    identity: &Arc<Identity>,
    runner: &Arc<Runner>,
    liveness: Liveness,
) -> Ended {
    let mut subscribed = false;
    let ended = |subscribed: bool, why: String| Ended { subscribed, why };
    let jobs = || json!(["REQ", JOBS, runner.filter()]);
    let mut socket = match liveness::subscribe(url, identity, jobs()).await {
        Ok(socket) => socket,
        Err(why) => return ended(false, why),
    };
    let probe_every = liveness.probe;
    let mut probes =
        tokio::time::interval_at(tokio::time::Instant::now() + probe_every, probe_every);
    probes.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut probe_count: u64 = 0;
    // The probe the relay hasn't answered yet.
    let mut outstanding: Option<String> = None;
    let mut renew_at = tokio::time::Instant::now() + liveness.renew;
    let mut renewing: Option<Renewal> = None;
    // The replaced connection and when reading it stops.
    let mut draining: Option<(Socket, tokio::time::Instant)> = None;
    loop {
        tokio::select! {
            _ = probes.tick() => {
                if outstanding.is_some() {
                    return ended(subscribed, format!(
                        "the relay stopped answering: a liveness probe had no reply in {} s",
                        probe_every.as_secs_f64()
                    ));
                }
                probe_count += 1;
                let id = format!("{}{probe_count}", liveness::PROBE_PREFIX);
                if let Err(error) = send(&mut socket, liveness::probe_request(&id, runner.filter())).await {
                    return ended(subscribed, error.to_string());
                }
                outstanding = Some(id);
            }
            () = tokio::time::sleep_until(renew_at), if subscribed && renewing.is_none() => {
                renewing = Some(Box::pin(liveness::successor(url.to_string(), Arc::clone(identity), JOBS, jobs())));
            }
            made = liveness::poll_some(&mut renewing) => {
                renewing = None;
                match made {
                    Ok(Successor { socket: next, early }) => {
                        let mut old = std::mem::replace(&mut socket, next);
                        // The old subscription stops after the new one is
                        // live; whatever it delivered meanwhile is still
                        // read for a moment.
                        let _ = send(&mut old, json!(["CLOSE", JOBS])).await;
                        draining = Some((old, tokio::time::Instant::now() + liveness::DRAIN));
                        outstanding = None;
                        probes.reset();
                        renew_at = tokio::time::Instant::now() + liveness.renew;
                        log("renewed the requests subscription on a new connection");
                        liveness::notify_watchdog();
                        for value in &early {
                            deliver(runner, value);
                        }
                    }
                    Err(why) => {
                        // The current connection still proves itself with
                        // probes; try again after the next one.
                        log(&format!("relay: renewing the subscription failed: {why}; keeping the current connection"));
                        renew_at = tokio::time::Instant::now() + probe_every;
                    }
                }
            }
            frame = liveness::next_draining(&mut draining) => {
                match frame {
                    Some(Ok(tungstenite::Message::Text(text))) => {
                        let Ok(value) = serde_json::from_str::<Value>(&text) else { continue };
                        if value[1].as_str() != Some(JOBS) {
                            continue;
                        }
                        match value[0].as_str() {
                            Some("EVENT") => deliver(runner, &value),
                            Some("CLOSED") => draining = None,
                            _ => {}
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => draining = None,
                }
            }
            frame = socket.next() => {
                let Some(frame) = frame else {
                    return ended(subscribed, "the relay closed the socket".into());
                };
                let text = match frame {
                    Ok(tungstenite::Message::Text(text)) => text,
                    Ok(_) => continue,
                    Err(error) => return ended(subscribed, error.to_string()),
                };
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                // The answer to a liveness probe: the relay is there and
                // serving this connection.
                if outstanding.as_deref().is_some_and(|id| value[1].as_str() == Some(id)) {
                    match value[0].as_str() {
                        Some("EOSE") => {
                            let id = outstanding.take().unwrap_or_default();
                            if let Err(error) = send(&mut socket, json!(["CLOSE", id])).await {
                                return ended(subscribed, error.to_string());
                            }
                            liveness::notify_watchdog();
                        }
                        // A refused probe still came from the relay.
                        Some("CLOSED") => {
                            outstanding = None;
                            liveness::notify_watchdog();
                        }
                        _ => {}
                    }
                    continue;
                }
                if value[1].as_str() != Some(JOBS) {
                    continue;
                }
                match value[0].as_str() {
                    Some("EOSE") => {
                        log("subscribed; requests arrive live from here");
                        subscribed = true;
                        liveness::notify_watchdog();
                    }
                    Some("CLOSED") => {
                        return ended(subscribed, format!(
                            "the relay closed the subscription: {}",
                            value[2].as_str().unwrap_or_default()
                        ));
                    }
                    Some("EVENT") => deliver(runner, &value),
                    _ => {}
                }
            }
        }
    }
}
