//! File evidence opened from a transcript, never from the reader's live workspace.

use std::cell::Cell;
use std::fs;
use std::io::Read;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

use coder_terminal::{Intensity, Ladder, frame, wrap_rows};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::runs::Run;
use crate::runs_transcript::{Block, Kind};
use crate::runs_tui::Key;

const FILE_LIMIT: u64 = 2 * 1024 * 1024;
const INDEX_LIMIT: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct Hit {
    pub cells: Rect,
    pub step: usize,
    pub path: String,
}

/// Recognizes paths in shell commands, tool arguments, and rendered prose.
/// A line suffix such as `:12:4` belongs to the link, not the file name.
fn paths(text: &str) -> Vec<(Range<usize>, String)> {
    let mut found = Vec::new();
    let mut start = None;
    for (at, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        let part = c.is_alphanumeric() || "/._-~:@+".contains(c);
        if part {
            start.get_or_insert(at);
        } else if let Some(begin) = start.take() {
            let token = text[begin..at].trim_end_matches(['.', ':']);
            let file = token.split(':').next().unwrap_or_default();
            let leaf = file.rsplit('/').next().unwrap_or_default();
            if !file.is_empty()
                && !token.contains("://")
                && !matches!(file, "http" | "https")
                && !file.ends_with('/')
                && (file.contains('/') || leaf.contains('.'))
                && leaf.chars().any(char::is_alphabetic)
            {
                found.push((begin..begin + token.len(), file.to_owned()));
            }
        }
    }
    found
}

/// Uses the cells actually drawn, so mouse hitboxes follow scrolling, resizing,
/// Markdown, and wide characters rather than guessed character offsets.
pub(crate) fn decorate(
    area: Rect,
    drawn: &[(u16, usize)],
    blocks: &[Block],
    buf: &mut Buffer,
) -> Vec<Hit> {
    let mut hits = Vec::new();
    for &(y, step) in drawn {
        if y < area.top() || y >= area.bottom() {
            continue;
        }
        let mut text = String::new();
        let mut cells = Vec::new();
        let mut x = area.left();
        while x < area.right() {
            let symbol = buf[(x, y)].symbol();
            let width = ratatui::text::Span::raw(symbol).width().max(1) as u16;
            cells.extend(std::iter::repeat_n(x, symbol.len()));
            text.push_str(symbol);
            x = x.saturating_add(width);
        }
        let known = blocks.get(step).map(block_paths).unwrap_or_default();
        let mut tokens = paths(&text);
        // A read/edit's explicit path also covers names without an extension
        // and paths containing spaces. Prefer it over individual word tokens.
        let mut complete = Vec::new();
        for path in &known {
            for (start, _) in text.match_indices(path) {
                complete.push((start..start + path.len(), path.clone()));
            }
        }
        tokens.retain(|(range, _)| {
            !complete
                .iter()
                .any(|(full, _)| full.start <= range.start && full.end >= range.end)
        });
        complete.extend(tokens);
        for (range, mut path) in complete {
            // A narrow pane can wrap or clip a path. Resolve a visible path
            // fragment only when exactly one path in this step contains it.
            if !known.contains(&path) && path.len() >= 4 {
                let matches: Vec<_> = known.iter().filter(|p| p.contains(&path)).collect();
                if matches.len() == 1 {
                    path = matches[0].clone();
                }
            }
            let Some(&left) = cells.get(range.start) else {
                continue;
            };
            let right = cells.get(range.end).copied().unwrap_or(area.right());
            if right <= left {
                continue;
            }
            for x in left..right {
                buf[(x, y)].modifier.insert(Modifier::UNDERLINED);
            }
            hits.push(Hit {
                cells: Rect::new(left, y, right - left, 1),
                step,
                path,
            });
        }
    }
    hits
}

