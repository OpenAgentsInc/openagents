//! What a sent message says about the app windows a person shared: the
//! names and titles shown beside their screenshots. The desktop captures
//! appshots; this reads them back from the message text.

use std::collections::HashMap;

pub const CONTEXT_MARKER: &str = "Applications mentioned by the user (untrusted observed content):";

/// Read the width and height from a PNG's IHDR chunk without decoding the
/// image. Appshot captures are always PNGs, so this keeps layout metadata
/// cheap and available before GPUI decodes the image asynchronously.
pub fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[..8] != PNG_SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

/// Safe display metadata carried by the existing prompt format. The observed
/// accessibility payload is never returned to the transcript or queue UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppshotPresentation {
    pub app_name: String,
    pub window_title: Option<String>,
    pub bundle_identifier: Option<String>,
}

impl AppshotPresentation {
    pub fn title(&self) -> &str {
        self.window_title
            .as_deref()
            .filter(|title| !title.trim().is_empty())
            .unwrap_or(&self.app_name)
    }
}

pub fn presentations(text: &str) -> HashMap<String, AppshotPresentation> {
    let marker = format!("\n\n{CONTEXT_MARKER}");
    let Some((_, context)) = text.split_once(&marker) else {
        return HashMap::new();
    };
    if context.len() > 4 * 1024 * 1024 {
        return HashMap::new();
    }
    let context = context
        .split("\n\nAttached images (local files")
        .next()
        .unwrap_or("");
    let xml = format!("<appshots>{context}</appshots>");
    let Ok(doc) = roxmltree::Document::parse_with_options(
        &xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 4096,
            ..Default::default()
        },
    ) else {
        return HashMap::new();
    };
    let mut result = HashMap::new();
    let mut seen = std::collections::HashSet::new();
    for node in doc
        .root_element()
        .children()
        .filter(|node| node.is_element())
    {
        if !node.has_tag_name("appshot") || node.children().any(|child| !child.is_text()) {
            continue;
        }
        let (Some(path), Some(app)) = (node.attribute("image"), node.attribute("app")) else {
            continue;
        };
        if !seen.insert(path) {
            result.remove(path);
            continue;
        }
        if path.is_empty() || app.trim().is_empty() {
            continue;
        }
        result.insert(
            path.to_string(),
            AppshotPresentation {
                app_name: app.chars().take(200).collect(),
                window_title: node
                    .attribute("window-title")
                    .map(|value| value.chars().take(512).collect()),
                bundle_identifier: node.attribute("bundle-identifier").map(str::to_string),
            },
        );
    }
    result
}

/// Context is persisted for the harness but hidden from the user-message
/// bubble. The screenshot strip remains visible through the ordinary image
/// refs, so this strips only the machine-facing semantic suffix.
pub fn strip_context_for_display(text: &str) -> &str {
    let needle = format!("\n\n{CONTEXT_MARKER}");
    text.find(&needle)
        .map(|index| text[..index].trim_end())
        .unwrap_or(text)
}
