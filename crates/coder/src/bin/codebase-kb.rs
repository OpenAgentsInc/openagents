//! `codebase-kb`: build, query, and evaluate the codebase knowledge index.
//!
//! ```sh
//! # Build (or refresh) the index at a commit; reuses the previous file's
//! # vectors for unchanged chunks.
//! codebase-kb build --repo . --commit origin/main --out ~/.cache/openagents/codebase-kb/codebase-kb.gz
//! # Ask one question through the whole route: retrieve, judge, answer.
//! codebase-kb ask "where is the chat worker's quota implemented?"
//! # Answer quality and citation accuracy on the held-out questions.
//! codebase-kb eval crates/coder/fixtures/chat-router/codebase-questions-v1.json
//! ```
//!
//! Embeddings come from the AI Gateway with the chat worker's door key
//! (`CODER_AI_GATEWAY_KEY` or `CODER_DOOR_KEY`), else OpenAI, else
//! OpenRouter (`coder::codebase::embedder`). The judge is
//! the configured decision profile (`TYPESAFE_API_KEY`). Answers are written
//! by the gateway door when `CODER_DOOR_KEY` or `CODER_AI_GATEWAY_KEY` is
//! set, else by `CODEBASE_KB_MODEL` on OpenAI when `OPENAI_API_KEY` is set
//! (default `gpt-4.1-mini`), else on OpenRouter (default: the Gemini
//! lane's model). Read `docs/coder/design/codebase-kb.md`.

use std::path::PathBuf;
use std::time::Instant;

use coder::codebase::{self, Answered, Codebase, Compose, Reply};
use coder::generate::ResponsesDoor;
use jev::{Answer, Noul, NoulCriteria, Questions};
use knowledge::codebase::{Index, build, read_commit};
use serde::Deserialize;
use serde_json::json;

const USAGE: &str = "usage:
  codebase-kb build [--repo DIR] [--commit REV] [--out FILE] [--previous FILE]
  codebase-kb ask QUESTION [--index FILE]
  codebase-kb eval [FIXTURE] [--index FILE] [--json FILE]";

/// The OpenRouter model that answers when no gateway door or OpenAI key is
/// configured.
const DEFAULT_MODEL: &str = coder::generate::Lane::Gemini.model();

