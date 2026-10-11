//! `chat-goldens`: run the web chat's goldens (`bench/web-chat/goldens-v1.json`,
//! [`coder::chat_goldens`], `docs/web/chat-goldens.md`).
//!
//! ```sh
//! chat-goldens check                                  # offline, for CI
//! chat-goldens router                                 # Jev and the router here
//! chat-goldens http --base http://127.0.0.1:4301      # the site's own endpoints
//! chat-goldens http --base https://openagents.com --golden github.connect_repo
//! ```
//!
//! `http` sends each phrasing exactly as a visitor's browser does: it opens
//! the homepage for a fresh visitor cookie and the form's ticket, posts the
//! message to `/chat` (later turns to `/chat/{id}`), and reads
//! `/chat/{id}/transcript` until the reply is whole, timing the first
//! words and the whole reply. The reply's hidden marker names its tier,
//! route, and prepared answer. Each chat is deleted afterwards
//! (`--keep` keeps them). Cases run one at a time.
//!
//! `router` asks Jev (`TYPESAFE_API_KEY` or another decision profile) and
//! decides the tier as the worker does on the website, reading the product
//! notes when an embeddings key is set (`OPENAGENTS_PRODUCT_KB_EMBEDDINGS`,
//! as the worker). It reports routes, tiers, answers, and prepared text;
//! a model reply's text isn't written in this mode.
//!
//! Every run writes `report.json` and `report.md` to `--out` (default
//! `target/chat-goldens/<mode>-<unix seconds>/`). A run of the whole set
//! exits 1 when it misses the set's launch bar (`gate`: the share of cases
//! that must be right, slow ones counted, and no wrong case in a critical
//! flow; #11106); a filtered run exits 1 when any case fails. 2 is a usage
//! or setup error.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use coder::chat_goldens::{self, Case, Grade, Observed, Report, Set};
use coder::generate::{DEFAULT_DOOR_URL, Message, Role};
use coder::router::{self, Bank};

const USAGE: &str = "\
usage: chat-goldens check|router|http [options]

  check                 offline: every accepted answer's own text meets its golden
  router                Jev and the router in this process (TYPESAFE_API_KEY)
  http --base URL       the website's chat endpoints (a local server, or https://openagents.com)

options:
  --goldens PATH        the set (default bench/web-chat/goldens-v1.json, compiled in)
  --flow ID             only this flow (repeatable)
  --golden ID           only this golden (repeatable)
  --first N             only the first N cases
  --out DIR             where report.json and report.md go
  --keep                http: keep the chats instead of deleting them
  --pause-ms N          http: wait between cases (default 300)
";

struct Options {
    mode: String,
    base: Option<String>,
    goldens: Option<PathBuf>,
    flows: Vec<String>,
    only: Vec<String>,
    first: Option<usize>,
    out: Option<PathBuf>,
    keep: bool,
    pause: Duration,
}

