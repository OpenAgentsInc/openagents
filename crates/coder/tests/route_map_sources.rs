//! The route map's sources (#10085), built from the router's own code and
//! data and pinned against the committed snapshot the phone and the
//! desktop read (`crates/openagents-chat-app/src/route_map/sources.json`).
//!
//! ```sh
//! cargo test -p coder --test route_map_sources               # check
//! ROUTE_MAP_WRITE=1 cargo test -p coder --test route_map_sources  # rewrite
//! ```
//!
//! A change to the routes, the rubric, the bank, the labeled set, the
//! knowledge, the capability registry, the engines, the decks, the
//! screens, or a plugin's parts fails the check until the snapshot is
//! rewritten, so the map never shows a hand-copied list.
//!
//! `live_route_map_records` (ignored: it reads the relay) writes the
//! published records the map reads beside it (`records.json`):
//!
//! ```sh
//! ROUTE_MAP_WRITE=1 cargo test -p coder --test route_map_sources live_route_map_records -- --ignored
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use coder::router::{self, Bank, CodingEngine, RouteId};
use openagents_chat_app::route_map::records::{
    AdoptionRecord, Records, ReleaseRecord, ResultRecord, Verdict,
};
use openagents_chat_app::route_map::sources::{
    AnswerOffer, AnswerSource, CapabilitySource, CodebaseKb, CodingKnowledge, DeckSource,
    EngineSource, Knowledge, KnowledgeSource, Labeled, Measurement, PluginSource, ProductKb,
    Question, RouteRows, RouteScore, RouteSource, SCHEMA, Sources, TestSetSource,
};
use serde_json::Value;

/// The latest committed per-route router record and the page that
/// explains it.
const MEASUREMENT: (&str, &str) = (
    "docs/coder/measurements/2026-10-01-essays-summary-claims",
    "docs/coder/measurements/2026-10-01-essays-route.md",
);

/// The product knowledge base's question set record.
const PRODUCT_KB: &str = "docs/coder/measurements/2026-09-28-product-kb.json";

/// The codebase route's question set.
const CODEBASE_KB: &str = "crates/coder/fixtures/chat-router/codebase-questions-v1.json";

/// The labeled route set.
const LABELED: &str = "crates/coder/fixtures/chat-router/routes-v4.json";

/// The plugin the map shows as the example to copy, by directory, when it
/// is in this repository. None today: the old example, Explain this error,
/// is one of the hosted runner's sample plugins, which are never shown.
const SHOWCASE: &[&str] = &[];

/// Directories under `crates/` named `plugin-*` that are not plugins: the
/// guest development kit.
const NOT_PLUGINS: &[&str] = &["crates/plugin-pdk"];

