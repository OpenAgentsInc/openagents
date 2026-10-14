//! The Gmail tools: `gmail.search`, `gmail.read`, and `gmail.labels` read
//! (policy allow); `gmail.draft` makes a draft the person can open, change,
//! or throw away in Gmail (policy allow: nobody sees a draft); and
//! `gmail.send` sends a draft, which always asks (policy
//! require-approval): the host runs it only after the person confirmed the
//! exact draft [`card`] showed them.
//!
//! Results carry what a model needs to answer (who, subject, date,
//! snippet, text, attachments by name) and a link to each message.
//! Nothing here logs or keeps mail.

use std::fmt::Write as _;

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use serde::Deserialize;
use serde_json::{Value, json};

use super::rest::{ApiError, Cite, Rest, capped, valid_address, valid_id};
use super::{Endpoints, GMAIL_COMPOSE, GMAIL_FULL, GMAIL_MODIFY, GMAIL_READONLY};
use crate::core::{HttpMethod, Policy, ToolSpec};

/// The most messages one search returns.
pub const MAX_RESULTS: u32 = 20;
/// The most text one read returns.
pub const MAX_TEXT_CHARS: usize = 30_000;
/// The most messages of a thread read.
pub const MAX_THREAD_MESSAGES: usize = 15;
/// The most body characters a draft's card shows.
pub const CARD_BODY_CHARS: usize = 1_500;
/// The longest draft body.
const MAX_BODY_CHARS: usize = 20_000;

/// Gmail as one connection.
#[derive(Clone, Debug)]
pub struct Gmail {
    rest: Rest,
    root: String,
}

/// One message as a search lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    pub id: String,
    pub thread: String,
    pub from: String,
    pub subject: String,
    pub date: String,
    pub snippet: String,
    pub unread: bool,
}

impl Summary {
    #[must_use]
    pub fn link(&self) -> String {
        thread_link(&self.thread)
    }

    #[must_use]
    pub fn json(&self) -> Value {
        json!({
            "id": self.id,
            "thread": self.thread,
            "from": self.from,
            "subject": self.subject,
            "date": self.date,
            "snippet": self.snippet,
            "unread": self.unread,
            "link": self.link(),
        })
    }

    #[must_use]
    pub fn cite(&self) -> Cite {
        Cite {
            title: title_or(&self.subject, "(no subject)"),
            link: self.link(),
        }
    }
}

/// Where a thread opens in Gmail.
#[must_use]
pub fn thread_link(thread: &str) -> String {
    format!("https://mail.google.com/mail/#all/{thread}")
}

/// Where a draft opens in Gmail.
#[must_use]
pub fn draft_link(message: &str) -> String {
    format!("https://mail.google.com/mail/#drafts?compose={message}")
}

fn title_or(text: &str, empty: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        empty.to_owned()
    } else {
        text.to_owned()
    }
}

/// An attachment, by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub name: String,
    pub mime_type: String,
    pub size: u64,
}

/// One message read as text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub id: String,
    pub thread: String,
    pub from: String,
    pub to: String,
    pub cc: String,
    pub subject: String,
    pub date: String,
    pub text: String,
    pub attachments: Vec<Attachment>,
    /// The `Message-ID` header, for a reply.
    pub message_id: String,
}

impl Message {
    fn json(&self) -> Value {
        json!({
            "id": self.id,
            "from": self.from,
            "to": self.to,
            "cc": self.cc,
            "subject": self.subject,
            "date": self.date,
            "text": self.text,
            "attachments": self.attachments.iter().map(|a| json!({
                "name": a.name, "type": a.mime_type, "size": a.size,
            })).collect::<Vec<_>>(),
        })
    }
}

/// A draft as Gmail keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    pub id: String,
    pub message: String,
    pub thread: String,
    pub to: String,
    pub cc: String,
    pub subject: String,
    pub body: String,
}

impl Draft {
    #[must_use]
    pub fn link(&self) -> String {
        draft_link(&self.message)
    }

