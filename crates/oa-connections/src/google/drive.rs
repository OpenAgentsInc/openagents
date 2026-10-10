//! The read-only Drive tools: `drive.search`, `drive.list_folder`, and
//! `drive.read`, and the Drive client they run on.
//!
//! `drive.read` returns text: a Google Doc (or Slides) exported as plain
//! text, a Google Sheet as CSV (every tab through the Sheets API when the
//! connection has Sheets access, else the first tab through Drive's CSV
//! export), a PDF as text through the host's [`PdfText`], and plain text,
//! CSV, Markdown, and JSON files as they are. Everything is capped
//! ([`MAX_TEXT_CHARS`], [`MAX_TABS`], [`MAX_ROWS`], [`MAX_PDF_BYTES`]) and
//! says when it was cut.
//!
//! Every tool here calls with `GET`, so each one's default policy is
//! [`crate::core::Policy::Allow`].

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{DRIVE_READONLY, Endpoints, SHEETS_READONLY};
use crate::core::{HttpMethod, ToolSpec};

pub const FOLDER: &str = "application/vnd.google-apps.folder";
pub const DOC: &str = "application/vnd.google-apps.document";
pub const SHEET: &str = "application/vnd.google-apps.spreadsheet";
pub const SLIDES: &str = "application/vnd.google-apps.presentation";
pub const PDF: &str = "application/pdf";

/// The most text one read returns.
pub const MAX_TEXT_CHARS: usize = 60_000;
/// The most tabs of a spreadsheet read.
pub const MAX_TABS: usize = 12;
/// The most rows read from one tab.
pub const MAX_ROWS: usize = 2_000;
/// The largest PDF read.
pub const MAX_PDF_BYTES: usize = 12 * 1024 * 1024;
/// The largest export or plain file read.
const MAX_EXPORT_BYTES: usize = 4 * 1024 * 1024;
/// The most files one search or listing returns.
pub const MAX_RESULTS: u32 = 50;
const FIELDS: &str = "id,name,mimeType,webViewLink,modifiedTime,size";

/// Turns a PDF's bytes into its text; the host provides it (the website
/// uses Gemini on Vertex).
pub type PdfText = Arc<
    dyn Fn(Vec<u8>) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>> + Send + Sync,
>;

/// One file or folder as Drive describes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_view_link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
}

impl File {
    #[must_use]
    pub fn is_folder(&self) -> bool {
        self.mime_type == FOLDER
    }

    /// What it is, in a word a person reads.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self.mime_type.as_str() {
            FOLDER => "folder",
            DOC => "document",
            SHEET => "spreadsheet",
            SLIDES => "presentation",
            PDF => "pdf",
            _ => "file",
        }
    }

    /// Where it opens in the browser.
    #[must_use]
    pub fn link(&self) -> String {
        self.web_view_link.clone().unwrap_or_else(|| {
            if self.is_folder() {
                format!("https://drive.google.com/drive/folders/{}", self.id)
            } else {
                format!("https://drive.google.com/file/d/{}/view", self.id)
            }
        })
    }

    /// The tool result's description of it.
    #[must_use]
    pub fn json(&self) -> Value {
        let mut out = json!({
            "id": self.id,
            "name": self.name,
            "kind": self.kind(),
            "link": self.link(),
        });
        if let Some(modified) = &self.modified_time {
            out["modified"] = json!(modified);
        }
        out
    }
}

/// Why a Drive call didn't work, in words for the person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DriveError {
    /// The id names nothing this connection can see.
    NotFound,
    /// Google refused the token (expired or access removed).
    Unauthorized,
    /// The connection lacks the access, or the file isn't shared with it.
    Forbidden,
    /// Too large to read.
    TooLarge,
    /// A kind of file that can't be read as text.
    Unsupported(String),
    /// Bad arguments.
    Invalid(String),
    /// Google couldn't be reached or answered with an error.
    Unreachable,
    /// The PDF's text couldn't be read.
    Pdf(String),
}

