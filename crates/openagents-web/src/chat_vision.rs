//! Answers about the images and PDFs sent to a chat (#11174).
//!
//! The hosted chat ([`openagents_chat::basic_coder::Relay`]) carries words
//! only, so a message with images or PDFs goes to a door that takes them:
//! [`VisionDoor`], one Open Responses call to the model the environments
//! setup agent uses on this server's own key
//! (`coder_environment_operator::studio::Studio::model_api`, the gateway on
//! the same host). The message's turn gets the files as content parts:
//! images as `input_image` data URLs, made smaller first when they are
//! large, and PDFs as `input_image` data URLs of type `application/pdf`,
//! which the gateway hands Gemini as inline data. What doesn't fit the
//! gateway's request size is named in the words and left out.
//!
//! The doors, in order (#11221, "Google first"):
//!
//! 1. Gemini (`gemini-3.8-flash`) on Vertex AI, on the prepaid Google
//!    credit: the same request translated by the gateway's own Vertex
//!    adapter ([`inference::upstream::vertex::body`]), the files as
//!    `inlineData` parts, sent to `generateContent` with the token
//!    [`TokenSource::from_env`] finds (the Cloud Run metadata server in
//!    production; `GOOGLE_APPLICATION_CREDENTIALS` on a laptop).
//! 2. The Open Responses door above ([`Endpoint`]).
//! 3. The hosted chat, with the words only (`fallback`): the files are
//!    named there, and the answer says it can't open them.
//!
//! A door that fails before saying anything hands the turn to the next,
//! and the reason is logged.

use std::future::Future;
use std::io::Cursor;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use inference::upstream::google::TokenSource;
use inference::upstream::vertex;
use openagents_chat::basic_coder::{self, Door, Failure, Reply, Role as TurnRole, Turn};
use openagents_chat::router::Context;
use serde_json::{Value, json};

use crate::App;
use crate::chat_files::{FileRef, Kind, read};
use crate::chat_store::Store;

/// The most bytes of encoded files one call carries: the gateway takes
/// request bodies up to 1 MiB, and the words and instructions need room.
pub(crate) const PART_BUDGET: usize = 760 * 1024;
/// The longest side an image is sent at.
const MAX_SIDE: u32 = 1568;
/// How long the model has to answer, leaving the hosted chat time to
/// answer instead within the chat's own deadline.
const DEADLINE: Duration = Duration::from_secs(60);
/// The most answer text kept.
const MAX_ANSWER_BYTES: usize = 256 * 1024;

/// Where the model is: an Open Responses address, its key, and the model.
#[derive(Clone)]
pub(crate) struct Endpoint {
    pub url: String,
    pub key: String,
    pub model: String,
}

impl std::fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Endpoint({}, {}, key redacted)", self.url, self.model)
    }
}

/// The model this server answers about files with, when it has one.
pub(crate) fn endpoint(app: &App) -> Option<Endpoint> {
    let (url, key, model) = app.config.environments.as_ref()?.model_api()?;
    Some(Endpoint { url, key, model })
}

/// The Gemini model on Vertex the files go to first.
pub(crate) const GEMINI: &str = "google/gemini-3.8-flash";

/// Gemini on Vertex AI: where `generateContent` is and the credential.
#[derive(Clone)]
pub(crate) struct Gemini {
    pub url: String,
    pub token: TokenSource,
    pub row: inference::upstream::ModelRow,
    pub thinking: vertex::Thinking,
}

impl std::fmt::Debug for Gemini {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Gemini({}, {:?})", self.url, self.token)
    }
}

impl Gemini {
    /// Gemini from the environment (`VERTEX_PROJECT`, `VERTEX_LOCATION`,
    /// `VERTEX_BASE_URL`, and a Google credential), when there is a
    /// credential.
    #[must_use]
    pub(crate) fn from_env() -> Option<Self> {
        let config = vertex::Config::from_env();
        if !config.token.present() {
            return None;
        }
        let (row, thinking) = config
            .models
            .iter()
            .find(|(row, _)| row.id == GEMINI)
            .cloned()?;
        let token = config.token.clone();
        let url = vertex::Vertex::new(config)
            .url(&row.upstream_model)
            .replace(":streamGenerateContent?alt=sse", ":generateContent");
        Some(Self {
            url,
            token,
            row,
            thinking,
        })
    }