fn options() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().ok_or_else(|| USAGE.to_string())?;
    if !matches!(mode.as_str(), "check" | "router" | "http") {
        return Err(USAGE.to_string());
    }
    let mut o = Options {
        mode,
        base: None,
        goldens: None,
        flows: vec![],
        only: vec![],
        first: None,
        out: None,
        keep: false,
        pause: Duration::from_millis(300),
    };
    while let Some(arg) = args.next() {
        let mut value = || {
            args.next()
                .ok_or_else(|| format!("{arg} needs a value\n\n{USAGE}"))
        };
        match arg.as_str() {
            "--base" => o.base = Some(value()?.trim_end_matches('/').to_string()),
            "--goldens" => o.goldens = Some(value()?.into()),
            "--flow" => o.flows.push(value()?),
            "--golden" => o.only.push(value()?),
            "--first" => o.first = Some(value()?.parse().map_err(|_| "--first takes a number")?),
            "--out" => o.out = Some(value()?.into()),
            "--keep" => o.keep = true,
            "--pause-ms" => {
                o.pause = Duration::from_millis(
                    value()?.parse().map_err(|_| "--pause-ms takes a number")?,
                );
            }
            "-h" | "--help" => return Err(USAGE.to_string()),
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }
    if o.mode == "http" && o.base.is_none() {
        return Err(format!("http needs --base URL\n\n{USAGE}"));
    }
    Ok(o)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

fn ms(since: Instant) -> u64 {
    u64::try_from(since.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn product_corpus() -> Result<knowledge::product::Corpus, String> {
    let root = knowledge::product::repository();
    knowledge::product::Corpus::load(
        &knowledge::product::default_dir(),
        root.join("knowledge").exists().then_some(&*root),
    )
}

#[tokio::main]
async fn main() -> ExitCode {
    let o = match options() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let set = match &o.goldens {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|e| format!("{}: {e}", path.display()))
            .and_then(|json| Set::parse(&json)),
        None => Ok(Set::fixture()),
    };
    let set = match set {
        Ok(set) => set,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let base_facts = router::worker_facts(
        coder::generate::DEFAULT_MODEL,
        Some(DEFAULT_DOOR_URL),
        &router::Seams::default(),
    );
    if o.mode == "check" {
        let corpus = match product_corpus() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("product notes: {e}");
                return ExitCode::from(2);
            }
        };
        let problems = chat_goldens::check(
            &set,
            Bank::builtin(),
            &chat_goldens::web_facts(&base_facts),
            &chat_goldens::note_answers(&corpus),
        );
        for problem in &problems {
            println!("{problem}");
        }
        println!(
            "{} cases in {} goldens: {}",
            set.cases().len(),
            set.flows.iter().map(|f| f.goldens.len()).sum::<usize>(),
            if problems.is_empty() {
                "every accepted answer meets its golden".to_string()
            } else {
                format!("{} problems", problems.len())
            }
        );
        return if problems.is_empty() {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        };
    }
    let mut cases: Vec<Case<'_>> = set
        .cases()
        .into_iter()
        .filter(|c| o.flows.is_empty() || o.flows.contains(&c.flow.id))
        .filter(|c| o.only.is_empty() || o.only.contains(&c.golden.id))
        .collect();
    if let Some(n) = o.first {
        cases.truncate(n);
    }
    let started = now();
    let (grades, target) = match o.mode.as_str() {
        "router" => match Routed::new(&base_facts) {
            Ok(routed) => {
                let target = routed.target.clone();
                let mut grades = Vec::new();
                for case in &cases {
                    let observed = routed.ask(case).await;
                    grades.push(progress(chat_goldens::grade(&set, case, observed)));
                }
                (grades, target)
            }
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(2);
            }
        },
        _ => {
            let base = o.base.clone().unwrap_or_default();
            let site = match Site::new(&base, o.keep) {
                Ok(site) => site,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::from(2);
                }
            };
            let mut grades = Vec::new();
            for (n, case) in cases.iter().enumerate() {
                if n > 0 {
                    tokio::time::sleep(o.pause).await;
                }
                let observed = site.ask(case).await;
                grades.push(progress(chat_goldens::grade(&set, case, observed)));
            }
            (grades, base)
        }
    };
    let report = Report::of(&set, &o.mode, &target, started, grades);
    let out = o.out.clone().unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/chat-goldens")
            .join(format!("{}-{started}", o.mode))
    });
    let markdown = report.markdown();
    let written = std::fs::create_dir_all(&out)
        .and_then(|()| {
            std::fs::write(
                out.join("report.json"),
                serde_json::to_vec_pretty(&report).unwrap_or_default(),
            )
        })
        .and_then(|()| std::fs::write(out.join("report.md"), &markdown));
    println!("\n{markdown}");
    match written {
        Ok(()) => println!("Wrote {}", out.join("report.md").display()),
        Err(e) => eprintln!("couldn't write the report to {}: {e}", out.display()),
    }
    // The set's launch bar decides when it has one and the whole set ran
    // (#11106); a filtered run, or a set without one, needs every case.
    let whole = o.flows.is_empty() && o.only.is_empty() && o.first.is_none();
    let ok = match (&report.gate, whole) {
        (Some(gate), true) => gate.met,
        _ => report.passed == report.cases,
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn progress(grade: Grade) -> Grade {
    let o = &grade.observed;
    let timing = o
        .first_ms
        .map(|f| format!("{f}/{} ms", o.total_ms.unwrap_or_default()))
        .or_else(|| o.judge_ms.map(|j| format!("judge {j} ms")))
        .unwrap_or_default();
    eprintln!(
        "{} {:<32} {:<10} {:<8} {:<34} {}{}",
        if grade.pass { "pass" } else { "FAIL" },
        grade.case,
        o.route.as_deref().unwrap_or("-"),
        o.tier.as_deref().unwrap_or("-"),
        o.answer.as_deref().unwrap_or("-"),
        timing,
        grade
            .failed()
            .map(|c| format!("  [{}: {}]", c.name, c.detail))
            .collect::<String>()
    );
    grade
}

/// Jev, the router, and the product notes, as the worker holds them for a
/// website turn.
struct Routed {
    judge: Arc<jev::Client>,
    bank: &'static Bank,
    facts: router::Facts,
    seams: router::Seams,
    /// The product notes, when an embeddings key is set.
    kb: Option<Arc<coder::product_kb::ProductKnowledge>>,
    tools: Vec<router::gym::Tool>,
    admitted: router::Admitted,
    plugin_names: Vec<String>,
    target: String,
    /// The worker's calibration maps (`CODER_WORKER_ROUTER_CALIBRATION`),
    /// Jev's and Clef's, applied by the model that answered.
    calibration: Option<router::calibration::Calibration>,
    clef_calibration: Option<router::calibration::Calibration>,
}

impl Routed {
    fn new(base: &router::Facts) -> Result<Self, String> {
        let judge = Arc::new(
            coder::decision::from_env()?
                .ok_or("router needs Jev: set TYPESAFE_API_KEY (or another decision profile)")?,
        );
        let mut seams = router::Seams::default();
        seams.cli = Arc::new(coder::cli_route::CommandRoute::new(
            (*judge).clone(),
            Arc::new(coder::cli_route::NoFill),
        ));
        let mut target = "jev".to_string();
        let kb = match coder::product_kb::ProductKnowledge::from_env(
            judge.clone() as Arc<dyn coder::product_kb::Judge>
        ) {
            Ok(kb) => {
                target.push_str(&format!(" + product notes ({})", kb.recipient()));
                let kb = Arc::new(kb);
                seams.product = kb.clone();
                Some(kb)
            }
            Err(why) => {
                eprintln!("product notes off ({why}): a knowledge turn reports tier grounded");
                None
            }
        };
        let corpus = product_corpus()?;
        let tools = coder::gym_kb::tools(&corpus);
        let admitted = router::Admitted::of(&tools, &[]);
        let facts = chat_goldens::web_facts(base);
        let bank = Bank::builtin();
        let calibration = router::calibration::Calibration::from_env(&bank.id())?;
        let clef_calibration = router::calibration::Calibration::clef_from_env(&bank.id())?;
        Ok(Routed {
            judge,
            bank,
            facts,
            seams,
            kb,
            tools,
            admitted,
            plugin_names: coder::builtin_plugins::names(),
            target,
            calibration,
            clef_calibration,
        })
    }

    async fn ask(&self, case: &Case<'_>) -> Observed {
        let mut transcript: Vec<Message> = case
            .golden
            .earlier
            .iter()
            .map(|text| Message {
                role: Role::User,
                text: text.clone(),
            })
            .collect();
        transcript.push(Message {
            role: Role::User,
            text: case.phrasing.to_string(),
        });
        let mut wire = serde_json::json!({"surface": "web"});
        if let Some(repository) = &case.golden.repository {
            wire["repository"] = repository.json();
        }
        let context = router::Context::of(&wire);
        // Jev reads what the worker gives it: the transcript with the
        // context's fixed lines (the project's repository, by name).
        let transcript = context.judged(&transcript);
        let situation = router::Situation {
            mode: router::Mode::Router,
            context: &context,
            personalize: true,
            draft: false,
            earlier: !case.golden.earlier.is_empty(),
            plugin: false,
        };
        let started = Instant::now();
        // The split router (#11193): the main request and its side
        // requests, asked at once.
        let request = router::split(
            case.phrasing,
            &transcript,
            self.bank,
            &self.facts,
            &self.seams.cli.groups(),
            &self.tools,
            &self.admitted,
            &[],
        );
        let request = if context.repository.is_some() {
            request.with_repository(case.phrasing, &transcript)
        } else {
            request
        };
        // Only the questions a rule reads on the website (#11247).
        let request = request.for_surface(context.surface());
        let response = match router::ask(&self.judge, request).await {
            Ok(r) => r,
            Err(e) => {
                return Observed {
                    error: Some(format!("Jev: {e}")),
                    judge_ms: Some(ms(started)),
                    ..Observed::default()
                };
            }
        };
        let mut routing = router::reading(&response, self.bank, &self.facts, &self.admitted);
        // As the worker reads it: each question on the map of the model
        // that answered it (Jev's, or Clef's when a Clef door answered).
        router::calibration::apply_served(
            &response,
            &mut routing,
            self.calibration.as_ref(),
            self.clef_calibration.as_ref(),
        );
        let tier = router::decide(&routing, self.bank, &self.facts, &situation);
        let mut why = format!("route {} {:.2}", routing.route.word(), routing.route_p);
        if let Some((second, p)) = routing.runner_up {
            why.push_str(&format!(" (then {} {p:.2})", second.word()));
        }
        match &routing.answer {
            Some((entry, p)) => why.push_str(&format!("; answer {} {p:.2}", entry.id)),
            None => why.push_str("; answer none"),
        }
        why.push_str(&format!("; specifics {:.2}", routing.needs_specifics));
        if let Some((ask, p)) = routing.repository {
            why.push_str(&format!("; repository {} {p:.2}", ask.word()));
        }
        let mut observed = Observed {
            route: Some(routing.route.word().to_string()),
            tier: Some(tier.word().to_string()),
            ..Observed::default()
        };
        match &tier {
            router::Tier::CannedFinal { answer, text, .. }
            | router::Tier::Capability { answer, text, .. } => {
                observed.answer = Some(answer.tag());
                // The answer as written, so its components are read too.
                observed.ui = Some(chat_goldens::ui_seen(text));
                observed.text = Some(if answer.plugins {
                    format!("{text}\n{}", self.plugin_names.join("\n"))
                } else {
                    text.clone()
                });
            }
            router::Tier::Refuse { text, .. } => observed.text = Some(text.clone()),
            router::Tier::Model { note, .. } => {
                observed.note = Some(
                    match note {
                        Some(note) if *note == router::policy::REPO_NOTE => "repo",
                        Some(note) if *note == router::policy::WEB_NOTE => "web",
                        Some(_) => "other",
                        None => "none",
                    }
                    .to_string(),
                );
            }
            router::Tier::CannedStem {
                answer,
                stem,
                generic_end,
                ..
            } => {
                observed.answer = Some(answer.tag());
                observed.text = Some(format!("{stem}{generic_end}"));
            }
            router::Tier::Grounded {
                corpus: router::Corpus::Product,
                ..
            } if self.kb.is_some() => {
                let lookup = router::seams::Lookup {
                    message: router::redact(case.phrasing),
                    transcript: transcript.clone(),
                };
                let kb = self.kb.as_ref().expect("the guard holds it");
                match kb.find(&lookup).await {
                    Ok(found) => {
                        match &found.answer {
                            Some((id, p)) => why.push_str(&format!("; whole {id} {p:.2}")),
                            None => why.push_str("; whole none"),
                        }
                        let found = found.grounding;
                        let mut top: Vec<&router::seams::Passage> = found.passages.iter().collect();
                        top.sort_by(|a, b| b.relevance.total_cmp(&a.relevance));
                        for passage in top.iter().take(2) {
                            why.push_str(&format!(
                                "; note {} {:.2}",
                                passage.id, passage.relevance
                            ));
                        }
                        if let router::Grounded::Answer(passage) =
                            router::grounded(&found, routing.needs_specifics, false, true)
                        {
                            observed.tier = Some("canned".into());
                            observed.answer = Some(passage.id.clone());
                            observed.text = passage.answer.clone();
                        }
                    }
                    Err(e) => observed.error = Some(format!("product notes: {e:?}")),
                }
            }
            _ => {}
        }
        observed.judge_ms = Some(ms(started));
        observed.why = Some(why);
        observed
    }
}

/// The website's chat, as a visitor's browser reaches it.
struct Site {
    base: String,
    client: reqwest::Client,
    keep: bool,
}

/// One visitor: the cookie the homepage set and the form's ticket.
struct Visitor {
    cookie: String,
    fields: BTreeMap<String, String>,
}

impl Site {
    fn new(base: &str, keep: bool) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .user_agent("openagents-chat-goldens/1")
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Site {
            base: base.to_string(),
            client,
            keep,
        })
    }

    async fn visitor(&self) -> Result<Visitor, String> {
        let response = self
            .client
            .get(format!("{}/", self.base))
            .send()
            .await
            .map_err(|e| format!("GET /: {e}"))?;
        let cookie = response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find_map(|v| {
                v.split(';')
                    .next()
                    .filter(|pair| pair.starts_with("oa_visitor="))
            })
            .map(str::to_string)
            .ok_or("GET / set no visitor cookie")?;
        let html = response.text().await.map_err(|e| format!("GET /: {e}"))?;
        let fields = hidden_fields(&html);
        if !fields.contains_key("csrf") {
            return Err("GET / has no chat form ticket".into());
        }
        Ok(Visitor { cookie, fields })
    }

    async fn post(
        &self,
        visitor: &Visitor,
        path: &str,
        form: &[(&str, &str)],
    ) -> Result<(), String> {
        let response = self
            .client
            .post(format!("{}{path}", self.base))
            .header(reqwest::header::COOKIE, &visitor.cookie)
            .header(reqwest::header::ORIGIN, &self.base)
            .header("Sec-Fetch-Site", "same-origin")
            .form(form)
            .send()
            .await
            .map_err(|e| format!("POST {path}: {e}"))?;
        let status = response.status();
        if status.is_success() || status.is_redirection() {
            Ok(())
        } else {
            let body = response.text().await.unwrap_or_default();
            Err(format!(
                "POST {path}: {status}: {}",
                plain(&body).chars().take(200).collect::<String>()
            ))
        }
    }

    async fn transcript(&self, visitor: &Visitor, id: &str) -> Result<String, String> {
        let response = self
            .client
            .get(format!("{}/chat/{id}/transcript", self.base))
            .header(reqwest::header::COOKIE, &visitor.cookie)
            .send()
            .await
            .map_err(|e| format!("GET transcript: {e}"))?;
        let status = response.status();
        let body = response.text().await.map_err(|e| e.to_string())?;
        if status.is_success() {
            Ok(body)
        } else {
            Err(format!("GET transcript: {status}"))
        }
    }

    /// Message `index` of chat `id` as it was written: the Markdown with
    /// any component blocks, which the page draws instead of showing.
    async fn original(&self, visitor: &Visitor, id: &str, index: usize) -> Result<String, String> {
        let path = format!("/chat/{id}/messages/{index}/original");
        let response = self
            .client
            .get(format!("{}{path}", self.base))
            .header(reqwest::header::COOKIE, &visitor.cookie)
            .send()
            .await
            .map_err(|e| format!("GET {path}: {e}"))?;
        let status = response.status();
        let body = response.text().await.map_err(|e| e.to_string())?;
        if status.is_success() {
            Ok(body)
        } else {
            Err(format!("GET {path}: {status}"))
        }
    }

    /// Sends one message and waits for its reply, the `nth` (from 1).
    async fn turn(
        &self,
        visitor: &Visitor,
        chat: &str,
        first: bool,
        text: &str,
        nth: usize,
    ) -> Observed {
        let request_id = if first { chat.to_string() } else { new_id() };
        let csrf = visitor.fields.get("csrf").cloned().unwrap_or_default();
        let selection = visitor.fields.get("selection").cloned().unwrap_or_default();
        let form = [
            ("q", text),
            ("request_id", request_id.as_str()),
            ("csrf", csrf.as_str()),
            ("selection", selection.as_str()),
            ("project", ""),
        ];
        let path = if first {
            "/chat".to_string()
        } else {
            format!("/chat/{chat}")
        };
        let started = Instant::now();
        if let Err(e) = self.post(visitor, &path, &form).await {
            return Observed {
                error: Some(e),
                ..Observed::default()
            };
        }
        let mut observed = Observed::default();
        let deadline = Instant::now() + Duration::from_secs(150);
        loop {
            let html = match self.transcript(visitor, chat).await {
                Ok(html) => html,
                Err(e) => {
                    observed.error = Some(e);
                    return observed;
                }
            };
            let working = working(&html);
            let replies = replies(&html);
            if let Some(reply) = replies.get(nth - 1) {
                if observed.first_ms.is_none() && !reply.text.is_empty() {
                    observed.first_ms = Some(ms(started));
                }
                if !working {
                    observed.total_ms = Some(ms(started));
                    observed.route = reply.route.clone();
                    observed.tier = reply.tier.clone();
                    observed.answer = reply.answer.clone();
                    if reply.text.is_empty() {
                        observed.error = Some(format!("no reply: {}", reply.after));
                    } else {
                        observed.text = Some(reply.text.clone());
                        // The reply as written, for the components it
                        // draws (#11113): the `nth` reply follows the
                        // `nth` message sent.
                        match self.original(visitor, chat, 2 * nth - 1).await {
                            Ok(source) => observed.ui = Some(chat_goldens::ui_seen(&source)),
                            Err(e) => eprintln!("{e}"),
                        }
                    }
                    return observed;
                }
            }
            if Instant::now() > deadline {
                observed.error = Some("no whole reply in 150 s".into());
                return observed;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    async fn ask(&self, case: &Case<'_>) -> Observed {
        if case.golden.repository.is_some() {
            // A visitor's chat has no project; the router mode asks it.
            return Observed {
                why: Some("a project chat: asked in router mode only".into()),
                ..Observed::default()
            };
        }
        let visitor = match self.visitor().await {
            Ok(v) => v,
            Err(e) => {
                return Observed {
                    error: Some(e),
                    ..Observed::default()
                };
            }
        };
        let chat = new_id();
        let mut nth = 0;
        let mut observed = Observed::default();
        for text in case
            .golden
            .earlier
            .iter()
            .map(String::as_str)
            .chain([case.phrasing])
        {
            nth += 1;
            observed = self.turn(&visitor, &chat, nth == 1, text, nth).await;
            if observed.error.is_some() {
                break;
            }
        }
        if !self.keep {
            let csrf = visitor.fields.get("csrf").cloned().unwrap_or_default();
            let _ = self
                .post(
                    &visitor,
                    &format!("/chat/{chat}/delete"),
                    &[("csrf", csrf.as_str())],
                )
                .await;
        }
        observed
    }
}

/// A UUID v4, as the website's own form makes one.
fn new_id() -> String {
    let mut b = secp256k1::rand::random::<[u8; 16]>();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// The value of `name="…"` / `value="…"` on each hidden input.
fn hidden_fields(html: &str) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    for tag in html.split("<input").skip(1) {
        let tag = tag.split('>').next().unwrap_or_default();
        if !tag.contains(r#"type="hidden""#) {
            continue;
        }
        if let (Some(name), Some(value)) = (attribute(tag, "name"), attribute(tag, "value")) {
            fields.entry(name).or_insert(value);
        }
    }
    fields
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let at = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    let end = tag[at..].find('"')? + at;
    Some(unescape(&tag[at..end]))
}

/// Whether the chat's status line says a reply is still on its way: the
/// `Busy` "Working" indicator inside `#chat-status`.
fn working(html: &str) -> bool {
    let Some(at) = html.find("id=\"chat-status\"") else {
        return false;
    };
    let status = &html[at..];
    let end = status.find("</div>").unwrap_or(status.len());
    status[..end].contains("oa-busy")
}

/// One reply between the page's reply markers.
struct Reply {
    tier: Option<String>,
    route: Option<String>,
    answer: Option<String>,
    text: String,
    /// What the page shows after an empty reply (its status line).
    after: String,
}

fn replies(html: &str) -> Vec<Reply> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(start) = rest.find("data-oa-reply=\"") {
        let open = &rest[start..];
        let tag_end = open.find('>').unwrap_or(open.len());
        let tag = format!(" {}", &open[..tag_end]);
        let body_start = (tag_end + 1).min(open.len());
        let end = open.find("data-oa-reply-end").unwrap_or(open.len());
        let body = &open[body_start.min(end)..end];
        let after_end = open[end..]
            .find("data-oa-reply=\"")
            .map_or(open.len(), |i| end + i);
        out.push(Reply {
            tier: attribute(&tag, "data-oa-tier"),
            route: attribute(&tag, "data-oa-route"),
            answer: attribute(&tag, "data-oa-answer"),
            text: with_links(&plain(body), body),
            after: plain(&open[end..after_end]).chars().take(200).collect(),
        });
        rest = &open[end.max(1)..];
    }
    out
}

/// `text` followed by the target of each link in `html` that the text does
/// not already show, site paths made absolute: a button or a linked label
/// reads as where it goes (#11187).
fn with_links(text: &str, html: &str) -> String {
    let mut out = text.to_owned();
    let mut rest = html;
    while let Some(at) = rest.find("<a ") {
        let tag = &rest[at..];
        let tag = &tag[..tag.find('>').unwrap_or(tag.len())];
        if let Some(href) = attribute(&format!(" {tag}"), "href") {
            let shown = if href.starts_with('/') {
                format!("https://openagents.com{href}")
            } else {
                href
            };
            if !out.contains(&shown) {
                out.push_str(&format!(" {shown}"));
            }
        }
        rest = &rest[at + 3..];
    }
    out
}

/// The visible words of an HTML fragment, entities decoded and spaces
/// joined.
fn plain(html: &str) -> String {
    let text = unescape(&oa_copy::visible_text(html));
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_reply_is_read_between_its_markers() {
        let html = r#"<div id="chat-message-0">how do i connect github repo</div>
<div id="chat-message-1"><span hidden data-oa-reply="1" data-oa-tier="canned" data-oa-route="meta" data-oa-answer="meta.github.website@1"></span><div class="md"><p>Open <strong>Projects</strong> &amp; connect GitHub. We&#39;re here.</p></div><span hidden data-oa-reply-end></span><div>Copy</div></div>
<div id="chat-status"></div>"#;
        let replies = replies(html);
        assert_eq!(replies.len(), 1);
        let r = &replies[0];
        assert_eq!(r.tier.as_deref(), Some("canned"));
        assert_eq!(r.route.as_deref(), Some("meta"));
        assert_eq!(r.answer.as_deref(), Some("meta.github.website@1"));
        assert_eq!(r.text, "Open Projects & connect GitHub. We're here.");
        let streaming = r#"<span hidden data-oa-reply="1"></span><p>Open</p><span hidden data-oa-reply-end></span>"#;
        let replies = super::replies(streaming);
        assert_eq!(
            (replies[0].tier.as_deref(), replies[0].text.as_str()),
            (None, "Open")
        );
    }

    #[test]
    fn working_is_the_busy_indicator_in_the_status_line() {
        let busy = r#"<div id="chat-status" class="oa-thread-status" role="status" aria-live="polite"><span class="oa-busy" role="status"><span class="oa-loading-indicator"></span><span class="oa-busy-text">Working</span></span></div>"#;
        assert!(working(busy));
        let done = r#"<div id="chat-status" class="oa-thread-status" role="status" aria-live="polite"></div><span class="oa-busy">elsewhere</span>"#;
        assert!(!working(done));
        assert!(!working("<p>no status line</p>"));
    }

    #[test]
    fn the_form_ticket_comes_from_the_homepage_inputs() {
        let html = r#"<input type="hidden" name="request_id" value="x" form="chat-form"><input id="composer-state" name="selection" type="hidden" form="chat-form" value="tok"><input type="hidden" name="csrf" value="abc&amp;d" form="chat-form">"#;
        let fields = hidden_fields(html);
        assert_eq!(fields["csrf"], "abc&d");
        assert_eq!(fields["selection"], "tok");
        let id = new_id();
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "4");
    }
}