    #[must_use]
    pub fn json(&self) -> Value {
        json!({
            "draft": self.id,
            "to": self.to,
            "cc": self.cc,
            "subject": self.subject,
            "link": self.link(),
            "sent": false,
        })
    }
}

/// What a draft says, as the person is shown it before it is sent. A send
/// in a chat runs only when this exact text was shown, so it is the same
/// every time it is made from the same draft.
#[must_use]
pub fn card(draft: &Draft) -> String {
    let mut out = format!("**Draft email** · [Open in Gmail]({})\n\n", draft.link());
    let _ = writeln!(out, "To: {}  ", one_line(&draft.to));
    if !draft.cc.trim().is_empty() {
        let _ = writeln!(out, "Cc: {}  ", one_line(&draft.cc));
    }
    let _ = writeln!(
        out,
        "Subject: {}\n",
        one_line(&title_or(&draft.subject, "(no subject)"))
    );
    let body = draft.body.replace("\r\n", "\n");
    let (body, cut) = capped(body.trim(), CARD_BODY_CHARS);
    for line in body.lines() {
        if line.trim().is_empty() {
            out.push_str(">\n");
        } else {
            let _ = writeln!(out, "> {}", line.trim_end());
        }
    }
    if cut {
        out.push_str("> …\n");
    }
    out.trim_end().to_owned()
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A header's value in a Gmail `payload`.
fn header(payload: &Value, name: &str) -> String {
    payload["headers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|h| {
            h["name"]
                .as_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        })
        .and_then(|h| h["value"].as_str())
        .unwrap_or_default()
        .to_owned()
}

/// Gmail's base64 (URL-safe, padded or not).
fn decode(data: &str) -> Option<Vec<u8>> {
    let data = data.trim_end_matches('=');
    URL_SAFE_NO_PAD
        .decode(data)
        .ok()
        .or_else(|| STANDARD.decode(data).ok())
}

/// The plain text and the HTML of a message's parts, and its attachments.
fn walk(part: &Value, plain: &mut String, html: &mut String, attachments: &mut Vec<Attachment>) {
    let mime = part["mimeType"].as_str().unwrap_or_default();
    let name = part["filename"].as_str().unwrap_or_default();
    if !name.is_empty() {
        attachments.push(Attachment {
            name: name.to_owned(),
            mime_type: mime.to_owned(),
            size: part["body"]["size"].as_u64().unwrap_or(0),
        });
        return;
    }
    if let Some(parts) = part["parts"].as_array() {
        for inner in parts {
            walk(inner, plain, html, attachments);
        }
        return;
    }
    let Some(bytes) = part["body"]["data"].as_str().and_then(decode) else {
        return;
    };
    let text = String::from_utf8_lossy(&bytes);
    if mime == "text/plain" {
        plain.push_str(&text);
        plain.push('\n');
    } else if mime == "text/html" {
        html.push_str(&text);
        html.push('\n');
    }
}

/// HTML as plain text: tags dropped, blocks as lines, common entities
/// decoded.
#[must_use]
pub fn html_text(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(html.len() / 2);
    let mut at = 0;
    while at < html.len() {
        let rest = &lower[at..];
        if rest.starts_with("<script") || rest.starts_with("<style") {
            let close = if rest.starts_with("<script") {
                "</script>"
            } else {
                "</style>"
            };
            at = rest
                .find(close)
                .map_or(html.len(), |i| at + i + close.len());
            continue;
        }
        if rest.starts_with('<') {
            let end = rest.find('>').map_or(html.len(), |i| at + i + 1);
            let tag = &lower[at..end];
            if ["<br", "</p", "</div", "</tr", "</li", "</h", "<p", "<li"]
                .iter()
                .any(|t| tag.starts_with(t))
            {
                out.push('\n');
            }
            at = end;
            continue;
        }
        let next = rest.find('<').map_or(html.len(), |i| at + i);
        out.push_str(&html[at..next]);
        at = next;
    }
    let out = out
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    let mut lines: Vec<&str> = Vec::new();
    for line in out.lines().map(str::trim) {
        if line.is_empty() && lines.last().is_none_or(|l| l.is_empty()) {
            continue;
        }
        lines.push(line);
    }
    lines.join("\n").trim().to_owned()
}