    /// The `generateContent` body for an Open Responses `request`.
    pub(crate) fn body(&self, request: &Value) -> Result<Value, String> {
        let mut request = request.clone();
        request["model"] = json!(self.row.id);
        // Thinking costs time the chat's deadline needs.
        request["reasoning"] = json!({"effort": "low"});
        let request: inference::CreateResponse =
            serde_json::from_value(request).map_err(|error| error.to_string())?;
        vertex::body(&request, &self.row, self.thinking).map_err(|error| error.to_string())
    }

    async fn call(&self, request: &Value) -> Result<(String, Option<String>), String> {
        let body = self.body(request)?;
        let token = self.token.token().await?;
        let client = reqwest::Client::builder()
            .timeout(DEADLINE)
            .build()
            .map_err(|error| error.to_string())?;
        let response = client
            .post(&self.url)
            .bearer_auth(token.expose())
            .json(&body)
            .send()
            .await
            .map_err(|error| format!("Vertex could not be reached ({})", error.without_url()))?;
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            self.token.forget().await;
        }
        let reply: Value = response.json().await.map_err(|error| error.to_string())?;
        if !status.is_success() {
            let why = reply["error"]["message"].as_str().unwrap_or_default();
            return Err(format!(
                "Vertex answered {status}: {}",
                why.chars().take(300).collect::<String>()
            ));
        }
        let text = gemini_text(&reply).ok_or("Gemini answered nothing")?;
        Ok((text, Some(self.row.id.clone())))
    }
}

/// The answer's text in a `generateContent` reply: the first candidate's
/// text parts that are not thoughts, in order.
pub(crate) fn gemini_text(reply: &Value) -> Option<String> {
    let mut text = String::new();
    for part in reply["candidates"][0]["content"]["parts"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if part["thought"].as_bool() == Some(true) {
            continue;
        }
        if let Some(words) = part["text"].as_str() {
            text.push_str(words);
        }
    }
    let text = text.trim();
    (!text.is_empty()).then(|| text.chars().take(MAX_ANSWER_BYTES).collect())
}

/// The doors that take files on this server: Gemini on Vertex first,
/// then the Open Responses endpoint. `None` when there is neither.
#[derive(Clone, Debug)]
pub(crate) struct Doors {
    pub gemini: Option<Gemini>,
    pub endpoint: Option<Endpoint>,
}

/// This server's file doors (the Gemini credential is read once).
pub(crate) fn doors(app: &App) -> Option<Doors> {
    static GEMINI_DOOR: std::sync::OnceLock<Option<Gemini>> = std::sync::OnceLock::new();
    let gemini = GEMINI_DOOR.get_or_init(Gemini::from_env).clone();
    let endpoint = endpoint(app);
    (gemini.is_some() || endpoint.is_some()).then_some(Doors { gemini, endpoint })
}

/// One file as a content part.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Part {
    pub id: String,
    pub name: String,
    pub mime: &'static str,
    pub base64: String,
}

impl Part {
    fn content(&self) -> Value {
        json!({
            "type": "input_image",
            "image_url": format!("data:{};base64,{}", self.mime, self.base64),
        })
    }
}

/// The images and PDFs of `files` as content parts, in order, within
/// [`PART_BUDGET`] in all. A file that can't be read or doesn't fit is
/// left out (the words still name it).
pub(crate) async fn parts(store: &Store, owner: &str, chat: &str, files: &[FileRef]) -> Vec<Part> {
    let mut out = Vec::new();
    let mut left = PART_BUDGET;
    for file in files.iter().filter(|file| file.kind != Kind::Text) {
        let Ok(Some((_, bytes))) = read(store, owner, chat, &file.id).await else {
            continue;
        };
        let Some((mime, bytes)) = prepared(file.kind, bytes) else {
            continue;
        };
        let base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        if base64.len() > left {
            continue;
        }
        left -= base64.len();
        out.push(Part {
            id: file.id.clone(),
            name: file.name.clone(),
            mime,
            base64,
        });
    }
    out
}