impl fmt::Display for DriveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("That file or folder wasn't found in Google Drive."),
            Self::Unauthorized => {
                f.write_str("Google no longer accepts this connection. Connect Google again.")
            }
            Self::Forbidden => f.write_str("Google Drive didn't allow reading that file."),
            Self::TooLarge => f.write_str("That file is too large to read."),
            Self::Unsupported(kind) => write!(f, "A {kind} can't be read as text."),
            Self::Invalid(why) => f.write_str(why),
            Self::Unreachable => f.write_str("Google Drive couldn't be reached. Try again."),
            Self::Pdf(why) => write!(f, "The PDF's text couldn't be read ({why})."),
        }
    }
}

/// Drive as one connection: an access token and whether it may use the
/// Sheets API.
#[derive(Clone)]
pub struct Drive {
    http: reqwest::Client,
    endpoints: Endpoints,
    token: String,
    sheets: bool,
}

impl fmt::Debug for Drive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Drive(sheets: {}, token redacted)", self.sheets)
    }
}

/// What `drive.read` read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read {
    pub file: File,
    /// `text` or `csv`.
    pub format: &'static str,
    pub text: String,
    pub truncated: bool,
    /// Something to say about how it was read (only the first tab, ...).
    pub note: Option<String>,
}

impl Read {
    #[must_use]
    pub fn json(&self) -> Value {
        let mut out = json!({
            "file": self.file.json(),
            "format": self.format,
            "text": self.text,
            "truncated": self.truncated,
        });
        if let Some(note) = &self.note {
            out["note"] = json!(note);
        }
        out
    }
}

