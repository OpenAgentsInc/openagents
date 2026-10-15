//! Warm-up before `/health` (#11636): a few seeded synthetic decisions of
//! different shapes run through a GPU lane before the server listens, so
//! the first real requests do not pay first-use costs. On the M5 Max a
//! fresh server's first prefill took 382 ms against 206 ms steady; after
//! the warm-up it takes the steady time.
//!
//! The idea is REAX's (it runs about 200 seeded random batch shapes before
//! answering health); the shapes here are prompt lengths of one chunk and
//! two, and every question type. With batching on, the warm-up requests
//! run concurrently so cohorts of several requests are exercised too. The
//! logit cache is cleared afterwards, so no warm-up answer is ever served.
//!
//! `PSIONIC_CLEF_WARMUP` sets the number of warm-up requests (default
//! [`DEFAULT_REQUESTS`]; `0` turns it off).

use std::time::Instant;

use serde_json::{Map, Value, json};

use super::ClefDecisionLane;

/// Warm-up requests when `PSIONIC_CLEF_WARMUP` is unset.
pub const DEFAULT_REQUESTS: usize = 4;

/// Most warm-up requests `PSIONIC_CLEF_WARMUP` takes.
const MAX_REQUESTS: usize = 256;

const WORDS: &[&str] = &[
    "deploy", "billing", "webhook", "checkout", "refund", "invoice", "calendar", "meeting",
    "review", "branch", "merge", "release", "latency", "timeout", "password", "admin", "user",
    "plan", "limit", "ticket", "outage", "region", "import", "export", "expense", "receipt",
    "agent", "report", "schema", "query", "retry", "queue", "cache", "token", "error", "crash",
    "upgrade", "customer", "support", "approve", "decline", "urgent", "weekly", "budget",
];

/// What a warm-up did.
#[derive(Clone, Copy, Debug)]
pub struct WarmUp {
    pub requests: usize,
    pub failed: usize,
    pub seconds: f64,
}

/// Warm-up requests from `PSIONIC_CLEF_WARMUP`.
///
/// # Errors
///
/// When the value is not a whole number up to [`MAX_REQUESTS`].
pub fn requests_from_env() -> Result<usize, String> {
    match std::env::var("PSIONIC_CLEF_WARMUP") {
        Err(std::env::VarError::NotPresent) => Ok(DEFAULT_REQUESTS),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(String::from("PSIONIC_CLEF_WARMUP must be a whole number"))
        }
        Ok(value) => match value.trim().parse::<usize>() {
            Ok(count) if count <= MAX_REQUESTS => Ok(count),
            _ => Err(format!(
                "PSIONIC_CLEF_WARMUP must be a whole number from 0 to {MAX_REQUESTS}"
            )),
        },
    }
}

/// A small deterministic generator (xorshift64*), so warm-up shapes are the
/// same on every start.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn words(&mut self, count: usize) -> String {
        let mut text = String::with_capacity(count * 8);
        for index in 0..count {
            if index > 0 {
                text.push(if index % 13 == 0 { '\n' } else { ' ' });
            }
            text.push_str(WORDS[self.below(WORDS.len())]);
        }
        text
    }
}

/// Approximate prompt lengths (in words, about one token each) for `count`
/// requests: the first three are a short prompt, a medium one and one just
/// over a chunk (two chunks); the rest are log-uniform up to `max_words`.
fn lengths(count: usize, chunk: usize, max_words: usize, rng: &mut Rng) -> Vec<usize> {
    let max_words = max_words.max(64);
    let fixed = [48, 200, chunk + chunk / 4];
    (0..count)
        .map(|index| {
            let words = fixed.get(index).copied().unwrap_or_else(|| {
                let log_min = 32f64.ln();
                let log_max = (max_words as f64).ln();
                let unit = (rng.next() >> 11) as f64 / (1u64 << 53) as f64;
                (log_min + unit * (log_max - log_min)).exp() as usize
            });
            words.clamp(16, max_words)
        })
        .collect()
}

/// One synthetic request of about `words` prompt words, with a question of
/// each type rotating by `index`.
fn request(model: &str, words: usize, index: usize, rng: &mut Rng) -> String {
    let mut questions = Map::new();
    let kinds = 1 + index % 3;
    for kind in 0..kinds {
        let id = format!("q{kind}");
        let question = match (index + kind) % 3 {
            0 => json!({"type": "noul", "instructions": rng.words(8)}),
            1 => {
                let options = 2 + rng.below(7);
                let criteria: Map<String, Value> = (0..options)
                    .map(|option| (format!("o{option}"), Value::String(rng.words(4))))
                    .collect();
                json!({"type": "choice", "instructions": rng.words(6), "criteria": criteria})
            }
            _ => {
                let levels = 3 + rng.below(3);
                let criteria: Vec<Value> =
                    (0..levels).map(|_| Value::String(rng.words(3))).collect();
                json!({"type": "score", "instructions": rng.words(6), "criteria": criteria})
            }
        };
        questions.insert(id, question);
    }
    json!({
        "model": model,
        "state": format!("warm-up {index}\n{}", rng.words(words)),
        "questions": questions,
    })
    .to_string()
}

impl ClefDecisionLane {
    /// Runs `count` seeded synthetic decisions before the server answers,
    /// `batch_slots` at a time, then clears the logit cache. A failed
    /// warm-up request is counted, not fatal: the lane loaded and real
    /// requests report their own errors.
    pub fn warm_up(&self, count: usize) -> WarmUp {
        let began = Instant::now();
        // Metal and CUDA trunks only: a CPU lane's first-use cost is small
        // next to its prefill, and a remote worker is a separate server.
        if count == 0 || self.cuda.is_none() || self.remote.is_some() {
            return WarmUp {
                requests: 0,
                failed: 0,
                seconds: 0.0,
            };
        }
        let mut rng = Rng(0x11636_5EED_C1EF_u64);
        let chunk = self.prefill_chunk().max(16);
        // Leave room for the questions and template inside the token budget.
        let max_words = (self.limits.max_tokens / 2).min(2 * chunk);
        let bodies: Vec<String> = lengths(count, chunk, max_words, &mut rng)
            .into_iter()
            .enumerate()
            .map(|(index, words)| request(&self.id, words, index, &mut rng))
            .collect();
        let threads = self.batch_slots.clamp(1, bodies.len());
        let next = std::sync::atomic::AtomicUsize::new(0);
        let failed = std::sync::atomic::AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    loop {
                        let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(body) = bodies.get(index) else {
                            break;
                        };
                        if self.decide(body).is_err() {
                            failed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                });
            }
        });
        self.logit_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        WarmUp {
            requests: bodies.len(),
            failed: failed.into_inner(),
            seconds: began.elapsed().as_secs_f64(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warm_up_shapes_are_seeded_and_cover_chunks() {
        let mut a = Rng(7);
        let mut b = Rng(7);
        assert_eq!(lengths(8, 512, 2048, &mut a), lengths(8, 512, 2048, &mut b));
        let mut rng = Rng(7);
        let shapes = lengths(6, 512, 2048, &mut rng);
        assert_eq!(&shapes[..3], &[48, 200, 640]);
        assert!(shapes.iter().all(|&words| (16..=2048).contains(&words)));
        let mut rng = Rng(7);
        for index in 0..6 {
            let body: Value =
                serde_json::from_str(&request("clef-flash", 40, index, &mut rng)).expect("json");
            let questions = body["questions"].as_object().expect("questions");
            assert_eq!(questions.len(), 1 + index % 3);
        }
    }
}
