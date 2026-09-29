//! The held-out product questions and the live evaluation over them.
//!
//! `crates/coder/fixtures/product-kb/questions-v1.json` holds 100 product
//! questions with the entries that answer them, and 10 that nothing in the
//! corpus answers. They were written before retrieval first ran and are
//! never used to tune entry text or thresholds.
//!
//! `live_product_kb_eval` (ignored; it spends a few cents) runs every
//! question through the real lookup (embeddings and Jev), then through the
//! grounded model, and has Jev read each reply. The grounded model is the
//! chat worker's, Gemini 3.8 Flash, through Google's OpenAI-compatible
//! endpoint when `GEMINI_API_KEY` is set, or else through OpenRouter with
//! `OPENROUTER_API_KEY`. Embeddings come from [`embedder_from_env`]. It
//! prints a summary and writes every row to `PRODUCT_KB_EVAL_OUT`
//! (default: the system temporary directory):
//!
//! ```sh
//! cargo test -p coder --lib product_kb::eval::live_product_kb_eval -- --ignored --nocapture
//! ```

use std::sync::Arc;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::*;
use crate::router::seams::Lookup;

/// The questions file.
const QUESTIONS: &str = include_str!("../../fixtures/product-kb/questions-v1.json");

/// The grounded model: the chat worker's model, as the gateway and
/// OpenRouter name it.
const MODEL: &str = crate::generate::Lane::Gemini.model();

/// The same model as Google names it, without the vendor prefix.
fn google_model() -> &'static str {
    MODEL.strip_prefix("google/").unwrap_or(MODEL)
}

/// Its list price in dollars per million input and output tokens, as
/// OpenRouter lists [`MODEL`] (retrieved 2026-09-28).
const USD_PER_MILLION: (f64, f64) = (0.75, 3.75);

/// An OpenAI-compatible chat endpoint for [`MODEL`]: its URL, the model
/// name it takes, and the key.
struct Door {
    url: &'static str,
    model: String,
    key: String,
    http: reqwest::Client,
}

impl Door {
    fn from_env() -> Self {
        let http = reqwest::Client::new();
        if let Ok(key) = std::env::var("GEMINI_API_KEY") {
            return Door {
                url: "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
                model: google_model().to_string(),
                key,
                http,
            };
        }
        Door {
            url: "https://openrouter.ai/api/v1/chat/completions",
            model: MODEL.to_string(),
            key: std::env::var("OPENROUTER_API_KEY").expect("GEMINI_API_KEY or OPENROUTER_API_KEY"),
            http,
        }
    }

    /// The reply to `system` and `user`, and its cost at list price.
    async fn chat(&self, system: &str, user: &str) -> Result<(String, Option<f64>), String> {
        let body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
        });
        let response = self
            .http
            .post(self.url)
            .bearer_auth(&self.key)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = response.status();
        let value: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("HTTP {status}"));
        }
        let text = value["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let usd = match (
            value["usage"]["prompt_tokens"].as_f64(),
            value["usage"]["completion_tokens"].as_f64(),
        ) {
            (Some(input), Some(output)) => {
                Some((input * USD_PER_MILLION.0 + output * USD_PER_MILLION.1) / 1_000_000.0)
            }
            _ => None,
        };
        Ok((text, usd))
    }
}

/// One held-out question.
#[derive(Clone, Debug, Deserialize)]
pub(super) struct Question {
    pub id: String,
    pub question: String,
    /// The entries that answer it; empty when nothing in the corpus does.
    pub expect: Vec<String>,
    /// Whether one entry's answer fully answers it as asked.
    pub t0: bool,
}

#[derive(Deserialize)]
struct File {
    questions: Vec<Question>,
}

/// The held-out questions.
pub(super) fn questions() -> Vec<Question> {
    serde_json::from_str::<File>(QUESTIONS)
        .expect("the questions file parses")
        .questions
}

