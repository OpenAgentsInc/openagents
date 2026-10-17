//! Design Lab: every version of every mobile app screen on one board.
//!
//! The board is read from git history. Every commit that added or changed a
//! phone screenshot is a version of that screen, so screens removed from the
//! tree keep their past versions. A screen is the image's file name without
//! dates or widths; its area is the folder it was captured into.
//!
//! The board is a `board.v1` document, so it is also a Lab kind
//! ([`kind::Design`]): renames and areas are checked changes, and the card
//! shows the newest version of each screen.

pub mod kind;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

pub const SCHEMA: &str = "board.v1";

/// Words in a path that mark a phone screenshot.
const PHONE_WORDS: &[&str] = &["mobile", "ios", "iphone", "phone", "android", "appshots"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Board {
    pub schema: String,
    pub name: String,
    pub screens: Vec<Screen>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screen {
    pub id: String,
    pub title: String,
    pub area: String,
    /// Oldest first.
    pub versions: Vec<ScreenVersion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenVersion {
    /// The image's git blob id.
    pub blob: String,
    pub path: String,
    pub commit: String,
    pub unix: u64,
    pub subject: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
}

impl Board {
    pub fn versions(&self) -> usize {
        self.screens.iter().map(|s| s.versions.len()).sum()
    }
}

/// True when a repository path looks like a phone screenshot.
pub fn is_phone_screenshot(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let image = [".png", ".jpg", ".jpeg"].iter().any(|e| lower.ends_with(e));
    if !image || lower.contains(".xcassets/") || lower.contains("icon") {
        return false;
    }
    let words = lower.split(['/', '-', '_', '.', ' ']);
    words
        .into_iter()
        .any(|w| PHONE_WORDS.contains(&w) || w.starts_with("iphone"))
}

fn strip_date(word: &str) -> &str {
    let b = word.as_bytes();
    let dated = b.len() >= 11
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
        && b[10] == b'-';
    if dated { &word[11..] } else { word }
}

/// The screen a path belongs to: its file name without a date, an order
/// number or a trailing width.
pub fn screen_id(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    let stem = strip_date(&stem.to_ascii_lowercase()).to_string();
    let mut words: Vec<&str> = stem
        .split(['-', '_', ' '])
        .filter(|w| !w.is_empty())
        .collect();
    while words.len() > 1 && words[0].chars().all(|c| c.is_ascii_digit()) {
        words.remove(0);
    }
    while words.len() > 1
        && words
            .last()
            .is_some_and(|w| w.chars().all(|c| c.is_ascii_digit()))
    {
        words.pop();
    }
    words.join("-")
}

/// The area a path was captured into: its folder, without a date.
pub fn area(path: &str) -> String {
    let mut dirs = path.rsplit('/').skip(1);
    let folder = dirs.next().unwrap_or("");
    let folder = strip_date(folder);
    // Dated capture folders say little alone; name the area by what holds them.
    let generic = folder.is_empty()
        || ["verification", "screenshots", "attachments"].contains(&folder)
        || folder.starts_with("native-");
    if generic {
        dirs.next().map(strip_date).unwrap_or(folder).to_string()
    } else {
        folder.to_string()
    }
}

fn title(id: &str) -> String {
    let mut words = id.split('-');
    let first = words.next().unwrap_or_default();
    let mut out = String::new();
    let mut chars = first.chars();
    if let Some(c) = chars.next() {
        out.extend(c.to_uppercase());
        out.push_str(chars.as_str());
    }
    for w in words {
        out.push(' ');
        out.push_str(w);
    }
    out
}

/// One image change found in history.
#[derive(Clone, Debug)]
struct Change {
    blob: String,
    path: String,
    commit: String,
    unix: u64,
    subject: String,
}

fn git(repo: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo);
    command
}

/// Every added or changed phone screenshot on the current branch's history.
fn changes(repo: &Path) -> Result<Vec<Change>, String> {
    let out = git(repo)
        .args([
            "log",
            "--format=%x01%H%x09%ct%x09%s",
            "--raw",
            "--no-abbrev",
            "--no-renames",
            "--diff-filter=AM",
            "--",
            "*.png",
            "*.jpg",
            "*.jpeg",
        ])
        .output()
        .map_err(|e| format!("can't run git ({e})"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut found = Vec::new();
    let (mut commit, mut unix, mut subject) = (String::new(), 0, String::new());
    for line in text.lines() {
        if let Some(head) = line.strip_prefix('\u{1}') {
            let mut parts = head.splitn(3, '\t');
            commit = parts.next().unwrap_or_default().to_string();
            unix = parts.next().and_then(|t| t.parse().ok()).unwrap_or(0);
            subject = parts.next().unwrap_or_default().to_string();
        } else if let Some(raw) = line.strip_prefix(':') {
            // ":100644 100644 <old> <new> M\tpath"
            let Some((meta, path)) = raw.split_once('\t') else {
                continue;
            };
            let blob = meta.split(' ').nth(3).unwrap_or_default();
            if blob.is_empty() || !is_phone_screenshot(path) {
                continue;
            }
            found.push(Change {
                blob: blob.to_string(),
                path: path.to_string(),
                commit: commit.clone(),
                unix,
                subject: subject.clone(),
            });
        }
    }
    Ok(found)
}

/// Width and height from a PNG's header, or zero for other images.
fn png_size(bytes: &[u8]) -> (u32, u32) {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return (0, 0);
    }
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    (w, h)
}

/// Reads each blob's image size with one `git cat-file --batch`.
fn sizes(repo: &Path, blobs: &[String]) -> Result<BTreeMap<String, (u32, u32)>, String> {
    let mut child = git(repo)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("can't run git ({e})"))?;
    let mut stdin = child.stdin.take().ok_or("git has no input")?;
    let input = blobs.join("\n") + "\n";
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let mut reader = BufReader::new(child.stdout.take().ok_or("git has no output")?);
    let mut found = BTreeMap::new();
    for blob in blobs {
        let mut header = String::new();
        reader.read_line(&mut header).map_err(|e| e.to_string())?;
        let size: usize = match header.trim_end().rsplit(' ').next() {
            Some(n) if !header.contains("missing") => n.parse().unwrap_or(0),
            _ => continue,
        };
        let mut body = vec![0; size + 1];
        reader.read_exact(&mut body).map_err(|e| e.to_string())?;
        found.insert(blob.clone(), png_size(&body));
    }
    let _ = writer.join();
    let _ = child.wait();
    Ok(found)
}

/// Builds the board from the repository's history.
pub fn scan(repo: &Path) -> Result<Board, String> {
    let mut changes = changes(repo)?;
    changes.sort_by_key(|c| c.unix);
    let mut blobs: Vec<String> = changes.iter().map(|c| c.blob.clone()).collect();
    blobs.sort();
    blobs.dedup();
    let sizes = sizes(repo, &blobs)?;
    let mut screens: BTreeMap<String, Screen> = BTreeMap::new();
    for change in changes {
        let (width, height) = sizes.get(&change.blob).copied().unwrap_or((0, 0));
        // Icons and thumbnails aren't screens.
        if width != 0 && width.min(height) < 200 {
            continue;
        }
        let id = screen_id(&change.path);
        let screen = screens.entry(id.clone()).or_insert_with(|| Screen {
            title: title(&id),
            area: area(&change.path),
            id,
            versions: Vec::new(),
        });
        // The same picture copied to a second place is one version.
        if screen.versions.iter().any(|v| v.blob == change.blob) {
            continue;
        }
        screen.versions.push(ScreenVersion {
            blob: change.blob,
            path: change.path,
            commit: change.commit,
            unix: change.unix,
            subject: change.subject,
            width,
            height,
        });
    }
    let mut screens: Vec<Screen> = screens.into_values().collect();
    screens.sort_by(|a, b| (&a.area, &a.id).cmp(&(&b.area, &b.id)));
    Ok(Board {
        schema: SCHEMA.into(),
        name: "OpenAgents mobile app".into(),
        screens,
    })
}

/// A blob's bytes, only if the board holds it.
pub fn image(repo: &Path, board: &Board, blob: &str) -> Option<Vec<u8>> {
    let known = board
        .screens
        .iter()
        .flat_map(|s| &s.versions)
        .any(|v| v.blob == blob);
    if !known {
        return None;
    }
    let out = git(repo).args(["cat-file", "blob", blob]).output().ok()?;
    out.status.success().then_some(out.stdout)
}

/// The board page. It fetches `board.json` and images from `img/{blob}`, so
/// the served and exported boards are the same file.
pub const PAGE: &str = include_str!("board.html");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_screenshots_are_found_by_folder_words() {
        assert!(is_phone_screenshot(
            "apps/openagents/docs/screenshots/mobile-polish/home-glass.png"
        ));
        assert!(is_phone_screenshot(
            "bins/coder-ios/verification/2026-09-27-computer-hud/a.png"
        ));
        assert!(is_phone_screenshot(
            "x/appshots/appshots-chat-light-390.png"
        ));
        assert!(!is_phone_screenshot(
            "crates/openagents-desktop/screenshots/a.png"
        ));
        assert!(!is_phone_screenshot("docs/scenarios/a.png"));
        assert!(!is_phone_screenshot(
            "apps/ios/OpenAgents/Assets.xcassets/AppIcon.appiconset/a.png"
        ));
        assert!(!is_phone_screenshot("docs/mobile/notes.md"));
    }

    #[test]
    fn a_screen_is_its_name_without_dates_order_or_width() {
        assert_eq!(
            screen_id("a/appshots-chat-light-1100.png"),
            "appshots-chat-light"
        );
        assert_eq!(
            screen_id("a/appshots-chat-light-390.png"),
            "appshots-chat-light"
        );
        assert_eq!(screen_id("a/2026-09-27-01-home.png"), "home");
        assert_eq!(screen_id("a/CARD-04-worse.png"), "card-04-worse");
        assert_eq!(screen_id("a/42.png"), "42");
        assert_eq!(title("card-04-worse"), "Card 04 worse");
    }

    #[test]
    fn the_area_is_the_capture_folder_without_its_date() {
        assert_eq!(area("x/mobile-polish/home.png"), "mobile-polish");
        assert_eq!(
            area("bins/coder-ios/verification/2026-09-27-forest-zones/a.png"),
            "forest-zones"
        );
        assert_eq!(
            area("bins/openagents-mockup-ios/verification/a.png"),
            "openagents-mockup-ios"
        );
        assert_eq!(
            area("x/2026-09-26-world-pairing/native-attachments/a.png"),
            "world-pairing"
        );
    }

    #[test]
    fn png_headers_give_the_size() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend(1179u32.to_be_bytes());
        png.extend(2556u32.to_be_bytes());
        assert_eq!(png_size(&png), (1179, 2556));
        assert_eq!(png_size(b"GIF89a"), (0, 0));
    }
}
