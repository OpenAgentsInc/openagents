//! Pasted screenshots and attached files reach the model (#11173).
//!
//! A pasted `data:image/...` URL, a dropped or pasted path to an image or a
//! PDF, and an `@path` in the prompt each become an attachment. On send,
//! each attachment is saved under `~/.openagents/coder/attachments` by its
//! digest, and the prompt gains one line per attachment:
//! `[Image #1: /path/to/file.png]`. The line is the plain note every reader
//! gets: a model that cannot see images, a delegated Claude Code or Codex
//! child (which opens the file by its path), and the transcript. When the
//! chat's model accepts images, [`expand`] turns each such line into the
//! image itself (`image_url`, or a `file` part for a PDF) on the request.
//!
//! Limits: an image is at most [`MAX_IMAGE_BYTES`], a PDF at most
//! [`MAX_PDF_BYTES`], and a prompt carries at most [`MAX_ATTACHMENTS`].

use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The largest image sent (the common provider limit).
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
/// The largest PDF sent.
pub const MAX_PDF_BYTES: usize = 20 * 1024 * 1024;
/// Attachments one prompt carries.
pub const MAX_ATTACHMENTS: usize = 8;

/// One attachment's bytes and what they are.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub media_type: &'static str,
    pub bytes: Vec<u8>,
    /// The file it came from, for the chip; a pasted screenshot has none.
    pub name: Option<String>,
}

impl Attachment {
    #[must_use]
    pub fn is_image(&self) -> bool {
        self.media_type.starts_with("image/")
    }

    /// `Image` or `PDF`, as the note and the chip name it.
    #[must_use]
    pub fn label(&self) -> &'static str {
        if self.is_image() { "Image" } else { "PDF" }
    }

    /// The `data:` URL of the bytes.
    #[must_use]
    pub fn data_url(&self) -> String {
        format!(
            "data:{};base64,{}",
            self.media_type,
            base64::engine::general_purpose::STANDARD.encode(&self.bytes)
        )
    }

    /// What the composer chip shows: the name and the size.
    #[must_use]
    pub fn chip(&self, number: usize) -> String {
        self.chip_sized(number, self.bytes.len())
    }

    fn chip_sized(&self, number: usize, bytes: usize) -> String {
        let size = if bytes >= 1024 * 1024 {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{} KB", bytes.div_ceil(1024))
        };
        match &self.name {
            Some(name) => format!("{} #{number} {name} · {size}", self.label()),
            None => format!("{} #{number} · {size}", self.label()),
        }
    }

    /// Saves the bytes under `dir` by digest, and returns the file.
    ///
    /// # Errors
    /// The folder or the file cannot be written.
    pub fn save(&self, dir: &Path) -> Result<PathBuf, String> {
        std::fs::create_dir_all(dir)
            .map_err(|_| "The attachment folder cannot be created.".to_string())?;
        let digest = Sha256::digest(&self.bytes);
        let hex: String = digest.iter().take(16).map(|b| format!("{b:02x}")).collect();
        let path = dir.join(format!("{hex}.{}", extension(self.media_type)));
        if !path.exists() {
            std::fs::write(&path, &self.bytes)
                .map_err(|_| "The attachment cannot be saved.".to_string())?;
        }
        Ok(path)
    }
}

/// Where sent attachments are kept: `~/.openagents/coder/attachments`.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".openagents")
            .join("coder")
            .join("attachments")
    })
}

fn extension(media_type: &str) -> &'static str {
    match media_type {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => "pdf",
    }
}

/// The media type of `bytes`, by their signature.
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"%PDF-") {
        Some("application/pdf")
    } else {
        None
    }
}

fn checked(bytes: Vec<u8>, name: Option<String>) -> Result<Attachment, String> {
    let media_type = sniff(&bytes)
        .ok_or("Only PNG, JPEG, GIF and WebP images and PDFs can be attached.".to_string())?;
    let limit = if media_type.starts_with("image/") {
        MAX_IMAGE_BYTES
    } else {
        MAX_PDF_BYTES
    };
    if bytes.len() > limit {
        return Err(format!(
            "{} is {:.1} MB; the limit is {} MB.",
            name.as_deref().unwrap_or("The image"),
            bytes.len() as f64 / (1024.0 * 1024.0),
            limit / (1024 * 1024)
        ));
    }
    Ok(Attachment {
        media_type,
        bytes,
        name,
    })
}

/// An attachment from a pasted `data:` URL.
///
/// # Errors
/// The URL is not base64 image data, or is over the limit.
pub fn from_data_url(url: &str) -> Result<Attachment, String> {
    let rest = url
        .trim()
        .strip_prefix("data:")
        .ok_or("That paste is not an image.")?;
    let (header, data) = rest.split_once(',').ok_or("That paste is not an image.")?;
    if !header.ends_with(";base64") {
        return Err("That paste is not an image.".into());
    }
    if data.len() > MAX_PDF_BYTES * 4 / 3 + 4 {
        return Err(format!(
            "The image is over the {} MB limit.",
            MAX_IMAGE_BYTES / (1024 * 1024)
        ));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| "That pasted image could not be read.".to_string())?;
    checked(bytes, None)
}