/// A file's bytes as they are sent, with their type: a PNG or JPEG image
/// larger than [`MAX_SIDE`] or than a quarter of the budget is sent as a
/// smaller JPEG; anything else as it is.
pub(crate) fn prepared(kind: Kind, bytes: Vec<u8>) -> Option<(&'static str, Vec<u8>)> {
    let mime = match kind {
        Kind::Text => return None,
        Kind::Pdf => return Some(("application/pdf", bytes)),
        other => other.mime(),
    };
    if !matches!(kind, Kind::Png | Kind::Jpeg) {
        return Some((mime, bytes));
    }
    let format = if kind == Kind::Png {
        image::ImageFormat::Png
    } else {
        image::ImageFormat::Jpeg
    };
    let mut reader = image::ImageReader::with_format(Cursor::new(&bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let Ok(decoded) = reader.decode() else {
        return Some((mime, bytes));
    };
    let large = decoded.width().max(decoded.height()) > MAX_SIDE;
    if !large && bytes.len() <= PART_BUDGET / 4 {
        return Some((mime, bytes));
    }
    let smaller = if large {
        decoded.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Triangle)
    } else {
        decoded
    };
    let mut out = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80);
    if smaller.to_rgb8().write_with_encoder(encoder).is_err() {
        return Some((mime, bytes));
    }
    Some(("image/jpeg", out))
}

/// The Open Responses request for `turns` with `parts` on the last user
/// turn.
pub(crate) fn request(model: &str, instructions: &str, turns: &[Turn], parts: &[Part]) -> Value {
    let last_user = turns.iter().rposition(|turn| turn.role == TurnRole::User);
    let input: Vec<Value> = turns
        .iter()
        .enumerate()
        .map(|(index, turn)| match turn.role {
            TurnRole::User if Some(index) == last_user => {
                let mut content = vec![json!({"type": "input_text", "text": turn.text})];
                content.extend(parts.iter().map(Part::content));
                json!({"type": "message", "role": "user", "content": content})
            }
            TurnRole::User => json!({"type": "message", "role": "user", "content": turn.text}),
            TurnRole::Assistant => {
                json!({"type": "message", "role": "assistant", "content": turn.text})
            }
        })
        .collect();
    json!({
        "model": model,
        "instructions": instructions,
        "input": input,
        "stream": false,
        "store": false,
    })
}

/// The answer's text in an Open Responses reply: its output messages'
/// text, in order.
pub(crate) fn answer_text(reply: &Value) -> Option<String> {
    let mut text = String::new();
    for item in reply["output"].as_array()? {
        if item["type"] != "message" {
            continue;
        }
        for part in item["content"].as_array().into_iter().flatten() {
            if part["type"] == "output_text"
                && let Some(words) = part["text"].as_str()
            {
                text.push_str(words);
            }
        }
    }
    let text = text.trim();
    (!text.is_empty()).then(|| text.chars().take(MAX_ANSWER_BYTES).collect())
}

/// A door that answers with the message's images and PDFs
/// ([`request`]): Gemini on Vertex, then the endpoint; `fallback` answers
/// when neither can.
pub(crate) struct VisionDoor {
    pub doors: Doors,
    pub parts: Vec<Part>,
    /// The hosted chat and the turns it answers instead (the files named
    /// as ones it can't open).
    pub fallback: Option<(Box<dyn Door>, Vec<Turn>)>,
}

impl VisionDoor {
    async fn call(endpoint: &Endpoint, body: Value) -> Result<(String, Option<String>), String> {
        let client = reqwest::Client::builder()
            .timeout(DEADLINE)
            .build()
            .map_err(|error| error.to_string())?;
        let response = client
            .post(&endpoint.url)
            .bearer_auth(&endpoint.key)
            .json(&body)
            .send()
            .await
            .map_err(|error| error.to_string())?;
        let status = response.status();
        let reply: Value = response.json().await.map_err(|error| error.to_string())?;
        if !status.is_success() {
            return Err(format!("the model answered {status}"));
        }
        let text = answer_text(&reply).ok_or("the model answered nothing")?;
        Ok((text, reply["model"].as_str().map(str::to_owned)))
    }
}