fn block_paths(block: &Block) -> Vec<String> {
    let mut out = match &block.kind {
        Kind::Look { what, .. } if what.starts_with("Read ") => vec![what[5..].to_owned()],
        Kind::Edit { path, .. } => path.split(", ").map(str::to_owned).collect(),
        Kind::Command { command, .. } => {
            let mut paths: Vec<_> = paths(command).into_iter().map(|(_, p)| p).collect();
            if let Some(p) = read_command_path(command) {
                paths.push(p.to_owned());
            }
            paths
        }
        _ => Vec::new(),
    };
    out.retain(|p| !p.is_empty());
    out.sort();
    out.dedup();
    out
}

pub(crate) fn clicked(hits: &[Hit], column: u16, row: u16) -> Option<&Hit> {
    hits.iter()
        .find(|hit| hit.cells.contains((column, row).into()))
}

struct Version {
    label: String,
    text: String,
    /// Only a transcript observation at or before the clicked step opens by
    /// default. Undated and later snapshots require explicit selection.
    observed: bool,
}

pub(crate) struct Viewer {
    path: String,
    versions: Vec<Version>,
    index: usize,
    scroll: Cell<usize>,
    page: Cell<usize>,
    bottom: Cell<usize>,
}

impl Viewer {
    pub fn open(run: Option<&Run>, blocks: &[Block], step: usize, path: &str) -> Self {
        let mut versions = Vec::new();
        let workdir = run
            .and_then(|r| r.files.episode.as_deref())
            .and_then(|root| json(root, "manifest.json"))
            .and_then(|m| m["workdir"].as_str().map(str::to_owned));
        let matches = |other: &str| {
            same(other, path)
                || workdir.as_deref().is_some_and(|root| {
                    let absolute = |p: &str| {
                        if Path::new(p).is_absolute() {
                            PathBuf::from(p)
                        } else {
                            Path::new(root).join(p)
                        }
                    };
                    absolute(other) == absolute(path)
                })
        };
        for (index, block) in blocks.iter().enumerate().take(step + 1).rev() {
            let observation = match &block.kind {
                Kind::Look { what, output } if what.strip_prefix("Read ").is_some_and(&matches) => {
                    Some((
                        "Recorded read output (may be a region or truncated)",
                        output,
                    ))
                }
                Kind::Edit {
                    path: edited,
                    action,
                    body,
                    ..
                } if edited.split(", ").any(&matches) => Some((
                    if *action == "Wrote" {
                        "Recorded write request (not proof of successful execution)"
                    } else {
                        "Recorded edit or patch (not a complete file)"
                    },
                    body,
                )),
                Kind::Command {
                    command,
                    output,
                    exit: Some(0),
                    failed: false,
                } if read_command_path(command).is_some_and(&matches) => Some((
                    "Recorded file-read command output (may be a region or truncated)",
                    output,
                )),
                _ => None,
            };
            if let Some((kind, text)) = observation {
                versions.push(Version {
                    label: format!("{kind}; step {}. This is evidence from that step, not reconstructed later state.", index + 1),
                    text: if text.len() as u64 <= FILE_LIMIT { text.clone() } else { format!("Recorded content exceeds the {FILE_LIMIT}-byte viewer limit; inspect the source trace.") },
                    observed: true,
                });
                if versions.len() >= 64 {
                    break;
                }
            }
        }
        if let Some(run) = run {
            retained(run, path, &mut versions);
        }
        if versions.first().is_none_or(|v| !v.observed) {
            versions.insert(0, Version {
                label: format!("No file contents recorded at or before step {}.", step + 1),
                text: if versions.is_empty() {
                    "No retained file matches this path. The run may have saved only a transcript, a partial artifact set, or no file snapshot. Gym does not read your current workspace or rerun a command to fill the gap.".to_owned()
                } else {
                    "Retained artifacts or snapshots are available. Press Tab to inspect them explicitly. Their contents may come from after the selected step; they are not the file at this replay time.".to_owned()
                },
                observed: false,
            });
        }
        Self {
            path: path.to_owned(),
            versions,
            index: 0,
            scroll: Cell::new(0),
            page: Cell::new(10),
            bottom: Cell::new(0),
        }
    }