fn message_of(value: &Value) -> Message {
    let payload = &value["payload"];
    let mut plain = String::new();
    let mut html = String::new();
    let mut attachments = Vec::new();
    walk(payload, &mut plain, &mut html, &mut attachments);
    let text = if plain.trim().is_empty() {
        html_text(&html)
    } else {
        plain.replace("\r\n", "\n").trim().to_owned()
    };
    Message {
        id: value["id"].as_str().unwrap_or_default().to_owned(),
        thread: value["threadId"].as_str().unwrap_or_default().to_owned(),
        from: header(payload, "From"),
        to: header(payload, "To"),
        cc: header(payload, "Cc"),
        subject: header(payload, "Subject"),
        date: header(payload, "Date"),
        message_id: header(payload, "Message-ID"),
        text,
        attachments,
    }
}

/// A header value as RFC 2047 when it isn't plain ASCII.
fn encoded_word(text: &str) -> String {
    if text.is_ascii() {
        text.to_owned()
    } else {
        format!("=?UTF-8?B?{}?=", STANDARD.encode(text.as_bytes()))
    }
}

/// What `gmail.draft` takes.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftArgs {
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub body: String,
    /// A message id this answers, to keep it in that thread.
    #[serde(default)]
    pub reply_to: Option<String>,
}

impl DraftArgs {
    /// Checked: one to 20 plain addresses, a one-line subject, a body
    /// under the limit.
    fn check(&self) -> Result<(), ApiError> {
        let bad = |why: &str| Err(ApiError::Invalid(why.into()));
        if self.to.is_empty() || self.to.len() + self.cc.len() > 20 {
            return bad("A draft goes to one to 20 addresses.");
        }
        if !self
            .to
            .iter()
            .chain(&self.cc)
            .all(|a| valid_address(a.trim()))
        {
            return bad("Each recipient is one plain email address, like name@example.com.");
        }
        if self.subject.contains(['\r', '\n']) || self.subject.chars().count() > 300 {
            return bad("The subject is one line of up to 300 characters.");
        }
        if self.body.chars().count() > MAX_BODY_CHARS {
            return bad("The body is too long for one draft.");
        }
        if self.reply_to.as_deref().is_some_and(|id| !valid_id(id)) {
            return bad("reply_to is a Gmail message id.");
        }
        Ok(())
    }

    /// The RFC 2822 message, `In-Reply-To` and `References` set to
    /// `replying` when it answers a message.
    #[must_use]
    pub fn raw(&self, replying: Option<&str>) -> String {
        let join = |list: &[String]| list.iter().map(|a| a.trim()).collect::<Vec<_>>().join(", ");
        let mut out = format!("To: {}\r\n", join(&self.to));
        if !self.cc.is_empty() {
            let _ = write!(out, "Cc: {}\r\n", join(&self.cc));
        }
        let _ = write!(out, "Subject: {}\r\n", encoded_word(self.subject.trim()));
        if let Some(id) = replying.filter(|id| !id.is_empty() && !id.contains(['\r', '\n'])) {
            let _ = write!(out, "In-Reply-To: {id}\r\nReferences: {id}\r\n");
        }
        out.push_str("MIME-Version: 1.0\r\nContent-Type: text/plain; charset=\"UTF-8\"\r\nContent-Transfer-Encoding: 8bit\r\n\r\n");
        out.push_str(&self.body.replace("\r\n", "\n").replace('\n', "\r\n"));
        out
    }
}

impl Gmail {
    #[must_use]
    pub fn new(http: reqwest::Client, endpoints: &Endpoints, token: String) -> Self {
        Self {
            rest: Rest::new(http, token),
            root: format!(
                "{}/gmail/v1/users/me",
                endpoints.gmail.trim_end_matches('/')
            ),
        }
    }