/// An attachment from a file.
///
/// # Errors
/// The file is missing, not an image or PDF, or over the limit.
pub fn from_path(path: &Path) -> Result<Attachment, String> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned());
    let shown = name.clone().unwrap_or_else(|| path.display().to_string());
    let meta = std::fs::metadata(path).map_err(|_| format!("{shown} does not exist."))?;
    if !meta.is_file() {
        return Err(format!("{shown} is not a file."));
    }
    if meta.len() > MAX_PDF_BYTES as u64 {
        return Err(format!(
            "{shown} is {:.1} MB; the limit is {} MB.",
            meta.len() as f64 / (1024.0 * 1024.0),
            MAX_PDF_BYTES / (1024 * 1024)
        ));
    }
    let bytes = std::fs::read(path).map_err(|_| format!("{shown} cannot be read."))?;
    checked(bytes, name)
}

/// Whether `path` names a kind of file that attaches.
fn attachable_name(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp", ".pdf"]
        .iter()
        .any(|extension| lower.ends_with(extension))
}

/// The files a paste names when it is nothing but paths to images or PDFs,
/// as a terminal pastes a dropped file: plain, quoted, with `\ ` escapes,
/// or as `file://` URLs, one or more separated by spaces or lines. `None`
/// when the paste is anything else, so ordinary text stays text.
#[must_use]
pub fn dropped_paths(text: &str, cwd: &Path) -> Option<Vec<PathBuf>> {
    let text = text.trim();
    if text.is_empty() || text.len() > 16 * 1024 {
        return None;
    }
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (None, '\\') => current.push(chars.next()?),
            (None, '\'' | '"') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, c) if c.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            (_, c) => current.push(c),
        }
    }
    if quote.is_some() {
        return None;
    }
    if !current.is_empty() {
        words.push(current);
    }
    let mut paths = Vec::new();
    for word in words {
        let word = match word.strip_prefix("file://") {
            Some(rest) => percent_decode(rest)?,
            None => word,
        };
        if !attachable_name(&word) {
            return None;
        }
        let path = if let Some(rest) = word.strip_prefix("~/") {
            PathBuf::from(std::env::var_os("HOME")?).join(rest)
        } else {
            cwd.join(&word)
        };
        if !path.is_file() {
            return None;
        }
        paths.push(path);
    }
    (!paths.is_empty() && paths.len() <= MAX_ATTACHMENTS).then_some(paths)
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The `@path` words of a prompt that name an image or PDF that exists.
#[must_use]
pub fn mentioned_paths(prompt: &str, cwd: &Path) -> Vec<PathBuf> {
    prompt
        .split_whitespace()
        .filter_map(|word| word.strip_prefix('@'))
        .map(|word| word.trim_end_matches([',', '.', ';', ':', ')', '?', '!']))
        .filter(|word| attachable_name(word))
        .map(|word| {
            word.strip_prefix("~/")
                .and_then(|rest| {
                    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(rest))
                })
                .unwrap_or_else(|| cwd.join(word))
        })
        .filter(|path| path.is_file())
        .take(MAX_ATTACHMENTS)
        .collect()
}

/// The note line for an attachment saved at `path`.
#[must_use]
pub fn note(label: &str, number: usize, path: &Path) -> String {
    format!("[{label} #{number}: {}]", path.display())
}

/// The saved files a message's note lines name: only files in a folder
/// named `attachments` with the digest names [`Attachment::save`] gives, so
/// a note cannot send any other file.
fn noted(text: &str) -> Vec<PathBuf> {
    text.lines()
        .filter_map(|line| {
            let inner = line.trim().strip_prefix('[')?.strip_suffix(']')?;
            let (head, path) = inner.split_once(": ")?;
            if !(head.starts_with("Image #") || head.starts_with("PDF #")) {
                return None;
            }
            let path = PathBuf::from(path);
            let stem = path.file_stem()?.to_str()?;
            let saved = path.parent()?.file_name()? == "attachments"
                && stem.len() == 32
                && stem
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
            saved.then_some(path)
        })
        .take(MAX_ATTACHMENTS)
        .collect()
}

/// Whether `text` carries attachment note lines.
#[must_use]
pub fn mentions(text: &str) -> bool {
    !noted(text).is_empty()
}

/// What a model that sees the attachments reads about them.
pub const VISION_NOTE: &str = "Lines such as [Image #1: PATH] are files the user attached; the images and PDFs follow the message. When you hand work to another agent, include those lines so it can open the files.";

/// What a model that cannot see images reads about attachments.
pub const TEXT_ONLY_NOTE: &str = "This model cannot view images or PDFs. A line such as [Image #1: PATH] is a file the user attached: say you cannot see it and work from the user's words, or hand the task to an agent that can open the file, including that line.";

/// What the person sees when the chat's model cannot see images.
pub const TEXT_ONLY_NOTICE: &str = "This model can't see images, so it got each file's location instead. Pick a model that takes images in /models to send the picture itself.";