impl Door for VisionDoor {
    fn ask(
        &self,
        turns: Vec<Turn>,
        context: Context,
        reply: Arc<Mutex<Reply>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let model = self
            .doors
            .endpoint
            .as_ref()
            .map_or(GEMINI, |endpoint| endpoint.model.as_str());
        let body = request(
            model,
            basic_coder::instructions(&context),
            &turns,
            &self.parts,
        );
        let doors = self.doors.clone();
        let fallback = self
            .fallback
            .as_ref()
            .map(|(door, turns)| door.ask(turns.clone(), context.clone(), reply.clone()));
        Box::pin(async move {
            let mut answer = Err("no door takes files".to_owned());
            if let Some(gemini) = &doors.gemini {
                let started = std::time::Instant::now();
                answer = gemini.call(&body).await;
                match &answer {
                    Ok(_) => eprintln!(
                        "openagents-web: chat vision: gemini-on-vertex answered in {} ms",
                        started.elapsed().as_millis()
                    ),
                    Err(why) => eprintln!(
                        "openagents-web: chat vision: gemini-on-vertex missed after {} ms, \
                         trying the next door: {why}",
                        started.elapsed().as_millis()
                    ),
                }
            }
            if answer.is_err()
                && let Some(endpoint) = &doors.endpoint
            {
                answer = Self::call(endpoint, body).await;
            }
            match answer {
                Ok((text, model)) => {
                    let mut reply = basic_coder::lock(&reply);
                    reply.text = text;
                    reply.model = model;
                    reply.meta.tier = Some("model".into());
                    reply.meta.route = Some("vision".into());
                    reply.done = true;
                }
                Err(error) => {
                    eprintln!("openagents-web: chat vision: {error}");
                    match fallback {
                        Some(job) => job.await,
                        None => {
                            basic_coder::lock(&reply).failure = Some(Failure::Transport(error));
                        }
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parts_ride_on_the_last_user_turn() {
        let part = Part {
            id: "a".repeat(32),
            name: "shot.png".into(),
            mime: "image/png",
            base64: "iVBO".into(),
        };
        let turns = vec![
            Turn::user("first"),
            Turn::assistant("an answer", None),
            Turn::user("what is in this picture?"),
        ];
        let body = request("google/gemini-3.8-flash", "Be brief.", &turns, &[part]);
        assert_eq!(body["model"], "google/gemini-3.8-flash");
        assert_eq!(body["input"][0]["content"], "first");
        assert_eq!(body["input"][1]["role"], "assistant");
        let last = &body["input"][2]["content"];
        assert_eq!(last[0]["type"], "input_text");
        assert_eq!(last[1]["type"], "input_image");
        assert_eq!(last[1]["image_url"], "data:image/png;base64,iVBO");
    }

    #[test]
    fn the_answer_is_the_output_messages_text() {
        let reply = json!({"model": "m", "output": [
            {"type": "reasoning", "summary": []},
            {"type": "message", "content": [
                {"type": "output_text", "text": "A cat "},
                {"type": "output_text", "text": "on a desk."}
            ]}
        ]});
        assert_eq!(answer_text(&reply).as_deref(), Some("A cat on a desk."));
        assert_eq!(answer_text(&json!({"output": []})), None);
    }

    #[test]
    fn gemini_gets_inline_data_and_answers_without_thoughts() {
        let gemini = Gemini {
            url: "http://vertex.invalid/v1/x:generateContent".into(),
            token: TokenSource::none(),
            row: vertex::default_models()
                .into_iter()
                .find(|(row, _)| row.id == GEMINI)
                .unwrap()
                .0,
            thinking: vertex::Thinking::Level,
        };
        let parts = [
            Part {
                id: "a".repeat(32),
                name: "shot.png".into(),
                mime: "image/png",
                base64: "iVBO".into(),
            },
            Part {
                id: "b".repeat(32),
                name: "paper.pdf".into(),
                mime: "application/pdf",
                base64: "JVBE".into(),
            },
        ];
        let turns = vec![Turn::user("what is in these?")];
        let open = request("openrouter/whatever", "Be brief.", &turns, &parts);
        let body = gemini.body(&open).unwrap();
        let user = &body["contents"][0];
        assert_eq!(user["role"], "user");
        assert_eq!(user["parts"][0]["text"], "what is in these?");
        assert_eq!(user["parts"][1]["inlineData"]["mimeType"], "image/png");
        assert_eq!(user["parts"][1]["inlineData"]["data"], "iVBO");
        assert_eq!(
            user["parts"][2]["inlineData"]["mimeType"],
            "application/pdf"
        );
        assert_eq!(body["systemInstruction"]["parts"][0]["text"], "Be brief.");
        assert_eq!(
            body["generationConfig"]["thinkingConfig"]["thinkingLevel"],
            "low"
        );
        let reply = json!({"candidates": [{"content": {"parts": [
            {"text": "Let me look.", "thought": true},
            {"text": "A cat "},
            {"text": "and a paper."}
        ]}}]});
        assert_eq!(gemini_text(&reply).as_deref(), Some("A cat and a paper."));
        assert_eq!(gemini_text(&json!({"candidates": []})), None);
    }

    /// A real call to Gemini on Vertex with an image and a PDF; needs a
    /// Google credential (`GOOGLE_APPLICATION_CREDENTIALS`).
    /// `cargo test -p openagents-web --lib live_gemini -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "calls Vertex AI"]
    async fn live_gemini_reads_an_image_and_a_pdf() {
        let gemini = Gemini::from_env().expect("a Google credential");
        let red = image::RgbImage::from_pixel(64, 64, image::Rgb([220, 20, 20]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(red)
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let stream = "BT /F1 24 Tf 72 720 Td (The secret word is PELICAN.) Tj ET";
        let pdf = format!(
            "%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 612 792]/Contents 4 0 R/Resources<</Font<</F1 5 0 R>>>>>>endobj\n4 0 obj<</Length {}>>stream\n{stream}\nendstream endobj\n5 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj\ntrailer<</Root 1 0 R>>\n%%EOF\n",
            stream.len()
        );
        let encode = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
        let parts = [
            Part {
                id: "a".repeat(32),
                name: "red.png".into(),
                mime: "image/png",
                base64: encode(&png),
            },
            Part {
                id: "b".repeat(32),
                name: "word.pdf".into(),
                mime: "application/pdf",
                base64: encode(pdf.as_bytes()),
            },
        ];
        let turns = vec![Turn::user(
            "What color is the image, and what is the secret word in the PDF? One line.",
        )];
        let body = request(GEMINI, "Be brief.", &turns, &parts);
        let started = std::time::Instant::now();
        let (text, model) = gemini.call(&body).await.unwrap();
        println!(
            "gemini-on-vertex vision: {} ms, {model:?}: {text}",
            started.elapsed().as_millis()
        );
        assert!(text.to_ascii_lowercase().contains("red"), "{text}");
        assert!(text.to_ascii_uppercase().contains("PELICAN"), "{text}");
    }

    #[test]
    fn a_large_image_is_sent_smaller_and_a_pdf_as_it_is() {
        let big = image::RgbImage::from_pixel(3000, 1000, image::Rgb([200, 30, 30]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(big)
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let (mime, bytes) = prepared(Kind::Png, png).unwrap();
        assert_eq!(mime, "image/jpeg");
        let shrunk = image::load_from_memory(&bytes).unwrap();
        assert_eq!(shrunk.width().max(shrunk.height()), MAX_SIDE);
        let pdf = b"%PDF-1.7\n".to_vec();
        assert_eq!(
            prepared(Kind::Pdf, pdf.clone()),
            Some(("application/pdf", pdf))
        );
        assert_eq!(prepared(Kind::Text, b"x".to_vec()), None);
    }
}