/// One question's result.
#[derive(Clone, Debug, Default, Serialize)]
struct Row {
    id: String,
    question: String,
    expect: Vec<String>,
    t0_expected: bool,
    /// The candidates by cosine similarity, nearest first.
    nearest: Vec<String>,
    /// 1-based rank of the first expected entry among the candidates.
    rank: Option<usize>,
    /// The passages kept (relevance at or above the floor), most relevant first.
    kept: Vec<(String, f64)>,
    /// The `answer` choice and its probability.
    answer: Option<(String, f64)>,
    /// The entry whose answer the router would serve whole (T0), when any.
    served: Option<String>,
    /// The grounded model's reply, when the turn was not T0.
    reply: Option<String>,
    cited_known: Vec<String>,
    cited_unknown: Vec<String>,
    /// Jev: every statement about OpenAgents is in the references.
    supported: Option<f64>,
    /// Jev: the reply says we don't have it documented.
    admits: Option<f64>,
    /// Jev: the reply answers the question.
    answers: Option<f64>,
    embed_ms: u64,
    judge_ms: u64,
    generate_ms: u64,
    generate_usd: Option<f64>,
    error: Option<String>,
}

/// The id without a version.
fn bare(id: &str) -> String {
    id.split_once('@').map_or(id, |(id, _)| id).to_string()
}

/// Jev's reading of a grounded reply.
async fn read_reply(
    judge: &jev::Client,
    question: &str,
    grounding: &Grounding,
    reply: &str,
) -> Result<(f64, f64, f64), String> {
    let references: Vec<serde_json::Value> = grounding
        .passages
        .iter()
        .map(|p| json!({"id": bare(&p.id), "text": p.text}))
        .collect();
    let state = json!({
        "question": question,
        "reference_entries": references,
        "reply": reply,
    });
    let questions = jev::Questions::new()
        .with(
            "supported",
            jev::Noul::new(
                "Is every statement the reply makes about OpenAgents, its app, or its services \
                 stated in the reference entries? (Advice with no product claim counts as \
                 supported.)",
            ),
        )
        .with(
            "admits",
            jev::Noul::new(
                "Does the reply say that this isn't documented or that we don't know, instead of \
                 answering the question?",
            ),
        )
        .with(
            "answers",
            jev::Noul::new("Does the reply correctly answer what the user asked?"),
        );
    let response = judge
        .system_one(jev::SystemOneRequest::new(state, questions))
        .await
        .map_err(|e| e.to_string())?;
    let p = |id: &str| match response.answers.get(id) {
        Some(jev::Answer::Noul(noul)) => noul.noul,
        _ => f64::NAN,
    };
    Ok((p("supported"), p("admits"), p("answers")))
}

async fn one(
    kb: &ProductKnowledge<Embedder>,
    judge: &jev::Client,
    model: &Door,
    q: Question,
) -> Row {
    let mut row = Row {
        id: q.id.clone(),
        question: q.question.clone(),
        expect: q.expect.clone(),
        t0_expected: q.t0,
        ..Row::default()
    };
    let lookup = Lookup {
        message: q.question.clone(),
        transcript: vec![Message {
            role: Role::User,
            text: q.question.clone(),
        }],
    };
    let found = match kb.find(&lookup).await {
        Ok(found) => found,
        Err(error) => {
            row.error = Some(error.to_string());
            return row;
        }
    };
    row.nearest = found.candidates.iter().map(|c| c.id.clone()).collect();
    row.rank = row
        .nearest
        .iter()
        .position(|id| q.expect.contains(id))
        .map(|n| n + 1);
    row.kept = found
        .grounding
        .passages
        .iter()
        .map(|p| (bare(&p.id), p.relevance))
        .collect();
    row.answer = found.answer.clone();
    row.embed_ms = found.embed_ms;
    row.judge_ms = found.judge_ms;
    // The router's T0 gate: the top passage's reviewed answer at
    // KB_ANSWER_CONFIDENCE relevance (needs_specifics is the router's own
    // question and is not asked here).
    row.served = found
        .grounding
        .passages
        .first()
        .filter(|p| p.answer.is_some() && p.relevance >= KB_ANSWER_CONFIDENCE)
        .map(|p| bare(&p.id));
    if row.served.is_some() && q.t0 {
        return row;
    }
    let instructions = instructions(&found.grounding);
    let started = std::time::Instant::now();
    match model.chat(&instructions, &q.question).await {
        Ok((reply, usd)) => {
            row.generate_ms = started.elapsed().as_millis() as u64;
            row.generate_usd = usd;
            let checked = cited(&reply, &found.grounding);
            row.cited_known = checked.known;
            row.cited_unknown = checked.unknown;
            match read_reply(judge, &q.question, &found.grounding, &reply).await {
                Ok((supported, admits, answers)) => {
                    row.supported = Some(supported);
                    row.admits = Some(admits);
                    row.answers = Some(answers);
                }
                Err(error) => row.error = Some(format!("reading the reply: {error}")),
            }
            row.reply = Some(reply);
        }
        Err(error) => row.error = Some(format!("generating: {error}")),
    }
    row
}

