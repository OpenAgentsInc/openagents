//! Answers on this device: a local Psionic server
//! (`psionic-openai-server`) speaking the OpenAI chat-completions API on
//! `127.0.0.1`. The plaintext never leaves this computer.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use zeroize::Zeroizing;

use super::api::Plain;

/// Where the local server answers unless `--psionic` or
/// `OPENAGENTS_PSIONIC_URL` says otherwise.
pub(crate) const DEFAULT_URL: &str = "http://127.0.0.1:8091";
/// The most text one question sends to a local model, in characters.
pub(crate) const MAX_TEXT: usize = 48_000;

pub(crate) trait LocalModel {
    /// The model the server serves, or `None` when nothing answers.
    fn model(&self) -> Option<String>;
    /// One answer to `request` (an OpenAI chat-completions body).
    fn complete(&self, request: &Value) -> Result<String, String>;
}

/// The local server at `url`.
pub(crate) struct Psionic {
    pub url: String,
}

impl Psionic {
    pub(crate) fn from(option: Option<&str>) -> Self {
        let url = option
            .map(str::to_owned)
            .or_else(|| std::env::var("OPENAGENTS_PSIONIC_URL").ok())
            .unwrap_or_else(|| DEFAULT_URL.to_owned());
        Self {
            url: url.trim_end_matches('/').to_owned(),
        }
    }

    fn client(timeout: Duration) -> Option<reqwest::blocking::Client> {
        reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(timeout)
            .build()
            .ok()
    }
}

impl LocalModel for Psionic {
    fn model(&self) -> Option<String> {
        let body: Value = Self::client(Duration::from_secs(5))?
            .get(format!("{}/v1/models", self.url))
            .send()
            .ok()
            .filter(|response| response.status().is_success())?
            .json()
            .ok()?;
        body["data"][0]["id"].as_str().map(str::to_owned)
    }

    fn complete(&self, request: &Value) -> Result<String, String> {
        let client =
            Self::client(Duration::from_secs(900)).ok_or("Couldn't start the local web client.")?;
        let body = Zeroizing::new(
            serde_json::to_vec(request).map_err(|_| "The question couldn't be written.")?,
        );
        let response = client
            .post(format!("{}/v1/chat/completions", self.url))
            .header("content-type", "application/json")
            .body(body.to_vec())
            .send()
            .map_err(|_| {
                format!(
                    "The model on this computer ({}) stopped answering.",
                    self.url
                )
            })?;
        let status = response.status();
        let value: Value = response.json().unwrap_or(Value::Null);
        if !status.is_success() {
            let why = value["error"]["message"]
                .as_str()
                .or_else(|| value["error"].as_str())
                .unwrap_or("no reason given");
            return Err(format!("The model on this computer refused: {why}"));
        }
        answer_text(&value).ok_or_else(|| "The model on this computer sent no answer.".to_owned())
    }
}

/// The answer in a chat-completions reply.
pub(crate) fn answer_text(value: &Value) -> Option<String> {
    let text = value["choices"][0]["message"]["content"].as_str()?.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// The chat-completions request for `question` about `files`. Each file
/// must be text.
pub(crate) fn chat_request(model: &str, question: &str, files: &[Plain]) -> Result<Value, String> {
    let mut context = Zeroizing::new(String::new());
    for file in files {
        let text = std::str::from_utf8(&file.data).map_err(|_| {
            format!(
                "{} isn't a text file, and the model on this computer reads text only. To have Google Gemini read it, pass --route fast.",
                file.name
            )
        })?;
        context.push_str(&format!("--- {} ---\n{}\n\n", file.name, text));
    }
    if context.chars().count() > MAX_TEXT {
        return Err(format!(
            "These files are too long for the model on this computer (over {MAX_TEXT} characters)."
        ));
    }
    Ok(json!({
        "model": model,
        "stream": false,
        "max_tokens": 1024,
        "messages": [
            {
                "role": "system",
                "content": "You answer questions about the person's own files. Use only the files given. If they don't hold the answer, say so."
            },
            {
                "role": "user",
                "content": format!("{}Question: {question}", context.as_str()),
            }
        ]
    }))
}

/// The server program. `psionic-gpt-oss-server` (GPT-OSS on Metal) also
/// takes `--allow-origin`, but on 2026-10-10 its Metal path answered with
/// blank text on this Mac, so GPT-OSS goes through this one, on the CPU.
pub(crate) const SERVER: &str = "psionic-openai-server";

/// The server program named `name`, if it's on this computer: the folder
/// in `OPENAGENTS_PSIONIC_BIN`, then `PATH`, then `~/.openagents/bin`.
pub(crate) fn find_server(home: &Path, name: &str) -> Option<PathBuf> {
    std::env::var_os("OPENAGENTS_PSIONIC_BIN")
        .map(PathBuf::from)
        .into_iter()
        .chain(
            std::env::var_os("PATH")
                .into_iter()
                .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>()),
        )
        .chain([home.join(".openagents/bin")])
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}

/// The small model the vault answers with by default: Qwen2.5 0.5B
/// Instruct, about 15 s for a short answer on a laptop's CPU.
pub(crate) const SMALL_MODEL: &str = "qwen2.5-0.5b-instruct-q8_0.gguf";
/// Where to get it.
pub(crate) const SMALL_MODEL_URL: &str = "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q8_0.gguf";

/// A model file on this computer: [`SMALL_MODEL`] in `~/.openagents/models`,
/// else `gpt-oss-20b-MXFP4.gguf` there, else the first `.gguf` there.
pub(crate) fn find_model(home: &Path) -> Option<PathBuf> {
    let dir = home.join(".openagents/models");
    for preferred in [SMALL_MODEL, "gpt-oss-20b-MXFP4.gguf"] {
        let path = dir.join(preferred);
        if path.is_file() {
            return Some(path);
        }
    }
    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "gguf"))
        .collect();
    found.sort();
    found.into_iter().next()
}

/// The command that serves `model` on `port` to pages on `origin`.
pub(crate) fn serve_command(server: &Path, model: &Path, port: u16, origin: &str) -> Vec<String> {
    let mut command = vec![
        server.display().to_string(),
        "-m".into(),
        model.display().to_string(),
    ];
    let name = model.to_string_lossy().to_ascii_lowercase();
    // Psionic's Metal decoders are Gemma 4 and Qwen 3.5/3.8; everything
    // else (Qwen2.5, gpt-oss) runs on the CPU.
    let metal = [
        "gemma4", "gemma-4", "qwen3.5", "qwen35", "qwen3.8", "qwen38",
    ];
    if cfg!(target_os = "macos") && metal.iter().any(|family| name.contains(family)) {
        command.extend(["--backend".into(), "metal".into()]);
    }
    command.extend([
        "--host".into(),
        "127.0.0.1".into(),
        "--port".into(),
        port.to_string(),
        "--allow-origin".into(),
        origin.to_owned(),
    ]);
    command
}