    /// Returns true when Escape closes only the viewer.
    pub fn key(&mut self, key: Key) -> bool {
        let scroll = self.scroll.get();
        match key {
            Key::Back => return true,
            Key::Tab | Key::Right | Key::Char(']') => {
                self.index = (self.index + 1) % self.versions.len();
                self.scroll.set(0);
            }
            Key::Left | Key::Char('[') => {
                self.index = (self.index + self.versions.len() - 1) % self.versions.len();
                self.scroll.set(0);
            }
            Key::Up | Key::Char('k') => self.scroll.set(scroll.saturating_sub(1)),
            Key::Down | Key::Char('j') => self
                .scroll
                .set(scroll.saturating_add(1).min(self.bottom.get())),
            Key::WheelUp { .. } => self.scroll.set(scroll.saturating_sub(3)),
            Key::WheelDown { .. } => self
                .scroll
                .set(scroll.saturating_add(3).min(self.bottom.get())),
            Key::PageUp => self.scroll.set(scroll.saturating_sub(self.page.get())),
            Key::PageDown => self.scroll.set(
                scroll
                    .saturating_add(self.page.get())
                    .min(self.bottom.get()),
            ),
            Key::Home | Key::Char('g') => self.scroll.set(0),
            Key::End | Key::Char('G') => self.scroll.set(self.bottom.get()),
            _ => {}
        }
        false
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, ladder: Ladder) {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].reset();
                buf[(x, y)].set_style(ladder.style(Intensity::Half).bg(ladder.background()));
            }
        }
        if area.width < 8 || area.height < 6 {
            return;
        }
        frame(area, buf, ladder.style(Intensity::Full));
        let width = usize::from(area.width - 4);
        let left = area.x + 2;
        buf.set_stringn(
            left,
            area.y + 1,
            safe(&format!(
                "File: {} · evidence {}/{}",
                self.path,
                self.index + 1,
                self.versions.len()
            )),
            width,
            ladder.style(Intensity::Full),
        );
        let version = &self.versions[self.index];
        let mut rows = Vec::new();
        let label = safe(&version.label);
        for range in wrap_rows(&label, width) {
            rows.push(label[range].to_owned());
        }
        rows.push(String::new());
        // Literal source, not Markdown: preserve comments, whitespace, and patches.
        for (number, line) in safe(&version.text).lines().enumerate() {
            let prefix = format!("{:>5} │ ", number + 1);
            let room = width.saturating_sub(prefix.len()).max(1);
            let ranges = wrap_rows(line, room);
            if ranges.is_empty() {
                rows.push(prefix);
                continue;
            }
            for (part, range) in ranges.into_iter().enumerate() {
                rows.push(format!(
                    "{}{}",
                    if part == 0 {
                        prefix.clone()
                    } else {
                        "      │ ".to_owned()
                    },
                    &line[range]
                ));
            }
        }
        let page = usize::from(area.height - 4);
        self.page.set(page);
        self.bottom.set(rows.len().saturating_sub(page));
        self.scroll.set(self.scroll.get().min(self.bottom.get()));
        for (offset, row) in rows.iter().skip(self.scroll.get()).take(page).enumerate() {
            buf.set_stringn(
                left,
                area.y + 2 + offset as u16,
                row,
                width,
                ladder.style(Intensity::ThreeQuarters),
            );
        }
        buf.set_stringn(
            left,
            area.bottom() - 2,
            "↑/↓ PgUp/PgDn scroll · Home/End · Tab/[/] evidence · Esc transcript",
            width,
            ladder.style(Intensity::Full),
        );
    }
}

