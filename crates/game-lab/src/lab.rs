//! The Lab on this computer: game documents and their versions, notes,
//! the game's build, and the `game_*` tools the chat calls
//! (docs/game-lab/README.md).
//!
//! ```text
//! ROOT/the-ice-cream-man/vN/doc.json       the version's game.v1 document
//! ROOT/the-ice-cream-man/vN/version.json   its summary, author, digest, bot stats
//! ROOT/the-ice-cream-man/notes.json        notes at a time and place
//! ROOT/builds/<code digest>/               the game's build for that code
//! ```
//!
//! ROOT is `$GAME_LAB_DIR`, else `~/work/film-out/lab/games`: private,
//! beside Video Lab's films, never in git.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::doc::{self, ICE_CREAM_MAN, KIND};
use crate::patch;
use crate::sim::{self, Stats};

/// The card kind a tool result carries.
pub const CARD: &str = "game_lab";

/// The build's files (`scripts/build-icecream-web.sh`).
pub const BUILD_FILES: [&str; 3] = ["icecream_web.js", "icecream_web_bg.wasm", "start.js"];

/// Where the game's build stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Build {
    Ready,
    Building,
    Failed(String),
}

pub struct Lab {
    pub root: PathBuf,
    /// The checkout the game builds from.
    pub repo: PathBuf,
    /// Where this Lab answers, for the play links it gives.
    pub base: String,
    build: Mutex<Build>,
    writes: Mutex<()>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// A document's digest: SHA-256 of its JSON, the first 16 hex digits.
#[must_use]
pub fn digest(doc: &Value) -> String {
    let hash = Sha256::digest(doc.to_string().as_bytes());
    hash.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, text + "\n").map_err(|e| e.to_string())?;
    std::fs::rename(&temporary, path).map_err(|e| e.to_string())
}

fn arg_u64(args: &Value, key: &str) -> Option<u64> {
    match &args[key] {
        Value::Number(n) => n.as_u64().or_else(|| n.as_f64().map(|f| f as u64)),
        Value::String(s) => s.trim().trim_start_matches('v').parse().ok(),
        _ => None,
    }
}

