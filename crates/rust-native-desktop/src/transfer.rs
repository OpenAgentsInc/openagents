//! What a drop or a paste carries, decided before any bytes move: which of
//! the offered types to ask for, and the files a `text/uri-list` names.
//! Platform-free, so every rule here is tested on every system.

use std::path::PathBuf;

/// Text, in the order a paste asks for it.
pub const TEXT: &[&str] = &[
    "text/plain;charset=utf-8",
    "UTF8_STRING",
    "text/plain",
    "STRING",
    "TEXT",
];

/// An image, in the order a paste or a drop asks for it: pixels first, then
/// the files a file manager offers.
pub const IMAGE: &[&str] = &["image/png", "image/jpeg", "text/uri-list"];

/// What a drop asks for: the files first (a file manager's drag), then the
/// pixels a browser or an image viewer offers without a file.
pub const DROP: &[&str] = &["text/uri-list", "image/png", "image/jpeg"];

/// The most bytes read from one offer. Larger than any image the composer
/// attaches, so an oversized image reaches its own "too large" message.
pub const MAX_BYTES: usize = 64 * 1024 * 1024;

/// The first of `wanted` that `offered` has.
#[must_use]
pub fn pick<'a>(offered: &[String], wanted: &[&'a str]) -> Option<&'a str> {
    wanted
        .iter()
        .find(|want| offered.iter().any(|offer| offer == *want))
        .copied()
}

/// Every type of `wanted` that `offered` has, in `wanted`'s order.
#[must_use]
pub fn all<'a>(offered: &[String], wanted: &[&'a str]) -> Vec<&'a str> {
    wanted
        .iter()
        .filter(|want| offered.iter().any(|offer| offer == *want))
        .copied()
        .collect()
}

/// The local files a `text/uri-list` names (RFC 2483): `file:` URIs on
/// this computer, percent-decoded, in order. Comments, blank lines, other
/// hosts, and other schemes are skipped.
#[must_use]
pub fn uri_list_paths(bytes: &[u8]) -> Vec<PathBuf> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(file_uri_path)
        .collect()
}

/// The path of one `file:` URI on this computer.
#[must_use]
pub fn file_uri_path(uri: &str) -> Option<PathBuf> {
    let rest = uri
        .strip_prefix("file://")
        .or_else(|| uri.strip_prefix("FILE://"))?;
    let path = if rest.starts_with('/') {
        rest
    } else {
        let (host, path) = rest.split_at(rest.find('/')?);
        if !host.eq_ignore_ascii_case("localhost") {
            return None;
        }
        path
    };
    let bytes = percent_decode(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
    }
    #[cfg(not(unix))]
    {
        String::from_utf8(bytes).ok().map(PathBuf::from)
    }
}

fn percent_decode(text: &str) -> Option<Vec<u8>> {
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
    Some(out)
}

/// The file extension for dropped or pasted pixels of `mime`.
#[must_use]
pub fn extension(mime: &str) -> &'static str {
    match mime {
        "image/jpeg" => "jpg",
        _ => "png",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offered(types: &[&str]) -> Vec<String> {
        types.iter().map(|t| (*t).to_owned()).collect()
    }

    #[test]
    fn a_drop_asks_for_files_first_then_pixels() {
        let nautilus = offered(&["x-special/gnome-icon-list", "text/uri-list", "UTF8_STRING"]);
        assert_eq!(pick(&nautilus, DROP), Some("text/uri-list"));
        let firefox_image = offered(&["text/x-moz-url", "text/uri-list", "image/png"]);
        assert_eq!(all(&firefox_image, DROP), ["text/uri-list", "image/png"]);
        let screenshot = offered(&["image/png"]);
        assert_eq!(pick(&screenshot, DROP), Some("image/png"));
        assert_eq!(pick(&offered(&["text/html"]), DROP), None);
    }

    #[test]
    fn a_paste_asks_for_pixels_then_files_and_text_by_its_own_list() {
        let copied = offered(&["text/plain", "text/plain;charset=utf-8"]);
        assert_eq!(pick(&copied, TEXT), Some("text/plain;charset=utf-8"));
        assert_eq!(pick(&copied, IMAGE), None);
        let file_manager = offered(&["text/uri-list", "x-special/gnome-copied-files"]);
        assert_eq!(pick(&file_manager, IMAGE), Some("text/uri-list"));
        let both = offered(&["image/jpeg", "image/png"]);
        assert_eq!(pick(&both, IMAGE), Some("image/png"));
    }

    #[test]
    fn a_uri_list_names_local_files_only() {
        let list = b"# dragged from Files\r\nfile:///home/kai/My%20Pictures/cat.png\r\n\
file://localhost/tmp/a%25b.jpg\nhttps://example.com/dog.png\nfile://otherhost/x.png\n\n\
file:///home/kai/caf%C3%A9.txt\n";
        assert_eq!(
            uri_list_paths(list),
            [
                PathBuf::from("/home/kai/My Pictures/cat.png"),
                PathBuf::from("/tmp/a%b.jpg"),
                PathBuf::from("/home/kai/café.txt"),
            ]
        );
        // A broken escape is not guessed at.
        assert_eq!(
            uri_list_paths(b"file:///tmp/%zz.png"),
            Vec::<PathBuf>::new()
        );
        assert_eq!(uri_list_paths(b"file:///tmp/%4"), Vec::<PathBuf>::new());
    }

    #[cfg(unix)]
    #[test]
    fn a_uri_keeps_bytes_that_are_not_utf8() {
        use std::os::unix::ffi::OsStrExt;
        let path = file_uri_path("file:///tmp/%FF.png").unwrap();
        assert_eq!(path.as_os_str().as_bytes(), b"/tmp/\xff.png");
    }
}