fn read_command_path(command: &str) -> Option<&str> {
    // Deliberately exclude compound shells: their combined stdout cannot be
    // attributed to one file. Never execute a command to recover its contents.
    if command.chars().any(|c| ";|&<>$`\n".contains(c)) {
        return None;
    }
    let parts: Vec<_> = command.split_whitespace().collect();
    let path = match parts.as_slice() {
        ["cat", path] | ["cat", "--", path] | ["head" | "tail", path] => *path,
        ["head" | "tail", "-n", count, path] if count.parse::<usize>().is_ok() => *path,
        ["sed", "-n", range, path]
            if range
                .trim_matches(['\'', '"'])
                .strip_suffix('p')
                .is_some_and(|r| r.chars().all(|c| c.is_ascii_digit() || c == ',')) =>
        {
            *path
        }
        _ => return None,
    };
    (!path.starts_with('-')).then(|| path.trim_matches(['\'', '"']))
}

fn same(a: &str, b: &str) -> bool {
    a.trim_start_matches("./") == b.trim_start_matches("./")
}

fn safe(text: &str) -> String {
    text.chars()
        .flat_map(|c| match c {
            '\n' => "\n".chars().collect::<Vec<_>>(),
            '\t' => "    ".chars().collect(),
            c if c.is_control() => c.escape_default().collect(),
            c => vec![c],
        })
        .collect()
}

/// No absolute path, parent traversal, or symlink in a retained reference can
/// redirect this read into the viewer's host filesystem.
fn read(root: &Path, relative: &Path, limit: u64) -> Result<Vec<u8>, String> {
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err("Refused an unsafe retained path".to_owned());
    }
    let mut current = root.to_path_buf();
    if fs::symlink_metadata(root)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Refused a symlink evidence root".to_owned());
    }
    for part in relative.components() {
        current.push(part);
        if fs::symlink_metadata(&current)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("Refused a symlink in retained evidence".to_owned());
        }
    }
    let meta = fs::metadata(&current).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > limit {
        return Err(format!("Not a regular file within the {limit}-byte limit"));
    }
    let mut file = fs::File::open(current).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Retained file grew past the read limit".to_owned());
    }
    Ok(bytes)
}

fn json(root: &Path, path: &str) -> Option<Value> {
    serde_json::from_slice(&read(root, Path::new(path), INDEX_LIMIT).ok()?).ok()
}

fn add_file(
    versions: &mut Vec<Version>,
    root: &Path,
    relative: &Path,
    expected: Option<&str>,
    label: String,
) {
    if versions.len() >= 128 {
        return;
    }
    let result = read(root, relative, FILE_LIMIT).and_then(|bytes| {
        if expected.is_some_and(|hash| format!("{:x}", Sha256::digest(&bytes)) != hash) {
            return Err("Retained file failed its SHA-256 check".to_owned());
        }
        if bytes.contains(&0) {
            return Err("Binary file: no text preview".to_owned());
        }
        String::from_utf8(bytes).map_err(|_| "File is not UTF-8 text".to_owned())
    });
    versions.push(Version {
        label: format!(
            "{label}. Source: {}. {}",
            root.join(relative).display(),
            if expected.is_some() {
                "SHA-256 is checked before display."
            } else {
                "No content digest was retained."
            }
        ),
        text: result.unwrap_or_else(|why| format!("Cannot display retained file: {why}")),
        observed: false,
    });
}