/// The OpenAI model that answers when `OPENAI_API_KEY` is set and no
/// gateway door is.
const DEFAULT_OPENAI_MODEL: &str = "gpt-4.1-mini";

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("build") => build_command(&args[1..]).await,
        Some("ask") => ask_command(&args[1..]).await,
        Some("eval") => eval_command(&args[1..]).await,
        _ => Err(USAGE.to_string()),
    };
    if let Err(error) = result {
        eprintln!("codebase-kb: {error}");
        std::process::exit(1);
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

fn positional(args: &[String]) -> Option<String> {
    let mut skip = false;
    for arg in args {
        if skip {
            skip = false;
            continue;
        }
        if arg.starts_with("--") {
            skip = true;
            continue;
        }
        return Some(arg.clone());
    }
    None
}

fn index_path(args: &[String]) -> Result<PathBuf, String> {
    flag(args, "--index")
        .map(PathBuf::from)
        .or_else(codebase::configured_path)
        .ok_or_else(|| "no index: pass --index or set CODER_CODEBASE_KB".to_string())
}

fn today() -> String {
    knowledge::today()
}

async fn build_command(args: &[String]) -> Result<(), String> {
    let repo = PathBuf::from(flag(args, "--repo").unwrap_or_else(|| ".".to_string()));
    let commit = flag(args, "--commit").unwrap_or_else(|| "HEAD".to_string());
    let out = flag(args, "--out")
        .map(PathBuf::from)
        .or_else(codebase::default_path)
        .ok_or("no --out and no HOME")?;
    let previous_path = flag(args, "--previous")
        .map(PathBuf::from)
        .unwrap_or_else(|| out.clone());
    let previous = Index::read(&previous_path).ok();
    let embedder = codebase::embedder()?;
    let started = Instant::now();
    let (full, files) = read_commit(&repo, &commit)?;
    eprintln!(
        "read {} files at {full}; previous index: {}",
        files.len(),
        previous
            .as_ref()
            .map_or("none".to_string(), |p| p.commit.clone())
    );
    let (index, report) = build(
        "OpenAgentsInc/openagents",
        &full,
        &today(),
        &files,
        &embedder,
        previous.as_ref(),
        |done, all| {
            if done % (knowledge::codebase::BATCH * 20) == 0 || done == all {
                eprintln!("embedded {done}/{all}");
            }
        },
    )
    .await?;
    let bytes = index.write(&out)?;
    println!(
        "{}",
        json!({
            "out": out.display().to_string(),
            "commit": full,
            "model": index.model,
            "bytes": bytes,
            "files": report.files,
            "chunks": report.chunks,
            "doc_chunks": report.doc_chunks,
            "code_chunks": report.code_chunks,
            "reused": report.reused,
            "embedded": report.embedded,
            "usd": report.usd,
            "seconds": started.elapsed().as_secs(),
        })
    );
    Ok(())
}

/// The composer: Gemini on Vertex AI when the Vertex switch is on
/// (`VERTEX_PROJECT` with a Google credential; Google first on our keys,
/// 2026-10-10), else the gateway door when configured, else OpenAI, else
/// OpenRouter.
enum Composer {
    Door(ResponsesDoor),
    OpenAi(codebase::OpenAiChat),
    OpenRouter(codebase::OpenRouter),
}

impl Composer {
    fn from_env() -> Result<Self, String> {
        if let Some(door) = coder::generate::vertex_door_from_env()? {
            return Ok(Composer::Door(door));
        }
        if let Some(door) = ResponsesDoor::from_env() {
            return Ok(Composer::Door(door));
        }
        let asked = std::env::var("CODEBASE_KB_MODEL").ok();
        if let Some(chat) =
            codebase::OpenAiChat::from_env(asked.as_deref().unwrap_or(DEFAULT_OPENAI_MODEL))
        {
            return Ok(Composer::OpenAi(chat));
        }
        let config = openrouter::Config::from_env().map_err(|e| e.to_string())?;
        Ok(Composer::OpenRouter(codebase::OpenRouter {
            client: openrouter::Client::new(config).map_err(|e| e.to_string())?,
            model: asked.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
        }))
    }

    fn name(&self) -> String {
        match self {
            Composer::Door(door) if door.is_vertex() => format!("vertex:{}", door.model),
            Composer::Door(door) => format!("gateway:{}", door.model),
            Composer::OpenAi(chat) => format!("openai:{}", chat.model),
            Composer::OpenRouter(or) => format!("openrouter:{}", or.model),
        }
    }
}

impl Compose for Composer {
    async fn compose(&self, instructions: &str, input: &str) -> Result<String, String> {
        match self {
            Composer::Door(door) => codebase::Door(door).compose(instructions, input).await,
            Composer::OpenAi(chat) => chat.compose(instructions, input).await,
            Composer::OpenRouter(or) => or.compose(instructions, input).await,
        }
    }
}

fn judge() -> Result<jev::Client, String> {
    coder::decision::from_env()?
        .ok_or_else(|| "no decision profile: set TYPESAFE_API_KEY".to_string())
}

async fn ask_command(args: &[String]) -> Result<(), String> {
    let question = positional(args).ok_or(USAGE)?;
    let kb = {
        let index = Index::read(&index_path(args)?)?;
        let embedder = codebase::embedder_for(&index.model)?;
        Codebase::new(index, embedder)?
    };
    let composer = Composer::from_env()?;
    let answered = kb.answer(&judge()?, &composer, &question).await?;
    print_answered(&answered);
    Ok(())
}

fn print_answered(answered: &Answered) {
    for (at, similarity) in &answered.retrieved {
        let kept = answered
            .judged
            .kept
            .iter()
            .find(|e| &e.chunk.cite() == at)
            .map_or(String::new(), |e| format!(" kept {:.2}", e.relevance));
        eprintln!("  {similarity:.3} {at}{kept}");
    }
    eprintln!(
        "needs_live {:.2}; embed {} ms, judge {} ms, compose {} ms",
        answered.judged.needs_live,
        answered.timing.embed_ms,
        answered.timing.judge_ms,
        answered.timing.compose_ms
    );
    match &answered.reply {
        Reply::Grounded {
            text, unsupported, ..
        } => {
            println!("{text}");
            if !unsupported.is_empty() {
                eprintln!(
                    "unsupported citations: {}",
                    unsupported
                        .iter()
                        .map(codebase::Citation::cite)
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
        Reply::Escalate { why, commit } => {
            println!("escalate to Coder ({}) at {commit}", why.word());
        }
    }
}

#[derive(Deserialize)]
struct Fixture {
    commit: String,
    questions: Vec<Question>,
}

#[derive(Deserialize)]
struct Question {
    id: String,
    question: String,
    expect: String,
    gold: Vec<String>,
    reference: String,
}

/// The quality judge: does the answer agree with the reference?
fn quality_request(question: &Question, answer: &str) -> jev::SystemOneRequest {
    jev::SystemOneRequest::new(
        json!({
            "question": question.question,
            "reference_answer": question.reference,
            "candidate_answer": answer,
        }),
        Questions::new().with(
            "correct",
            Noul::with_criteria(
                "Does the candidate answer correctly answer the question, stating the key facts \
                 of the reference answer without contradicting it? Extra correct detail and \
                 citations are fine.",
                NoulCriteria::new()
                    .when_true(
                        "Yes: it gives the reference's key facts and nothing contradicts them",
                    )
                    .when_false(
                        "No: it misses the key facts, is vague, or contradicts the reference",
                    ),
            ),
        ),
    )
}

async fn eval_command(args: &[String]) -> Result<(), String> {
    let fixture_path = positional(args)
        .unwrap_or_else(|| "crates/coder/fixtures/chat-router/codebase-questions-v1.json".into());
    let fixture: Fixture = serde_json::from_str(
        &std::fs::read_to_string(&fixture_path).map_err(|e| format!("{fixture_path}: {e}"))?,
    )
    .map_err(|e| format!("{fixture_path}: {e}"))?;
    let kb = {
        let index = Index::read(&index_path(args)?)?;
        let embedder = codebase::embedder_for(&index.model)?;
        Codebase::new(index, embedder)?
    };
    let composer = Composer::from_env()?;
    let judge = judge()?;
    eprintln!(
        "{} questions; fixture labeled at {}, index at {}; composer {}",
        fixture.questions.len(),
        &fixture.commit[..10.min(fixture.commit.len())],
        kb.index.short_commit(),
        composer.name()
    );
    let mut rows = Vec::new();
    let mut totals = Totals::default();
    for q in &fixture.questions {
        let started = Instant::now();
        let answered = match kb.answer(&judge, &composer, &q.question).await {
            Ok(answered) => answered,
            Err(error) => {
                eprintln!("{}: error: {error}", q.id);
                totals.errors += 1;
                continue;
            }
        };
        let ms = started.elapsed().as_millis();
        totals.latency.push(ms);
        let in_gold = |path: &str| q.gold.iter().any(|g| g == path);
        let retrieved_gold = answered
            .retrieved
            .iter()
            .any(|(at, _)| in_gold(at.split(':').next().unwrap_or("")));
        let kept_gold = answered.judged.kept.iter().any(|e| in_gold(&e.chunk.path));
        let mut row = json!({
            "id": q.id, "expect": q.expect, "ms": ms,
            "needs_live": answered.judged.needs_live,
            "kept": answered.judged.kept.iter().map(|e| e.chunk.cite()).collect::<Vec<_>>(),
            "retrieved_gold": retrieved_gold, "kept_gold": kept_gold,
        });
        match (&answered.reply, q.expect.as_str()) {
            (Reply::Escalate { why, .. }, expect) => {
                row["outcome"] = json!("escalate");
                row["why"] = json!(why.word());
                if expect == "escalate" {
                    totals.escalate_right += 1;
                } else {
                    totals.answer_escalated += 1;
                    totals.retrieval(retrieved_gold, kept_gold);
                }
            }
            (
                Reply::Grounded {
                    text,
                    citations,
                    unsupported,
                    ..
                },
                expect,
            ) => {
                row["outcome"] = json!("answer");
                row["text"] = json!(text);
                row["citations"] = json!(
                    citations
                        .iter()
                        .map(codebase::Citation::cite)
                        .collect::<Vec<_>>()
                );
                row["unsupported"] = json!(
                    unsupported
                        .iter()
                        .map(codebase::Citation::cite)
                        .collect::<Vec<_>>()
                );
                if expect == "escalate" {
                    totals.escalate_missed += 1;
                } else {
                    totals.answered += 1;
                    totals.retrieval(retrieved_gold, kept_gold);
                    totals.cited += citations.len() + unsupported.len();
                    totals.cited_supported += citations.len();
                    let gold_cited = citations.iter().filter(|c| in_gold(&c.path)).count();
                    totals.cited_gold += gold_cited;
                    if gold_cited > 0 {
                        totals.answers_citing_gold += 1;
                    }
                    let verdict = judge
                        .system_one(quality_request(q, text))
                        .await
                        .map_err(|e| format!("the quality judge: {e}"))?;
                    let p = match verdict.answers.get("correct") {
                        Some(Answer::Noul(n)) => n.noul,
                        _ => 0.0,
                    };
                    row["correct_p"] = json!(p);
                    totals.quality.push(p);
                }
            }
        }
        eprintln!(
            "{} {} -> {} ({} ms)",
            q.id,
            q.expect,
            row["outcome"].as_str().unwrap_or(""),
            ms
        );
        rows.push(row);
    }
    let summary = totals.summary(&fixture.questions);
    println!(
        "{}",
        serde_json::to_string_pretty(&summary).unwrap_or_default()
    );
    if let Some(path) = flag(args, "--json") {
        let doc = json!({
            "schema": "openagents.codebase-kb.eval.v1",
            "fixture": fixture_path,
            "index_commit": kb.index.commit,
            "composer": composer.name(),
            "summary": summary,
            "rows": rows,
        });
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&doc).unwrap_or_default(),
        )
        .map_err(|e| format!("{path}: {e}"))?;
    }
    Ok(())
}

#[derive(Default)]
struct Totals {
    errors: usize,
    escalate_right: usize,
    escalate_missed: usize,
    answer_escalated: usize,
    answered: usize,
    retrieved_gold: usize,
    kept_gold: usize,
    cited: usize,
    cited_supported: usize,
    cited_gold: usize,
    answers_citing_gold: usize,
    quality: Vec<f64>,
    latency: Vec<u128>,
}

impl Totals {
    fn retrieval(&mut self, retrieved: bool, kept: bool) {
        self.retrieved_gold += usize::from(retrieved);
        self.kept_gold += usize::from(kept);
    }

    fn summary(&self, questions: &[Question]) -> serde_json::Value {
        let answerable = questions.iter().filter(|q| q.expect == "answer").count();
        let escalations = questions.len() - answerable;
        let ratio = |a: usize, b: usize| {
            if b == 0 {
                None
            } else {
                Some((a as f64 / b as f64 * 1000.0).round() / 1000.0)
            }
        };
        let mut latency = self.latency.clone();
        latency.sort_unstable();
        let pct = |p: f64| {
            latency
                .get(((latency.len() as f64 - 1.0) * p).round() as usize)
                .copied()
        };
        let correct = self.quality.iter().filter(|p| **p >= 0.5).count();
        json!({
            "questions": questions.len(),
            "errors": self.errors,
            "answerable": answerable,
            "answered": self.answered,
            "answerable_escalated": self.answer_escalated,
            "escalation_expected": escalations,
            "escalation_recall": ratio(self.escalate_right, escalations),
            "escalation_missed": self.escalate_missed,
            "gold_retrieved_rate": ratio(self.retrieved_gold, answerable),
            "gold_kept_rate": ratio(self.kept_gold, answerable),
            "answer_correct_rate": ratio(correct, self.answered),
            "answer_correct_rate_of_answerable": ratio(correct, answerable),
            "answer_quality_mean_p": if self.quality.is_empty() { None } else {
                Some((self.quality.iter().sum::<f64>() / self.quality.len() as f64 * 1000.0).round() / 1000.0)
            },
            "citations": self.cited,
            "citation_validity": ratio(self.cited_supported, self.cited),
            "citation_gold_precision": ratio(self.cited_gold, self.cited_supported),
            "answers_citing_gold": ratio(self.answers_citing_gold, self.answered),
            "latency_ms_p50": pct(0.5),
            "latency_ms_p95": pct(0.95),
        })
    }
}
