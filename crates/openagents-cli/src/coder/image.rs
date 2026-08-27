//! Image attachments pasted or dropped into the Coder composer.
//!
//! Terminal emulators represent a dropped file as bracketed-paste text. This
//! module recognizes a paste made entirely of supported image paths, reads the
//! bytes immediately, and returns real multimodal attachments. The composer
//! receives only path-free `[Image #N]` markers.

use std::path::{Path, PathBuf};

use base64::Engine as _;

use crate::runtime::ImageAttachment;

const MAX_IMAGE_BYTES: u64 = 4_000_000;

/// Read a dropped-image paste.
///
/// `Ok(None)` means the paste is ordinary text. `Err` means it looked like an
/// image drop but the image could not be attached.
pub fn read_dropped_images(
    text: &str,
    next_id: &mut usize,
) -> Result<Option<Vec<ImageAttachment>>, String> {
    let paths = dropped_paths(text);
    if paths.is_empty() {
        return Ok(None);
    }

    let mut attachments = Vec::with_capacity(paths.len());
    for path in paths {
        let metadata = std::fs::metadata(&path)
            .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
        if !metadata.is_file() {
            return Err(format!("{} is not a file.", path.display()));
        }
        if metadata.len() > MAX_IMAGE_BYTES {
            return Err(format!(
                "{} is larger than the 4 MB image limit.",
                path.display()
            ));
        }
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
        let mime_type = image_mime(&bytes)
            .ok_or_else(|| format!("{} is not a supported image.", path.display()))?;
        let id = *next_id;
        *next_id = next_id.saturating_add(1);
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        attachments.push(ImageAttachment {
            id,
            filename: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("image")
                .to_string(),
            mime_type: mime_type.to_string(),
            data_url: format!("data:{mime_type};base64,{encoded}"),
        });
    }
    Ok(Some(attachments))
}

fn dropped_paths(text: &str) -> Vec<PathBuf> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    // A copied path can contain unescaped spaces. Admit the whole paste first
    // when it resolves to one supported file.
    let whole = normalize_path(trimmed);
    if supported_extension(&whole) && whole.is_file() {
        return vec![whole];
    }

    let words = split_shell_words(trimmed);
    if words.is_empty() {
        return Vec::new();
    }
    let paths = words
        .into_iter()
        .map(|word| normalize_path(&word))
        .collect::<Vec<_>>();
    if paths.iter().all(|path| supported_extension(path)) {
        paths
    } else {
        Vec::new()
    }
}

fn normalize_path(raw: &str) -> PathBuf {
    let raw = raw.trim();
    let unquoted = if (raw.starts_with('"') && raw.ends_with('"'))
        || (raw.starts_with('\'') && raw.ends_with('\''))
    {
        &raw[1..raw.len().saturating_sub(1)]
    } else {
        raw
    };
    let unescaped = if cfg!(windows) {
        unquoted.to_string()
    } else {
        let mut value = String::new();
        let mut chars = unquoted.chars();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                value.push(chars.next().unwrap_or(ch));
            } else {
                value.push(ch);
            }
        }
        value
    };
    if let Ok(url) = url::Url::parse(&unescaped)
        && url.scheme() == "file"
        && let Ok(path) = url.to_file_path()
    {
        return path;
    }
    PathBuf::from(unescaped)
}

fn supported_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp"
            )
        })
}

fn split_shell_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut escaped = false;
    for ch in text.chars() {
        if escaped {
            word.push(ch);
            escaped = false;
        } else if ch == '\\' && !cfg!(windows) {
            escaped = true;
        } else if matches!(ch, '\'' | '"') {
            if quote == Some(ch) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(ch);
            } else {
                word.push(ch);
            }
        } else if ch.is_whitespace() && quote.is_none() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
        } else {
            word.push(ch);
        }
    }
    if escaped {
        word.push('\\');
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_image_path_with_escaped_spaces_becomes_an_attachment() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("screen shot.png");
        std::fs::write(&path, b"\x89PNG\r\n\x1a\nimage").unwrap();
        let dropped = path.display().to_string().replace(' ', "\\ ");
        let mut next = 1;

        let images = read_dropped_images(&dropped, &mut next)
            .unwrap()
            .expect("an image paste");
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].id, 1);
        assert_eq!(images[0].mime_type, "image/png");
        assert!(images[0].data_url.starts_with("data:image/png;base64,"));
        assert_eq!(next, 2);
    }

    #[test]
    fn ordinary_pasted_text_stays_text() {
        let mut next = 1;
        assert_eq!(
            read_dropped_images("look at /tmp/image.png later", &mut next),
            Ok(None)
        );
    }
}