fn retained(run: &Run, path: &str, versions: &mut Vec<Version>) {
    let Some(root) = run.files.episode.as_deref() else {
        // Harbor's final artifact export uses the same source/destination map.
        produced(&run.files.dir, "artifacts", path, versions);
        return;
    };
    let manifest = json(root, "manifest.json").unwrap_or(Value::Null);
    let workdir = manifest["workdir"].as_str().unwrap_or("");
    let requested = Path::new(path);
    let relative = if requested.is_absolute() {
        requested
            .strip_prefix(workdir)
            .ok()
            .filter(|_| !workdir.is_empty())
    } else {
        Some(requested)
    };
    // Artifact paths are rooted in this episode, never in the current checkout.
    let artifact = path.split_once("/episode/").map_or(path, |(_, tail)| tail);
    let files = manifest["files"].as_object();
    if let Some(files) = files {
        for record in files.values() {
            let Some(name) = record["path"].as_str() else {
                continue;
            };
            if same(artifact, name) {
                add_file(
                    versions,
                    root,
                    Path::new(name),
                    record["sha256"].as_str(),
                    "Retained artifact; capture time unknown".to_owned(),
                );
            }
        }
    }
    // The retention inventory also names evaluator inputs and other artifacts
    // that the episode manifest does not list individually. A relative suffix
    // can match several versions; keep each source explicit instead of guessing.
    if let Some(inventory) = json(root, "retention.json") {
        for record in inventory["files"]
            .as_array()
            .into_iter()
            .flatten()
            .take(20_000)
        {
            let Some(name) = record["path"].as_str() else {
                continue;
            };
            let artifact_path = Path::new(artifact);
            if name.starts_with("artifacts/")
                && !artifact_path.is_absolute()
                && Path::new(name).ends_with(artifact_path)
                && !name.contains("/session-")
            {
                add_file(
                    versions,
                    root,
                    Path::new(name),
                    record["sha256"].as_str(),
                    "Retained artifact; capture time unknown, may be later than this step"
                        .to_owned(),
                );
            }
        }
    }
    // Candidate snapshots are labeled by dispatch/session, not passed off as
    // the workspace at the clicked command. Selection records have no capture
    // timestamp, and evaluator execution can follow the native session end.
    if let Some(relative) = relative {
        let Ok(dirs) = fs::read_dir(root.join("artifacts")) else {
            produced(root, "produced", path, versions);
            return;
        };
        let mut dirs: Vec<_> = dirs
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with("lean-"))
            .collect();
        dirs.sort_by_key(|e| e.file_name());
        for dir in dirs.into_iter().take(64) {
            let name = dir.file_name().to_string_lossy().into_owned();
            let selection = format!("artifacts/{name}/selection.json");
            let Some(records) = json(root, &selection) else {
                continue;
            };
            for record in records.as_array().into_iter().flatten().rev() {
                let Some(candidate) = record["candidate"].as_str() else {
                    continue;
                };
                let candidate = candidate
                    .split_once("/episode/")
                    .map_or(candidate, |(_, tail)| tail);
                if !candidate.starts_with(&format!("artifacts/{name}/session-")) {
                    continue;
                }
                if let Some(hash) = record["workspace_files"]
                    .get(relative.to_string_lossy().as_ref())
                    .and_then(Value::as_str)
                {
                    add_file(
                        versions,
                        root,
                        &Path::new(candidate).join(relative),
                        Some(hash),
                        format!(
                            "Retained candidate {name}, session {}; capture time unknown, may be later than this step",
                            record["after_session"]
                        ),
                    );
                }
            }
        }
    }
    let absolute = if requested.is_absolute() || workdir.is_empty() {
        path.to_owned()
    } else {
        Path::new(workdir)
            .join(requested)
            .to_string_lossy()
            .into_owned()
    };
    produced(root, "produced", &absolute, versions);
}

