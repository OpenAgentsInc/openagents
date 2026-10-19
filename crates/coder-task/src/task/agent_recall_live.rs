//! The live halves of the scored memory stream: Jev's importance judge and
//! embedding relevance with an on-disk vector cache. Each blocks on a
//! runtime of its own, because a briefing is built on the agent's worker
//! thread. Unit tests use fakes through [`super::Judge`] and
//! [`knowledge::search::Embed`]; nothing here runs in a test against a
//! network.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use jev::Answer;
use knowledge::search::{Embed, bm25_texts, cosine};
use sha2::{Digest, Sha256};

use super::{Judge, Judged, Record, Relevance, SET, importance_from, judge_request};

fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("cannot start a runtime: {e}"))
}

/// Jev rates importance with `questions/memory-importance.json`.
pub struct JevJudge {
    client: jev::Client,
    runtime: tokio::runtime::Runtime,
}

impl JevJudge {
    /// # Errors
    /// When the runtime doesn't start.
    pub fn new(client: jev::Client) -> Result<Self, String> {
        Ok(Self {
            client,
            runtime: runtime()?,
        })
    }
}

impl Judge for JevJudge {
    fn judge(&mut self, agent: &str, records: &[&Record]) -> Vec<Result<Judged, String>> {
        let levels = SET
            .questions
            .get(&SET.gate)
            .and_then(|q| q.get("criteria"))
            .and_then(serde_json::Value::as_array)
            .map_or(4, Vec::len);
        let client = &self.client;
        let calls = records.iter().map(|record| async move {
            let request = judge_request(agent, record)?;
            let response = client
                .system_one(request)
                .await
                .map_err(|e| format!("Jev: {e}"))?;
            match response.answers.get(SET.gate.as_str()) {
                Some(Answer::Score(answer)) => Ok(Judged {
                    importance: importance_from(&answer.probabilities, answer.score, levels),
                    probabilities: answer.probabilities.clone(),
                    model: response.model.clone(),
                }),
                _ => Err("Jev didn't answer the importance question".to_string()),
            }
        });
        self.runtime.block_on(futures_util::future::join_all(calls))
    }
}

/// Cosine similarity over an embedder, its document vectors cached in
/// `embeddings-MODEL.bin` beside the agent's memory and keyed by the
/// SHA-256 of each text. After a failed call it ranks by BM25 for the rest
/// of its life, and says so in its basis.
pub struct Embedded<E: Embed> {
    embedder: E,
    dir: PathBuf,
    runtime: Option<tokio::runtime::Runtime>,
    failed: Option<String>,
}

impl<E: Embed> Embedded<E> {
    #[must_use]
    pub fn new(embedder: E, dir: &Path) -> Self {
        let (runtime, failed) = match runtime() {
            Ok(runtime) => (Some(runtime), None),
            Err(why) => (None, Some(why)),
        };
        Self {
            embedder,
            dir: dir.to_path_buf(),
            runtime,
            failed,
        }
    }

    fn cache_path(&self) -> PathBuf {
        let model: String = self
            .embedder
            .model()
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        self.dir.join(format!("embeddings-{model}.bin"))
    }

    fn similarities(&mut self, query: &str, texts: &[String]) -> Result<Vec<f64>, String> {
        let runtime = self.runtime.as_ref().ok_or("no runtime")?;
        let path = self.cache_path();
        let mut cache = read_vectors(&path);
        let digests: Vec<[u8; 32]> = texts.iter().map(|t| Sha256::digest(t).into()).collect();
        let mut missing: Vec<([u8; 32], String)> = Vec::new();
        for (digest, text) in digests.iter().zip(texts) {
            if !cache.contains_key(digest) && !missing.iter().any(|(d, _)| d == digest) {
                missing.push((*digest, text.clone()));
            }
        }
        let documents: Vec<String> = missing.iter().map(|(_, t)| t.clone()).collect();
        let (vectors, query, _) = runtime
            .block_on(
                self.embedder
                    .embed_with_query(documents, Some(query.to_string())),
            )
            .map_err(|e| e.message)?;
        let query = query.ok_or("the embedder returned no query vector")?;
        if vectors.len() != missing.len() {
            return Err("the embedder returned the wrong number of vectors".into());
        }
        let added = !missing.is_empty();
        for ((digest, _), vector) in missing.into_iter().zip(vectors) {
            cache.insert(digest, vector);
        }
        if added {
            // Keep only what the stream still holds.
            cache.retain(|digest, _| digests.contains(digest));
            write_vectors(&path, &cache)?;
        }
        Ok(digests
            .iter()
            .map(|digest| {
                cache
                    .get(digest)
                    .filter(|v| v.len() == query.len())
                    .map_or(0.0, |v| cosine(v, &query))
            })
            .collect())
    }
}

impl<E: Embed> Relevance for Embedded<E> {
    fn basis(&self) -> String {
        match &self.failed {
            Some(why) => format!("bm25 ({why})"),
            None => format!("cosine {}", self.embedder.model()),
        }
    }

    fn relevance(&mut self, query: &str, texts: &[String]) -> Vec<f64> {
        if self.failed.is_none() {
            match self.similarities(query, texts) {
                Ok(similarities) => return similarities,
                Err(why) => self.failed = Some(why),
            }
        }
        bm25_texts(texts, query)
    }
}

/// The vector cache: per record, a 32-byte digest, a little-endian `u32`
/// length, then that many little-endian `f32`s. A truncated tail is
/// dropped.
fn read_vectors(path: &Path) -> HashMap<[u8; 32], Vec<f32>> {
    let mut cache = HashMap::new();
    let Ok(bytes) = std::fs::read(path) else {
        return cache;
    };
    let mut at = 0;
    while at + 36 <= bytes.len() {
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&bytes[at..at + 32]);
        let len = u32::from_le_bytes(bytes[at + 32..at + 36].try_into().unwrap_or([0; 4])) as usize;
        at += 36;
        let Some(end) = len.checked_mul(4).map(|n| at + n) else {
            break;
        };
        if end > bytes.len() {
            break;
        }
        let vector = bytes[at..end]
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        cache.insert(digest, vector);
        at = end;
    }
    cache
}

fn write_vectors(path: &Path, cache: &HashMap<[u8; 32], Vec<f32>>) -> Result<(), String> {
    let ordered: BTreeMap<&[u8; 32], &Vec<f32>> = cache.iter().collect();
    let mut body = Vec::new();
    for (digest, vector) in ordered {
        body.extend_from_slice(digest.as_slice());
        body.extend_from_slice(&u32::try_from(vector.len()).unwrap_or(0).to_le_bytes());
        for value in vector {
            body.extend_from_slice(&value.to_le_bytes());
        }
    }
    let temp = path.with_extension("bin.tmp");
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    options
        .open(&temp)
        .and_then(|mut file| file.write_all(&body))
        .and_then(|()| std::fs::rename(&temp, path))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}