/// Directories under `packages/` that are not plugins: the set of
/// adopted plugins itself.
const NOT_PACKAGES: &[&str] = &["packages/coder-defaults"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository")
}

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn json(path: &str) -> Value {
    serde_json::from_str(&read(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn rel(path: &Path) -> String {
    path.strip_prefix(root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn routes() -> Vec<RouteSource> {
    RouteId::ALL
        .into_iter()
        .map(|route| {
            let rubric = router::rubric::route(route);
            RouteSource {
                id: route.word().to_string(),
                family: route
                    .family()
                    .expect("every offered route has a family")
                    .word()
                    .to_string(),
                description: route.description().to_string(),
                what: rubric["what"].as_str().unwrap_or_default().to_string(),
                not_for: rubric["not_for"].as_str().map(str::to_string),
                examples: rubric["examples"].as_array().map_or(0, Vec::len),
            }
        })
        .collect()
}

fn answers(bank: &Bank) -> Vec<AnswerSource> {
    bank.answers
        .iter()
        .map(|entry| AnswerSource {
            id: entry.id.clone(),
            version: entry.version,
            routes: entry.routes.clone(),
            chip: entry.chip.clone(),
            place: format!("{:?}", entry.place).to_lowercase(),
            records: entry.records,
            offer: entry.offer.as_ref().map(|offer| AnswerOffer {
                run_coder: offer.run_coder,
                screen: offer.screen.clone(),
                label: offer.label.clone(),
            }),
            sources: entry.sources.clone(),
        })
        .collect()
}

fn labeled() -> Labeled {
    let set = coder::router_eval::Set::fixture();
    let mut routes: BTreeMap<String, RouteRows> = BTreeMap::new();
    for row in &set.rows {
        let rows = routes.entry(row.route.clone()).or_default();
        rows.rows += 1;
        if row.split == "held_out" {
            rows.held_out += 1;
        } else {
            rows.tune += 1;
        }
        if row
            .tags
            .iter()
            .any(|t| t == "near-miss" || t == "near_miss")
        {
            rows.near_misses += 1;
        }
    }
    Labeled {
        set: set.set,
        path: LABELED.into(),
        created: set.created,
        routes,
    }
}

/// A precision or recall the record wrote as `count / denominator`, as
/// that exact quotient. `serde_json` parses a float to within one unit in
/// the last place unless something in the build enables its
/// `float_roundtrip` feature, as the desktop's dependencies do, so the
/// parsed value depends on which crates one `cargo test` builds; the
/// count does not, and dividing it again is exact (#10085, #10089).
#[allow(clippy::cast_precision_loss)]
fn exact_ratio(value: f64, denominator: u64) -> f64 {
    if denominator == 0 || !value.is_finite() {
        return value;
    }
    let count = (value * denominator as f64).round();
    count / denominator as f64
}

fn measurement() -> Measurement {
    let (dir, page) = MEASUREMENT;
    let report = json(&format!("{dir}/report.json"));
    let configuration = json(&format!("{dir}/configuration.json"));
    let mut metrics: BTreeMap<String, (f64, u64)> = BTreeMap::new();
    for m in report["measurements"].as_array().expect("measurements") {
        if m["arm"] != "subject" {
            continue;
        }
        let denominator = m["denominator"].as_u64().unwrap_or_default();
        metrics.insert(
            m["metric"].as_str().unwrap_or_default().to_string(),
            (
                exact_ratio(m["value"].as_f64().unwrap_or_default(), denominator),
                denominator,
            ),
        );
    }
    let mut routes = BTreeMap::new();
    for route in RouteId::ALL {
        let word = route.word();
        if let (Some(&(precision, predicted)), Some(&(recall, labeled))) = (
            metrics.get(&format!("route.{word}.precision")),
            metrics.get(&format!("route.{word}.recall")),
        ) {
            routes.insert(
                word.to_string(),
                RouteScore {
                    precision,
                    predicted,
                    recall,
                    labeled,
                },
            );
        }
    }
    Measurement {
        record: format!("{dir}/report.json"),
        page: page.into(),
        set: configuration["set"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        rows: metrics.get("rows_read").map_or(0, |m| m.1),
        route_accuracy: metrics.get("route_accuracy").map_or(0.0, |m| m.0),
        ended_at: report["ended_at"].as_u64().unwrap_or_default(),
        routes,
    }
}

fn knowledge() -> Knowledge {
    let dir = root().join("knowledge").join(knowledge::product::DIR);
    let (mut entries, problems) = knowledge::Base::read(&dir);
    assert!(problems.is_empty(), "product knowledge: {problems:?}");
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    let product = entries
        .iter()
        .map(|entry| KnowledgeSource {
            id: entry.id.clone(),
            title: entry.title.clone(),
            path: format!("knowledge/{}/{}.md", knowledge::product::DIR, entry.id),
            status: status(entry.status),
            tags: entry.tags.clone(),
            answer: entry.answer.is_some(),
        })
        .collect();
    let (coding, problems) = knowledge::Base::read(&root().join("knowledge"));
    assert!(problems.is_empty(), "coding knowledge: {problems:?}");
    let mut kinds = BTreeMap::new();
    for entry in &coding {
        *kinds.entry(kind(entry.kind)).or_insert(0) += 1;
    }
    Knowledge {
        product,
        coding: CodingKnowledge {
            path: "knowledge".into(),
            entries: coding.len() as u64,
            admitted: coding
                .iter()
                .filter(|e| e.status == knowledge::Status::Admitted)
                .count() as u64,
            candidates: coding
                .iter()
                .filter(|e| e.status == knowledge::Status::Candidate)
                .count() as u64,
            kinds,
        },
    }
}

fn status(status: knowledge::Status) -> String {
    match status {
        knowledge::Status::Candidate => "candidate",
        knowledge::Status::Admitted => "admitted",
        knowledge::Status::Withdrawn => "withdrawn",
    }
    .to_string()
}

fn kind(kind: knowledge::Kind) -> String {
    match kind {
        knowledge::Kind::Method => "method",
        knowledge::Kind::EdgeCase => "edge-case",
        knowledge::Kind::Slip => "slip",
        knowledge::Kind::Environment => "environment",
        knowledge::Kind::Tool => "tool",
        knowledge::Kind::Product => "product",
    }
    .to_string()
}

fn product_kb() -> ProductKb {
    let record = json(PRODUCT_KB);
    let rows = record["rows"].as_array().expect("rows");
    ProductKb {
        record: PRODUCT_KB.into(),
        set: record["summary"]["set"].as_str().unwrap_or_default().into(),
        questions: rows.len() as u64,
        unanswerable: rows
            .iter()
            .filter(|row| row["expect"].as_array().is_some_and(Vec::is_empty))
            .map(|row| Question {
                id: row["id"].as_str().unwrap_or_default().into(),
                question: row["question"].as_str().unwrap_or_default().into(),
            })
            .collect(),
    }
}

fn capabilities() -> Vec<CapabilitySource> {
    let corpus = knowledge::product::Corpus::load(
        &root().join("knowledge").join(knowledge::product::DIR),
        Some(&root()),
    )
    .expect("the product corpus loads");
    let tools = coder::gym_kb::tools(&corpus);
    router::Admitted::of(&tools, &[])
        .entries
        .into_iter()
        .map(|c| CapabilitySource {
            id: c.id,
            name: c.name,
            kind: c.kind.word().into(),
            reach: c.reach.word().into(),
            route: c.route.map(|r| r.word().to_string()),
            source: c.source,
            line: c.line,
        })
        .collect()
}

fn names(dir: &Path, extension: &str) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<String> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some(extension))
        .filter_map(|p| p.file_stem().and_then(|s| s.to_str()).map(str::to_string))
        .collect();
    out.sort();
    out
}

fn plugin(dir: &Path, product: &[KnowledgeSource]) -> PluginSource {
    let at = rel(dir);
    let cargo = std::fs::read_to_string(dir.join("Cargo.toml")).ok();
    let cargo: Option<toml::Value> = cargo.as_deref().and_then(|t| toml::from_str(t).ok());
    let package: Option<Value> = std::fs::read_to_string(dir.join("package.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    let text = |key: &str| {
        package
            .as_ref()
            .and_then(|p| p[key].as_str())
            .map(str::to_string)
    };
    let cargo_name = cargo
        .as_ref()
        .and_then(|c| c["package"]["name"].as_str())
        .map(str::to_string);
    let wasm = cargo.as_ref().is_some_and(|c| {
        c.get("lib")
            .and_then(|lib| lib.get("crate-type"))
            .and_then(toml::Value::as_array)
            .is_some_and(|types| types.iter().any(|t| t.as_str() == Some("cdylib")))
    });
    let fallback = || {
        let stem = at.rsplit('/').next().unwrap_or(&at);
        let stem = stem.strip_prefix("plugin-").unwrap_or(stem);
        let mut name: String = stem.replace('-', " ");
        if let Some(first) = name.get(0..1) {
            name = first.to_uppercase() + &name[1..];
        }
        name
    };
    let mut tests = Vec::new();
    let mut sets: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|read| {
            read.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.is_dir()
                        && p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.starts_with("evals"))
                })
                .collect()
        })
        .unwrap_or_default();
    sets.sort();
    for set in sets {
        let suite = ext_eval::Suite::load(&set, ext_eval::LoadOptions::default())
            .unwrap_or_else(|e| panic!("{}: {e}", set.display()));
        let fire = suite
            .cases
            .iter()
            .filter(|c| c.kind == ext_eval::Kind::ShouldFire)
            .count() as u64;
        tests.push(TestSetSource {
            dir: rel(&set),
            should_fire: fire,
            should_not_fire: suite.cases.len() as u64 - fire,
        });
    }
    let knowledge = product
        .iter()
        .filter(|entry| {
            let body = read(&entry.path);
            body.lines().any(|line| {
                line.trim_start()
                    .trim_start_matches("- ")
                    .starts_with(&format!("{at}/"))
            })
        })
        .map(|entry| entry.id.clone())
        .collect();
    PluginSource {
        dir: at.clone(),
        slug: text("slug"),
        name: text("name")
            .filter(|n| !n.is_empty())
            .unwrap_or_else(fallback),
        summary: text("summary")
            .filter(|s| !s.is_empty())
            .or_else(|| {
                cargo
                    .as_ref()
                    .and_then(|c| c["package"]["description"].as_str())
                    .map(str::to_string)
            })
            .unwrap_or_default(),
        publisher: text("publisher"),
        version: text("version"),
        wasm: if wasm {
            cargo_name.into_iter().collect()
        } else {
            Vec::new()
        },
        workflows: names(&dir.join("programs"), "json"),
        skills: names(&dir.join("skills"), "md"),
        knowledge,
        tests,
    }
}

fn plugins(product: &[KnowledgeSource]) -> Vec<PluginSource> {
    let mut dirs = Vec::new();
    for (parent, prefix, skip) in [
        ("crates", "plugin-", NOT_PLUGINS),
        ("packages", "", NOT_PACKAGES),
    ] {
        for entry in std::fs::read_dir(root().join(parent))
            .expect(parent)
            .flatten()
        {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let at = format!("{parent}/{name}");
            // The hosted runner's sample plugins are its test fixtures,
            // never shown (`deploy/eval-runner/catalog`).
            let sample = coder::gym_kb::catalog_dirs().contains(&at.as_str());
            if path.is_dir() && name.starts_with(prefix) && !skip.contains(&at.as_str()) && !sample
            {
                dirs.push(path);
            }
        }
    }
    dirs.sort();
    dirs.iter().map(|dir| plugin(dir, product)).collect()
}

fn sources() -> Sources {
    let bank = Bank::builtin();
    let knowledge = knowledge();
    let plugins = plugins(&knowledge.product);
    let showcase = SHOWCASE
        .iter()
        .find(|dir| plugins.iter().any(|p| p.dir == **dir))
        .map(|dir| (*dir).to_string());
    let codebase = json(CODEBASE_KB);
    Sources {
        schema: SCHEMA.into(),
        generated_by: "crates/coder/tests/route_map_sources.rs".into(),
        set: router::set_id(),
        bank: bank.id(),
        routes: routes(),
        answers: answers(bank),
        labeled: labeled(),
        measurement: measurement(),
        knowledge,
        product_kb: product_kb(),
        codebase_kb: CodebaseKb {
            path: CODEBASE_KB.into(),
            questions: codebase["questions"].as_array().map_or(0, Vec::len) as u64,
        },
        capabilities: capabilities(),
        engines: CodingEngine::ALL
            .into_iter()
            .map(|e| EngineSource {
                id: e.word().into(),
                name: e.name().into(),
            })
            .collect(),
        decks: router::decks()
            .iter()
            .map(|d| DeckSource {
                id: d.id.to_string(),
                title: d.title.clone(),
            })
            .collect(),
        screens: router::Screen::ALL
            .into_iter()
            .map(|s| s.word().to_string())
            .collect(),
        plugins,
        showcase,
    }
}

fn target(name: &str) -> PathBuf {
    root()
        .join("crates/openagents-chat-app/src/route_map")
        .join(name)
}

/// The committed snapshot is what the router's code and data say now.
#[test]
fn the_route_map_sources_match_the_router() {
    let built = sources();
    let document = built.document();
    let path = target("sources.json");
    if std::env::var_os("ROUTE_MAP_WRITE").is_some() {
        std::fs::write(&path, &document).expect("the snapshot is written");
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == document,
        "crates/openagents-chat-app/src/route_map/sources.json is stale; rewrite it with \
         ROUTE_MAP_WRITE=1 cargo test -p coder --test route_map_sources"
    );
    // Every route the router offers is on the map, in its order.
    let words: Vec<&str> = built.routes.iter().map(|r| r.id.as_str()).collect();
    let all: Vec<&str> = RouteId::ALL.iter().map(|r| r.word()).collect();
    assert_eq!(words, all);
}

/// The relay's published records, verified, as the map reads them.
#[tokio::test]
#[ignore = "reads the production relay"]
async fn live_route_map_records() {
    use nostr::eval_ext::{self, Linkage};
    let relay = std::env::var("ROUTE_MAP_RELAY")
        .unwrap_or_else(|_| "wss://relay.openagents.com".to_string());
    let secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    let identity = coder::relay::Identity::from_text(&secret.display_secret().to_string(), "map")
        .expect("a reader key");
    let events = coder::gym_kb::fetch_results(&relay, &identity)
        .await
        .expect("the relay answers");
    let mut publications = Vec::new();
    let mut releases = Vec::new();
    for event in &events {
        if let Ok(publication) = eval_ext::parse_publication(event) {
            publications.push(publication);
        } else if event.kind == nostr::ext::RELEASE_KIND
            && let Ok(body) = nostr::ext::parse_record(event)
            && body["type"] == "release"
        {
            releases.push(ReleaseRecord {
                id: event.id.clone(),
                pubkey: event.pubkey.clone(),
                package: body["package"].as_str().unwrap_or_default().into(),
                version: body["version"].as_str().unwrap_or_default().into(),
                created_at: event.created_at,
            });
        }
    }
    // The plugin releases the results name, which the starter filter
    // doesn't fetch, and the defaults' dependencies.
    let manifests: Vec<(String, Value)> =
        std::fs::read_dir(root().join("packages/coder-defaults/documents"))
            .expect("the defaults documents")
            .flatten()
            .filter_map(|e| std::fs::read(e.path()).ok())
            .filter_map(|bytes| {
                let value: Value = serde_json::from_slice(&bytes).ok()?;
                (value["v"] == "openagents.package.v1")
                    .then(|| (nostr::contracts::digest_bytes(&bytes), value))
            })
            .collect();
    let mut wanted: Vec<String> = publications
        .iter()
        .filter_map(|p| p.subject_release.as_ref().map(|r| r.id.clone()))
        .chain(
            manifests
                .iter()
                .flat_map(|(_, m)| m["dependencies"].as_array().cloned().unwrap_or_default())
                .filter_map(|d| d.as_str().map(str::to_string)),
        )
        .filter(|id| !releases.iter().any(|r: &ReleaseRecord| &r.id == id))
        .collect();
    wanted.sort();
    wanted.dedup();
    if !wanted.is_empty() {
        let found = fetch_ids(&relay, &identity, &wanted).await;
        for event in found {
            if let Ok(body) = nostr::ext::parse_record(&event) {
                releases.push(ReleaseRecord {
                    id: event.id.clone(),
                    pubkey: event.pubkey.clone(),
                    package: body["package"].as_str().unwrap_or_default().into(),
                    version: body["version"].as_str().unwrap_or_default().into(),
                    created_at: event.created_at,
                });
            }
        }
    }
    let mut results: Vec<ResultRecord> = publications
        .iter()
        .map(|p| {
            let mut confirmed = 0;
            let mut disputed = 0;
            for other in &publications {
                match eval_ext::linkage(p, other) {
                    Linkage::Confirm => confirmed += 1,
                    Linkage::Dispute => disputed += 1,
                    Linkage::NotACheck => {}
                }
            }
            let headline = p.report.profile.headline;
            ResultRecord {
                id: p.id.clone(),
                created_at: p.created_at,
                evaluator: p.evaluator.clone(),
                trainer: p.trainer().to_string(),
                subject: p.report.subject.definition.id.clone(),
                subject_release: p.subject_release.as_ref().map(|r| r.id.clone()),
                suite_release: p.suite_release.id.clone(),
                verdict: match p.verdict() {
                    eval_ext::Verdict::Pass => Verdict::Better,
                    eval_ext::Verdict::Fail => Verdict::Worse,
                    eval_ext::Verdict::Inconclusive => Verdict::NoClearChange,
                },
                with: headline.subject_passed,
                without: headline.baseline_passed,
                total: headline.total,
                checks: p.checks.clone(),
                validates: p.validates.clone(),
                confirmed,
                disputed,
            }
        })
        .collect();
    results.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(a.id.cmp(&b.id)));
    // Adoptions: the newest defaults release whose manifest is a committed
    // document, and each dependency it admits.
    let root_key = coder::gym_kb::defaults_root();
    let mut adoptions = Vec::new();
    for event in events
        .iter()
        .filter(|e| e.kind == nostr::ext::RELEASE_KIND && e.pubkey == root_key)
    {
        let Ok(body) = nostr::ext::parse_record(event) else {
            continue;
        };
        let digest = body["manifest"]["digest"].as_str().unwrap_or_default();
        let Some((_, manifest)) = manifests.iter().find(|(d, _)| d == digest) else {
            continue;
        };
        let admission = manifest["provenance"]["receipts"][0]["digest"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        for dependency in manifest["dependencies"].as_array().into_iter().flatten() {
            adoptions.push(AdoptionRecord {
                defaults_release: event.id.clone(),
                release: dependency.as_str().unwrap_or_default().into(),
                admission: admission.clone(),
                at: event.created_at,
            });
        }
    }
    adoptions.sort_by_key(|a| std::cmp::Reverse(a.at));
    releases.sort_by(|a, b| a.id.cmp(&b.id));
    releases.dedup_by(|a, b| a.id == b.id);
    let records = Records {
        schema: openagents_chat_app::route_map::records::SCHEMA.into(),
        generated_by: "crates/coder/tests/route_map_sources.rs (live_route_map_records)".into(),
        relay,
        fetched_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        releases,
        results,
        adoptions,
    };
    println!(
        "{} results, {} releases, {} adoptions",
        records.results.len(),
        records.releases.len(),
        records.adoptions.len()
    );
    if std::env::var_os("ROUTE_MAP_WRITE").is_some() {
        std::fs::write(target("records.json"), records.document()).expect("written");
    }
}

async fn fetch_ids(
    relay: &str,
    identity: &coder::relay::Identity,
    ids: &[String],
) -> Vec<nostr::domain::Event> {
    use futures_util::StreamExt;
    let mut socket = coder::relay::connect(relay, identity)
        .await
        .expect("the relay answers");
    coder::relay::send(
        &mut socket,
        serde_json::json!(["REQ", "ids", {"ids": ids, "kinds": [nostr::ext::RELEASE_KIND]}]),
    )
    .await
    .expect("sent");
    let mut out = Vec::new();
    let reading = async {
        while let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))) =
            socket.next().await
        {
            let Ok(value) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            match value[0].as_str() {
                Some("EVENT") => {
                    if let Ok(event) = serde_json::from_value(value[2].clone()) {
                        out.push(event);
                    }
                }
                Some("EOSE" | "CLOSED") => break,
                _ => {}
            }
        }
    };
    let _ = tokio::time::timeout(std::time::Duration::from_secs(20), reading).await;
    out
}