fn share(n: usize, of: usize) -> String {
    if of == 0 {
        return "n/a".to_string();
    }
    format!("{n}/{of} ({:.0}%)", 100.0 * n as f64 / of as f64)
}

fn percentile(values: &mut [u64], p: f64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    let index = ((values.len() as f64 - 1.0) * p).round() as usize;
    values[index]
}

#[tokio::test]
#[ignore = "calls an embeddings provider, Gemini, and TypeSafe's Jev; costs a few cents"]
async fn live_product_kb_eval() {
    let judge = Arc::new(
        crate::decision::from_env()
            .expect("a decision profile")
            .expect("a Jev key"),
    );
    let kb = ProductKnowledge::new(
        super::tests::committed(),
        embedder_from_env().expect("an embedder"),
        "embeddings",
        judge.clone(),
    );
    kb.warm().await.expect("the corpus embeds");
    let model = Door::from_env();
    let rows: Vec<Row> = futures_util::stream::iter(questions())
        .map(|q| one(&kb, &judge, &model, q))
        .buffered(4)
        .collect()
        .await;

    let answerable: Vec<&Row> = rows.iter().filter(|r| !r.expect.is_empty()).collect();
    let unanswerable: Vec<&Row> = rows.iter().filter(|r| r.expect.is_empty()).collect();
    let errors = rows.iter().filter(|r| r.error.is_some()).count();
    let at = |k: usize| {
        answerable
            .iter()
            .filter(|r| r.rank.is_some_and(|n| n <= k))
            .count()
    };
    let kept_expected = answerable
        .iter()
        .filter(|r| r.kept.iter().any(|(id, _)| r.expect.contains(id)))
        .count();
    let top_kept_expected = answerable
        .iter()
        .filter(|r| r.kept.first().is_some_and(|(id, _)| r.expect.contains(id)))
        .count();
    let kept_total: usize = answerable.iter().map(|r| r.kept.len()).sum();
    let kept_right: usize = answerable
        .iter()
        .map(|r| {
            r.kept
                .iter()
                .filter(|(id, _)| r.expect.contains(id))
                .count()
        })
        .sum();
    let served: Vec<&&Row> = answerable.iter().filter(|r| r.served.is_some()).collect();
    let served_right = served
        .iter()
        .filter(|r| r.served.as_ref().is_some_and(|id| r.expect.contains(id)))
        .count();
    let t0_questions = answerable.iter().filter(|r| r.t0_expected).count();
    let t0_covered = answerable
        .iter()
        .filter(|r| r.t0_expected && r.served.as_ref().is_some_and(|id| r.expect.contains(id)))
        .count();
    let served_on_specific = answerable
        .iter()
        .filter(|r| !r.t0_expected && r.served.is_some())
        .count();
    let unanswerable_kept_nothing = unanswerable.iter().filter(|r| r.kept.is_empty()).count();
    let unanswerable_served = unanswerable.iter().filter(|r| r.served.is_some()).count();
    let generated: Vec<&Row> = rows.iter().filter(|r| r.reply.is_some()).collect();
    let generated_answerable: Vec<&&Row> =
        generated.iter().filter(|r| !r.expect.is_empty()).collect();
    let cites_valid = generated
        .iter()
        .filter(|r| r.cited_unknown.is_empty())
        .count();
    let cites_expected = generated_answerable
        .iter()
        .filter(|r| r.cited_known.iter().any(|id| r.expect.contains(id)))
        .count();
    let supported = generated
        .iter()
        .filter(|r| r.supported.is_some_and(|p| p >= 0.5))
        .count();
    let answers = generated_answerable
        .iter()
        .filter(|r| r.answers.is_some_and(|p| p >= 0.5))
        .count();
    let admits_unanswerable = generated
        .iter()
        .filter(|r| r.expect.is_empty() && r.admits.is_some_and(|p| p >= 0.5))
        .count();
    let generated_unanswerable = generated.iter().filter(|r| r.expect.is_empty()).count();
    let mut embed: Vec<u64> = rows.iter().map(|r| r.embed_ms).collect();
    let mut judge_ms: Vec<u64> = rows.iter().map(|r| r.judge_ms).collect();
    let mut generate: Vec<u64> = generated.iter().map(|r| r.generate_ms).collect();
    let usd: f64 = rows.iter().filter_map(|r| r.generate_usd).sum();

    let summary = json!({
        "set": "product-kb-questions-v1",
        "corpus": kb.corpus().tag(),
        "entries": kb.corpus().base.entries.len(),
        "relevance_set": SET,
        "model": google_model(),
        "embedding_model": kb.embedder.model(),
        "errors": errors,
        "retrieval": {
            "recall_at_1": share(at(1), answerable.len()),
            "recall_at_3": share(at(3), answerable.len()),
            "recall_at_8": share(at(CANDIDATES), answerable.len()),
        },
        "relevance": {
            "expected_kept": share(kept_expected, answerable.len()),
            "expected_first": share(top_kept_expected, answerable.len()),
            "kept_that_were_expected": share(kept_right, kept_total),
            "unanswerable_kept_nothing": share(unanswerable_kept_nothing, unanswerable.len()),
        },
        "t0": {
            "served": served.len(),
            "precision": share(served_right, served.len()),
            "coverage_of_t0_questions": share(t0_covered, t0_questions),
            "served_where_specifics_needed": served_on_specific,
            "served_on_unanswerable": unanswerable_served,
        },
        "t2": {
            "replies": generated.len(),
            "no_invented_citation": share(cites_valid, generated.len()),
            "cites_an_expected_entry": share(cites_expected, generated_answerable.len()),
            "jev_supported": share(supported, generated.len()),
            "jev_answers": share(answers, generated_answerable.len()),
            "unanswerable_admitted": share(admits_unanswerable, generated_unanswerable),
            "usd": usd,
        },
        "latency_ms": {
            "embed_p50": percentile(&mut embed, 0.5),
            "embed_p95": percentile(&mut embed, 0.95),
            "judge_p50": percentile(&mut judge_ms, 0.5),
            "judge_p95": percentile(&mut judge_ms, 0.95),
            "generate_p50": percentile(&mut generate, 0.5),
            "generate_p95": percentile(&mut generate, 0.95),
        },
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&summary).unwrap_or_default()
    );
    let out = std::env::var_os("PRODUCT_KB_EVAL_OUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("product-kb-eval.json"));
    let report = json!({"summary": summary, "rows": rows});
    std::fs::write(
        &out,
        serde_json::to_string_pretty(&report).unwrap_or_default(),
    )
    .expect("the report is written");
    println!("rows: {}", out.display());
    assert_eq!(errors, 0, "some questions failed; see the rows");
}