/// Whether `id` looks like a Drive id (or `root`).
#[must_use]
pub fn valid_id(id: &str) -> bool {
    id == "root"
        || ((10..=128).contains(&id.len())
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
}

/// A Drive query string literal: backslashes and quotes escaped.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('\'');
    for c in text.chars() {
        if c == '\\' || c == '\'' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('\'');
    out
}

/// Cut `text` to `limit` characters; whether it was cut.
fn capped(text: &str, limit: usize) -> (String, bool) {
    match text.char_indices().nth(limit) {
        Some((at, _)) => (text[..at].to_owned(), true),
        None => (text.to_owned(), false),
    }
}

/// One CSV field, quoted when it needs to be.
fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// Rows of cells as CSV.
#[must_use]
pub fn csv(rows: &[Vec<Value>]) -> String {
    let mut out = String::new();
    for row in rows {
        let cells: Vec<String> = row
            .iter()
            .map(|cell| match cell {
                Value::String(s) => csv_field(s),
                Value::Null => String::new(),
                other => csv_field(&other.to_string()),
            })
            .collect();
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

impl Drive {
    /// Drive with `token`; `sheets` when the connection was granted
    /// [`SHEETS_READONLY`].
    #[must_use]
    pub fn new(http: reqwest::Client, endpoints: Endpoints, token: String, sheets: bool) -> Self {
        Self {
            http,
            endpoints,
            token,
            sheets,
        }
    }

    async fn get(
        &self,
        url: &str,
        query: &[(&str, &str)],
        limit: usize,
    ) -> Result<Vec<u8>, DriveError> {
        let mut response = self
            .http
            .get(url)
            .query(query)
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|_| DriveError::Unreachable)?;
        match response.status().as_u16() {
            200..=299 => {}
            401 => return Err(DriveError::Unauthorized),
            403 => {
                // Drive's export limit answers 403 exportSizeLimitExceeded.
                let body = response.text().await.unwrap_or_default();
                return Err(if body.contains("exportSizeLimitExceeded") {
                    DriveError::TooLarge
                } else {
                    DriveError::Forbidden
                });
            }
            404 => return Err(DriveError::NotFound),
            _ => return Err(DriveError::Unreachable),
        }
        if response
            .content_length()
            .is_some_and(|length| length as usize > limit)
        {
            return Err(DriveError::TooLarge);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| DriveError::Unreachable)?
        {
            bytes.extend_from_slice(&chunk);
            if bytes.len() > limit {
                return Err(DriveError::TooLarge);
            }
        }
        Ok(bytes)
    }

    async fn get_json(&self, url: &str, query: &[(&str, &str)]) -> Result<Value, DriveError> {
        let bytes = self.get(url, query, 8 * 1024 * 1024).await?;
        serde_json::from_slice(&bytes).map_err(|_| DriveError::Unreachable)
    }

    fn files_url(&self) -> String {
        format!("{}/drive/v3/files", self.endpoints.drive)
    }

    async fn list(
        &self,
        q: &str,
        page_size: u32,
        page_token: Option<&str>,
    ) -> Result<(Vec<File>, Option<String>), DriveError> {
        let size = page_size.clamp(1, MAX_RESULTS).to_string();
        let fields = format!("nextPageToken,files({FIELDS})");
        let mut query = vec![
            ("q", q),
            ("pageSize", size.as_str()),
            ("fields", fields.as_str()),
            ("supportsAllDrives", "true"),
            ("includeItemsFromAllDrives", "true"),
        ];
        if let Some(token) = page_token {
            query.push(("pageToken", token));
        }
        let body = self.get_json(&self.files_url(), &query).await?;
        let files = body["files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|file| serde_json::from_value::<File>(file.clone()).ok())
            .collect();
        let next = body["nextPageToken"].as_str().map(str::to_owned);
        Ok((files, next))
    }

    /// Files whose name or text matches `query`, optionally only those
    /// directly in `folder`.
    ///
    /// # Errors
    ///
    /// [`DriveError`].
    pub async fn search(
        &self,
        query: &str,
        folder: Option<&str>,
        limit: u32,
    ) -> Result<Vec<File>, DriveError> {
        let query = query.trim();
        if query.is_empty() || query.chars().count() > 200 {
            return Err(DriveError::Invalid(
                "Search for one to 200 characters.".into(),
            ));
        }
        let mut q = format!(
            "(name contains {0} or fullText contains {0}) and trashed = false",
            quoted(query)
        );
        if let Some(folder) = folder {
            if !valid_id(folder) {
                return Err(DriveError::Invalid("That isn't a Drive folder id.".into()));
            }
            q.push_str(&format!(" and {} in parents", quoted(folder)));
        }
        Ok(self.list(&q, limit, None).await?.0)
    }

    /// What is directly in `folder`, folders first, and the token for the
    /// next page.
    ///
    /// # Errors
    ///
    /// [`DriveError`].
    pub async fn list_folder(
        &self,
        folder: &str,
        page_token: Option<&str>,
    ) -> Result<(Vec<File>, Option<String>), DriveError> {
        if !valid_id(folder) {
            return Err(DriveError::Invalid("That isn't a Drive folder id.".into()));
        }
        let q = format!("{} in parents and trashed = false", quoted(folder));
        let (mut files, next) = self.list(&q, MAX_RESULTS, page_token).await?;
        files.sort_by(|a, b| {
            b.is_folder()
                .cmp(&a.is_folder())
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok((files, next))
    }

    /// One file's description.
    ///
    /// # Errors
    ///
    /// [`DriveError`].
    pub async fn file(&self, id: &str) -> Result<File, DriveError> {
        if !valid_id(id) {
            return Err(DriveError::Invalid("That isn't a Drive file id.".into()));
        }
        let body = self
            .get_json(
                &format!("{}/{id}", self.files_url()),
                &[("fields", FIELDS), ("supportsAllDrives", "true")],
            )
            .await?;
        serde_json::from_value(body).map_err(|_| DriveError::Unreachable)
    }

    /// One file as text ([`Read`]).
    ///
    /// # Errors
    ///
    /// [`DriveError`].
    pub async fn read(&self, id: &str, pdf: Option<&PdfText>) -> Result<Read, DriveError> {
        let file = self.file(id).await?;
        let url = format!("{}/{id}", self.files_url());
        let text_of = |bytes: Vec<u8>| String::from_utf8_lossy(&bytes).into_owned();
        let (format, text, note) = match file.mime_type.as_str() {
            FOLDER => {
                return Err(DriveError::Invalid(
                    "That's a folder; list it with drive.list_folder.".into(),
                ));
            }
            DOC | SLIDES => {
                let bytes = self
                    .get(
                        &format!("{url}/export"),
                        &[("mimeType", "text/plain")],
                        MAX_EXPORT_BYTES,
                    )
                    .await?;
                ("text", text_of(bytes), None)
            }
            SHEET if self.sheets => {
                let (text, note) = self.sheet_tabs(id).await?;
                ("csv", text, note)
            }
            SHEET => {
                let bytes = self
                    .get(
                        &format!("{url}/export"),
                        &[("mimeType", "text/csv")],
                        MAX_EXPORT_BYTES,
                    )
                    .await?;
                (
                    "csv",
                    text_of(bytes),
                    Some("Only the first tab was read; allow Sheets in Settings, Connections to read every tab.".to_owned()),
                )
            }
            PDF => {
                let Some(pdf) = pdf else {
                    return Err(DriveError::Unsupported("PDF".into()));
                };
                let bytes = self
                    .get(
                        &url,
                        &[("alt", "media"), ("supportsAllDrives", "true")],
                        MAX_PDF_BYTES,
                    )
                    .await?;
                let text = pdf(bytes).await.map_err(DriveError::Pdf)?;
                ("text", text, None)
            }
            mime if mime.starts_with("text/")
                || mime == "application/json"
                || mime == "application/x-yaml" =>
            {
                let bytes = self
                    .get(
                        &url,
                        &[("alt", "media"), ("supportsAllDrives", "true")],
                        MAX_EXPORT_BYTES,
                    )
                    .await?;
                let format = if mime == "text/csv" { "csv" } else { "text" };
                (format, text_of(bytes), None)
            }
            _ => return Err(DriveError::Unsupported(file.kind().to_owned())),
        };
        let (text, truncated) = capped(&text, MAX_TEXT_CHARS);
        Ok(Read {
            file,
            format,
            text,
            truncated,
            note,
        })
    }

    /// Every tab of a spreadsheet as CSV, each under a `# Tab` line.
    async fn sheet_tabs(&self, id: &str) -> Result<(String, Option<String>), DriveError> {
        let root = format!("{}/v4/spreadsheets/{id}", self.endpoints.sheets);
        let meta = self
            .get_json(&root, &[("fields", "sheets.properties(title)")])
            .await?;
        let titles: Vec<String> = meta["sheets"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|sheet| sheet["properties"]["title"].as_str().map(str::to_owned))
            .collect();
        let skipped = titles.len().saturating_sub(MAX_TABS);
        let ranges: Vec<String> = titles
            .iter()
            .take(MAX_TABS)
            .map(|title| format!("'{}'!1:{MAX_ROWS}", title.replace('\'', "''")))
            .collect();
        if ranges.is_empty() {
            return Ok((String::new(), None));
        }
        let mut query: Vec<(&str, &str)> = ranges.iter().map(|r| ("ranges", r.as_str())).collect();
        query.push(("valueRenderOption", "FORMATTED_VALUE"));
        query.push(("majorDimension", "ROWS"));
        let values = self
            .get_json(&format!("{root}/values:batchGet"), &query)
            .await?;
        let mut out = String::new();
        let mut cut_rows = false;
        for (index, range) in values["valueRanges"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let title = titles.get(index).map_or("Sheet", String::as_str);
            let rows: Vec<Vec<Value>> = range["values"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|row| row.as_array().cloned().unwrap_or_default())
                .collect();
            if rows.len() >= MAX_ROWS {
                cut_rows = true;
            }
            out.push_str(&format!("# {title}\n"));
            out.push_str(&csv(&rows));
            out.push('\n');
        }
        let mut notes = Vec::new();
        if skipped > 0 {
            notes.push(format!("{skipped} more tabs were not read."));
        }
        if cut_rows {
            notes.push(format!(
                "Only the first {MAX_ROWS} rows of a tab were read."
            ));
        }
        Ok((out, (!notes.is_empty()).then(|| notes.join(" "))))
    }
}

/// A Drive item named in a link someone pasted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub id: String,
    /// Whether the link says it is a folder (else a file, or unknown for
    /// a bare id).
    pub folder: Option<bool>,
}

/// The Drive item in a pasted link or id: `drive.google.com/drive/folders/ID`,
/// `drive.google.com/file/d/ID/...`, `docs.google.com/{document,
/// spreadsheets,presentation}/d/ID/...`, `...open?id=ID`, or a bare id.
#[must_use]
pub fn parse_link(text: &str) -> Option<Link> {
    let text = text.trim();
    if valid_id(text) && text != "root" {
        return Some(Link {
            id: text.to_owned(),
            folder: None,
        });
    }
    let url = url::Url::parse(text).ok()?;
    let host = url.host_str()?;
    if !matches!(host, "drive.google.com" | "docs.google.com") {
        return None;
    }
    if let Some((_, id)) = url.query_pairs().find(|(key, _)| key == "id") {
        return valid_id(&id).then(|| Link {
            id: id.into_owned(),
            folder: None,
        });
    }
    let segments: Vec<&str> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
    for (index, segment) in segments.iter().enumerate() {
        let next = segments.get(index + 1).copied();
        let folder = match *segment {
            "folders" => Some(true),
            "d" => Some(false),
            _ => None,
        };
        if let (Some(folder), Some(id)) = (folder, next)
            && valid_id(id)
        {
            return Some(Link {
                id: id.to_owned(),
                folder: Some(folder),
            });
        }
    }
    None
}

/// The curated Drive tools.
#[must_use]
pub fn tool_specs() -> Vec<ToolSpec> {
    let scopes = vec![DRIVE_READONLY.to_owned()];
    vec![
        ToolSpec {
            id: "drive.search".into(),
            description: "Search the person's Google Drive for files whose name or text matches a query. Returns each file's id, name, kind, and link.".into(),
            method: HttpMethod::Get,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Words to look for in file names and text."},
                    "folder": {"type": "string", "description": "Only files directly in this folder id."}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
            scopes: scopes.clone(),
        },
        ToolSpec {
            id: "drive.list_folder".into(),
            description: "List what is directly in a Google Drive folder, folders first. Returns each item's id, name, kind, and link.".into(),
            method: HttpMethod::Get,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "folder": {"type": "string", "description": "The folder id."},
                    "page_token": {"type": "string", "description": "The next_page_token from the previous listing."}
                },
                "required": ["folder"],
                "additionalProperties": false
            }),
            scopes: scopes.clone(),
        },
        ToolSpec {
            id: "drive.read".into(),
            description: "Read one Google Drive file as text: a Doc as text, a Sheet as CSV (every tab), a PDF as text. Long files are cut and say so.".into(),
            method: HttpMethod::Get,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file": {"type": "string", "description": "The file id."}
                },
                "required": ["file"],
                "additionalProperties": false
            }),
            scopes,
        },
    ]
}