/// The composer chip for an attachment source (a `data:` URL or a path)
/// without reading or decoding the whole file.
#[must_use]
pub fn chip_for(source: &str, number: usize) -> String {
    let (pdf, name, bytes) = if let Some(rest) = source.strip_prefix("data:") {
        let (header, data) = rest.split_once(',').unwrap_or((rest, ""));
        (
            header.starts_with("application/pdf"),
            None,
            data.len() / 4 * 3,
        )
    } else {
        let path = Path::new(source);
        let bytes = std::fs::metadata(path)
            .map_or(0, |meta| usize::try_from(meta.len()).unwrap_or(usize::MAX));
        (
            source.to_ascii_lowercase().ends_with(".pdf"),
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned()),
            bytes,
        )
    };
    Attachment {
        media_type: if pdf { "application/pdf" } else { "image/png" },
        bytes: Vec::new(),
        name,
    }
    .chip_sized(number, bytes)
}

/// `content` as the request's content parts: the text, then each image or
/// PDF its note lines name. `None` when it names none.
#[must_use]
pub fn expand(content: &str) -> Option<Value> {
    let paths = noted(content);
    let mut parts = vec![json!({"type":"text","text":content})];
    for path in paths {
        let Ok(attachment) = from_path(&path) else {
            continue;
        };
        if attachment.is_image() {
            parts.push(json!({"type":"image_url","image_url":{"url":attachment.data_url()}}));
        } else {
            parts.push(json!({"type":"file","file":{
                "filename": path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
                "file_data": attachment.data_url(),
            }}));
        }
    }
    (parts.len() > 1).then(|| Value::Array(parts))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

    #[test]
    fn a_dropped_path_attaches_and_text_stays_text() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().canonicalize().unwrap();
        std::fs::write(cwd.join("shot one.png"), PNG).unwrap();
        std::fs::write(cwd.join("doc.pdf"), b"%PDF-1.7\n").unwrap();
        std::fs::write(cwd.join("notes.txt"), "x").unwrap();
        let shot = cwd.join("shot one.png");
        for paste in [
            format!("{}", cwd.join("shot\\ one.png").display()),
            format!("'{}'", shot.display()),
            format!("\"{}\" ", shot.display()),
            format!("file://{}", shot.display().to_string().replace(' ', "%20")),
            "shot\\ one.png".to_string(),
        ] {
            assert_eq!(
                dropped_paths(&paste, &cwd),
                Some(vec![shot.clone()]),
                "{paste}"
            );
        }
        assert_eq!(
            dropped_paths("shot\\ one.png doc.pdf", &cwd).unwrap().len(),
            2
        );
        for text in [
            "notes.txt",
            "look at shot.png",
            "missing.png",
            "",
            "'open.png",
        ] {
            assert_eq!(dropped_paths(text, &cwd), None, "{text}");
        }
        let mentioned = mentioned_paths("What's wrong in @doc.pdf? Not @notes.txt.", &cwd);
        assert_eq!(mentioned, [cwd.join("doc.pdf")]);
    }

    #[test]
    fn size_limits_and_kinds_are_enforced() {
        let dir = tempfile::tempdir().unwrap();
        let mut big = PNG.to_vec();
        big.resize(MAX_IMAGE_BYTES + 1, 0);
        std::fs::write(dir.path().join("big.png"), &big).unwrap();
        let error = from_path(&dir.path().join("big.png")).unwrap_err();
        assert!(error.contains("limit is 5 MB"), "{error}");
        std::fs::write(dir.path().join("fake.png"), b"not an image").unwrap();
        assert!(from_path(&dir.path().join("fake.png")).is_err());
        let pasted = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(PNG)
        );
        let image = from_data_url(&pasted).unwrap();
        assert_eq!(image.media_type, "image/png");
        assert_eq!(image.chip(1), "Image #1 · 1 KB");
        assert!(from_data_url("data:image/png;base64,AA").is_err());
        assert!(from_data_url("hello").is_err());
    }

    #[test]
    fn saved_notes_expand_into_image_parts_only_from_the_attachment_folder() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("attachments");
        let image = checked(PNG.to_vec(), Some("shot.png".into())).unwrap();
        let saved = image.save(&store).unwrap();
        assert_eq!(image.save(&store).unwrap(), saved);
        let elsewhere = dir.path().join("other.png");
        std::fs::write(&elsewhere, PNG).unwrap();
        let prompt = format!(
            "What's wrong here?\n{}\n{}",
            note("Image", 1, &saved),
            note("Image", 2, &elsewhere)
        );
        let parts = expand(&prompt).unwrap();
        let parts = parts.as_array().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["text"], prompt);
        assert!(
            parts[1]["image_url"]["url"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
        assert!(expand("no attachments").is_none());
        assert!(mentions(&prompt));
        assert!(!mentions(&note("Image", 1, &elsewhere)));
        assert_eq!(
            chip_for(&saved.display().to_string(), 3),
            format!(
                "Image #3 {} · 1 KB",
                saved.file_name().unwrap().to_string_lossy()
            )
        );
    }
}
