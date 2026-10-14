//! The Lab on this computer: CAD documents and their versions, and the
//! `cad_*` tools the chat calls (docs/cad-lab/README.md).
//!
//! ```text
//! ROOT/<doc>/vN/doc.json        the version's assembly.v1 (or part.v1) document
//! ROOT/<doc>/vN/version.json    its summary, author, digest and check report
//! ROOT/<doc>/vN/model.glb       its solids, for the card's viewer and Verse
//! ROOT/<doc>/vN/hero.png        its picture
//! ROOT/<doc>/vN/out/            renders and exports asked for
//! ```
//!
//! ROOT is `$CAD_LAB_DIR`, else `~/work/film-out/lab/cad`: private, beside
//! Video Lab's films and Game Lab's games, never in git.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use cad::doc;
use cad::render::View;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

/// The card kind a tool result carries.
pub const CARD: &str = "cad_lab";

/// Files of a version the Lab serves.
pub const FILES: [&str; 2] = ["model.glb", "hero.png"];

pub struct Lab {
    pub root: PathBuf,
    /// Where this Lab answers, for the links it gives.
    pub base: String,
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

fn arg_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args[key].as_str().map(str::trim).filter(|s| !s.is_empty())
}

/// A JSON argument the model may send as text or as JSON.
fn arg_json(args: &Value, key: &str) -> Result<Option<Value>, String> {
    match &args[key] {
        Value::Null => Ok(None),
        Value::String(text) if text.trim().is_empty() => Ok(None),
        Value::String(text) => serde_json::from_str(text)
            .map(Some)
            .map_err(|e| format!("{key} isn't valid JSON ({e}).")),
        other => Ok(Some(other.clone())),
    }
}

/// A list argument: a JSON array, or one object as a list of one.
fn arg_list(args: &Value, key: &str) -> Result<Vec<Value>, String> {
    Ok(match arg_json(args, key)? {
        None => Vec::new(),
        Some(Value::Array(items)) => items,
        Some(object @ Value::Object(_)) => vec![object],
        Some(other) => return Err(format!("{key} must be a list (got {other}).")),
    })
}

fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if (c == ' ' || c == '-' || c == '_') && !out.ends_with('-') && !out.is_empty() {
            out.push(if c == '_' { '_' } else { '-' });
        }
    }
    out.trim_end_matches(['-', '_']).chars().take(48).collect()
}

impl Lab {
    /// The Lab at `root`.
    ///
    /// # Errors
    /// The folder can't be made.
    pub fn open(root: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
        Ok(Self {
            root,
            base: "http://127.0.0.1:8794".into(),
            writes: Mutex::new(()),
        })
    }

    #[must_use]
    pub fn default_root() -> PathBuf {
        if let Some(dir) = std::env::var_os("CAD_LAB_DIR") {
            return PathBuf::from(dir);
        }
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("work/film-out/lab/cad")
    }

    fn doc_dir(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }

    /// The directory of a version.
    #[must_use]
    pub fn version_dir(&self, id: &str, n: u64) -> PathBuf {
        self.doc_dir(id).join(format!("v{n}"))
    }