    /// Messages matching `query` (Gmail's search words: `from:`,
    /// `subject:`, `newer_than:7d`, `is:unread`, ...; the inbox when
    /// empty), newest first.
    ///
    /// # Errors
    ///
    /// [`ApiError`].
    pub async fn search(&self, query: &str, limit: u32) -> Result<Vec<Summary>, ApiError> {
        let query = query.trim();
        if query.chars().count() > 500 {
            return Err(ApiError::Invalid(
                "Search with up to 500 characters.".into(),
            ));
        }
        let query = if query.is_empty() { "in:inbox" } else { query };
        let limit = limit.clamp(1, MAX_RESULTS).to_string();
        let listed = self
            .rest
            .get(
                &format!("{}/messages", self.root),
                &[("q", query), ("maxResults", limit.as_str())],
            )
            .await?;
        let ids: Vec<String> = listed["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| m["id"].as_str().map(str::to_owned))
            .collect();
        let urls: Vec<String> = ids
            .iter()
            .map(|id| format!("{}/messages/{id}", self.root))
            .collect();
        let fetched = futures::future::join_all(urls.iter().map(|url| {
            self.rest.get(
                url,
                &[
                    ("format", "metadata"),
                    ("metadataHeaders", "From"),
                    ("metadataHeaders", "Subject"),
                    ("metadataHeaders", "Date"),
                ],
            )
        }))
        .await;
        let mut out = Vec::new();
        for message in fetched {
            // A message deleted between the list and the read is skipped.
            let message = match message {
                Ok(message) => message,
                Err(ApiError::NotFound) => continue,
                Err(error) => return Err(error),
            };
            let payload = &message["payload"];
            out.push(Summary {
                id: message["id"].as_str().unwrap_or_default().to_owned(),
                thread: message["threadId"].as_str().unwrap_or_default().to_owned(),
                from: header(payload, "From"),
                subject: header(payload, "Subject"),
                date: header(payload, "Date"),
                snippet: html_text(message["snippet"].as_str().unwrap_or_default()),
                unread: message["labelIds"]
                    .as_array()
                    .is_some_and(|l| l.iter().any(|l| l == "UNREAD")),
            });
        }
        Ok(out)
    }

    /// One message as text.
    ///
    /// # Errors
    ///
    /// [`ApiError`].
    pub async fn message(&self, id: &str) -> Result<Message, ApiError> {
        if !valid_id(id) {
            return Err(ApiError::Invalid("That isn't a Gmail message id.".into()));
        }
        let value = self
            .rest
            .get(
                &format!("{}/messages/{id}", self.root),
                &[("format", "full")],
            )
            .await?;
        Ok(message_of(&value))
    }

    /// A thread's messages as text, oldest first.
    ///
    /// # Errors
    ///
    /// [`ApiError`].
    pub async fn thread(&self, id: &str) -> Result<Vec<Message>, ApiError> {
        if !valid_id(id) {
            return Err(ApiError::Invalid("That isn't a Gmail thread id.".into()));
        }
        let value = self
            .rest
            .get(
                &format!("{}/threads/{id}", self.root),
                &[("format", "full")],
            )
            .await?;
        Ok(value["messages"]
            .as_array()
            .into_iter()
            .flatten()
            .map(message_of)
            .collect())
    }

    /// The person's labels.
    ///
    /// # Errors
    ///
    /// [`ApiError`].
    pub async fn labels(&self) -> Result<Value, ApiError> {
        let value = self.rest.get(&format!("{}/labels", self.root), &[]).await?;
        Ok(json!(
            value["labels"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|l| json!({"id": l["id"], "name": l["name"], "type": l["type"]}))
                .collect::<Vec<_>>()
        ))
    }

    /// Make a draft; it is read back so what is shown is what Gmail has.
    ///
    /// # Errors
    ///
    /// [`ApiError`].
    pub async fn create_draft(&self, args: &DraftArgs) -> Result<Draft, ApiError> {
        args.check()?;
        let (replying, thread) = match &args.reply_to {
            Some(id) => {
                let original = self.message(id).await?;
                (Some(original.message_id), Some(original.thread))
            }
            None => (None, None),
        };
        let mut message = json!({"raw": URL_SAFE_NO_PAD.encode(args.raw(replying.as_deref()))});
        if let Some(thread) = thread.filter(|t| !t.is_empty()) {
            message["threadId"] = json!(thread);
        }
        let made = self
            .rest
            .post(
                &format!("{}/drafts", self.root),
                &[],
                &json!({"message": message}),
            )
            .await?;
        let id = made["id"].as_str().ok_or(ApiError::Unreachable)?.to_owned();
        self.draft(&id).await
    }

    /// One draft, as it is now.
    ///
    /// # Errors
    ///
    /// [`ApiError`].
    pub async fn draft(&self, id: &str) -> Result<Draft, ApiError> {
        if !valid_id(id) {
            return Err(ApiError::Invalid("That isn't a Gmail draft id.".into()));
        }
        let value = self
            .rest
            .get(&format!("{}/drafts/{id}", self.root), &[("format", "full")])
            .await?;
        let message = message_of(&value["message"]);
        Ok(Draft {
            id: value["id"].as_str().unwrap_or(id).to_owned(),
            message: message.id,
            thread: message.thread,
            to: message.to,
            cc: message.cc,
            subject: message.subject,
            body: message.text,
        })
    }

    /// The person's newest drafts, as (draft id, message id).
    ///
    /// # Errors
    ///
    /// [`ApiError`].
    pub async fn list_drafts(&self) -> Result<Vec<(String, String)>, ApiError> {
        let listed = self
            .rest
            .get(&format!("{}/drafts", self.root), &[("maxResults", "100")])
            .await?;
        Ok(listed["drafts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|d| {
                Some((
                    d["id"].as_str()?.to_owned(),
                    d["message"]["id"].as_str()?.to_owned(),
                ))
            })
            .collect())
    }

