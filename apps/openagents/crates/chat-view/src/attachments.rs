//! Attachments as a sent message carries them: plain local paths after the
//! prompt, which the transcript reads back into tiles and chips. The
//! desktop's staging, upload, and image cache stay in `openagents-ui`.

use std::path::Path;

use gpui::ImageFormat;

// ---------------------------------------------------------------------------
// Text transport (message-attachments.ts)
// ---------------------------------------------------------------------------

/// The body used for image-only sends (`use-attachments.ts`).
pub const ATTACHMENT_ONLY_TEXT: &str = "See the attached image(s).";
/// The body used when any attachment is not an image.
pub const FILE_ATTACHMENT_ONLY_TEXT: &str = "See the attached file(s).";

/// Whether `text` is the placeholder body of an attachments-only send.
pub fn is_attachment_only_text(text: &str) -> bool {
    text == ATTACHMENT_ONLY_TEXT || text == FILE_ATTACHMENT_ONLY_TEXT
}

/// The placeholder body for an attachments-only send of `paths`.
pub fn attachment_only_text(paths: &[String]) -> &'static str {
    if paths.iter().all(|path| is_image_path(path)) {
        ATTACHMENT_ONLY_TEXT
    } else {
        FILE_ATTACHMENT_ONLY_TEXT
    }
}

/// Whether an attachment ref names an image, as opposed to any other file.
pub fn is_image_path(path: &str) -> bool {
    format_by_extension(Path::new(path)).is_some()
}

pub use openagents_proto::attachment_mentions::attachment_display_name;

/// Whether `mention` is the chip for the attachment at `path`.
pub fn chip_names_attachment(
    mention: &openagents_proto::attachment_mentions::AttachmentMention,
    path: &str,
) -> bool {
    mention.names_attachment(path)
}

/// The attachments no chip in `text` stands for, in order. A chip is the
/// attachment's handle wherever it shows, so only these still need a tile;
/// anything a chip can't be matched to keeps its tile.
pub fn unchipped_attachments<'a>(text: &str, paths: &'a [String]) -> Vec<&'a String> {
    let mentions = openagents_proto::attachment_mentions::attachment_mentions(text);
    paths
        .iter()
        .filter(|path| {
            !mentions
                .iter()
                .any(|mention| chip_names_attachment(mention, path))
        })
        .collect()
}

/// How attachments ride the prompt (use-attachments.ts `withAttachments`):
/// plain local paths appended to the text — the files are staged on the device
/// that runs the agent, so the agent can open them with its own tools; the
/// same text is what persists as the user doc entry.
pub fn with_attachments(text: &str, paths: &[String]) -> String {
    if paths.is_empty() {
        return text.to_string();
    }
    let refs: Vec<String> = paths.iter().map(|p| format!("- {p}")).collect();
    let body = if text.is_empty() {
        attachment_only_text(paths)
    } else {
        text
    };
    format!(
        "{body}\n\nAttached images (local files — open them to view):\n{}",
        refs.join("\n")
    )
}

/// An attachment ref parsed back out of a user message's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserImageAttachment {
    pub appshot: Option<crate::appshots::AppshotPresentation>,
    pub id: String,
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedUserMessage {
    /// The visible prompt (the refs trailer stripped; empty for image-only sends).
    pub text: String,
    pub attachments: Vec<UserImageAttachment>,
}

/// The file name at the end of `path`, or "image".
pub fn name_from_path(path: &str) -> String {
    let name = path
        .rsplit(['/', '\\'])
        .next()
        .map(str::trim)
        .unwrap_or_default();
    if name.is_empty() {
        "image".to_string()
    } else {
        name.to_string()
    }
}

/// Find the refs trailer: a blank line, then a line starting (case-insensitive)
/// with `Attached images (local files` and ending `):`. Returns
/// `(body_end, refs_start)` byte offsets — the tolerant equivalent of openagents's
/// `ATTACHED_IMAGES_RE`.
fn find_refs_marker(content: &str) -> Option<(usize, usize)> {
    let needle = b"attached images (local files";
    let mut from = 0usize;
    while let Some(rel) = content[from..].find("\n\n") {
        let gap = from + rel;
        let line_start = gap + 2;
        if !content.as_bytes()[line_start..]
            .get(..needle.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(needle))
        {
            from = line_start;
            continue;
        }
        let line_end = content[line_start..]
            .find('\n')
            .map(|p| line_start + p)
            .unwrap_or(content.len());
        let line = content[line_start..line_end].trim_end_matches('\r');
        if line.ends_with("):") {
            let refs_start = (line_end + 1).min(content.len());
            return Some((gap, refs_start));
        }
        from = line_start;
    }
    None
}

/// message-attachments.ts `parseUserMessageImages`: split the visible prompt
/// from its attachment-ref trailer.
pub fn parse_user_message_images(content: &str) -> ParsedUserMessage {
    let Some((body_end, refs_start)) = find_refs_marker(content) else {
        return ParsedUserMessage {
            text: content.to_string(),
            attachments: Vec::new(),
        };
    };
    let body = crate::appshots::strip_context_for_display(content[..body_end].trim_end());
    let presentations = crate::appshots::presentations(content);
    let attachments: Vec<UserImageAttachment> = content[refs_start..]
        .lines()
        .filter_map(|line| {
            let path = line.trim_start().strip_prefix("- ")?.trim();
            (!path.is_empty()).then(|| path.to_string())
        })
        .enumerate()
        .map(|(index, path)| UserImageAttachment {
            appshot: presentations.get(&path).cloned(),
            id: format!("{index}:{path}"),
            name: name_from_path(&path),
            path,
        })
        .collect();
    if attachments.is_empty() {
        return ParsedUserMessage {
            text: content.to_string(),
            attachments,
        };
    }
    ParsedUserMessage {
        text: if is_attachment_only_text(body.trim()) {
            String::new()
        } else {
            body.to_string()
        },
        attachments,
    }
}

/// message-attachments.ts `userMessageRailText`: what the rail/sidebar shows
/// for a user message ("Attached image" / "N attached images" when image-only).
pub fn user_message_rail_text(content: &str) -> String {
    let parsed = parse_user_message_images(content);
    if !parsed.text.trim().is_empty() {
        return parsed.text;
    }
    let kind = if parsed
        .attachments
        .iter()
        .all(|att| is_image_path(&att.path))
    {
        "image"
    } else {
        "file"
    };
    match parsed.attachments.len() {
        0 => content.to_string(),
        1 => format!("Attached {kind}"),
        n => format!("{n} attached {kind}s"),
    }
}

/// Image formats the whole pipeline supports: intersection of gpui's decoders
/// and the engine's `mime_by_ext` read-back jail.
pub fn format_by_extension(path: &Path) -> Option<ImageFormat> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => Some(ImageFormat::Png),
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "gif" => Some(ImageFormat::Gif),
        "webp" => Some(ImageFormat::Webp),
        "svg" => Some(ImageFormat::Svg),
        "bmp" => Some(ImageFormat::Bmp),
        "tif" | "tiff" => Some(ImageFormat::Tiff),
        _ => None,
    }
}