fn arg_f64(args: &Value, key: &str) -> Option<f64> {
    match &args[key] {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

impl Lab {
    /// The Lab at `root`, building the game from `repo`; imports The Ice
    /// Cream Man as version 1 the first time.
    ///
    /// # Errors
    /// The folder can't be written.
    pub fn open(root: PathBuf, repo: PathBuf) -> Result<Self, String> {
        let lab = Self {
            root,
            repo,
            base: "http://127.0.0.1:8793".into(),
            build: Mutex::new(Build::Building),
            writes: Mutex::new(()),
        };
        if lab.latest(ICE_CREAM_MAN).is_none() {
            let original = doc::original();
            lab.save(
                ICE_CREAM_MAN,
                &original,
                "The Ice Cream Man as the game plays today",
                "imported",
                None,
            )?;
        }
        let state = if lab.build_dir().is_some_and(|d| ready(&d)) {
            Build::Ready
        } else {
            Build::Failed("not built yet".into())
        };
        *lab.build.lock().map_err(|e| e.to_string())? = state;
        Ok(lab)
    }

    /// The default folder.
    #[must_use]
    pub fn default_root() -> PathBuf {
        if let Some(dir) = std::env::var_os("GAME_LAB_DIR") {
            return PathBuf::from(dir);
        }
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("work/film-out/lab/games")
    }

    fn doc_dir(&self, game: &str) -> PathBuf {
        self.root.join(game)
    }

    /// The version numbers saved, in order.
    #[must_use]
    pub fn versions(&self, game: &str) -> Vec<u64> {
        let mut out: Vec<u64> = std::fs::read_dir(self.doc_dir(game))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_str()?.strip_prefix('v')?.parse().ok())
            .filter(|n| self.doc_dir(game).join(format!("v{n}/doc.json")).is_file())
            .collect();
        out.sort_unstable();
        out
    }

    #[must_use]
    pub fn latest(&self, game: &str) -> Option<u64> {
        self.versions(game).last().copied()
    }

    /// A version's document and its record.
    #[must_use]
    pub fn load(&self, game: &str, n: u64) -> Option<(Value, Value)> {
        let dir = self.doc_dir(game).join(format!("v{n}"));
        let mut document = read_json(&dir.join("doc.json"))?;
        // Saved before the game gained some settings: they're off.
        doc::upgrade(&mut document);
        Some((document, read_json(&dir.join("version.json"))?))
    }

    fn save(
        &self,
        game: &str,
        document: &Value,
        summary: &str,
        author: &str,
        base: Option<u64>,
    ) -> Result<(u64, Value), String> {
        let _guard = self.writes.lock().map_err(|e| e.to_string())?;
        let checked = doc::read(document).map_err(|p| p.join(" "))?;
        let stats = sim::measure(checked.tuning, checked.seed);
        let n = self.latest(game).unwrap_or(0) + 1;
        let record = json!({
            "n": n,
            "summary": summary,
            "author": author,
            "digest": digest(document),
            "created": now(),
            "base": base,
            "stats": stats.json(),
        });
        let dir = self.doc_dir(game).join(format!("v{n}"));
        write_json(&dir.join("doc.json"), document)?;
        write_json(&dir.join("version.json"), &record)?;
        Ok((n, record))
    }

    // ---- the build ----------------------------------------------------------

    /// The code the game is built from: its digest.
    fn code_digest(&self) -> Option<String> {
        let mut files = Vec::new();
        for dir in ["crates/icecream-rules/src", "crates/icecream-web/src"] {
            let entries = std::fs::read_dir(self.repo.join(dir)).ok()?;
            for entry in entries.flatten() {
                files.push(entry.path());
            }
        }
        for file in [
            "crates/icecream-web/start.js",
            "crates/icecream-web/Cargo.toml",
            "crates/icecream-rules/Cargo.toml",
            "scripts/build-icecream-web.sh",
        ] {
            files.push(self.repo.join(file));
        }
        files.sort();
        let mut hash = Sha256::new();
        for file in files {
            hash.update(
                file.strip_prefix(&self.repo)
                    .ok()?
                    .to_string_lossy()
                    .as_bytes(),
            );
            hash.update(std::fs::read(&file).ok()?);
        }
        Some(
            hash.finalize()
                .iter()
                .take(8)
                .map(|b| format!("{b:02x}"))
                .collect(),
        )
    }

    /// The build folder for the code as it is now.
    #[must_use]
    pub fn build_dir(&self) -> Option<PathBuf> {
        Some(self.root.join("builds").join(self.code_digest()?))
    }

    #[must_use]
    pub fn build_state(&self) -> Build {
        self.build
            .lock()
            .map_or(Build::Failed("the Lab is stuck".into()), |b| b.clone())
    }

    /// Builds the game for the code as it is, unless that build exists or
    /// is under way. Versions are data on top of one build; only a change
    /// to the game's code builds again.
    pub fn ensure_build(self: &std::sync::Arc<Self>) {
        let Some(dir) = self.build_dir() else {
            if let Ok(mut b) = self.build.lock() {
                *b = Build::Failed("the game's code isn't in this checkout".into());
            }
            return;
        };
        {
            let Ok(mut state) = self.build.lock() else {
                return;
            };
            if ready(&dir) {
                *state = Build::Ready;
                return;
            }
            if *state == Build::Building {
                return;
            }
            *state = Build::Building;
        }
        let lab = std::sync::Arc::clone(self);
        std::thread::spawn(move || {
            let result = lab.build_into(&dir);
            if let Ok(mut state) = lab.build.lock() {
                *state = match result {
                    Ok(()) => Build::Ready,
                    Err(why) => Build::Failed(why),
                };
            }
        });
    }

    fn build_into(&self, dir: &Path) -> Result<(), String> {
        let staging = dir.with_extension("building");
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(dir.parent().unwrap_or(dir)).map_err(|e| e.to_string())?;
        let log_path = dir.with_extension("log");
        let log = std::fs::File::create(&log_path).map_err(|e| e.to_string())?;
        let err = log.try_clone().map_err(|e| e.to_string())?;
        eprintln!("game-lab: building the game into {}", dir.display());
        let status = std::process::Command::new(self.repo.join("scripts/build-icecream-web.sh"))
            .arg(&staging)
            .current_dir(&self.repo)
            .stdout(log)
            .stderr(err)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() || !ready(&staging) {
            return Err(format!("the build failed; see {}", log_path.display()));
        }
        std::fs::rename(&staging, dir).map_err(|e| e.to_string())?;
        eprintln!("game-lab: the game is built");
        Ok(())
    }

    // ---- cards --------------------------------------------------------------

    fn side(&self, game: &str, n: u64) -> Result<Value, String> {
        let (document, record) = self
            .load(game, n)
            .ok_or_else(|| format!("There's no version {n}."))?;
        let checked = doc::read(&document).map_err(|p| p.join(" "))?;
        Ok(json!({
            "version": n,
            "summary": record["summary"],
            "title": document["copy"]["title"],
            "fragment": doc::fragment(&checked),
            "stats": record["stats"],
        }))
    }

    /// The card for a version, or for two side by side.
    ///
    /// # Errors
    /// No such version.
    pub fn card(&self, game: &str, n: u64, against: Option<u64>) -> Result<Value, String> {
        let side = self.side(game, n)?;
        let versions: Vec<Value> = self
            .versions(game)
            .into_iter()
            .filter_map(|v| {
                let (_, record) = self.load(game, v)?;
                Some(json!({"n": v, "summary": record["summary"]}))
            })
            .collect();
        let build = match self.build_state() {
            Build::Ready => json!("ready"),
            Build::Building => json!("building"),
            Build::Failed(why) => json!({"failed": why}),
        };
        let mut card = json!({
            "card": CARD,
            "doc": game,
            "title": side["title"],
            "version": n,
            "summary": side["summary"],
            "fragment": side["fragment"],
            "stats": side["stats"],
            "versions": versions,
            "notes": self.notes(game).len(),
            "build": build,
            "player": format!("{}/play?doc={game}&v={n}", self.base),
        });
        if let Some(a) = against {
            let left = self.side(game, a)?;
            let (da, _) = self.load(game, a).ok_or("no version")?;
            let (db, _) = self.load(game, n).ok_or("no version")?;
            let changes: Vec<Value> = doc::changes(&da, &db)
                .into_iter()
                .map(|(path, x, y)| json!([path, x, y]))
                .collect();
            card["compare"] = json!({
                "a": left,
                "b": side,
                "changes": changes,
                "differences": differences(&left["stats"], &card["stats"]),
            });
        }
        Ok(card)
    }

    // ---- notes --------------------------------------------------------------

    /// A page that plays a version on its own, outside the chat (the
    /// terminal's play link).
    ///
    /// # Errors
    /// No such version.
    pub fn play_page(&self, game: &str, n: u64) -> Result<String, String> {
        let side = self.side(game, n)?;
        let fragment: String = side["fragment"]
            .as_str()
            .unwrap_or_default()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || "=&._-~%".contains(*c))
            .collect();
        let title: String = side["title"]
            .as_str()
            .unwrap_or_default()
            .chars()
            .filter(|c| !"<>&\"".contains(*c))
            .collect();
        Ok(format!(
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>{title}, version {n}</title>\
<style>html,body{{margin:0;height:100%;background:#000;overflow:hidden}}\
#icecream{{position:fixed;inset:0}}canvas{{width:100%;height:100%;display:block}}\
#icecream-status{{position:fixed;left:0;right:0;bottom:16px;text-align:center;color:#fff;font:16px system-ui}}</style>\
</head><body><div id=\"icecream\"><canvas id=\"icecream-canvas\" tabindex=\"0\"></canvas>\
<p id=\"icecream-status\">Loading…</p></div>\
<script>if (!location.hash) location.hash = \"{fragment}\"; import(\"/build/start.js\");</script>\
</body></html>"
        ))
    }

    #[must_use]
    pub fn notes(&self, game: &str) -> Vec<Value> {
        read_json(&self.doc_dir(game).join("notes.json"))
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
    }

    /// Saves a note on a version, at a time and place in a run.
    ///
    /// # Errors
    /// No such version, an empty note, or the file can't be written.
    pub fn add_note(&self, game: &str, args: &Value) -> Result<Value, String> {
        let text = args["text"].as_str().unwrap_or_default().trim().to_owned();
        if text.is_empty() {
            return Err("The note is empty.".into());
        }
        let version = arg_u64(args, "version")
            .or_else(|| arg_u64(args, "v"))
            .or_else(|| self.latest(game))
            .ok_or("There's no version to note.")?;
        if self.load(game, version).is_none() {
            return Err(format!("There's no version {version}."));
        }
        let _guard = self.writes.lock().map_err(|e| e.to_string())?;
        let mut notes = self.notes(game);
        let note = json!({
            "version": version,
            "seconds": arg_f64(args, "seconds").map(|s| (s * 10.0).round() / 10.0),
            "room": args["room"].as_str().filter(|r| !r.is_empty()),
            "x": arg_f64(args, "x"),
            "z": arg_f64(args, "z"),
            "text": text,
            "at": now(),
        });
        notes.push(note.clone());
        write_json(&self.doc_dir(game).join("notes.json"), &Value::Array(notes))?;
        Ok(note)
    }

    // ---- the tools ----------------------------------------------------------

    /// Runs a tool: its JSON result, or `{"error": ...}`.
    #[must_use]
    pub fn run(&self, name: &str, args: &Value) -> Value {
        if let Some(problem) = unknown_arguments(name, args) {
            return json!({"error": problem});
        }
        // A tool works on the game it names, never on one picked for it: a
        // call meant for something else (a film) gets no game's numbers
        // (#11619).
        let game = args["game"]
            .as_str()
            .map(str::trim)
            .unwrap_or_default()
            .to_owned();
        if name != "game_list" && game.is_empty() {
            return json!({"error": format!("{name} needs the game it's about (game); the games are: {ICE_CREAM_MAN}. Nothing was done.")});
        }
        if name != "game_list" && self.latest(&game).is_none() {
            return json!({"error": format!("There's no game called {game} in Game Lab; the games are: {ICE_CREAM_MAN}. Nothing was done.")});
        }
        let result = match name {
            "game_list" => Ok(self.list()),
            "game_open" => self.open_tool(&game, args, false),
            "game_play" => self.open_tool(&game, args, true),
            "game_edit" => self.edit(&game, args),
            "game_versions" => Ok(self.versions_tool(&game)),
            "game_compare" => self.compare(&game, args),
            "game_notes" => self.notes_tool(&game, args),
            "game_propose_code" => self.propose_code(&game, args),
            "game_code_requests" => Ok(self.code_requests_tool(&game)),
            _ => Err(format!("{name} isn't a Game Lab tool.")),
        };
        result.unwrap_or_else(|why| json!({"error": why}))
    }

    fn list(&self) -> Value {
        let documents: Vec<Value> = [ICE_CREAM_MAN]
            .iter()
            .filter_map(|g| {
                let n = self.latest(g)?;
                let (document, _) = self.load(g, n)?;
                Some(json!({"id": g, "title": document["copy"]["title"], "kind": KIND, "versions": self.versions(g).len()}))
            })
            .collect();
        json!({"documents": documents})
    }

    fn version_arg(&self, game: &str, args: &Value) -> Result<u64, String> {
        match arg_u64(args, "version") {
            Some(n) if self.load(game, n).is_some() => Ok(n),
            Some(n) => Err(format!(
                "There's no version {n}; the versions are 1 to {}.",
                self.latest(game).unwrap_or(1)
            )),
            None => self
                .latest(game)
                .ok_or_else(|| "There are no versions.".into()),
        }
    }

    fn open_tool(&self, game: &str, args: &Value, play: bool) -> Result<Value, String> {
        let n = self.version_arg(game, args)?;
        let (document, record) = self.load(game, n).ok_or("no version")?;
        let mut card = self.card(game, n, None)?;
        card["autoplay"] = json!(play);
        let stats = stats_of(&record["stats"]);
        let text = if play {
            format!(
                "Version {n} ({}) is ready to play in the card: press Play there.",
                record["summary"].as_str().unwrap_or_default()
            )
        } else {
            format!(
                "Version {n} of {}: {}. {}",
                self.versions(game).len(),
                record["summary"].as_str().unwrap_or_default(),
                stats.map(|s| s.line()).unwrap_or_default()
            )
        };
        let mut result = json!({
            "game": game,
            "version": n,
            "versions": self.versions(game).len(),
            "summary": record["summary"],
            "stats": record["stats"],
            "text": text,
            "card": card,
        });
        if !play {
            result["document"] = document;
        }
        Ok(result)
    }

    fn edit(&self, game: &str, args: &Value) -> Result<Value, String> {
        let base = self.version_arg(game, args)?;
        let summary = args["summary"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or("Say in one line what the edit changes (summary).")?;
        let ops = patch::parse(&args["patch"])?;
        let (before, record) = self.load(game, base).ok_or("no version")?;
        let refuse = |reason: String| {
            json!({"refused": true, "saved": false, "reason": reason, "version": base,
                   "text": "Nothing was saved."})
        };
        let after = match patch::apply(&before, &ops) {
            Ok(after) => after,
            Err(why) => return Ok(refuse(why)),
        };
        let changes = doc::changes(&before, &after);
        if changes.is_empty() && after == before {
            return Ok(refuse("That edit changes nothing.".into()));
        }
        let checked = match doc::read(&after) {
            Ok(checked) => checked,
            Err(problems) => return Ok(refuse(problems.join(" "))),
        };
        let broken = doc::rule_problems(&checked);
        if !broken.is_empty() {
            return Ok(refuse(broken.join(" ")));
        }
        let measured = sim::measure(checked.tuning, checked.seed);
        if measured.wins() == 0 {
            let was = stats_of(&record["stats"]).map_or(String::new(), |s| {
                format!(" Before the edit it won {} of {}.", s.wins(), s.runs)
            });
            return Ok(refuse(format!(
                "That would make the game unwinnable: in {} seeded runs of up to {} minutes the bot never escaped or melted him.{was}",
                measured.runs,
                sim::LIMIT / 60
            )));
        }
        let author = args["author"].as_str().unwrap_or("the chat");
        let (n, saved) = self.save(game, &after, summary, author, Some(base))?;
        let lines: Vec<String> = changes
            .iter()
            .map(|(path, x, y)| format!("{path}: {} → {}", short(x), short(y)))
            .collect();
        Ok(json!({
            "saved": true,
            "version": n,
            "summary": summary,
            "changes": lines,
            "before": record["stats"],
            "after": saved["stats"],
            "differences": differences(&record["stats"], &saved["stats"]),
            "text": format!("Saved version {n}. {}", measured.line()),
            "card": self.card(game, n, None)?,
        }))
    }

    fn versions_tool(&self, game: &str) -> Value {
        let list: Vec<Value> = self
            .versions(game)
            .into_iter()
            .filter_map(|n| {
                let (_, r) = self.load(game, n)?;
                Some(
                    json!({"version": n, "summary": r["summary"], "author": r["author"],
                            "digest": r["digest"], "win_rate": r["stats"]["win_rate"]}),
                )
            })
            .collect();
        let latest = self.latest(game).unwrap_or(1);
        json!({"versions": list, "card": self.card(game, latest, None).ok()})
    }

    fn compare(&self, game: &str, args: &Value) -> Result<Value, String> {
        let latest = self.latest(game).ok_or("no versions")?;
        let b = arg_u64(args, "b").unwrap_or(latest);
        let a = arg_u64(args, "a").unwrap_or(if b > 1 { b - 1 } else { 1 });
        for n in [a, b] {
            if self.load(game, n).is_none() {
                return Err(format!(
                    "{game} has no version {n}; its versions are 1 to {latest}. Nothing was compared."
                ));
            }
        }
        if a == b {
            return Err(format!(
                "That's version {a} with itself; {game} has versions 1 to {latest}. Nothing was compared."
            ));
        }
        let card = self.card(game, b, Some(a))?;
        let compare = &card["compare"];
        let changes: Vec<String> = compare["changes"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| {
                format!(
                    "{}: {} → {}",
                    c[0].as_str().unwrap_or_default(),
                    short(&c[1]),
                    short(&c[2])
                )
            })
            .collect();
        Ok(json!({
            "a": {"version": a, "summary": compare["a"]["summary"], "stats": compare["a"]["stats"]},
            "b": {"version": b, "summary": compare["b"]["summary"], "stats": compare["b"]["stats"]},
            "changes": changes,
            "differences": compare["differences"],
            "card": card,
        }))
    }

    fn notes_tool(&self, game: &str, args: &Value) -> Result<Value, String> {
        if args["text"].as_str().is_some_and(|t| !t.trim().is_empty()) {
            let note = self.add_note(game, args)?;
            return Ok(json!({"saved": true, "note": note}));
        }
        let version = arg_u64(args, "version");
        let notes: Vec<Value> = self
            .notes(game)
            .into_iter()
            .filter(|n| version.is_none_or(|v| n["version"].as_u64() == Some(v)))
            .collect();
        let latest = self.latest(game).unwrap_or(1);
        Ok(json!({"notes": notes, "card": self.card(game, version.unwrap_or(latest), None).ok()}))
    }

    // ---- code requests ----------------------------------------------------

    /// The code requests recorded for a game, oldest first.
    #[must_use]
    pub fn code_requests(&self, game: &str) -> Vec<Value> {
        read_json(&self.doc_dir(game).join("code-requests.json"))
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
    }

    /// Records a change that needs the game's code changed, for Coder.
    /// Nothing is built or changed here: the request waits until a commit
    /// that names it (a `Game-Lab-Request: GAME/rN` line) is in the
    /// checkout the Lab builds from, and then it links that commit.
    ///
    /// # Errors
    /// An empty request, or the file can't be written.
    pub fn add_code_request(&self, game: &str, args: &Value) -> Result<Value, String> {
        let text = args["request"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_owned();
        if text.is_empty() {
            return Err("Say what the code change should do (request).".into());
        }
        if text.chars().count() > 4000 {
            return Err("The request is too long; 4000 characters at most.".into());
        }
        let version = self.version_arg(game, args)?;
        let _guard = self.writes.lock().map_err(|e| e.to_string())?;
        let mut requests = self.code_requests(game);
        let id = format!("r{}", requests.len() + 1);
        let request = json!({
            "id": id,
            "request": text,
            "version": version,
            "author": args["author"].as_str().unwrap_or("the chat"),
            "at": now(),
        });
        requests.push(request.clone());
        write_json(
            &self.doc_dir(game).join("code-requests.json"),
            &Value::Array(requests),
        )?;
        Ok(request)
    }

    /// A request with where it stands: waiting, or made in a commit.
    fn request_status(&self, game: &str, request: &Value) -> Value {
        let id = request["id"].as_str().unwrap_or_default();
        let mut out = request.clone();
        match commit_for(&self.repo, &format!("{REQUEST_TRAILER}: {game}/{id}")) {
            Some((hash, subject)) => {
                out["status"] = json!("made");
                out["commit"] = json!(hash);
                out["commit_subject"] = json!(subject);
                out["commit_url"] = json!(format!("{COMMITS}/{hash}"));
                out["build"] = json!(match self.build_state() {
                    Build::Ready => "ready",
                    Build::Building => "building",
                    Build::Failed(_) => "not built",
                });
            }
            None => out["status"] = json!("waiting for Coder"),
        }
        out
    }

    fn propose_code(&self, game: &str, args: &Value) -> Result<Value, String> {
        let request = self.add_code_request(game, args)?;
        let id = request["id"].as_str().unwrap_or_default().to_owned();
        let latest = self.latest(game).unwrap_or(1);
        Ok(json!({
            "recorded": true,
            "request": request,
            "text": format!(
                "Recorded code request {id} for Coder. Nothing in the game has changed yet: Coder makes code changes outside this chat, as a commit with tests. When a commit that makes request {id} is in this Mac's checkout, game_code_requests links it, and the new settings it adds can be changed here as versions."
            ),
            "card": self.card(game, latest, None).ok(),
        }))
    }

    fn code_requests_tool(&self, game: &str) -> Value {
        let list: Vec<Value> = self
            .code_requests(game)
            .iter()
            .map(|r| self.request_status(game, r))
            .collect();
        let latest = self.latest(game).unwrap_or(1);
        json!({"requests": list, "card": self.card(game, latest, None).ok()})
    }
}

/// The line a commit carries to say which code request it makes.
pub const REQUEST_TRAILER: &str = "Game-Lab-Request";
/// Where a commit can be read.
const COMMITS: &str = "https://github.com/OpenAgentsInc/openagents/commit";

/// The newest commit in `repo`'s history whose message has `line`: its
/// hash and subject.
fn commit_for(repo: &Path, line: &str) -> Option<(String, String)> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["log", "-1", "--fixed-strings", "--format=%H%x09%s"])
        .arg(format!("--grep={line}"))
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    let (hash, subject) = text.trim().split_once('\t')?;
    Some((hash.to_owned(), subject.to_owned()))
}

fn ready(dir: &Path) -> bool {
    BUILD_FILES.iter().all(|f| dir.join(f).is_file())
}

fn short(value: &Value) -> String {
    match value {
        Value::String(s) => format!("\"{s}\""),
        Value::Null => "nothing".into(),
        other => other.to_string(),
    }
}

fn stats_of(value: &Value) -> Option<Stats> {
    Some(Stats {
        runs: value["runs"].as_u64()? as u32,
        escaped: value["escaped"].as_u64()? as u32,
        melted: value["melted"].as_u64().unwrap_or(0) as u32,
        median_escape: value["median_escape"].as_f64().map(|f| f as f32),
        hits: value["hits"].as_f64()? as f32,
        spotted: value["spotted"].as_f64()? as f32,
        hiding_spots: value["hiding_spots"].as_u64()? as usize,
    })
}

/// The measured differences between two versions' bot runs, as lines.
#[must_use]
pub fn differences(a: &Value, b: &Value) -> Vec<String> {
    let (Some(x), Some(y)) = (stats_of(a), stats_of(b)) else {
        return Vec::new();
    };
    let escape = |s: &Stats| s.median_escape.map_or("none".to_owned(), sim::clock);
    vec![
        format!(
            "Bot wins: {} of {} ({}%) → {} of {} ({}%)",
            x.wins(),
            x.runs,
            x.win_rate(),
            y.wins(),
            y.runs,
            y.win_rate()
        ),
        format!(
            "Escaped / melted him: {} / {} → {} / {}",
            x.escaped, x.melted, y.escaped, y.melted
        ),
        format!("Middle escape time: {} → {}", escape(&x), escape(&y)),
        format!("Hits taken a run: {:.1} → {:.1}", x.hits, y.hits),
        format!("Times spotted a run: {:.1} → {:.1}", x.spotted, y.spotted),
        format!("Hiding spots: {} → {}", x.hiding_spots, y.hiding_spots),
    ]
}

/// The arguments each tool takes.
fn takes(name: &str) -> &'static [&'static str] {
    match name {
        "game_list" => &[],
        "game_open" | "game_play" => &["game", "version"],
        "game_edit" => &["game", "patch", "summary", "version", "author"],
        "game_versions" => &["game"],
        "game_compare" => &["game", "a", "b"],
        "game_notes" => &["game", "version", "text", "seconds", "room", "x", "z"],
        "game_propose_code" => &["game", "request", "version", "author"],
        "game_code_requests" => &["game"],
        _ => &[],
    }
}

fn unknown_arguments(name: &str, args: &Value) -> Option<String> {
    let allowed = takes(name);
    let extra: Vec<&String> = args
        .as_object()?
        .keys()
        .filter(|k| !allowed.contains(&k.as_str()))
        .collect();
    (!extra.is_empty()).then(|| {
        format!(
            "{name} doesn't take {}; it takes: {}.",
            extra
                .iter()
                .map(|k| k.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            if allowed.is_empty() {
                "nothing".to_owned()
            } else {
                allowed.join(", ")
            }
        )
    })
}

/// The tools' declarations, for the chat's model.
#[must_use]
pub fn tools() -> Value {
    let game = json!({"type": "string", "description": "the game's id, e.g. the-ice-cream-man"});
    let version =
        json!({"type": "integer", "description": "a version number; the latest when left out"});
    json!({"tools": [
        {"name": "game_list", "description": "List the games in Game Lab on this computer.",
         "parameters": {"type": "object", "properties": {}}},
        {"name": "game_open", "description": "Open a game in Game Lab: its latest version (or the one asked for), its whole game.v1 document (every setting you can change), its bot-run stats, and a card where it can be played.",
         "parameters": {"type": "object", "required": ["game"], "properties": {"game": game, "version": version}}},
        {"name": "game_play", "description": "Show a version in a card where the person can play it right in the chat.",
         "parameters": {"type": "object", "required": ["game"], "properties": {"game": game, "version": version}}},
        {"name": "game_edit", "description": "Change a game by a JSON Patch (RFC 6902) on its game.v1 document, making a new version. Read the document first with game_open. Paths are /SECTION/NAME, e.g. /man/man_chase or /layout/extra_closets or /copy/title. The edit is checked against the game's schema and rules, and the bot plays 24 seeded runs with it: an edit that breaks the rules or that nobody can win is refused with the reason, and nothing is saved.",
         "parameters": {"type": "object", "required": ["game", "patch", "summary"], "properties": {
            "game": game,
            "patch": {"type": "string", "description": "the JSON Patch as a JSON string, e.g. [{\"op\":\"replace\",\"path\":\"/man/man_chase\",\"value\":4.48}]"},
            "summary": {"type": "string", "description": "one line in plain words saying what the edit changes"},
            "version": {"type": "integer", "description": "the version to change; the latest when left out"}}}},
        {"name": "game_versions", "description": "The versions of a game: each one's summary, who made it, and its bot win rate.",
         "parameters": {"type": "object", "required": ["game"], "properties": {"game": game}}},
        {"name": "game_compare", "description": "Two versions side by side: what changed, and the measured differences from 24 seeded bot runs each (escapes, escape time, hits taken, times spotted, hiding spots). Version 1 is the original game. a defaults to the version before b; b to the latest.",
         "parameters": {"type": "object", "required": ["game"], "properties": {"game": game,
            "a": {"type": "integer", "description": "the first version, e.g. 1 for the original"},
            "b": {"type": "integer", "description": "the second version; the latest when left out"}}}},
        {"name": "game_notes", "description": "Read the notes on a game (each at a time and place in a run), or save one when text is given.",
         "parameters": {"type": "object", "required": ["game"], "properties": {"game": game, "version": version,
            "text": {"type": "string", "description": "a note to save"},
            "seconds": {"type": "number", "description": "the time in the run the note is about"},
            "room": {"type": "string", "description": "the room the note is about"}}}},
        {"name": "game_propose_code", "description": "Record a change that needs the game's code changed (game_edit refused it as needing code: new items, enemies, rules, looks or controls), for Coder to make, once the person has agreed to have Coder make it. Write the request in plain words with everything they asked for. This records it; it doesn't change the game. Coder makes it as a commit, and game_code_requests then links that commit.",
         "parameters": {"type": "object", "required": ["game", "request"], "properties": {"game": game,
            "request": {"type": "string", "description": "what the code change should do, in plain words, with every detail the person gave"},
            "version": {"type": "integer", "description": "the version the request is about; the latest when left out"}}}},
        {"name": "game_code_requests", "description": "The code requests recorded for a game and where each stands: waiting for Coder, or made in a commit (with its link). When one is made, its new settings are in the game's document (game_open) and can be changed with game_edit.",
         "parameters": {"type": "object", "required": ["game"], "properties": {"game": game}}}
    ]})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lab() -> (tempdir::Dir, Lab) {
        let dir = tempdir::Dir::new();
        let repo = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
        let lab = Lab::open(dir.path.clone(), repo).unwrap();
        (dir, lab)
    }

    mod tempdir {
        pub struct Dir {
            pub path: std::path::PathBuf,
        }
        impl Dir {
            pub fn new() -> Self {
                let n = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                // Tests run side by side: a counter keeps their folders apart.
                static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                let k = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let path = std::env::temp_dir()
                    .join(format!("game-lab-test-{n}-{}-{k}", std::process::id()));
                std::fs::create_dir_all(&path).unwrap();
                Self { path }
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.path);
            }
        }
    }

    #[test]
    fn version_one_is_the_game_as_it_plays_today() {
        let (_dir, lab) = lab();
        assert_eq!(lab.versions(ICE_CREAM_MAN), vec![1]);
        let (document, record) = lab.load(ICE_CREAM_MAN, 1).unwrap();
        assert_eq!(document, doc::original());
        assert_eq!(record["digest"], digest(&document));
        assert!(record["stats"]["escaped"].as_u64().unwrap() > 0);
        let opened = lab.run("game_open", &json!({"game": ICE_CREAM_MAN}));
        assert_eq!(opened["card"]["card"], CARD);
        assert_eq!(opened["card"]["fragment"], "seed=7");
    }

    #[test]
    fn an_edit_makes_a_version_and_compares_with_the_original() {
        let (_dir, lab) = lab();
        let patch = "[{\"op\":\"replace\",\"path\":\"/man/man_chase\",\"value\":5.6},{\"op\":\"replace\",\"path\":\"/man/man_wander\",\"value\":2.4},{\"op\":\"replace\",\"path\":\"/layout/extra_closets\",\"value\":0}]";
        let edited = lab.run(
            "game_edit",
            &json!({"game": ICE_CREAM_MAN, "patch": patch, "summary": "He's 25% faster; two fewer closets"}),
        );
        assert_eq!(edited["saved"], true, "{edited}");
        assert_eq!(edited["version"], 2);
        assert!(
            edited["card"]["fragment"]
                .as_str()
                .unwrap()
                .contains("t.man_chase=5.6")
        );
        let compared = lab.run("game_compare", &json!({"game": ICE_CREAM_MAN, "a": 1}));
        assert_eq!(
            compared["changes"].as_array().unwrap().len(),
            3,
            "{compared}"
        );
        let differences = compared["differences"].to_string();
        assert!(
            differences.contains("Hiding spots: 7 → 5") || differences.contains("Hiding spots"),
            "{differences}"
        );
        assert_eq!(compared["card"]["compare"]["a"]["version"], 1);
    }

    #[test]
    fn edits_that_break_the_rules_are_refused_and_nothing_is_saved() {
        let (_dir, lab) = lab();
        for (patch, says) in [
            (
                "[{\"op\":\"replace\",\"path\":\"/rules/front_door\",\"value\":\"locked\"}]",
                "no way out",
            ),
            (
                "[{\"op\":\"add\",\"path\":\"/rules/door_locked\",\"value\":true}]",
                "Coder",
            ),
            (
                // With the weapons he can still be melted: take them away too.
                "[{\"op\":\"replace\",\"path\":\"/rules/unlock\",\"value\":600},{\"op\":\"replace\",\"path\":\"/weapons/earned_weapons\",\"value\":0}]",
                "unwinnable",
            ),
            (
                "[{\"op\":\"replace\",\"path\":\"/man/man_chase\",\"value\":1000}]",
                "between",
            ),
            ("[{\"op\":\"remove\",\"path\":\"/man/sight\"}]", "missing"),
        ] {
            let result = lab.run(
                "game_edit",
                &json!({"game": ICE_CREAM_MAN, "patch": patch, "summary": "x"}),
            );
            assert_eq!(result["refused"], true, "{result}");
            assert!(
                result["reason"].as_str().unwrap().contains(says),
                "{result}"
            );
        }
        assert_eq!(lab.versions(ICE_CREAM_MAN), vec![1]);
        let wrong = lab.run("game_open", &json!({"verison": 1}));
        assert!(
            wrong["error"]
                .as_str()
                .unwrap()
                .contains("takes: game, version")
        );
    }

    #[test]
    fn notes_keep_a_time_and_place() {
        let (_dir, lab) = lab();
        let saved = lab.run(
            "game_notes",
            &json!({"game": ICE_CREAM_MAN, "text": "He's too slow here", "seconds": 42.25, "room": "Kitchen"}),
        );
        assert_eq!(saved["saved"], true, "{saved}");
        let read = lab.run("game_notes", &json!({"game": ICE_CREAM_MAN}));
        assert_eq!(read["notes"][0]["room"], "Kitchen");
        assert_eq!(read["notes"][0]["seconds"], 42.3);
        assert_eq!(read["card"]["notes"], 1);
    }

    #[test]
    fn a_code_request_is_recorded_and_waits_for_its_commit() {
        let (_dir, lab) = lab();
        let refused = lab.run(
            "game_propose_code",
            &json!({"game": ICE_CREAM_MAN, "request": "  "}),
        );
        assert!(refused["error"].as_str().unwrap().contains("request"));
        let recorded = lab.run(
            "game_propose_code",
            &json!({"game": ICE_CREAM_MAN, "request": "Minions that hunt you"}),
        );
        assert_eq!(recorded["recorded"], true, "{recorded}");
        assert_eq!(recorded["request"]["id"], "r1");
        assert!(
            recorded["text"]
                .as_str()
                .unwrap()
                .contains("Nothing in the game has changed")
        );
        // Nothing about the game changed: still one version.
        assert_eq!(lab.versions(ICE_CREAM_MAN), vec![1]);
        let second = lab.run(
            "game_propose_code",
            &json!({"game": ICE_CREAM_MAN, "request": "A camera toggle"}),
        );
        assert_eq!(second["request"]["id"], "r2");
        let listed = lab.run("game_code_requests", &json!({"game": ICE_CREAM_MAN}));
        let requests = listed["requests"].as_array().unwrap();
        assert_eq!(requests.len(), 2);
        // r1 of this game was made by a real commit (#11626), which this
        // checkout's history has: it's linked.
        assert_eq!(requests[0]["status"], "made", "{}", requests[0]);
        let hash = requests[0]["commit"].as_str().unwrap();
        assert!(requests[0]["commit_url"].as_str().unwrap().ends_with(hash));
        assert!(requests[1]["status"] == "waiting for Coder" || requests[1]["status"] == "made");
        // A commit that names a request is found by its line.
        assert!(commit_for(&lab.repo, "a line no commit has 7c1e3b0d").is_none());
    }

    #[test]
    fn a_compare_never_answers_for_a_game_it_was_not_asked_about() {
        let (_dir, lab) = lab();
        // A follow-up meant for a film: no game named, or a film named.
        for args in [
            json!({"a": 2}),
            json!({"game": "episode-290", "a": 2}),
            json!({"game": "", "a": 2}),
        ] {
            let result = lab.run("game_compare", &args);
            let error = result["error"].as_str().unwrap_or_default();
            assert!(error.contains("Nothing was done"), "{result}");
            assert!(result.get("differences").is_none() && result.get("card").is_none());
        }
        // Versions the game doesn't have are refused, not made up.
        let result = lab.run("game_compare", &json!({"game": ICE_CREAM_MAN, "a": 2}));
        assert!(
            result["error"]
                .as_str()
                .unwrap()
                .contains("has no version 2"),
            "{result}"
        );
        let result = lab.run("game_compare", &json!({"game": ICE_CREAM_MAN}));
        assert!(
            result["error"].as_str().unwrap().contains("with itself"),
            "{result}"
        );
        assert!(result.get("differences").is_none());
        // The declarations ask for the game on every tool but the list.
        for tool in tools()["tools"].as_array().unwrap() {
            let required = tool["parameters"]["required"].to_string();
            assert_eq!(
                required.contains("\"game\""),
                tool["name"] != "game_list",
                "{tool}"
            );
        }
    }
}