    /// Send a draft. The host calls this only after the person confirmed
    /// it ([`card`]).
    ///
    /// # Errors
    ///
    /// [`ApiError`].
    pub async fn send_draft(&self, id: &str) -> Result<Value, ApiError> {
        if !valid_id(id) {
            return Err(ApiError::Invalid("That isn't a Gmail draft id.".into()));
        }
        let sent = self
            .rest
            .post(
                &format!("{}/drafts/send", self.root),
                &[],
                &json!({"id": id}),
            )
            .await?;
        let thread = sent["threadId"].as_str().unwrap_or_default();
        Ok(json!({"sent": true, "link": thread_link(thread)}))
    }

    /// Throw a draft away.
    ///
    /// # Errors
    ///
    /// [`ApiError`].
    pub async fn delete_draft(&self, id: &str) -> Result<(), ApiError> {
        if !valid_id(id) {
            return Err(ApiError::Invalid("That isn't a Gmail draft id.".into()));
        }
        self.rest
            .delete(&format!("{}/drafts/{id}", self.root))
            .await
    }
}

fn scopes(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

/// The Gmail tools.
#[must_use]
pub fn tool_specs() -> Vec<ToolSpec> {
    let read = scopes(&[GMAIL_READONLY, GMAIL_MODIFY, GMAIL_FULL]);
    let compose = scopes(&[GMAIL_COMPOSE, GMAIL_MODIFY, GMAIL_FULL]);
    vec![
        ToolSpec {
            id: "gmail.search".into(),
            description: "Search the person's Gmail with Gmail's search words (from:, to:, subject:, newer_than:7d, is:unread, has:attachment, label:; empty for the inbox). Returns each message's sender, subject, date, snippet, and link, newest first.".into(),
            method: HttpMethod::Get,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Gmail search words; empty for the newest in the inbox."},
                    "limit": {"type": "integer", "description": "How many messages, 1 to 20 (10 by default)."}
                },
                "additionalProperties": false
            }),
            scopes: read.clone(),
            policy: None,
        },
        ToolSpec {
            id: "gmail.read".into(),
            description: "Read one Gmail message, or a whole thread, as text, with its attachments listed by name. Give message or thread (an id from gmail_search).".into(),
            method: HttpMethod::Get,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string", "description": "A message id."},
                    "thread": {"type": "string", "description": "A thread id."}
                },
                "additionalProperties": false
            }),
            scopes: read.clone(),
            policy: None,
        },
        ToolSpec {
            id: "gmail.labels".into(),
            description: "List the person's Gmail labels.".into(),
            method: HttpMethod::Get,
            input_schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
            scopes: {
                let mut s = read;
                s.push("https://www.googleapis.com/auth/gmail.labels".into());
                s
            },
            policy: None,
        },
        ToolSpec {
            id: "gmail.draft".into(),
            description: "Write a Gmail draft (it is not sent). Returns the draft's link; the person is shown the draft and sends it themselves.".into(),
            method: HttpMethod::Post,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "to": {"type": "array", "items": {"type": "string"}, "description": "Email addresses."},
                    "cc": {"type": "array", "items": {"type": "string"}},
                    "subject": {"type": "string"},
                    "body": {"type": "string", "description": "Plain text."},
                    "reply_to": {"type": "string", "description": "A message id this answers, to keep it in that thread."}
                },
                "required": ["to", "subject", "body"],
                "additionalProperties": false
            }),
            scopes: compose.clone(),
            policy: Some(Policy::Allow),
        },
        ToolSpec {
            id: "gmail.send".into(),
            description: "Send a draft the person was shown and told you to send. Never send without their explicit yes to that exact draft.".into(),
            method: HttpMethod::Post,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "draft": {"type": "string", "description": "The draft id."}
                },
                "additionalProperties": false
            }),
            scopes: compose,
            policy: Some(Policy::RequireApproval),
        },
    ]
}