fn produced(root: &Path, folder: &str, path: &str, versions: &mut Vec<Version>) {
    let Some(manifest) = json(root, &format!("{folder}/manifest.json")) else {
        return;
    };
    for record in manifest.as_array().into_iter().flatten().take(256) {
        if record["status"] != "ok" {
            continue;
        }
        let (Some(source), Some(dest)) =
            (record["source"].as_str(), record["destination"].as_str())
        else {
            continue;
        };
        // The retention collector copies Harbor's artifacts into produced/.
        let dest = if folder == "produced" {
            Path::new(folder).join(dest.strip_prefix("artifacts/").unwrap_or(dest))
        } else {
            PathBuf::from(dest)
        };
        let Ok(tail) = Path::new(path).strip_prefix(source) else {
            continue;
        };
        let relative = if record["type"] == "file" && tail.as_os_str().is_empty() {
            dest
        } else if record["type"] == "directory" {
            dest.join(tail)
        } else {
            continue;
        };
        let digest = json(root, "retention.json").and_then(|m| {
            m["files"]
                .as_array()?
                .iter()
                .find(|f| f["path"].as_str() == relative.to_str())
                .and_then(|f| f["sha256"].as_str().map(str::to_owned))
        });
        add_file(
            versions,
            root,
            &relative,
            digest.as_deref(),
            "Final exported artifact; not the file at the selected step".to_owned(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn block(kind: Kind) -> Block {
        Block { at: Some(1), kind }
    }
    fn run(root: &Path) -> Run {
        let (_dir, sources) = crate::runs::fixture_sources();
        let mut run = crate::runs::Catalog::load(sources).runs.remove(0);
        run.files.episode = Some(root.to_owned());
        run
    }
    fn put(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn text(view: &Viewer, width: u16, height: u16) -> String {
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf, Ladder::default());
        buf.content.iter().map(|c| c.symbol()).collect()
    }

    #[test]
    fn file_viewer_reads_only_observations_up_to_the_clicked_step() {
        let blocks = vec![
            block(Kind::Look {
                what: "Read facts.md".to_owned(),
                output: "early fact".to_owned(),
            }),
            block(Kind::Command {
                command: "cat facts.md".to_owned(),
                output: "current fact".to_owned(),
                exit: Some(0),
                failed: false,
            }),
            block(Kind::Edit {
                path: "facts.md".to_owned(),
                action: "Wrote",
                added: 1,
                removed: 0,
                body: "FUTURE SECRET".to_owned(),
            }),
        ];
        let mut view = Viewer::open(None, &blocks, 1, "facts.md");
        assert_eq!(view.versions.len(), 2);
        assert_eq!(view.versions[0].text, "current fact");
        assert!(!text(&view, 90, 20).contains("FUTURE SECRET"));
        view.key(Key::Tab);
        assert!(text(&view, 90, 20).contains("early fact"));
        assert!(view.key(Key::Back));
    }

    #[test]
    fn file_viewer_labels_partial_reads_patches_and_unsuccessful_writes() {
        let blocks = vec![block(Kind::Edit {
            path: "tests/T5.sh".to_owned(),
            action: "Failed to edit",
            added: 1,
            removed: 1,
            body: "@@\n-old\n+new\npermission denied".to_owned(),
        })];
        let view = Viewer::open(None, &blocks, 0, "tests/T5.sh");
        assert!(view.versions[0].label.contains("not a complete file"));
        assert!(view.versions[0].text.contains("permission denied"));
        assert!(read_command_path("cat facts.md; rm -rf x").is_none());
        assert!(read_command_path("cat facts.md other.md").is_none());
        assert_eq!(
            read_command_path("sed -n '12,40p' facts.md"),
            Some("facts.md")
        );
        assert_eq!(read_command_path("head -n 30 facts.md"), Some("facts.md"));
        assert!(read_command_path("sed -n 's/x/y/' facts.md").is_none());
        assert!(read_command_path("cat facts.md\ncat other.md").is_none());
    }

    #[test]
    fn file_viewer_requires_explicit_selection_of_snapshots_and_checks_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        put(root, "manifest.json", r#"{"workdir":"/app"}"#);
        put(
            root,
            "artifacts/lean-1/session-1/src/main.rs",
            "retained source",
        );
        let hash = format!("{:x}", Sha256::digest(b"retained source"));
        put(
            root,
            "artifacts/lean-1/selection.json",
            &json!([{
                "candidate":"/opt/openagents/episode/artifacts/lean-1/session-1",
                "after_session":1,"workspace_files":{"src/main.rs":hash}
            }])
            .to_string(),
        );
        let mut view = Viewer::open(Some(&run(root)), &[], 0, "/app/src/main.rs");
        assert!(!text(&view, 100, 20).contains("retained source"));
        view.key(Key::Tab);
        let screen = text(&view, 100, 20);
        assert!(screen.contains("retained source"));
        assert!(screen.contains("capture time unknown"));
        put(
            root,
            "artifacts/lean-1/session-1/src/main.rs",
            "modified source",
        );
        let view = Viewer::open(Some(&run(root)), &[], 0, "/app/src/main.rs");
        assert!(view.versions[1].text.contains("failed its SHA-256"));
        assert!(!view.versions[1].text.contains("modified source"));
    }

    #[test]
    fn file_viewer_rebases_final_exports_and_finds_retained_facts() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        put(root, "manifest.json", r#"{"workdir":"/app"}"#);
        put(root, "artifacts/accept-1/facts.md", "a retained fact");
        let hash = format!("{:x}", Sha256::digest(b"a retained fact"));
        put(
            root,
            "retention.json",
            &json!({"files":[{"path":"artifacts/accept-1/facts.md","sha256":hash}]}).to_string(),
        );
        put(root, "produced/app/src/main.rs", "final file");
        put(
            root,
            "produced/manifest.json",
            r#"[{"source":"/app/src","destination":"artifacts/app/src","type":"directory","status":"ok"}]"#,
        );
        let view = Viewer::open(Some(&run(root)), &[], 0, "facts.md");
        assert_eq!(view.versions[1].text, "a retained fact");
        let view = Viewer::open(Some(&run(root)), &[], 0, "src/main.rs");
        assert_eq!(view.versions[1].text, "final file");
        assert!(
            view.versions[1]
                .label
                .contains("not the file at the selected step")
        );
    }

    #[test]
    fn file_viewer_refuses_missing_binary_large_and_escaping_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert!(read(root, Path::new("../outside"), FILE_LIMIT).is_err());
        assert!(read(root, Path::new("/etc/passwd"), FILE_LIMIT).is_err());
        put(root, "large", "large");
        assert!(read(root, Path::new("large"), 2).is_err());
        fs::write(root.join("binary"), [0, 1, 2]).unwrap();
        let mut versions = Vec::new();
        add_file(
            &mut versions,
            root,
            Path::new("binary"),
            None,
            "test".to_owned(),
        );
        assert!(versions[0].text.contains("Binary file"));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/passwd", root.join("link")).unwrap();
            assert!(
                read(root, Path::new("link"), FILE_LIMIT)
                    .unwrap_err()
                    .contains("symlink")
            );
            std::os::unix::fs::symlink("/etc", root.join("escape")).unwrap();
            assert!(read(root, Path::new("escape/passwd"), FILE_LIMIT).is_err());
        }
        let view = Viewer::open(None, &[], 0, "/etc/passwd");
        assert!(
            view.versions[0]
                .text
                .contains("does not read your current workspace")
        );
    }

    #[test]
    fn file_viewer_wraps_scrolls_resizes_and_escapes_control_bytes() {
        let body = format!("\x1b[31m{}\nLAST-LINE", "αβγ source text ".repeat(500));
        let blocks = vec![block(Kind::Look {
            what: "Read long.txt".to_owned(),
            output: body,
        })];
        let mut view = Viewer::open(None, &blocks, 0, "long.txt");
        assert!(text(&view, 65, 12).contains("\\u{1b}"));
        view.key(Key::PageDown);
        assert!(view.scroll.get() > 0);
        view.key(Key::End);
        assert!(text(&view, 65, 12).contains("LAST-LINE"));
        assert!(text(&view, 95, 18).contains("LAST-LINE"));
        view.key(Key::Home);
        assert_eq!(view.scroll.get(), 0);
    }

    #[test]
    fn file_viewer_path_clicks_follow_wide_characters_and_line_suffixes() {
        let area = Rect::new(3, 2, 80, 6);
        let mut buf = Buffer::empty(area);
        buf.set_string(
            4,
            3,
            "中文 cat tests/T5.sh:12 and facts.md",
            ratatui::style::Style::default(),
        );
        let hits = decorate(area, &[(3, 7)], &[], &mut buf);
        let hit = hits.iter().find(|h| h.path == "tests/T5.sh").unwrap();
        assert_eq!(hit.cells.x, 13);
        assert_eq!(hit.step, 7);
        assert_eq!(clicked(&hits, 15, 3).unwrap().path, "tests/T5.sh");
        assert!(clicked(&hits, 4, 3).is_none());
        assert!(buf[(15, 3)].modifier.contains(Modifier::UNDERLINED));
    }
    #[test]
    fn file_viewer_clicks_resolve_wrapped_paths_spaces_and_extensionless_names() {
        let long = "/app/long-directory/another-directory/tests/T5.sh";
        let blocks = vec![block(Kind::Look {
            what: format!("Read {long}"),
            output: "source".to_owned(),
        })];
        let area = Rect::new(0, 0, 45, 4);
        let mut buf = Buffer::empty(area);
        buf.set_string(
            0,
            0,
            "Read /app/long-directory/another-",
            ratatui::style::Style::default(),
        );
        buf.set_string(
            0,
            1,
            "  directory/tests/T5.sh",
            ratatui::style::Style::default(),
        );
        let hits = decorate(area, &[(0, 0), (1, 0)], &blocks, &mut buf);
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|h| h.path == long));
        for path in ["Makefile", "my file.rs"] {
            let blocks = vec![block(Kind::Look {
                what: format!("Read {path}"),
                output: "source".to_owned(),
            })];
            let mut buf = Buffer::empty(area);
            buf.set_string(
                0,
                0,
                format!("Read {path}"),
                ratatui::style::Style::default(),
            );
            let hits = decorate(area, &[(0, 0)], &blocks, &mut buf);
            assert_eq!(hits.len(), 1);
            assert_eq!(hits[0].path, path);
        }
    }

    #[test]
    fn file_viewer_matches_absolute_reads_using_only_the_recorded_workdir() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "manifest.json", r#"{"workdir":"/app"}"#);
        let blocks = vec![block(Kind::Look {
            what: "Read /app/src/main.rs".to_owned(),
            output: "recorded source".to_owned(),
        })];
        let view = Viewer::open(Some(&run(dir.path())), &blocks, 0, "src/main.rs");
        assert_eq!(view.versions[0].text, "recorded source");
        let view = Viewer::open(None, &blocks, 0, "src/main.rs");
        assert!(!view.versions[0].observed);
    }

    #[test]
    #[ignore = "reads the retained TB4 bundle and optionally writes an acceptance record"]
    fn file_viewer_retained_acceptance() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/traces/tb4--coder-one-microluna-v13-retained--session-window-debug-3/session-window-debug__3KVqBUz.episode");
        let mut view = Viewer::open(Some(&run(&root)), &[], 0, "/app/app/gc.py");
        assert_eq!(
            view.versions.len(),
            4,
            "a notice, two candidates, and the final artifact"
        );
        assert!(!text(&view, 100, 30).contains("def "));
        let notice = text(&view, 100, 30);
        view.key(Key::Tab);
        assert!(view.versions[view.index].text.contains("def "));
        assert!(!view.versions[view.index].text.contains("Cannot display"));
        let snapshot = text(&view, 100, 30);
        let report = json!({
            "task":"session-window-debug", "trial":"session-window-debug__3KVqBUz",
            "path":"/app/app/gc.py", "versions":view.versions.len(),
            "snapshot_default_hidden":true,
            "sources":view.versions.iter().skip(1).map(|v| json!({
                "label":v.label, "bytes":v.text.len(), "sha256":format!("{:x}", Sha256::digest(v.text.as_bytes())),
                "read_ok":!v.text.starts_with("Cannot display")
            })).collect::<Vec<_>>()
        });
        assert!(
            report["sources"]
                .as_array()
                .unwrap()
                .iter()
                .all(|s| s["read_ok"] == true)
        );
        if let Some(output) = std::env::var_os("GYM_FILE_VIEW_AUDIT_DIR") {
            let output = PathBuf::from(output);
            fs::create_dir_all(&output).unwrap();
            fs::write(
                output.join("file-viewer.json"),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
            fs::write(output.join("missing-at-step.txt"), notice).unwrap();
            fs::write(output.join("retained-candidate.txt"), snapshot).unwrap();
        }
        println!("{report}");
    }
}