    #[must_use]
    pub fn versions(&self, id: &str) -> Vec<u64> {
        if slug(id) != id {
            return Vec::new();
        }
        let mut out: Vec<u64> = std::fs::read_dir(self.doc_dir(id))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_str()?.strip_prefix('v')?.parse().ok())
            .filter(|n| self.version_dir(id, *n).join("doc.json").is_file())
            .collect();
        out.sort_unstable();
        out
    }

    #[must_use]
    pub fn latest(&self, id: &str) -> Option<u64> {
        self.versions(id).last().copied()
    }

    #[must_use]
    pub fn load(&self, id: &str, n: u64) -> Option<(Value, Value)> {
        let dir = self.version_dir(id, n);
        Some((
            read_json(&dir.join("doc.json"))?,
            read_json(&dir.join("version.json"))?,
        ))
    }

    /// The documents, newest first.
    #[must_use]
    pub fn documents(&self) -> Vec<Value> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&self.root)
            .into_iter()
            .flatten()
            .flatten()
        {
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some(n) = self.latest(&id) else { continue };
            let Some((document, record)) = self.load(&id, n) else {
                continue;
            };
            out.push(json!({
                "id": id,
                "title": document["title"],
                "kind": document["kind"],
                "versions": n,
                "latest": record["summary"],
                "created": record["created"],
                "mass_kg": record["report"]["mass"]["mass_kg"],
            }));
        }
        out.sort_by_key(|d| std::cmp::Reverse(d["created"].as_u64().unwrap_or(0)));
        out
    }

    /// Checks a document and saves it as the next version, with its viewer
    /// model and picture. A document that fails a check isn't saved.
    fn save(
        &self,
        id: &str,
        document: &Value,
        summary: &str,
        author: &str,
        base: Option<u64>,
    ) -> Result<(u64, Value), String> {
        let _guard = self.writes.lock().map_err(|e| e.to_string())?;
        let eval = cad::evaluate_with(document, &self.root.join(".parts")).map_err(|problems| {
            format!(
                "I didn't save that; it doesn't pass the checks: {}",
                problems.join(" ")
            )
        })?;
        let n = self.latest(id).unwrap_or(0) + 1;
        let dir = self.version_dir(id, n);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("model.glb"), cad::export::glb(&eval.built.solids))
            .map_err(|e| e.to_string())?;
        if let Ok(png) = cad::render::png(&eval.built, &View::named("hero"), 1280, 800) {
            std::fs::write(dir.join("hero.png"), png).map_err(|e| e.to_string())?;
        }
        let record = json!({
            "n": n,
            "summary": summary,
            "author": author,
            "digest": digest(document),
            "created": now(),
            "base": base,
            "report": eval.report,
        });
        write_json(&dir.join("doc.json"), document)?;
        write_json(&dir.join("version.json"), &record)?;
        Ok((n, record))
    }

    // ---- the tools ----------------------------------------------------------

    /// Runs a tool. Every result is JSON; a refusal is `{"error": why}`.
    #[must_use]
    pub fn run(&self, name: &str, args: &Value) -> Value {
        if let Some(why) = unknown_arguments(name, args) {
            return json!({"error": why});
        }
        let result = match name {
            "cad_list" => Ok(json!({"documents": self.documents()})),
            "cad_open" => self.open_doc(args),
            "cad_new_part" => self.new_part(args),
            "cad_add_feature" => self.add_feature(args),
            "cad_edit_feature" => self.edit_feature(args),
            "cad_set_part" => self.set_part(args),
            "cad_assemble" => self.assemble(args),
            "cad_check" => self.check(args),
            "cad_versions" => self.versions_tool(args),
            "cad_compare" => self.compare(args),
            "cad_export" => self.export(args),
            "cad_render" => self.render(args),
            other => Err(format!("{other} isn't a CAD Lab tool.")),
        };
        result.unwrap_or_else(|why| json!({"error": why}))
    }

    /// The document named in `args` and the version asked for (the latest
    /// when left out).
    fn pick(&self, args: &Value) -> Result<(String, u64, Value, Value), String> {
        let id = arg_str(args, "doc")
            .ok_or("Name the document (doc), e.g. \"rover\".")?
            .to_owned();
        let versions = self.versions(&id);
        let Some(latest) = versions.last().copied() else {
            return Err(format!(
                "There's no CAD document called {id}; cad_list lists them."
            ));
        };
        let n = arg_u64(args, "version").unwrap_or(latest);
        let (document, record) = self
            .load(&id, n)
            .ok_or_else(|| format!("{id} has no version {n}; it has versions 1 to {latest}."))?;
        Ok((id, n, document, record))
    }

    fn author(args: &Value) -> String {
        arg_str(args, "author").unwrap_or("chat").to_owned()
    }

    fn saved(&self, id: &str, n: u64, record: &Value) -> Result<Value, String> {
        let report = &record["report"];
        Ok(json!({
            "saved": true,
            "doc": id,
            "version": n,
            "summary": record["summary"],
            "mass": report["mass"],
            "size_mm": report["size_mm"],
            "parts": report["parts"].as_array().map_or(0, Vec::len),
            "instances": report["instances"].as_array().map_or(0, Vec::len),
            "contacts": report["checks"]["contacts"],
            "warnings": report["warnings"],
            "card": self.card(id, n, None)?,
        }))
    }

    /// A change to the latest (or named) version, saved as the next one.
    fn change(
        &self,
        args: &Value,
        edit: impl FnOnce(&mut Value) -> Result<String, String>,
    ) -> Result<Value, String> {
        let (id, n, mut document, _) = self.pick(args)?;
        let fallback = edit(&mut document)?;
        let summary = arg_str(args, "summary").map_or(fallback, str::to_owned);
        let (m, record) = self.save(&id, &document, &summary, &Self::author(args), Some(n))?;
        self.saved(&id, m, &record)
    }

    fn open_doc(&self, args: &Value) -> Result<Value, String> {
        let (id, n, document, record) = self.pick(args)?;
        Ok(json!({
            "doc": id,
            "version": n,
            "summary": record["summary"],
            "document": document,
            "report": record["report"],
            "card": self.card(&id, n, None)?,
        }))
    }

    fn part_of<'a>(
        document: &'a mut Value,
        args: &Value,
    ) -> Result<(String, &'a mut Value), String> {
        let part = arg_str(args, "part")
            .ok_or("Name the part (part).")?
            .to_owned();
        let known: Vec<String> = document["parts"]
            .as_object()
            .map(|p| p.keys().cloned().collect())
            .unwrap_or_default();
        match document["parts"].get_mut(&part) {
            Some(found) => Ok((part, found)),
            None => Err(format!(
                "The document has no part called {part}; its parts are: {}.",
                if known.is_empty() {
                    "none yet".into()
                } else {
                    known.join(", ")
                }
            )),
        }
    }

    fn new_part(&self, args: &Value) -> Result<Value, String> {
        let id = arg_str(args, "doc").ok_or("Name the document (doc), e.g. \"rover\".")?;
        if slug(id) != id {
            return Err(format!(
                "A document id is lowercase letters, digits and -, like {}.",
                slug(id)
            ));
        }
        let part = arg_str(args, "part").ok_or("Name the part (part), e.g. \"chassis\".")?;
        let (mut document, base) = match self.latest(id) {
            Some(n) => (
                self.load(id, n).ok_or("The document can't be read.")?.0,
                Some(n),
            ),
            None => (
                doc::new_assembly(id, arg_str(args, "doc_title").unwrap_or(id)),
                None,
            ),
        };
        if document["kind"] != doc::ASSEMBLY {
            return Err(format!(
                "{id} is a single part; parts are added to assemblies."
            ));
        }
        if document["parts"].get(part).is_some() {
            return Err(format!(
                "{id} already has a part called {part}; change it with cad_add_feature, cad_edit_feature or cad_set_part."
            ));
        }
        let material = arg_str(args, "material").unwrap_or(doc::DEFAULT_MATERIAL);
        let mut new = doc::new_part(
            part,
            arg_str(args, "title").unwrap_or(part),
            material,
            arg_str(args, "color"),
        );
        new["features"] = Value::Array(arg_list(args, "features")?);
        if let Some(params) = arg_json(args, "parameters")? {
            new["parameters"] = params;
        }
        document["parts"][part] = new;
        if args["place"] != false {
            let mut instance = json!({"id": part, "part": part});
            for key in ["at", "rotate", "parent"] {
                if let Some(v) = arg_json(args, key)? {
                    instance[key] = v;
                }
            }
            document["instances"]
                .as_array_mut()
                .ok_or("bad document")?
                .push(instance);
        }
        let summary =
            arg_str(args, "summary").map_or_else(|| format!("New part: {part}"), str::to_owned);
        let (n, record) = self.save(id, &document, &summary, &Self::author(args), base)?;
        self.saved(id, n, &record)
    }

    fn add_feature(&self, args: &Value) -> Result<Value, String> {
        let features = arg_list(args, "features")?;
        if features.is_empty() {
            return Err("Give features: a list like [{\"id\":\"hub\",\"op\":\"solid.cylinder\",\"params\":{...}}].".into());
        }
        let after = arg_str(args, "after").map(str::to_owned);
        self.change(args, |document| {
            let (part, found) = Self::part_of(document, args)?;
            let list = found["features"].as_array_mut().ok_or("bad part")?;
            let at = match &after {
                Some(id) => list
                    .iter()
                    .position(|f| f["id"] == id.as_str())
                    .map(|i| i + 1)
                    .ok_or_else(|| format!("{part} has no feature {id}."))?,
                None => list.len(),
            };
            let names: Vec<String> = features
                .iter()
                .filter_map(|f| f["id"].as_str().map(str::to_owned))
                .collect();
            for (i, f) in features.iter().enumerate() {
                list.insert(at + i, f.clone());
            }
            Ok(format!("{part}: added {}", names.join(", ")))
        })
    }

    fn edit_feature(&self, args: &Value) -> Result<Value, String> {
        let feature = arg_str(args, "feature")
            .ok_or("Name the feature (feature).")?
            .to_owned();
        let params = arg_json(args, "params")?;
        self.change(args, |document| {
            let (part, found) = Self::part_of(document, args)?;
            let list = found["features"].as_array_mut().ok_or("bad part")?;
            let i = list
                .iter()
                .position(|f| f["id"] == feature.as_str())
                .ok_or_else(|| {
                    format!(
                        "{part} has no feature {feature}; its features are: {}.",
                        list.iter()
                            .filter_map(|f| f["id"].as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })?;
            if args["remove"] == true {
                list.remove(i);
                return Ok(format!("{part}: removed {feature}"));
            }
            let f = &mut list[i];
            if let Some(op) = arg_str(args, "op") {
                f["op"] = json!(op);
            }
            if let Some(Value::Object(changes)) = params {
                let target = f["params"]
                    .as_object_mut()
                    .map(std::mem::take)
                    .unwrap_or_default();
                let mut merged: Map<String, Value> = target;
                for (k, v) in changes {
                    if v.is_null() {
                        merged.remove(&k);
                    } else {
                        merged.insert(k, v);
                    }
                }
                f["params"] = Value::Object(merged);
            }
            if let Some(b) = args["suppressed"].as_bool() {
                f["suppressed"] = json!(b);
            }
            Ok(format!("{part}: changed {feature}"))
        })
    }

    fn set_part(&self, args: &Value) -> Result<Value, String> {
        self.change(args, |document| {
            if args["remove"] == true {
                let part = arg_str(args, "part")
                    .ok_or("Name the part (part).")?
                    .to_owned();
                let parts = document["parts"].as_object_mut().ok_or("bad document")?;
                parts
                    .remove(&part)
                    .ok_or_else(|| format!("There's no part {part}."))?;
                let gone: Vec<String> = document["instances"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|i| i["part"] == part.as_str())
                    .filter_map(|i| i["id"].as_str().map(str::to_owned))
                    .collect();
                remove_instances(document, &gone);
                return Ok(format!("Removed part {part}"));
            }
            if let Some(n) = arg_u64(args, "from_version") {
                let part = arg_str(args, "part")
                    .ok_or("Name the part (part).")?
                    .to_owned();
                let old = self
                    .load(&id_of(args), n)
                    .ok_or_else(|| format!("There's no version {n}."))?
                    .0;
                let earlier = old["parts"]
                    .get(&part)
                    .cloned()
                    .ok_or_else(|| format!("Version {n} has no part {part}."))?;
                document["parts"][&part] = earlier;
                return Ok(format!("{part}: back to how it was in v{n}"));
            }
            let (part, found) = Self::part_of(document, args)?;
            for key in ["title", "material", "color"] {
                if let Some(v) = arg_str(args, key) {
                    found[key] = json!(v);
                }
            }
            if let Some(params) = arg_json(args, "parameters")? {
                found["parameters"] = params;
            }
            Ok(format!("{part}: changed its properties"))
        })
    }

    fn assemble(&self, args: &Value) -> Result<Value, String> {
        let instances = arg_list(args, "instances")?;
        let joints = arg_list(args, "joints")?;
        let contacts = arg_list(args, "contacts")?;
        let remove: Vec<String> = arg_list(args, "remove")?
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        if instances.is_empty() && joints.is_empty() && contacts.is_empty() && remove.is_empty() {
            return Err("Give instances, joints, contacts or remove.".into());
        }
        self.change(args, |document| {
            let mut said = Vec::new();
            for instance in instances {
                let id = instance["id"]
                    .as_str()
                    .ok_or("Each instance needs an id.")?
                    .to_owned();
                let list = document["instances"].as_array_mut().ok_or("bad document")?;
                match list.iter_mut().find(|i| i["id"] == id.as_str()) {
                    Some(existing) => {
                        for (k, v) in instance.as_object().into_iter().flatten() {
                            if v.is_null() {
                                if let Some(o) = existing.as_object_mut() {
                                    o.remove(k);
                                }
                            } else {
                                existing[k] = v.clone();
                            }
                        }
                        said.push(format!("moved {id}"));
                    }
                    None => {
                        list.push(instance);
                        said.push(format!("placed {id}"));
                    }
                }
            }
            for joint in joints {
                let id = joint["id"]
                    .as_str()
                    .ok_or("Each joint needs an id.")?
                    .to_owned();
                let list = document["joints"].as_array_mut().ok_or("bad document")?;
                match list.iter_mut().find(|j| j["id"] == id.as_str()) {
                    Some(existing) => {
                        for (k, v) in joint.as_object().into_iter().flatten() {
                            existing[k] = v.clone();
                        }
                        said.push(format!("set joint {id}"));
                    }
                    None => {
                        list.push(joint);
                        said.push(format!("joint {id}"));
                    }
                }
            }
            for pair in contacts {
                let list = document["contacts"].as_array_mut().ok_or("bad document")?;
                if !list.contains(&pair) {
                    list.push(pair);
                }
            }
            if !remove.is_empty() {
                remove_instances(document, &remove);
                said.push(format!("removed {}", remove.join(", ")));
            }
            Ok(format!("Assembly: {}", said.join(", ")))
        })
    }

    fn check(&self, args: &Value) -> Result<Value, String> {
        let (id, n, document, _) = self.pick(args)?;
        match cad::evaluate_with(&document, &self.root.join(".parts")) {
            Ok(eval) => Ok(
                json!({"doc": id, "version": n, "passed": true, "report": eval.report, "card": self.card(&id, n, None)?}),
            ),
            Err(problems) => {
                Ok(json!({"doc": id, "version": n, "passed": false, "problems": problems}))
            }
        }
    }

    fn versions_tool(&self, args: &Value) -> Result<Value, String> {
        let (id, _, _, _) = self.pick(args)?;
        let versions: Vec<Value> = self
            .versions(&id)
            .into_iter()
            .filter_map(|n| {
                let (_, r) = self.load(&id, n)?;
                Some(json!({"n": n, "summary": r["summary"], "author": r["author"], "digest": r["digest"],
                            "mass_kg": r["report"]["mass"]["mass_kg"], "parts": r["report"]["parts"].as_array().map_or(0, Vec::len)}))
            })
            .collect();
        Ok(json!({"doc": id, "versions": versions}))
    }

    fn compare(&self, args: &Value) -> Result<Value, String> {
        let (id, b, db, rb) = self.pick(&json!({"doc": args["doc"], "version": args["b"]}))?;
        let a = arg_u64(args, "a").unwrap_or(b.saturating_sub(1).max(1));
        if a == b {
            return Err(format!(
                "That's version {b} with itself; name two different versions."
            ));
        }
        let (da, ra) = self
            .load(&id, a)
            .ok_or_else(|| format!("{id} has no version {a}."))?;
        let differences = differences(&da, &ra, &db, &rb);
        Ok(json!({
            "doc": id, "a": a, "b": b,
            "differences": differences,
            "card": self.card(&id, b, Some(a))?,
        }))
    }

    fn export(&self, args: &Value) -> Result<Value, String> {
        let (id, n, document, _) = self.pick(args)?;
        let formats: Vec<String> = match arg_list(args, "formats")? {
            list if !list.is_empty() => list
                .iter()
                .filter_map(|f| f.as_str().map(|s| s.trim_start_matches('.').to_lowercase()))
                .collect(),
            _ => vec!["glb".into(), "3mf".into(), "step".into()],
        };
        let mut eval =
            cad::evaluate_with(&document, &self.root.join(".parts")).map_err(|p| p.join(" "))?;
        let dir = self.version_dir(&id, n).join("out");
        let mut files = Vec::new();
        for format in formats {
            let path = dir.join(format!("{id}-v{n}.{format}"));
            let bytes = cad::export::write(&mut eval.built, &format, &path)?;
            files.push(json!({"format": format, "path": path, "bytes": bytes,
                              "url": format!("{}/api/out?doc={id}&v={n}&name={id}-v{n}.{format}", self.base)}));
        }
        let mut card = self.card(&id, n, None)?;
        card["exports"] = json!(files);
        Ok(json!({"doc": id, "version": n, "files": files, "card": card}))
    }

    fn render(&self, args: &Value) -> Result<Value, String> {
        let (id, n, document, _) = self.pick(args)?;
        let name = arg_str(args, "view").unwrap_or("hero").to_owned();
        if !doc::VIEWS.contains(&name.as_str()) {
            return Err(format!("The views are: {}.", doc::VIEWS.join(", ")));
        }
        let focus: Vec<String> = arg_list(args, "focus")?
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        let eval =
            cad::evaluate_with(&document, &self.root.join(".parts")).map_err(|p| p.join(" "))?;
        for f in &focus {
            if !eval.built.placements.contains_key(f) {
                return Err(format!("{id} v{n} has no instance {f}."));
            }
        }
        let width = arg_u64(args, "width").unwrap_or(1600).clamp(64, 3840) as usize;
        let height = arg_u64(args, "height").unwrap_or(1000).clamp(64, 2160) as usize;
        let view = View {
            name: name.clone(),
            focus: focus.clone(),
            yaw: args["yaw"].as_f64().unwrap_or(0.0),
            zoom: args["zoom"].as_f64().unwrap_or(1.0),
            frame: None,
        };
        let png = cad::render::png(&eval.built, &view, width, height)?;
        let file = format!(
            "{name}{}-{width}x{height}.png",
            if focus.is_empty() {
                String::new()
            } else {
                format!("-{}", focus.join("+"))
            }
        );
        let dir = self.version_dir(&id, n).join("out");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(&file), png).map_err(|e| e.to_string())?;
        let mut card = self.card(&id, n, None)?;
        card["render"] = json!({"view": name, "focus": focus, "file": file});
        Ok(json!({"doc": id, "version": n, "path": dir.join(&file), "card": card}))
    }

    // ---- cards --------------------------------------------------------------

    /// The card for a version (and, for a comparison, the version before).
    ///
    /// # Errors
    /// No such version.
    pub fn card(&self, id: &str, n: u64, against: Option<u64>) -> Result<Value, String> {
        let (document, record) = self
            .load(id, n)
            .ok_or_else(|| format!("{id} has no version {n}."))?;
        let versions: Vec<Value> = self
            .versions(id)
            .into_iter()
            .filter_map(|v| Some(json!({"n": v, "summary": self.load(id, v)?.1["summary"]})))
            .collect();
        let report = &record["report"];
        let mut card = json!({
            "card": CARD,
            "doc": id,
            "title": document["title"],
            "version": n,
            "summary": record["summary"],
            "mass": report["mass"],
            "size_mm": report["size_mm"],
            "parts": report["parts"].as_array().map_or(0, Vec::len),
            "instances": report["instances"].as_array().map_or(0, Vec::len),
            "features": report["features"],
            "joints": report["joints"],
            "solids": report["solids"],
            "versions": versions,
            "step": "exact solids (B-rep)",
        });
        if let Some(a) = against {
            let (da, ra) = self
                .load(id, a)
                .ok_or_else(|| format!("{id} has no version {a}."))?;
            card["compare"] = json!({"a": a, "a_summary": ra["summary"], "differences": differences(&da, &ra, &document, &record)});
        }
        Ok(card)
    }
}