/// One call of a Gmail tool, read from its name and arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    Search {
        query: String,
        limit: u32,
    },
    ReadMessage {
        id: String,
    },
    ReadThread {
        id: String,
    },
    Labels,
    Draft(DraftArgs),
    /// The draft to send; a chat sends the one it showed, whatever this
    /// names.
    Send {
        draft: Option<String>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchArgs {
    #[serde(default)]
    query: String,
    #[serde(default)]
    limit: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadArgs {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    thread: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SendArgs {
    #[serde(default)]
    draft: Option<String>,
}

/// A string or a list of strings, as models write recipients.
fn listed(value: &mut Value, field: &str) {
    if let Some(text) = value[field].as_str() {
        let list: Vec<Value> = text
            .split([',', ';'])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| json!(s))
            .collect();
        value[field] = Value::Array(list);
    }
}

impl Call {
    /// Whether `name` is a Gmail tool (its id or wire name).
    #[must_use]
    pub fn names(name: &str) -> bool {
        name.starts_with("gmail.") || name.starts_with("gmail_")
    }

    /// The call `name` with `arguments`.
    ///
    /// # Errors
    ///
    /// A sentence saying what was wrong.
    pub fn parse(name: &str, mut arguments: Value) -> Result<Self, String> {
        let bad = |tool: &str| format!("{tool} takes the fields its schema declares.");
        match name.replace('_', ".").as_str() {
            "gmail.search" => {
                let args: SearchArgs =
                    serde_json::from_value(arguments).map_err(|_| bad("gmail.search"))?;
                Ok(Self::Search {
                    query: args.query,
                    limit: args.limit.unwrap_or(10),
                })
            }
            "gmail.read" => {
                let args: ReadArgs =
                    serde_json::from_value(arguments).map_err(|_| bad("gmail.read"))?;
                match (args.message, args.thread) {
                    (Some(id), None) => Ok(Self::ReadMessage { id }),
                    (None, Some(id)) => Ok(Self::ReadThread { id }),
                    _ => Err("gmail.read takes message or thread.".into()),
                }
            }
            "gmail.labels" => {
                let _: Empty =
                    serde_json::from_value(arguments).map_err(|_| bad("gmail.labels"))?;
                Ok(Self::Labels)
            }
            "gmail.draft" => {
                listed(&mut arguments, "to");
                listed(&mut arguments, "cc");
                let args: DraftArgs =
                    serde_json::from_value(arguments).map_err(|_| bad("gmail.draft"))?;
                Ok(Self::Draft(args))
            }
            "gmail.send" => {
                let args: SendArgs =
                    serde_json::from_value(arguments).map_err(|_| bad("gmail.send"))?;
                Ok(Self::Send { draft: args.draft })
            }
            _ => Err(format!("There is no Gmail tool named {name}.")),
        }
    }

    /// The tool's id.
    #[must_use]
    pub fn tool(&self) -> &'static str {
        match self {
            Self::Search { .. } => "gmail.search",
            Self::ReadMessage { .. } | Self::ReadThread { .. } => "gmail.read",
            Self::Labels => "gmail.labels",
            Self::Draft(_) => "gmail.draft",
            Self::Send { .. } => "gmail.send",
        }
    }
}

/// What a call produced: the tool result, what it used (to link), and the
/// draft it made (to show).
#[derive(Clone, Debug)]
pub struct Outcome {
    pub result: Value,
    pub cited: Vec<Cite>,
    pub draft: Option<Draft>,
}

fn messages_text(messages: &[Message]) -> (Value, bool) {
    let mut left = MAX_TEXT_CHARS;
    let mut cut = false;
    let mut out = Vec::new();
    for message in messages.iter().rev().take(MAX_THREAD_MESSAGES).rev() {
        let (text, was_cut) = capped(&message.text, left);
        left = left.saturating_sub(text.chars().count());
        cut |= was_cut;
        let mut shown = message.clone();
        shown.text = text;
        out.push(shown.json());
    }
    cut |= messages.len() > MAX_THREAD_MESSAGES;
    (json!(out), cut)
}

/// Run `call` on `gmail`. [`Call::Send`] sends: the host runs it only
/// after the person's confirm.
///
/// # Errors
///
/// [`ApiError`].
pub async fn run(gmail: &Gmail, call: &Call) -> Result<Outcome, ApiError> {
    let plain = |result: Value, cited: Vec<Cite>| Outcome {
        result,
        cited,
        draft: None,
    };
    match call {
        Call::Search { query, limit } => {
            let found = gmail.search(query, *limit).await?;
            Ok(plain(
                json!({"messages": found.iter().map(Summary::json).collect::<Vec<_>>()}),
                Vec::new(),
            ))
        }
        Call::ReadMessage { id } => {
            let message = gmail.message(id).await?;
            let cite = Cite {
                title: title_or(&message.subject, "(no subject)"),
                link: thread_link(&message.thread),
            };
            let (messages, truncated) = messages_text(std::slice::from_ref(&message));
            Ok(plain(
                json!({"messages": messages, "truncated": truncated, "link": cite.link}),
                vec![cite],
            ))
        }
        Call::ReadThread { id } => {
            let messages = gmail.thread(id).await?;
            let subject = messages
                .first()
                .map(|m| m.subject.clone())
                .unwrap_or_default();
            let cite = Cite {
                title: title_or(&subject, "(no subject)"),
                link: thread_link(id),
            };
            let (shown, truncated) = messages_text(&messages);
            Ok(plain(
                json!({"messages": shown, "truncated": truncated, "link": cite.link}),
                vec![cite],
            ))
        }
        Call::Labels => Ok(plain(json!({"labels": gmail.labels().await?}), Vec::new())),
        Call::Draft(args) => {
            let draft = gmail.create_draft(args).await?;
            Ok(Outcome {
                result: draft.json(),
                cited: Vec::new(),
                draft: Some(draft),
            })
        }
        Call::Send { draft } => {
            let draft = draft
                .as_deref()
                .ok_or_else(|| ApiError::Invalid("gmail.send takes the draft id.".into()))?;
            Ok(plain(gmail.send_draft(draft).await?, Vec::new()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_take_their_declared_fields_and_recipients_as_text_or_lists() {
        assert_eq!(
            Call::parse("gmail_search", json!({"query": "from:a@b.c"})),
            Ok(Call::Search {
                query: "from:a@b.c".into(),
                limit: 10
            })
        );
        assert!(Call::parse("gmail_read", json!({})).is_err());
        assert!(Call::parse("gmail_read", json!({"message": "a", "token": "x"})).is_err());
        let Ok(Call::Draft(args)) = Call::parse(
            "gmail_draft",
            json!({"to": "a@b.co, c@d.co", "subject": "Hi", "body": "Yo"}),
        ) else {
            panic!("a draft");
        };
        assert_eq!(args.to, ["a@b.co", "c@d.co"]);
        assert_eq!(
            Call::parse("gmail_send", json!({"draft": "r1"})),
            Ok(Call::Send {
                draft: Some("r1".into())
            })
        );
        assert!(Call::parse("gmail_delete", json!({})).is_err());
    }

    #[test]
    fn reads_and_drafts_are_allowed_and_sending_always_asks() {
        let specs = tool_specs();
        let policy = |id: &str| specs.iter().find(|s| s.id == id).unwrap().default_policy();
        assert_eq!(policy("gmail.search"), Policy::Allow);
        assert_eq!(policy("gmail.read"), Policy::Allow);
        assert_eq!(policy("gmail.labels"), Policy::Allow);
        assert_eq!(policy("gmail.draft"), Policy::Allow);
        assert_eq!(policy("gmail.send"), Policy::RequireApproval);
    }

    #[test]
    fn a_draft_is_a_plain_text_message_with_safe_headers() {
        let args = DraftArgs {
            to: vec!["chris@openagents.com".into()],
            cc: Vec::new(),
            subject: "Café".into(),
            body: "Line one\nLine two".into(),
            reply_to: None,
        };
        let raw = args.raw(Some("<abc@mail>"));
        assert!(raw.starts_with("To: chris@openagents.com\r\nSubject: =?UTF-8?B?"));
        assert!(raw.contains("In-Reply-To: <abc@mail>\r\n"));
        assert!(raw.ends_with("\r\n\r\nLine one\r\nLine two"));
        let mut bad = args.clone();
        bad.to = vec!["a@b.co\r\nBcc: x@y.co".into()];
        assert!(bad.check().is_err());
        bad.to = vec!["Name <a@b.co>".into()];
        assert!(bad.check().is_err());
        let mut bad = args;
        bad.subject = "a\r\nBcc: x@y.co".into();
        assert!(bad.check().is_err());
    }

    #[test]
    fn the_card_is_the_same_for_the_same_draft_and_shows_who_and_what() {
        let draft = Draft {
            id: "r1".into(),
            message: "m1".into(),
            thread: "t1".into(),
            to: "chris@openagents.com".into(),
            cc: String::new(),
            subject: "OpenAgents test draft".into(),
            body: "Hello\r\n\r\nThere".into(),
        };
        let shown = card(&draft);
        assert_eq!(shown, card(&draft.clone()));
        assert!(shown.contains("To: chris@openagents.com"));
        assert!(shown.contains("Subject: OpenAgents test draft"));
        assert!(shown.contains("> Hello\n>\n> There"));
        assert!(shown.contains("https://mail.google.com/mail/#drafts?compose=m1"));
        let mut changed = draft;
        changed.to = "someone@else.com".into();
        assert_ne!(card(&changed), shown);
    }

    #[test]
    fn parts_give_the_plain_text_else_the_html_and_attachments_by_name() {
        let b64 = |s: &str| URL_SAFE_NO_PAD.encode(s);
        let message = json!({
            "id": "m1", "threadId": "t1",
            "payload": {
                "mimeType": "multipart/mixed",
                "headers": [{"name": "Subject", "value": "Hi"}, {"name": "from", "value": "A <a@b.co>"}],
                "parts": [
                    {"mimeType": "multipart/alternative", "parts": [
                        {"mimeType": "text/html", "body": {"data": b64("<p>Hello <b>there</b></p><style>x{}</style>")}}
                    ]},
                    {"mimeType": "application/pdf", "filename": "invoice.pdf", "body": {"size": 1200, "attachmentId": "a1"}}
                ]
            }
        });
        let read = message_of(&message);
        assert_eq!(read.text, "Hello there");
        assert_eq!(read.from, "A <a@b.co>");
        assert_eq!(read.attachments[0].name, "invoice.pdf");
        assert_eq!(html_text("a &amp; b<br>c"), "a & b\nc");
    }
}