/// One call of a Drive tool, read from its name and arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    Search {
        query: String,
        folder: Option<String>,
    },
    ListFolder {
        folder: String,
        page_token: Option<String>,
    },
    Read {
        file: String,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchArgs {
    query: String,
    #[serde(default)]
    folder: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListArgs {
    folder: String,
    #[serde(default)]
    page_token: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadArgs {
    file: String,
}

impl Call {
    /// The call `name` (a tool id or its wire name) with `arguments`.
    ///
    /// # Errors
    ///
    /// A sentence saying what was wrong.
    pub fn parse(name: &str, arguments: Value) -> Result<Self, String> {
        let bad = |tool: &str| format!("{tool} takes the fields its schema declares.");
        match name {
            "drive.search" | "drive_search" => {
                let args: SearchArgs =
                    serde_json::from_value(arguments).map_err(|_| bad("drive.search"))?;
                Ok(Self::Search {
                    query: args.query,
                    folder: args.folder.filter(|f| !f.trim().is_empty()),
                })
            }
            "drive.list_folder" | "drive_list_folder" => {
                let args: ListArgs =
                    serde_json::from_value(arguments).map_err(|_| bad("drive.list_folder"))?;
                Ok(Self::ListFolder {
                    folder: args.folder,
                    page_token: args.page_token.filter(|t| !t.is_empty()),
                })
            }
            "drive.read" | "drive_read" => {
                let args: ReadArgs =
                    serde_json::from_value(arguments).map_err(|_| bad("drive.read"))?;
                Ok(Self::Read { file: args.file })
            }
            other => Err(format!("There is no Drive tool named {other}.")),
        }
    }

    /// The tool's id.
    #[must_use]
    pub fn tool(&self) -> &'static str {
        match self {
            Self::Search { .. } => "drive.search",
            Self::ListFolder { .. } => "drive.list_folder",
            Self::Read { .. } => "drive.read",
        }
    }
}

/// What a call produced: the tool result, and the files it read (for
/// citing) or listed (which may be read next).
#[derive(Clone, Debug)]
pub struct Outcome {
    pub result: Value,
    pub read: Vec<File>,
    pub listed: Vec<File>,
}

/// Run `call` on `drive`.
///
/// # Errors
///
/// [`DriveError`].
pub async fn run(drive: &Drive, call: &Call, pdf: Option<&PdfText>) -> Result<Outcome, DriveError> {
    match call {
        Call::Search { query, folder } => {
            let files = drive.search(query, folder.as_deref(), 20).await?;
            Ok(Outcome {
                result: json!({"files": files.iter().map(File::json).collect::<Vec<_>>()}),
                read: Vec::new(),
                listed: files,
            })
        }
        Call::ListFolder { folder, page_token } => {
            let (files, next) = drive.list_folder(folder, page_token.as_deref()).await?;
            let mut result = json!({"files": files.iter().map(File::json).collect::<Vec<_>>()});
            if let Some(next) = next {
                result["next_page_token"] = json!(next);
            }
            Ok(Outcome {
                result,
                read: Vec::new(),
                listed: files,
            })
        }
        Call::Read { file } => {
            let read = drive.read(file, pdf).await?;
            Ok(Outcome {
                result: read.json(),
                read: vec![read.file.clone()],
                listed: Vec::new(),
            })
        }
    }
}

/// Whether `scopes` lets the Sheets API read every tab.
#[must_use]
pub fn has_sheets(scopes: &[String]) -> bool {
    scopes.iter().any(|scope| scope == SHEETS_READONLY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_name_folders_and_files() {
        let id = "1AbCdEfGhIjKlMnOpQrStUv";
        for (text, folder) in [
            (
                format!("https://drive.google.com/drive/folders/{id}?usp=sharing"),
                Some(true),
            ),
            (
                format!("https://drive.google.com/drive/u/1/folders/{id}"),
                Some(true),
            ),
            (
                format!("https://drive.google.com/file/d/{id}/view"),
                Some(false),
            ),
            (
                format!("https://docs.google.com/spreadsheets/d/{id}/edit#gid=0"),
                Some(false),
            ),
            (format!("https://drive.google.com/open?id={id}"), None),
            (id.to_owned(), None),
        ] {
            assert_eq!(
                parse_link(&text),
                Some(Link {
                    id: id.into(),
                    folder
                }),
                "{text}"
            );
        }
        assert_eq!(
            parse_link("https://example.com/drive/folders/1AbCdEfGhIjKl"),
            None
        );
        assert_eq!(parse_link("not a link"), None);
    }

    #[test]
    fn queries_and_cells_are_escaped() {
        assert_eq!(quoted("it's a\\b"), r"'it\'s a\\b'");
        assert_eq!(
            csv(&[vec![
                json!("a,b"),
                json!("say \"hi\""),
                json!(3),
                Value::Null
            ]]),
            "\"a,b\",\"say \"\"hi\"\"\",3,\n"
        );
        assert_eq!(capped("héllo", 2), ("hé".to_owned(), true));
        assert_eq!(capped("hi", 2), ("hi".to_owned(), false));
    }

    #[test]
    fn calls_take_only_their_declared_fields() {
        assert_eq!(
            Call::parse("drive_read", json!({"file": "abc"})),
            Ok(Call::Read { file: "abc".into() })
        );
        assert!(Call::parse("drive.read", json!({"file": "abc", "token": "x"})).is_err());
        assert!(Call::parse("drive.delete", json!({})).is_err());
        let specs = tool_specs();
        assert!(
            specs
                .iter()
                .all(|s| s.default_policy() == crate::core::Policy::Allow)
        );
    }
}