fn id_of(args: &Value) -> String {
    arg_str(args, "doc").unwrap_or_default().to_owned()
}

fn remove_instances(document: &mut Value, gone: &[String]) {
    let keep = |v: &Value, key: &str| !v[key].as_str().is_some_and(|s| gone.iter().any(|g| g == s));
    if let Some(list) = document["instances"].as_array_mut() {
        list.retain(|i| keep(i, "id"));
        for i in list.iter_mut() {
            if !keep(i, "parent")
                && let Some(o) = i.as_object_mut()
            {
                o.remove("parent");
            }
        }
    }
    if let Some(list) = document["joints"].as_array_mut() {
        list.retain(|j| keep(j, "child") && keep(j, "parent"));
    }
    if let Some(list) = document["contacts"].as_array_mut() {
        list.retain(|p| {
            keep(p, "0")
                && p.as_array().is_some_and(|a| {
                    a.iter()
                        .all(|x| !x.as_str().is_some_and(|s| gone.iter().any(|g| g == s)))
                })
        });
    }
}

/// What changed between two versions, in plain lines.
#[must_use]
pub fn differences(da: &Value, ra: &Value, db: &Value, rb: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let parts = |d: &Value| d["parts"].as_object().cloned().unwrap_or_default();
    let (pa, pb) = (parts(da), parts(db));
    for (k, part) in &pb {
        match pa.get(k) {
            None => out.push(format!(
                "New part {k} ({} features)",
                part["features"].as_array().map_or(0, Vec::len)
            )),
            Some(old) if old != part => {
                let ids = |p: &Value| -> Vec<String> {
                    p["features"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|f| f["id"].as_str().map(str::to_owned))
                        .collect()
                };
                let (fa, fb) = (ids(old), ids(part));
                let added: Vec<&String> = fb.iter().filter(|f| !fa.contains(f)).collect();
                let removed: Vec<&String> = fa.iter().filter(|f| !fb.contains(f)).collect();
                let changed: Vec<&String> = fb
                    .iter()
                    .filter(|f| {
                        fa.contains(f)
                            && old["features"]
                                .as_array()
                                .and_then(|l| l.iter().find(|x| x["id"] == f.as_str()))
                                != part["features"]
                                    .as_array()
                                    .and_then(|l| l.iter().find(|x| x["id"] == f.as_str()))
                    })
                    .collect();
                let mut line = format!("Part {k}:");
                if !added.is_empty() {
                    line.push_str(&format!(
                        " added {}",
                        added
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                if !removed.is_empty() {
                    line.push_str(&format!(
                        " removed {}",
                        removed
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                if !changed.is_empty() {
                    line.push_str(&format!(
                        " changed {}",
                        changed
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                for key in ["material", "color", "title"] {
                    if old[key] != part[key] {
                        line.push_str(&format!(" {key} {} → {}", old[key], part[key]));
                    }
                }
                out.push(line);
            }
            _ => {}
        }
    }
    for k in pa.keys().filter(|k| !pb.contains_key(*k)) {
        out.push(format!("Removed part {k}"));
    }
    let ids =
        |d: &Value, key: &str| -> Vec<Value> { d[key].as_array().cloned().unwrap_or_default() };
    let (ia, ib) = (ids(da, "instances"), ids(db, "instances"));
    let find = |l: &[Value], id: &Value| l.iter().find(|i| i["id"] == *id).cloned();
    for i in &ib {
        match find(&ia, &i["id"]) {
            None => out.push(format!("Placed {}", i["id"].as_str().unwrap_or_default())),
            Some(old) if old != *i => {
                out.push(format!("Moved {}", i["id"].as_str().unwrap_or_default()))
            }
            _ => {}
        }
    }
    for i in &ia {
        if find(&ib, &i["id"]).is_none() {
            out.push(format!("Removed {}", i["id"].as_str().unwrap_or_default()));
        }
    }
    let (ja, jb) = (ids(da, "joints"), ids(db, "joints"));
    for j in &jb {
        match find(&ja, &j["id"]) {
            None => out.push(format!(
                "New {} joint {}",
                j["type"].as_str().unwrap_or_default(),
                j["id"].as_str().unwrap_or_default()
            )),
            Some(old) if old["value"] != j["value"] => out.push(format!(
                "Joint {} moved from {} to {}",
                j["id"].as_str().unwrap_or_default(),
                old["value"],
                j["value"]
            )),
            Some(old) if old != *j => out.push(format!(
                "Joint {} changed",
                j["id"].as_str().unwrap_or_default()
            )),
            _ => {}
        }
    }
    let m = |r: &Value| r["report"]["mass"]["mass_kg"].as_f64();
    if let (Some(x), Some(y)) = (m(ra), m(rb))
        && (x - y).abs() > 1e-6
    {
        out.push(format!("Mass {x:.3} kg → {y:.3} kg"));
    }
    let c = |r: &Value| r["report"]["mass"]["center_of_mass_mm"].clone();
    if c(ra) != c(rb) {
        out.push(format!("Centre of mass {} → {} mm", c(ra), c(rb)));
    }
    if out.is_empty() {
        out.push("Nothing changed.".into());
    }
    out
}

fn takes(name: &str) -> &'static [&'static str] {
    match name {
        "cad_list" => &[],
        "cad_open" | "cad_check" => &["doc", "version"],
        "cad_new_part" => &[
            "doc",
            "doc_title",
            "part",
            "title",
            "material",
            "color",
            "features",
            "parameters",
            "place",
            "at",
            "rotate",
            "parent",
            "summary",
            "author",
        ],
        "cad_add_feature" => &["doc", "part", "features", "after", "summary", "author"],
        "cad_edit_feature" => &[
            "doc",
            "part",
            "feature",
            "op",
            "params",
            "suppressed",
            "remove",
            "summary",
            "author",
        ],
        "cad_set_part" => &[
            "doc",
            "part",
            "title",
            "material",
            "color",
            "parameters",
            "remove",
            "from_version",
            "summary",
            "author",
        ],
        "cad_assemble" => &[
            "doc",
            "instances",
            "joints",
            "contacts",
            "remove",
            "summary",
            "author",
        ],
        "cad_versions" => &["doc"],
        "cad_compare" => &["doc", "a", "b"],
        "cad_export" => &["doc", "version", "formats"],
        "cad_render" => &[
            "doc", "version", "view", "focus", "yaw", "zoom", "width", "height",
        ],
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
    let doc = json!({"type": "string", "description": "the document's id, e.g. rover"});
    let part = json!({"type": "string", "description": "a part's id in the document, e.g. wheel"});
    let version =
        json!({"type": "integer", "description": "a version number; the latest when left out"});
    let summary = json!({"type": "string", "description": "one line in plain words saying what this step changes"});
    json!({"tools": [
        {"name": "cad_list", "description": "List the CAD documents in CAD Lab on this computer.",
         "parameters": {"type": "object", "properties": {}}},
        {"name": "cad_open", "description": "Open a CAD document: its whole assembly.v1 document (parts with their feature histories, instances, joints, contacts), its mass properties and checks, and a card with a 3D view.",
         "parameters": {"type": "object", "required": ["doc"], "properties": {"doc": doc, "version": version}}},
        {"name": "cad_new_part", "description": "Add a new part to a CAD assembly (making the document when it doesn't exist yet), with its material, colour and first features, and place one instance of it (id = the part id) unless place is false. Saved as a new version only if every check passes.",
         "parameters": {"type": "object", "required": ["doc", "part", "summary"], "properties": {
            "doc": doc, "doc_title": {"type": "string", "description": "the document's title, when making it"},
            "part": part, "title": {"type": "string"},
            "material": {"type": "string", "description": "one of: Aluminum, Titanium, Steel, Stainless Steel, Brass, Copper, ABS Plastic, PLA, Nylon, Polycarbonate, Glass, Carbon Fibre, Rubber, Circuit Board, Solar Cell, Gold, Foam, Kapton"},
            "color": {"type": "string", "description": "#rrggbb"},
            "features": {"type": "string", "description": "the part's features as a JSON array (see the feature guide)"},
            "place": {"type": "boolean", "description": "place an instance of it (default true)"},
            "at": {"type": "string", "description": "the instance's position [x,y,z] in mm, as JSON"},
            "rotate": {"type": "string", "description": "the instance's rotation [rx,ry,rz] in degrees, as JSON"},
            "parent": {"type": "string", "description": "the instance this one is placed relative to"},
            "summary": summary}}},
        {"name": "cad_add_feature", "description": "Add features to a part's history (after the last one, or after the feature named in after). The whole assembly is rebuilt and checked; an edit that fails a check is refused with the reason and nothing is saved.",
         "parameters": {"type": "object", "required": ["doc", "part", "features", "summary"], "properties": {
            "doc": doc, "part": part,
            "features": {"type": "string", "description": "a JSON array of features: [{\"id\":\"hub\",\"op\":\"solid.cylinder\",\"params\":{...}}]"},
            "after": {"type": "string", "description": "insert after this feature id"},
            "summary": summary}}},
        {"name": "cad_edit_feature", "description": "Change one feature of a part: new params (merged into its params; a null value removes that key), a different op, suppress it, or remove it. Later features rebuild from it.",
         "parameters": {"type": "object", "required": ["doc", "part", "feature", "summary"], "properties": {
            "doc": doc, "part": part, "feature": {"type": "string", "description": "the feature's id"},
            "params": {"type": "string", "description": "a JSON object of params to change"},
            "op": {"type": "string"}, "suppressed": {"type": "boolean"}, "remove": {"type": "boolean"},
            "summary": summary}}},
        {"name": "cad_set_part", "description": "Change a part's title, material or colour, bring a part back to how it was in an earlier version (from_version), or remove the part and its instances (remove: true).",
         "parameters": {"type": "object", "required": ["doc", "part", "summary"], "properties": {
            "doc": doc, "part": part, "title": {"type": "string"}, "material": {"type": "string"}, "color": {"type": "string"},
            "remove": {"type": "boolean"},
            "from_version": {"type": "integer", "description": "bring the part back to how it was in this version"},
            "summary": summary}}},
        {"name": "cad_assemble", "description": "Place, move or remove instances of parts, add or set joints (revolute, slider, cylindrical, fastened; a joint's value opens a lid or turns a wheel), and declare contacts between instances that are meant to touch. Instances and joints with an existing id are updated (only the fields given). Checked like every edit: overlaps between instances that aren't declared contacts or joined by a joint are refused.",
         "parameters": {"type": "object", "required": ["doc", "summary"], "properties": {
            "doc": doc,
            "instances": {"type": "string", "description": "a JSON array: [{\"id\":\"wheel_lf\",\"part\":\"wheel\",\"parent\":\"bogie_l\",\"at\":[x,y,z],\"rotate\":[rx,ry,rz]}]"},
            "joints": {"type": "string", "description": "a JSON array: [{\"id\":\"lid_hinge\",\"type\":\"revolute\",\"parent\":\"bay\",\"child\":\"lid\",\"origin\":[x,y,z],\"axis\":[x,y,z],\"value\":0,\"limits\":[[0,110]]}] (origin and axis in the parent's frame; parent must be the child's parent instance)"},
            "contacts": {"type": "string", "description": "a JSON array of instance pairs meant to touch: [[\"a\",\"b\"]]"},
            "remove": {"type": "string", "description": "a JSON array of instance ids to remove"},
            "summary": summary}}},
        {"name": "cad_check", "description": "Run the checks on a version again: every feature rebuilds, every solid is closed, no undeclared overlaps; with its mass, centre of mass and inertia.",
         "parameters": {"type": "object", "required": ["doc"], "properties": {"doc": doc, "version": version}}},
        {"name": "cad_versions", "description": "The versions of a CAD document: each one's summary, author, mass and part count.",
         "parameters": {"type": "object", "required": ["doc"], "properties": {"doc": doc}}},
        {"name": "cad_compare", "description": "What changed between two versions: parts, features, instances, joints, and the mass and centre of mass. a defaults to the version before b; b to the latest.",
         "parameters": {"type": "object", "required": ["doc"], "properties": {"doc": doc,
            "a": {"type": "integer", "description": "the first version, e.g. 1"}, "b": {"type": "integer", "description": "the second version; the latest when left out"}}}},
        {"name": "cad_export", "description": "Export a version as files: glb (glTF, for the web and Verse), 3mf (for printing) and step (exact solids for other CAD). Default: all three.",
         "parameters": {"type": "object", "required": ["doc"], "properties": {"doc": doc, "version": version,
            "formats": {"type": "string", "description": "a JSON array of formats, e.g. [\"step\",\"3mf\",\"glb\"]"}}}},
        {"name": "cad_render", "description": "A picture of a version from a view: hero, rear, plan, front, side, iso, under or detail; focus frames some instances (e.g. [\"camera_head\"]). The card shows it.",
         "parameters": {"type": "object", "required": ["doc"], "properties": {"doc": doc, "version": version,
            "view": {"type": "string", "description": "hero, rear, plan, front, side, iso, under or detail"},
            "focus": {"type": "string", "description": "a JSON array of instance ids to frame"},
            "zoom": {"type": "number"}, "yaw": {"type": "number", "description": "extra turn, degrees"}}}}
    ]})
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn lab() -> (Dir, Lab) {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("cad-lab-test-{n}-{}", std::process::id()));
        (Dir(path.clone()), Lab::open(path).unwrap())
    }

    #[test]
    fn parts_features_and_assembly_are_versions_and_checked() {
        let (_dir, lab) = lab();
        let made = lab.run("cad_new_part", &json!({"doc": "cart", "part": "deck", "material": "Aluminum", "color": "#c0c4cc",
            "features": "[{\"id\":\"plate\",\"op\":\"solid.box\",\"params\":{\"length\":300,\"width\":200,\"height\":10,\"center\":[0,0,60]}}]",
            "summary": "A deck"}));
        assert_eq!(made["saved"], true, "{made}");
        assert_eq!(made["card"]["card"], CARD);
        let wheel = lab.run("cad_new_part", &json!({"doc": "cart", "part": "wheel", "material": "Rubber", "color": "#222222", "place": false,
            "features": [{"id": "tyre", "op": "solid.cylinder", "params": {"radius": 40, "height": 20, "axis": "Y", "base": [0, -10, 0]}}],
            "summary": "A wheel"}));
        assert_eq!(wheel["saved"], true, "{wheel}");
        // A wheel through the deck is refused, and nothing is saved.
        let bad = lab.run("cad_assemble", &json!({"doc": "cart", "instances": "[{\"id\":\"w1\",\"part\":\"wheel\",\"at\":[0,0,60]}]", "summary": "x"}));
        assert!(
            bad["error"]
                .as_str()
                .unwrap_or_default()
                .contains("overlap"),
            "{bad}"
        );
        assert_eq!(lab.latest("cart"), Some(2));
        let good = lab.run("cad_assemble", &json!({"doc": "cart", "summary": "Four wheels", "instances": [
            {"id": "w1", "part": "wheel", "at": [120, 115, 40]}, {"id": "w2", "part": "wheel", "at": [-120, 115, 40]},
            {"id": "w3", "part": "wheel", "at": [120, -115, 40]}, {"id": "w4", "part": "wheel", "at": [-120, -115, 40]}]}));
        assert_eq!(good["saved"], true, "{good}");
        assert_eq!(good["instances"], 5);
        let edit = lab.run("cad_edit_feature", &json!({"doc": "cart", "part": "deck", "feature": "plate", "params": "{\"height\": 12}", "summary": "Thicker deck"}));
        assert_eq!(edit["version"], 4, "{edit}");
        let compared = lab.run("cad_compare", &json!({"doc": "cart", "a": 2}));
        let text = compared["differences"].to_string();
        assert!(
            text.contains("Placed w1") && text.contains("changed plate") && text.contains("Mass"),
            "{text}"
        );
        let back = lab.run("cad_set_part", &json!({"doc": "cart", "part": "deck", "from_version": 3, "summary": "Back to the thin deck"}));
        assert_eq!(back["version"], 5, "{back}");
        let (doc5, _) = lab.load("cart", 5).unwrap();
        assert_eq!(doc5["parts"]["deck"]["features"][0]["params"]["height"], 10);
        let same = lab.run("cad_compare", &json!({"doc": "cart", "a": 4, "b": 4}));
        assert!(same["error"].is_string());
        let missing = lab.run("cad_open", &json!({"doc": "cart", "version": 9}));
        assert!(
            missing["error"]
                .as_str()
                .unwrap()
                .contains("versions 1 to 5")
        );
        let exported = lab.run("cad_export", &json!({"doc": "cart"}));
        assert_eq!(exported["files"].as_array().unwrap().len(), 3, "{exported}");
        let rendered = lab.run("cad_render", &json!({"doc": "cart", "view": "plan", "focus": "[\"w1\"]", "width": 320, "height": 200}));
        assert!(
            rendered["card"]["render"]["file"]
                .as_str()
                .unwrap()
                .starts_with("plan-w1"),
            "{rendered}"
        );
        assert!(
            lab.run("cad_open", &json!({"doc": "cart", "nope": 1}))["error"]
                .as_str()
                .unwrap()
                .contains("doesn't take")
        );
        assert_eq!(
            lab.run("cad_list", &json!({}))["documents"][0]["id"],
            "cart"
        );
    }

    #[test]
    fn every_tool_is_declared_and_takes_what_it_declares() {
        for tool in tools()["tools"].as_array().unwrap() {
            let name = tool["name"].as_str().unwrap();
            for key in tool["parameters"]["properties"].as_object().unwrap().keys() {
                assert!(takes(name).contains(&key.as_str()), "{name} {key}");
            }
        }
    }
}
