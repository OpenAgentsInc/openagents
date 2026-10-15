//! The Lab on this computer: CAD documents and their versions, and the
//! `cad_*` tools the chat calls (docs/cad-lab/README.md).
//!
//! ```text
//! ROOT/<doc>/vN/doc.json        the version's assembly.v1 (or part.v1) document
//! ROOT/<doc>/vN/version.json    its summary, author, digest and check report
//! ROOT/<doc>/vN/model.glb       its solids, for the card's viewer and Verse
//! ROOT/<doc>/vN/view.json       the viewer's model, with face and edge names
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

/// The kind of the file of CAD models placed in Verse zones.
pub const PLACEMENTS: &str = "verse-cad-placements.v1";

/// The Verse zones a CAD version can be placed in (world id, label): the
/// zones on Everglade's lit stage, which draws the glTF.
pub const VERSE_ZONES: [(&str, &str); 3] = [
    ("verse-everglade", "Everglade"),
    ("verse-meteor-showcase", "Meteor Showcase"),
    ("verse-meteor-stress-test", "Meteor Stress Test"),
];

/// Where the placements live: `$VERSE_CAD_PLACEMENTS`, else
/// `ROOT/.verse/placements.json`. Verse reads the same file
/// (`verse::zones::cad_place`).
#[must_use]
pub fn placements_path(root: &Path) -> PathBuf {
    std::env::var_os("VERSE_CAD_PLACEMENTS").map_or_else(
        || root.join(".verse").join("placements.json"),
        PathBuf::from,
    )
}

/// The revision a version's URDF pins: the SHA-256 of its document.
#[must_use]
pub fn robot_revision(document: &Value) -> String {
    Sha256::digest(document.to_string().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The card kind a tool result carries.
pub const CARD: &str = "cad_lab";

/// Files of a version the Lab serves: the viewer's model with its face
/// names (`cad::view`), the glTF, and the picture.
pub const FILES: [&str; 3] = ["view.json", "model.glb", "hero.png"];

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
        std::fs::write(
            dir.join("view.json"),
            cad::view::model(&eval.built).to_string(),
        )
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
            "cad_from_json" => self.from_json(args),
            "cad_faces" => self.faces(args),
            "cad_motion" => self.motion(args),
            "cad_solid" => self.solid(args),
            "cad_import_dxf" => self.import_dxf(args),
            "cad_import_step" => self.import_step(args),
            "cad_export_dxf" => self.export_dxf(args),
            "cad_place" => self.place(args),
            "cad_drawing" => self.drawing(args),
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
        let mut out = json!({
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
        });
        // Where the joints of each loop went, and the freedom left.
        if !report["solver"].is_null() {
            out["solver"] = report["solver"].clone();
        }
        Ok(out)
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
        // A single-part document is its own part.
        if document["kind"] == doc::PART {
            return if document["id"] == part.as_str() {
                Ok((part, document))
            } else {
                Err(format!(
                    "This document is the single part {}.",
                    document["id"].as_str().unwrap_or_default()
                ))
            };
        }
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
        if let Some(finish) = arg_str(args, "finish") {
            new["finish"] = json!(finish);
        }
        if let Some(params) = arg_json(args, "parameters")? {
            new["parameters"] = params;
        }
        document["parts"][part] = new;
        if args["place"] != false {
            let mut instance = json!({"id": part, "part": part});
            for key in ["at", "rotate"] {
                if let Some(v) = arg_json(args, key)? {
                    instance[key] = v;
                }
            }
            // The parent is an instance id, plain ("body") or quoted.
            if let Some(parent) = arg_str(args, "parent") {
                instance["parent"] = json!(parent.trim_matches('"'));
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
            for key in ["title", "material", "color", "finish"] {
                if let Some(v) = arg_str(args, key) {
                    found[key] = json!(v);
                }
            }
            if let Some(flag) = arg_json(args, "exact_mass")? {
                if flag == false {
                    if let Some(o) = found.as_object_mut() {
                        o.remove("exact_mass");
                    }
                } else {
                    found["exact_mass"] = flag;
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
        let templates = match arg_json(args, "templates")? {
            None => Map::new(),
            Some(Value::Object(m)) => m,
            Some(other) => {
                return Err(format!(
                    "templates is an object of subassemblies by id (got {other})."
                ));
            }
        };
        let drive = match arg_json(args, "drive")? {
            None => None,
            Some(d) => {
                let joint = d["joint"]
                    .as_str()
                    .ok_or("drive is {\"joint\": id, \"value\": number or list}.")?
                    .to_owned();
                if !(d["value"].is_number() || d["value"].is_array()) {
                    return Err(format!("Give the value to drive {joint} to."));
                }
                Some((joint, d["value"].clone()))
            }
        };
        if instances.is_empty()
            && joints.is_empty()
            && contacts.is_empty()
            && remove.is_empty()
            && templates.is_empty()
            && drive.is_none()
        {
            return Err("Give instances, joints, contacts, templates, drive or remove.".into());
        }
        self.change(args, |document| {
            let mut said = Vec::new();
            for (key, template) in templates {
                if template.is_null() {
                    if let Some(m) = document["assemblies"].as_object_mut() {
                        m.remove(&key);
                    }
                    said.push(format!("removed subassembly {key}"));
                } else {
                    if !document["assemblies"].is_object() {
                        document["assemblies"] = json!({});
                    }
                    document["assemblies"][&key] = template;
                    said.push(format!("subassembly {key}"));
                }
            }
            self.resolve_templates(document)?;
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
            if let Some((joint, value)) = drive {
                cad::solve::drive(document, &joint, &value)?;
                said.push(format!("drove {joint} to {value}"));
            }
            // The joints that follow keep where the solver puts them, so the
            // next move starts from there.
            cad::solve::settle(document);
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
        let (id, n, document, record) = self.pick(args)?;
        let formats: Vec<String> = match arg_list(args, "formats")? {
            list if !list.is_empty() => list
                .iter()
                .filter_map(|f| f.as_str().map(|s| s.trim_start_matches('.').to_lowercase()))
                .collect(),
            _ => vec!["glb".into(), "3mf".into(), "step".into()],
        };
        for format in &formats {
            if !cad::export::FORMATS.contains(&format.as_str())
                && !matches!(format.as_str(), "urdf" | "feed")
            {
                return Err(format!(
                    "I can export {}, urdf and feed, not {format}.",
                    cad::export::FORMATS.join(", ")
                ));
            }
        }
        let mut eval =
            cad::evaluate_with(&document, &self.root.join(".parts")).map_err(|p| p.join(" "))?;
        let dir = self.version_dir(&id, n).join("out");
        let mut files = Vec::new();
        for format in formats {
            if format == "urdf" {
                let (path, bytes) = self.write_urdf(&id, n, &document, &eval)?;
                files.push(json!({"format": format, "path": path, "bytes": bytes,
                                  "robot_lab": {"source": format!("cad:{id}/v{n}"), "revision": robot_revision(&document)}}));
                continue;
            }
            if format == "feed" {
                let source = cad::feed::Source {
                    doc: id.clone(),
                    version: n,
                    digest: record["digest"].as_str().unwrap_or_default().to_owned(),
                };
                let feed = cad::feed::feed(source, &document, &eval.report)?;
                let name = format!("{id}-v{n}.mass.json");
                let value = serde_json::to_value(&feed).map_err(|e| e.to_string())?;
                write_json(&dir.join(&name), &value)?;
                let bytes = std::fs::metadata(dir.join(&name)).map_or(0, |m| m.len());
                files.push(json!({"format": format, "path": dir.join(&name), "bytes": bytes,
                                  "url": format!("{}/api/out?doc={id}&v={n}&name={name}", self.base)}));
                continue;
            }
            let path = dir.join(format!("{id}-v{n}.{format}"));
            let bytes = cad::export::write(&mut eval.built, &format, &path)?;
            files.push(json!({"format": format, "path": path, "bytes": bytes,
                              "url": format!("{}/api/out?doc={id}&v={n}&name={id}-v{n}.{format}", self.base)}));
        }
        let mut card = self.card(&id, n, None)?;
        card["exports"] = json!(files);
        Ok(json!({"doc": id, "version": n, "files": files, "card": card}))
    }

    /// Writes a version's URDF as a pinned folder Robot Lab imports
    /// (`out/urdf/`: the description, its STL meshes, a notice and
    /// `source.json` with every file's SHA-256). Returns the description's
    /// path and its size.
    fn write_urdf(
        &self,
        id: &str,
        n: u64,
        document: &Value,
        eval: &cad::Evaluation,
    ) -> Result<(PathBuf, u64), String> {
        // As built: joint frames named on faces are numbers there.
        let urdf = cad::urdf::write(
            id,
            &eval.built.assembly,
            &eval.report,
            Some(&eval.built.solids),
        )?;
        let dir = self.version_dir(id, n).join("out").join("urdf");
        // The pin lists every file, so the folder is written afresh.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("meshes")).map_err(|e| e.to_string())?;
        let description = format!("{id}.urdf");
        let notice = format!(
            "{id}.urdf and meshes/ were written by OpenAgents CAD Lab from the CAD document {id}, version {n} \
             (docs/cad-lab/README.md). They describe that design; whoever made it in CAD Lab decides how it may be used.\n"
        );
        let mut files: Vec<(String, Vec<u8>)> = vec![
            (description.clone(), urdf.text.into_bytes()),
            ("NOTICE".into(), notice.into_bytes()),
        ];
        files.extend(urdf.meshes);
        let mut pinned = Vec::new();
        for (path, bytes) in &files {
            std::fs::write(dir.join(path), bytes).map_err(|e| e.to_string())?;
            let hash = Sha256::digest(bytes);
            pinned.push(json!({"path": path, "sha256": hash.iter().map(|b| format!("{b:02x}")).collect::<String>()}));
        }
        write_json(
            &dir.join("source.json"),
            &json!({
                "repository": "https://github.com/OpenAgentsInc/openagents",
                "revision": robot_revision(document),
                "source_path": format!("CAD Lab {id} v{n} (crates/cad/src/urdf.rs)"),
                "license": "LicenseRef-CAD-Lab-design",
                "description": description,
                "notices": ["NOTICE"],
                "files": pinned,
            }),
        )?;
        let bytes = files[0].1.len() as u64;
        Ok((dir.join(&description), bytes))
    }

    /// Puts a version in a Verse zone: its glTF at a place on the ground,
    /// turned to a heading, in the placements file the zone loads
    /// ([`placements_path`]). A later version of the same document in the
    /// same zone replaces the earlier one.
    fn place(&self, args: &Value) -> Result<Value, String> {
        let (id, n, _, _) = self.pick(args)?;
        let zone = arg_str(args, "zone").unwrap_or("everglade");
        let wanted = zone.to_ascii_lowercase().replace([' ', '_'], "-");
        let Some((zone, label)) = VERSE_ZONES.iter().copied().find(|(world, label)| {
            *world == wanted
                || world.trim_start_matches("verse-") == wanted
                || label.to_ascii_lowercase().replace(' ', "-") == wanted
        }) else {
            return Err(format!(
                "CAD Lab places models in {}.",
                VERSE_ZONES
                    .iter()
                    .map(|(_, l)| *l)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        };
        let glb = self.version_dir(&id, n).join("model.glb");
        if !glb.is_file() {
            return Err(format!(
                "{id} v{n} has no glTF (model.glb); cad_check rebuilds it."
            ));
        }
        let at = match arg_json(args, "at")? {
            None => None,
            Some(v) => {
                let list: Vec<f64> = v
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_f64).collect())
                    .unwrap_or_default();
                match list.as_slice() {
                    [x, z] => Some([*x, 0.0, *z]),
                    [x, y, z] => Some([*x, *y, *z]),
                    _ => {
                        return Err(
                            "at is [x, z] or [x, y, z] in metres (y above the ground).".into()
                        );
                    }
                }
            }
        };
        if at.is_some_and(|p| p.iter().any(|x| !x.is_finite() || x.abs() > 5000.0)) {
            return Err("at must be within 5 km of the zone's centre.".into());
        }
        let heading = args["heading"].as_f64().unwrap_or(0.0);
        let scale = args["scale"].as_f64().unwrap_or(1.0);
        if !(0.01..=100.0).contains(&scale) || !heading.is_finite() {
            return Err("scale is between 0.01 and 100, heading in degrees.".into());
        }
        let remove = args["remove"].as_bool().unwrap_or(false);
        let _guard = self.writes.lock().map_err(|_| "The Lab is busy.")?;
        let path = placements_path(&self.root);
        let mut file =
            read_json(&path).unwrap_or_else(|| json!({"kind": PLACEMENTS, "placements": []}));
        let mut list: Vec<Value> = file["placements"].as_array().cloned().unwrap_or_default();
        let before = list.len();
        list.retain(|p| !(p["doc"] == id.as_str() && p["zone"] == zone));
        let replaced = list.len() < before;
        let placement = json!({
            "doc": id, "version": n, "zone": zone, "glb": glb,
            "at": at, "heading_deg": heading, "scale": scale,
            "placed": now(), "author": Self::author(args),
        });
        if !remove {
            list.push(placement.clone());
        }
        file["kind"] = json!(PLACEMENTS);
        file["placements"] = json!(list);
        write_json(&path, &file)?;
        let summary = if remove {
            format!("Took {id} out of {label}.")
        } else if replaced {
            format!("{id} v{n} replaces the earlier version in {label}.")
        } else {
            format!("{id} v{n} is in {label}.")
        };
        Ok(json!({
            "doc": id, "version": n, "zone": zone, "summary": summary,
            "placement": if remove { Value::Null } else { placement },
            "file": path,
            "open": format!("verse --{}", zone.trim_start_matches("verse-")),
        }))
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
            pitch: args["pitch"].as_f64(),
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

    /// A whole document from JSON: an `assembly.v1` (or `part.v1`), saved as
    /// the next version of `doc` (or its first). Part and subassembly
    /// templates may come from other documents' versions
    /// (`{"from": {"doc": "wheel", "version": 3, "part": "wheel"}}`); they
    /// are copied in and remember where they came from (`source`).
    fn from_json(&self, args: &Value) -> Result<Value, String> {
        let id = arg_str(args, "doc").ok_or("Name the document (doc), e.g. \"rover\".")?;
        if slug(id) != id {
            return Err(format!(
                "A document id is lowercase letters, digits and -, like {}.",
                slug(id)
            ));
        }
        let mut document = arg_json(args, "document")?
            .ok_or("Give the document: a whole assembly.v1 (or part.v1) as JSON.")?;
        if !document.is_object() {
            return Err("The document must be a JSON object.".into());
        }
        if document["kind"] == doc::PART {
            document["id"] = json!(id);
        } else {
            if document["kind"].is_null() {
                document["kind"] = json!(doc::ASSEMBLY);
            }
            if document["id"].is_null() {
                document["id"] = json!(id);
            }
            if document["parts"].is_null() {
                document["parts"] = json!({});
            }
            if document["instances"].is_null() {
                document["instances"] = json!([]);
            }
            self.resolve_templates(&mut document)?;
        }
        let summary = arg_str(args, "summary").map_or_else(
            || format!("From JSON: {}", document["title"].as_str().unwrap_or(id)),
            str::to_owned,
        );
        let base = self.latest(id);
        let (n, record) = self.save(id, &document, &summary, &Self::author(args), base)?;
        self.saved(id, n, &record)
    }

    /// The Solid API: direct operations measured and checked with no
    /// document or history; exports land in `ROOT/.solid/<digest>/`.
    fn solid(&self, args: &Value) -> Result<Value, String> {
        let ops = arg_list(args, "ops")?;
        if ops.is_empty() {
            return Err(
                "Give ops: a list of direct steps like [{\"op\":\"solid.box\",\"params\":{...}}]."
                    .into(),
            );
        }
        let sketches = arg_list(args, "sketches")?;
        let material = arg_str(args, "material").unwrap_or(doc::DEFAULT_MATERIAL);
        let mut eval = cad::solid(&ops, &sketches, material, &self.root.join(".parts"))
            .map_err(|p| format!("Those operations don't make a valid solid: {}", p.join(" ")))?;
        let key = digest(&json!({"ops": ops, "sketches": sketches, "material": material}));
        let mut files = Vec::new();
        for format in arg_list(args, "formats")?
            .iter()
            .filter_map(Value::as_str)
            .map(|f| f.trim_start_matches('.').to_lowercase())
        {
            let path = self
                .root
                .join(".solid")
                .join(&key)
                .join(format!("solid.{format}"));
            let bytes = cad::export::write(&mut eval.built, &format, &path)?;
            files.push(json!({"format": format, "path": path, "bytes": bytes}));
        }
        Ok(json!({
            "saved": false,
            "mass": eval.report["mass"],
            "size_mm": eval.report["size_mm"],
            "solids": eval.report["solids"],
            "checks": eval.report["checks"],
            "files": files,
        }))
    }

    /// A DXF drawing (its text) as a new sketch feature of a part: lines and
    /// arcs that touch become polylines, so the document holds small JSON.
    fn import_dxf(&self, args: &Value) -> Result<Value, String> {
        let text = arg_str(args, "dxf").ok_or("Give the drawing's DXF text (dxf).")?;
        let feature = arg_str(args, "feature")
            .ok_or("Name the new sketch (feature), e.g. \"profile\".")?
            .to_owned();
        let scale = args["scale"].as_f64().unwrap_or(1.0);
        let at = match arg_json(args, "at")? {
            Some(v) => {
                let a = v.as_array().filter(|a| a.len() == 2);
                match a.and_then(|a| Some([a[0].as_f64()?, a[1].as_f64()?])) {
                    Some(p) => p,
                    None => return Err("at is [x, y] in mm.".into()),
                }
            }
            None => [0.0, 0.0],
        };
        let draw = cad::dxf::import(text, scale, at)?;
        let count = draw.len();
        let plane = match &args["plane"] {
            Value::String(s) if s.trim_start().starts_with('{') => arg_json(args, "plane")?,
            Value::String(s) if !s.trim().is_empty() => Some(json!(s.trim())),
            Value::Object(_) => Some(args["plane"].clone()),
            _ => None,
        };
        let mut params = json!({"plane": plane.unwrap_or(json!("XY")), "draw": draw});
        if let Some(offset) = args.get("offset").filter(|v| !v.is_null()) {
            params["offset"] = offset.clone();
        }
        let after = arg_str(args, "after").map(str::to_owned);
        let mut out = self.change(args, |document| {
            let (part, found) = Self::part_of(document, args)?;
            let list = found["features"].as_array_mut().ok_or("bad part")?;
            if list.iter().any(|f| f["id"] == feature.as_str()) {
                return Err(format!("{part} already has a feature {feature}."));
            }
            let at = match &after {
                Some(id) => list
                    .iter()
                    .position(|f| f["id"] == id.as_str())
                    .map(|i| i + 1)
                    .ok_or_else(|| format!("{part} has no feature {id}."))?,
                None => list.len(),
            };
            list.insert(at, json!({"id": feature, "op": "sketch", "params": params}));
            Ok(format!("{part}: sketch {feature} from DXF"))
        })?;
        out["steps"] = json!(count);
        Ok(out)
    }

    /// A STEP model (AP203, AP214 or AP242) as a feature of a part: the file
    /// is kept in the Lab's file store by its digest and the feature names
    /// it, so later features (cuts, fillets, holes) build on it.
    fn import_step(&self, args: &Value) -> Result<Value, String> {
        let feature = arg_str(args, "feature")
            .ok_or("Name the new feature (feature), e.g. \"bracket\".")?
            .to_owned();
        let bytes = match (arg_str(args, "path"), arg_str(args, "step")) {
            (Some(path), None) => {
                let lower = path.to_lowercase();
                if !(lower.ends_with(".step") || lower.ends_with(".stp")) {
                    return Err("path must be a .step or .stp file.".into());
                }
                std::fs::read(path).map_err(|e| format!("{path}: {e}"))?
            }
            (None, Some(text)) => text.as_bytes().to_vec(),
            _ => return Err("Give the STEP file's path, or its text (step), not both.".into()),
        };
        if !String::from_utf8_lossy(&bytes[..bytes.len().min(64)]).contains("ISO-10303-21") {
            return Err("That isn't a STEP file (it doesn't start with ISO-10303-21).".into());
        }
        let file = cad::build::store_file(&self.root.join(".parts"), &bytes, "step")?;
        let after = arg_str(args, "after").map(str::to_owned);
        let mut out = self.change(args, |document| {
            let (part, found) = Self::part_of(document, args)?;
            let list = found["features"].as_array_mut().ok_or("bad part")?;
            if list.iter().any(|f| f["id"] == feature.as_str()) {
                return Err(format!("{part} already has a feature {feature}."));
            }
            let at = match &after {
                Some(id) => list
                    .iter()
                    .position(|f| f["id"] == id.as_str())
                    .map(|i| i + 1)
                    .ok_or_else(|| format!("{part} has no feature {id}."))?,
                None => list.len(),
            };
            list.insert(
                at,
                json!({"id": feature, "op": "solid.import_step", "params": {"file": file}}),
            );
            Ok(format!("{part}: {feature} from a STEP file"))
        })?;
        out["file"] = json!(file);
        Ok(out)
    }

    /// DXF out: a sketch of a part, or the whole version cut by a plane.
    fn export_dxf(&self, args: &Value) -> Result<Value, String> {
        let (id, n, document, _) = self.pick(args)?;
        let (text, entities, name) = match arg_str(args, "sketch") {
            Some(sketch) => {
                let part = arg_str(args, "part").ok_or("Name the part (part) the sketch is in.")?;
                let (text, entities) =
                    cad::build::sketch_dxf(&document, part, sketch, &self.root.join(".parts"))
                        .map_err(|p| p.join(" "))?;
                (text, entities, format!("{part}-{sketch}"))
            }
            None => {
                let plane = arg_str(args, "plane").unwrap_or("XY").to_uppercase();
                let at = args["at"].as_f64().unwrap_or(0.0);
                let eval = cad::evaluate_with(&document, &self.root.join(".parts"))
                    .map_err(|p| p.join(" "))?;
                let (text, lines) = cad::dxf::section(&eval.built, &plane, at)?;
                (text, lines as u64, format!("section-{plane}-{at}"))
            }
        };
        let file = format!("{id}-v{n}-{name}.dxf");
        let dir = self.version_dir(&id, n).join("out");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(&file), &text).map_err(|e| e.to_string())?;
        Ok(
            json!({"doc": id, "version": n, "path": dir.join(&file), "entities": entities, "bytes": text.len(),
                  "url": format!("{}/api/out?doc={id}&v={n}&name={file}", self.base)}),
        )
    }

    /// A 2D drawing of a version: its views projected with visible and
    /// hidden lines, sections, dimensions and a title block, as SVG, PDF
    /// and DXF files.
    fn drawing(&self, args: &Value) -> Result<Value, String> {
        let (id, n, document, _) = self.pick(args)?;
        let formats: Vec<String> = match arg_list(args, "formats")? {
            list if !list.is_empty() => list
                .iter()
                .filter_map(|f| f.as_str().map(|s| s.trim_start_matches('.').to_lowercase()))
                .collect(),
            _ => vec!["svg".into(), "pdf".into(), "dxf".into()],
        };
        if let Some(bad) = formats
            .iter()
            .find(|f| !["svg", "pdf", "dxf"].contains(&f.as_str()))
        {
            return Err(format!("A drawing is svg, pdf or dxf, not {bad}."));
        }
        let mut options = args.clone();
        options["subtitle"] = json!(format!("{id} version {n}"));
        let drawing = cad::drawing::of_document(&document, &options, &self.root.join(".parts"))
            .map_err(|p| p.join(" "))?;
        let dir = self.version_dir(&id, n).join("out");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut files = Vec::new();
        for format in &formats {
            let file = format!("{id}-v{n}-drawing.{format}");
            let bytes = drawing.bytes(format)?;
            std::fs::write(dir.join(&file), &bytes).map_err(|e| e.to_string())?;
            files.push(
                json!({"format": format, "path": dir.join(&file), "bytes": bytes.len(),
                              "url": format!("{}/api/out?doc={id}&v={n}&name={file}", self.base)}),
            );
        }
        let mut card = self.card(&id, n, None)?;
        if formats.iter().any(|f| f == "svg") {
            card["drawing"] = json!({"file": format!("{id}-v{n}-drawing.svg")});
        }
        card["exports"] = json!(files);
        Ok(
            json!({"doc": id, "version": n, "drawing": drawing.report(), "files": files, "card": card}),
        )
    }

    /// A part's faces and edges by the names features can use for them.
    fn faces(&self, args: &Value) -> Result<Value, String> {
        let (id, n, document, _) = self.pick(args)?;
        let part = arg_str(args, "part").ok_or("Name the part (part).")?;
        let mut out = cad::build::faces_of(
            &document,
            part,
            arg_str(args, "after"),
            &self.root.join(".parts"),
        )
        .map_err(|p| p.join(" "))?;
        out["doc"] = json!(id);
        out["version"] = json!(n);
        out["how"] = json!(
            "Name a face as \"feature:role\" in faces lists (\"body:top\"); an edge as {\"between\": [face, face]} or {\"of\": face} in edges lists."
        );
        Ok(out)
    }

    /// Moves one joint through a range with the loops solved at each step
    /// (the others following, joint limits held) and says how far it gets;
    /// nothing is saved.
    fn motion(&self, args: &Value) -> Result<Value, String> {
        let (id, n, document, _) = self.pick(args)?;
        let joint = arg_str(args, "joint").ok_or("Name the joint to move (joint).")?;
        let built = cad::build::build_with(&document, &self.root.join(".parts"))
            .map_err(|p| p.join(" "))?;
        let found = built.assembly["joints"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|j| j["id"] == joint && !cad::solve::closes(j))
            .ok_or_else(|| format!("There's no joint {joint} to move."))?;
        let number = |key: &str| -> Result<Option<f64>, String> {
            match &args[key] {
                Value::Null => Ok(None),
                Value::Number(x) => Ok(x.as_f64()),
                Value::String(s) if s.trim().is_empty() => Ok(None),
                Value::String(s) => s
                    .trim()
                    .parse()
                    .map(Some)
                    .map_err(|_| format!("{key} is a number (got {s}).")),
                other => Err(format!("{key} is a number (got {other}).")),
            }
        };
        let current = doc::joint_values(found).first().copied().unwrap_or(0.0);
        let from = number("from")?.unwrap_or(current);
        let to = match number("to")? {
            Some(t) => t,
            None => found["limits"][0][1]
                .as_f64()
                .ok_or_else(|| format!("Say where to move {joint} to (to); it has no limits."))?,
        };
        let steps = arg_u64(args, "steps").unwrap_or(36).clamp(1, 720) as usize;
        let mut out = cad::solve::sweep(&built.assembly, joint, from, to, steps);
        out["doc"] = json!(id);
        out["version"] = json!(n);
        Ok(out)
    }

    /// Part and subassembly templates that point at another document's
    /// version, copied in.
    fn resolve_templates(&self, document: &mut Value) -> Result<(), String> {
        let source = |from: &Value, key: &str| -> Result<(String, u64, Value), String> {
            let doc_id = from["doc"]
                .as_str()
                .ok_or_else(|| format!("{key}'s from needs a doc."))?;
            let n = match arg_u64(from, "version") {
                Some(n) => n,
                None => self
                    .latest(doc_id)
                    .ok_or_else(|| format!("There's no CAD document called {doc_id}."))?,
            };
            let (found, _) = self
                .load(doc_id, n)
                .ok_or_else(|| format!("{doc_id} has no version {n}."))?;
            Ok((doc_id.to_owned(), n, found))
        };
        let mut extra_parts: Vec<(String, Value)> = Vec::new();
        let keys: Vec<String> = document["parts"]
            .as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        for key in keys {
            let from = document["parts"][&key]["from"].clone();
            if !from.is_object() {
                continue;
            }
            let (doc_id, n, found) = source(&from, &key)?;
            let name = from["part"].as_str().unwrap_or(&key).to_owned();
            let mut part = if found["kind"] == doc::PART {
                found
            } else {
                found["parts"]
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| format!("{doc_id} v{n} has no part {name}."))?
            };
            part["id"] = json!(key);
            part["source"] = json!({"doc": doc_id, "version": n, "part": name});
            document["parts"][&key] = part;
        }
        let keys: Vec<String> = document["assemblies"]
            .as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        for key in keys {
            let from = document["assemblies"][&key]["from"].clone();
            if !from.is_object() {
                continue;
            }
            let (doc_id, n, found) = source(&from, &key)?;
            if found["kind"] != doc::ASSEMBLY {
                return Err(format!(
                    "{doc_id} is a single part; place it as a part template instead."
                ));
            }
            let mut template = json!({"title": found["title"], "instances": found["instances"],
                "source": {"doc": doc_id, "version": n}});
            for field in ["joints", "contacts"] {
                if found[field].as_array().is_some_and(|a| !a.is_empty()) {
                    template[field] = found[field].clone();
                }
            }
            document["assemblies"][&key] = template;
            for (name, part) in found["parts"].as_object().into_iter().flatten() {
                extra_parts.push((name.clone(), part.clone()));
            }
            for (name, inner) in found["assemblies"].as_object().into_iter().flatten() {
                if document["assemblies"].get(name).is_none() {
                    document["assemblies"][name] = inner.clone();
                }
            }
        }
        for (name, part) in extra_parts {
            match document["parts"].get(&name) {
                None => document["parts"][&name] = part,
                Some(existing) if *existing == part => {}
                Some(_) => {
                    return Err(format!(
                        "Two different parts are called {name}; rename one before bringing them together."
                    ));
                }
            }
        }
        Ok(())
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
        if x.max(y) < 1.0 {
            out.push(format!("Mass {:.2} g → {:.2} g", x * 1000.0, y * 1000.0));
        } else {
            out.push(format!("Mass {x:.3} kg → {y:.3} kg"));
        }
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
            "finish",
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
            "finish",
            "parameters",
            "remove",
            "from_version",
            "exact_mass",
            "summary",
            "author",
        ],
        "cad_assemble" => &[
            "doc",
            "instances",
            "joints",
            "contacts",
            "templates",
            "remove",
            "drive",
            "summary",
            "author",
        ],
        "cad_from_json" => &["doc", "document", "summary", "author"],
        "cad_faces" => &["doc", "version", "part", "after"],
        "cad_motion" => &["doc", "version", "joint", "from", "to", "steps"],
        "cad_solid" => &["ops", "sketches", "material", "formats"],
        "cad_import_dxf" => &[
            "doc", "part", "feature", "dxf", "plane", "offset", "scale", "at", "after", "summary",
            "author",
        ],
        "cad_export_dxf" => &["doc", "version", "part", "sketch", "plane", "at"],
        "cad_drawing" => &[
            "doc",
            "version",
            "views",
            "sections",
            "angle",
            "sheet",
            "title",
            "hidden",
            "dimensions",
            "formats",
        ],
        "cad_import_step" => &[
            "doc", "part", "feature", "path", "step", "after", "summary", "author",
        ],
        "cad_versions" => &["doc"],
        "cad_compare" => &["doc", "a", "b"],
        "cad_export" => &["doc", "version", "formats"],
        "cad_place" => &[
            "doc", "version", "zone", "at", "heading", "scale", "remove", "author",
        ],
        "cad_render" => &[
            "doc", "version", "view", "focus", "yaw", "pitch", "zoom", "width", "height",
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
    let materials = format!("one of: {}", doc::MATERIALS.map(|(n, _)| n).join(", "));
    let finish = json!({"type": "string", "description": format!(
        "how the surface takes the light: {} (moulded toy plastic is gloss)",
        doc::FINISHES.map(|(n, _)| n).join(", ")
    )});
    json!({"tools": [
        {"name": "cad_list", "description": "List the CAD documents in CAD Lab on this computer.",
         "parameters": {"type": "object", "properties": {}}},
        {"name": "cad_open", "description": "Open a CAD document: its whole assembly.v1 document (parts with their feature histories, instances, joints, contacts), its mass properties and checks, and a card with a 3D view.",
         "parameters": {"type": "object", "required": ["doc"], "properties": {"doc": doc, "version": version}}},
        {"name": "cad_new_part", "description": "Add a new part to a CAD assembly (making the document when it doesn't exist yet), with its material, colour and first features, and place one instance of it (id = the part id) unless place is false. Saved as a new version only if every check passes.",
         "parameters": {"type": "object", "required": ["doc", "part", "summary"], "properties": {
            "doc": doc, "doc_title": {"type": "string", "description": "the document's title, when making it"},
            "part": part, "title": {"type": "string"},
            "material": {"type": "string", "description": materials},
            "color": {"type": "string", "description": "#rrggbb"},
            "finish": finish,
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
        {"name": "cad_set_part", "description": "Change a part's title, material, colour or finish, have its mass checked against SMLib's exact one on every edit (exact_mass), bring a part back to how it was in an earlier version (from_version), or remove the part and its instances (remove: true).",
         "parameters": {"type": "object", "required": ["doc", "part", "summary"], "properties": {
            "doc": doc, "part": part, "title": {"type": "string"}, "material": {"type": "string", "description": materials},
            "color": {"type": "string", "description": "#rrggbb"}, "finish": finish,
            "remove": {"type": "boolean"},
            "from_version": {"type": "integer", "description": "bring the part back to how it was in this version"},
            "exact_mass": {"type": "string", "description": "true to cross-check this part's mass properties against SMLib's exact ones on every edit (1%), {\"tolerance\":0.002} for another bound, false to stop"},
            "summary": summary}}},
        {"name": "cad_assemble", "description": "Place, move or remove instances of parts or subassemblies, add or set joints (fastened, revolute, slider, cylindrical, planar, spherical, gear; a joint's value opens a lid or turns a wheel), define subassembly templates, and declare contacts between instances that are meant to touch. Instances and joints with an existing id are updated (only the fields given). Checked like every edit: overlaps between instances that aren't declared contacts or joined by a joint are refused.",
         "parameters": {"type": "object", "required": ["doc", "summary"], "properties": {
            "doc": doc,
            "instances": {"type": "string", "description": "a JSON array: [{\"id\":\"wheel_lf\",\"part\":\"wheel\",\"parent\":\"bogie_l\",\"at\":[x,y,z],\"rotate\":[rx,ry,rz]}]; {\"assembly\":\"bogie\"} instead of part places a copy of a subassembly template, whose instances are then named by path (bogie_l.wheel); \"pattern\":{\"count\":6,\"step\":[x,y,z]} or {\"count\":6,\"axis\":[0,0,1],\"origin\":[x,y,z]} makes copies id-2, id-3, ... in a row, grid (count2, step2) or ring"},
            "joints": {"type": "string", "description": "a JSON array: [{\"id\":\"lid_hinge\",\"type\":\"revolute\",\"parent\":\"bay\",\"child\":\"lid\",\"origin\":[x,y,z],\"axis\":[x,y,z],\"value\":0,\"limits\":[[0,110]]}] (origin and axis in the parent's frame; parent must be the child's parent instance). slider: value mm along axis; cylindrical: [deg, mm]; planar: [u mm, v mm, deg] in the plane normal to axis (u along x_dir); spherical: [rx, ry, rz] deg about origin (no axis); gear: no value, follows driver (a joint id) times ratio (negative for meshing gears; degrees per mm when the driver is a slider). Loops: {\"id\":\"c\",\"type\":\"revolute\",\"closes\":true,\"parent\":\"coupler\",\"child\":\"rocker\",\"origin\":[x,y,z],\"axis\":[0,0,1],\"child_origin\":[x,y,z]} holds two instances the tree already places together (a four-bar's last pin; types fastened, revolute, slider, cylindrical, planar, spherical, parallel; no parent = the assembly's origin), and {\"type\":\"gear\",\"closes\":true,\"driver\":\"a\",\"driven\":\"b\",\"ratio\":2} couples two joints; the joints around a loop then follow, and \"drive\":true on one holds it. Mates: origin, axis, child_origin and child_axis may name a face or edge of that instance's part instead of numbers: {\"face\":\"base:+z\"} (its centre and normal, a round face's axis) or {\"edge\":[\"base:+z\",\"base:+x\"]}, \"flip\":true to turn the direction round"},
            "templates": {"type": "string", "description": "a JSON object of subassembly templates by id: {\"bogie\":{\"instances\":[...],\"joints\":[...],\"contacts\":[...]}} (their instances place this document's parts), or {\"bogie\":{\"from\":{\"doc\":\"bogie\",\"version\":2}}} to copy another document in; null removes one"},
            "contacts": {"type": "string", "description": "a JSON array of instance pairs meant to touch: [[\"a\",\"b\"]]"},
            "remove": {"type": "string", "description": "a JSON array of instance ids to remove"},
            "drive": {"type": "string", "description": "a JSON object {\"joint\":\"crank\",\"value\":45}: set that joint, hold it, and have the other joints of its loops follow (the report's solver says where each went, the degrees of freedom left, and any loop that can't close)"},
            "summary": summary}}},
        {"name": "cad_from_json", "description": "Make or replace a whole CAD document from JSON in one step: an assembly.v1 with its part templates (parts), subassembly templates (assemblies), instances, joints and contacts, or a single part.v1. A part or subassembly template may be {\"from\":{\"doc\":\"wheel\",\"version\":3,\"part\":\"wheel\"}} to copy it from another document's version. Checked like every edit and saved as the next version.",
         "parameters": {"type": "object", "required": ["doc", "document", "summary"], "properties": {
            "doc": doc,
            "document": {"type": "string", "description": "the whole document as JSON"},
            "summary": summary}}},
        {"name": "cad_faces", "description": "A part's faces and edges by the names features use to pick them (stable through edits, unlike points): faces like body:top, body:start, body:side:l3 (the side swept from sketch curve l3), boss:+z, round:blend:...; edges as the two faces they join. Use it before a fillet, chamfer, shell or draft to name what it works on.",
         "parameters": {"type": "object", "required": ["doc", "part"], "properties": {"doc": doc, "version": version, "part": part,
            "after": {"type": "string", "description": "list them as they stand after this feature (default: after the whole history)"}}}},
        {"name": "cad_motion", "description": "Move a mechanism: turn or slide one joint through a range with every loop closed at each step (the other joints follow, within their limits) and see how far it gets and where each joint goes; it stops where a limit or a loop that can't close stops it. Nothing is saved; cad_assemble with drive sets a position.",
         "parameters": {"type": "object", "required": ["doc", "joint"], "properties": {"doc": doc, "version": version,
            "joint": {"type": "string", "description": "the joint to move, e.g. crank"},
            "from": {"type": "number", "description": "where to start (default: its value now)"},
            "to": {"type": "number", "description": "where to end (default: its upper limit)"},
            "steps": {"type": "integer", "description": "how many steps (default 36)"}}}},
        {"name": "cad_solid", "description": "Direct solid modelling with no document: run direct operations (boxes, cylinders, booleans, moves, extrudes of the sketches given, ...) and get the solid's mass, size and checks back, optionally as files. Nothing is saved as a version. A step may repeat: {\"op\":\"solid.cylinder\",\"repeat\":{\"var\":\"i\",\"from\":0,\"to\":7},\"params\":{\"base\":[\"{i} * 12\",0,0],...}}.",
         "parameters": {"type": "object", "required": ["ops"], "properties": {
            "ops": {"type": "string", "description": "a JSON array of direct steps: [{\"op\":\"solid.box\",\"params\":{...}}, ...]"},
            "sketches": {"type": "string", "description": "a JSON array of sketch features the steps extrude or revolve"},
            "material": {"type": "string", "description": materials},
            "formats": {"type": "string", "description": "a JSON array of files to write: step, 3mf, glb, stl"}}}},
        {"name": "cad_import_dxf", "description": "Bring a 2D DXF drawing into a part as a new sketch feature: its lines and arcs become polylines (bulged points for arcs), with its circles, splines and text, as plain JSON the part keeps; extrude or revolve it next. Checked and saved like every edit.",
         "parameters": {"type": "object", "required": ["doc", "part", "feature", "dxf", "summary"], "properties": {
            "doc": doc, "part": part,
            "feature": {"type": "string", "description": "the new sketch's id, e.g. profile"},
            "dxf": {"type": "string", "description": "the drawing's ASCII DXF text"},
            "plane": {"type": "string", "description": "XY (default), YZ, or a frame {origin, x_dir, y_dir} as JSON"},
            "offset": {"type": "number", "description": "mm along the plane's normal"},
            "scale": {"type": "number", "description": "multiply the drawing by this (default 1)"},
            "at": {"type": "string", "description": "move the drawing by [x, y] mm, as JSON"},
            "after": {"type": "string", "description": "insert after this feature id"},
            "summary": summary}}},
        {"name": "cad_import_step", "description": "Bring a STEP model (AP203, AP214 or AP242, from any CAD) into a part as a feature: its exact solids, which later features can cut, fillet or join. The file is kept with the Lab by its digest.",
         "parameters": {"type": "object", "required": ["doc", "part", "feature", "summary"], "properties": {
            "doc": doc, "part": part,
            "feature": {"type": "string", "description": "the new feature's id, e.g. bracket"},
            "path": {"type": "string", "description": "a .step or .stp file on this computer"},
            "step": {"type": "string", "description": "or the STEP file's text"},
            "after": {"type": "string", "description": "insert after this feature id"},
            "summary": summary}}},
        {"name": "cad_export_dxf", "description": "A 2D DXF drawing out of a version: one sketch of a part (part and sketch), or the whole model cut by a plane (plane XY, XZ or YZ at a distance in mm), for laser cutting, CNC or another CAD's 2D tools.",
         "parameters": {"type": "object", "required": ["doc"], "properties": {"doc": doc, "version": version,
            "part": part, "sketch": {"type": "string", "description": "the sketch feature's id"},
            "plane": {"type": "string", "description": "XY (at a height), XZ (at a y) or YZ (at an x)"},
            "at": {"type": "number", "description": "where the plane cuts, mm"}}}},
        {"name": "cad_drawing", "description": "A 2D engineering drawing of a version: front, top, right and iso views (or those named) projected from the exact model, visible lines solid and hidden ones dashed, curved faces' silhouettes, section views cut through it and hatched, the overall sizes, hole diameters and distances between named flat faces, and a title block, on an A4, A3 or A2 sheet at one standard scale; as SVG, PDF and DXF (a layer per kind of line). The card shows it.",
         "parameters": {"type": "object", "required": ["doc"], "properties": {"doc": doc, "version": version,
            "views": {"type": "string", "description": "a JSON array or comma list of front, top, right, left, bottom, back, iso; default front, top, right, iso"},
            "sections": {"type": "string", "description": "a JSON array of sections, each the view it is seen like and where it cuts in mm along that view's direction, e.g. [{\"view\":\"front\",\"at\":0}] or [\"front@0\"]; through the middle when at is left out"},
            "angle": {"type": "string", "description": "third (top view above the front, the default) or first (below)"},
            "sheet": {"type": "string", "description": "A4, A3 (default) or A2"},
            "title": {"type": "string", "description": "the title block's title; the document's title when left out"},
            "hidden": {"type": "boolean", "description": "false leaves the hidden lines out"},
            "dimensions": {"type": "string", "description": "a JSON array of distances between two parallel flat faces, named like cad_faces names them, e.g. [{\"part\":\"plate\",\"faces\":[\"body:start\",\"body:top\"],\"view\":\"front\",\"label\":\"H\"}]"},
            "formats": {"type": "string", "description": "a JSON array of svg, pdf, dxf; default all three"}}}},
        {"name": "cad_check", "description": "Run the checks on a version again: every feature rebuilds, every solid is closed, no undeclared overlaps; with its mass, centre of mass and inertia.",
         "parameters": {"type": "object", "required": ["doc"], "properties": {"doc": doc, "version": version}}},
        {"name": "cad_versions", "description": "The versions of a CAD document: each one's summary, author, mass and part count.",
         "parameters": {"type": "object", "required": ["doc"], "properties": {"doc": doc}}},
        {"name": "cad_compare", "description": "What changed between two versions: parts, features, instances, joints, and the mass and centre of mass. a defaults to the version before b; b to the latest.",
         "parameters": {"type": "object", "required": ["doc"], "properties": {"doc": doc,
            "a": {"type": "integer", "description": "the first version, e.g. 1"}, "b": {"type": "integer", "description": "the second version; the latest when left out"}}}},
        {"name": "cad_export", "description": "Export a version as files: glb (glTF, for the web and Verse), 3mf (for printing), step (exact solids for other CAD), stl, urdf (a robot for Robot Lab: a link per instance with its mass and inertia, the joints with their limits) and feed (the mass-properties record for Lagrange 2's spacecraft document). Default: glb, 3mf and step.",
         "parameters": {"type": "object", "required": ["doc"], "properties": {"doc": doc, "version": version,
            "formats": {"type": "string", "description": "a JSON array of formats, e.g. [\"step\",\"3mf\",\"glb\",\"urdf\",\"feed\"]"}}}},
        {"name": "cad_place", "description": "Put a version in a Verse zone (Everglade, Meteor Showcase or Meteor Stress Test): its glTF stands on the ground there at a place and heading. Placing another version of the same document in the same zone replaces it; remove takes it out.",
         "parameters": {"type": "object", "required": ["doc"], "properties": {"doc": doc, "version": version,
            "zone": {"type": "string", "description": "everglade (default), meteor-showcase or meteor-stress-test"},
            "at": {"type": "string", "description": "where, as JSON [x, z] or [x, y, z] in metres (y above the ground); in front of where you arrive when left out"},
            "heading": {"type": "number", "description": "which way it faces, degrees about the vertical (0 faces +X)"},
            "scale": {"type": "number", "description": "1 is life size"},
            "remove": {"type": "boolean", "description": "take it out of the zone"}}}},
        {"name": "cad_render", "description": "A picture of a version from a view: hero, rear, plan, front, side, iso, under or detail; focus frames some instances (e.g. [\"camera_head\"]). The card shows it.",
         "parameters": {"type": "object", "required": ["doc"], "properties": {"doc": doc, "version": version,
            "view": {"type": "string", "description": "hero, rear, plan, front, side, iso, under or detail"},
            "focus": {"type": "string", "description": "a JSON array of instance ids to frame"},
            "zoom": {"type": "number"}, "yaw": {"type": "number", "description": "extra turn, degrees"},
            "pitch": {"type": "number", "description": "the camera's height above the floor in degrees (0 level, 90 straight down), instead of the view's own"}}}}
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
        // A robot for Robot Lab (a pinned folder) and the mass feed.
        let robot = lab.run(
            "cad_export",
            &json!({"doc": "cart", "formats": ["urdf", "feed"]}),
        );
        assert_eq!(
            robot["files"][0]["robot_lab"]["source"], "cad:cart/v5",
            "{robot}"
        );
        let urdf_dir = lab.version_dir("cart", 5).join("out/urdf");
        let pin = read_json(&urdf_dir.join("source.json")).unwrap();
        assert_eq!(pin["revision"], robot["files"][0]["robot_lab"]["revision"]);
        for file in pin["files"].as_array().unwrap() {
            let bytes = std::fs::read(urdf_dir.join(file["path"].as_str().unwrap())).unwrap();
            let hash: String = Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            assert_eq!(file["sha256"], hash.as_str());
        }
        let text = std::fs::read_to_string(urdf_dir.join("cart.urdf")).unwrap();
        assert!(
            text.contains("<link name=\"w4\">") && text.contains("<mass value="),
            "{text}"
        );
        let feed: cad::feed::MassFeed = serde_json::from_value(
            read_json(Path::new(robot["files"][1]["path"].as_str().unwrap())).unwrap(),
        )
        .unwrap();
        assert_eq!(feed.parts.len(), 5);
        assert_eq!(feed.source.version, 5);
        assert!(
            lab.run("cad_export", &json!({"doc": "cart", "formats": ["dwg"]}))["error"].is_string()
        );
        // Into Verse: a version at a place, then the latest replaces it.
        let placed = lab.run(
            "cad_place",
            &json!({"doc": "cart", "version": 4, "at": "[3, 4]", "heading": 30}),
        );
        assert_eq!(placed["zone"], "verse-everglade", "{placed}");
        assert_eq!(placed["placement"]["at"], json!([3.0, 0.0, 4.0]));
        let again = lab.run("cad_place", &json!({"doc": "cart", "zone": "Everglade"}));
        assert!(
            again["summary"].as_str().unwrap().contains("replaces"),
            "{again}"
        );
        let file = read_json(&placements_path(&lab.root)).unwrap();
        assert_eq!(file["kind"], PLACEMENTS);
        assert_eq!(file["placements"].as_array().unwrap().len(), 1);
        assert_eq!(file["placements"][0]["version"], 5);
        assert!(
            lab.run("cad_place", &json!({"doc": "cart", "zone": "plaza"}))["error"].is_string()
        );
        let removed = lab.run("cad_place", &json!({"doc": "cart", "remove": true}));
        assert!(
            removed["summary"].as_str().unwrap().contains("Took"),
            "{removed}"
        );
        let file = read_json(&placements_path(&lab.root)).unwrap();
        assert!(file["placements"].as_array().unwrap().is_empty());
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
        // A parent named plainly, as the tool declares it (a string).
        let cap = lab.run("cad_new_part", &json!({"doc": "cart", "part": "cap", "material": "ABS Plastic", "parent": "w1", "at": "[0,0,0]",
            "features": [{"id": "disc", "op": "solid.cylinder", "params": {"radius": 10, "height": 2, "axis": "Y", "base": [0, 10, 0]}}],
            "summary": "A hub cap on the first wheel"}));
        assert_eq!(cap["saved"], true, "{cap}");
        let (doc6, _) = lab.load("cart", 6).unwrap();
        let placed = doc6["instances"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["id"] == "cap")
            .cloned()
            .unwrap();
        assert_eq!(placed["parent"], "w1");
    }

    #[test]
    fn assemblies_come_from_json_with_templates_from_other_documents() {
        let (_dir, lab) = lab();
        let cube = json!({"kind": "part.v1", "id": "cube", "title": "Cube", "material": "Steel",
            "features": [{"id": "b", "op": "solid.box", "params": {"length": 10, "width": 10, "height": 10, "center": [0, 0, 5]}}]});
        let made = lab.run(
            "cad_from_json",
            &json!({"doc": "cube", "document": cube, "summary": "A cube"}),
        );
        assert_eq!(made["saved"], true, "{made}");
        // The Solid API: measured and exported, with no version saved.
        let direct = lab.run("cad_solid", &json!({"material": "Steel", "formats": ["step"],
            "ops": [{"op": "solid.box", "params": {"length": 10, "width": 10, "height": 10, "center": [0, 0, 0]}}]}));
        assert!(
            (direct["mass"]["volume_cm3"].as_f64().unwrap_or_default() - 1.0).abs() < 1e-3,
            "{direct}"
        );
        assert!(
            direct["files"][0]["bytes"].as_u64().unwrap_or_default() > 100,
            "{direct}"
        );
        assert_eq!(lab.latest("solid"), None);
        let faces = lab.run("cad_faces", &json!({"doc": "cube", "part": "cube"}));
        assert!(faces.to_string().contains("\"b:+z\""), "{faces}");
        // Its top edges rounded by name.
        let rounded = lab.run("cad_add_feature", &json!({"doc": "cube", "part": "cube", "summary": "Round the top",
            "features": [{"id": "round", "op": "solid.fillet", "params": {"radius": 1, "edges": [{"of": "b:+z"}]}}]}));
        assert_eq!(rounded["version"], 2, "{rounded}");
        // A hinge subassembly made of that cube, then two copies of it.
        let hinge = json!({"title": "Hinge", "parts": {"cube": {"from": {"doc": "cube"}}},
            "instances": [{"id": "frame", "part": "cube"}, {"id": "lid", "part": "cube", "parent": "frame", "at": [0, 0, 10]}],
            "joints": [{"id": "pin", "type": "revolute", "parent": "frame", "child": "lid", "origin": [5, 0, 10], "axis": [0, 1, 0], "value": 0}]});
        let made = lab.run(
            "cad_from_json",
            &json!({"doc": "hinge", "document": hinge.to_string(), "summary": "A hinge"}),
        );
        assert_eq!(made["saved"], true, "{made}");
        let (saved, _) = lab.load("hinge", 1).unwrap();
        assert_eq!(saved["parts"]["cube"]["source"]["doc"], "cube");
        assert_eq!(saved["parts"]["cube"]["source"]["version"], 2);
        let pair = json!({"title": "Pair", "assemblies": {"hinge": {"from": {"doc": "hinge", "version": 1}}},
            "instances": [{"id": "left", "assembly": "hinge", "at": [0, 40, 0]}, {"id": "right", "assembly": "hinge", "at": [0, -40, 0]}]});
        let made = lab.run(
            "cad_from_json",
            &json!({"doc": "pair", "document": pair, "summary": "Two hinges"}),
        );
        assert_eq!(made["saved"], true, "{made}");
        assert_eq!(made["instances"], 4, "{made}");
        // Opening the right lid through the template's joint, by a new template.
        let (mut doc1, _) = lab.load("pair", 1).unwrap();
        doc1["assemblies"]["hinge"]["joints"][0]["value"] = json!(45);
        let opened = lab.run(
            "cad_assemble",
            &json!({"doc": "pair", "summary": "Lids open",
            "templates": {"hinge": doc1["assemblies"]["hinge"]}}),
        );
        assert_eq!(opened["version"], 2, "{opened}");
        // A subassembly that isn't there is refused.
        let bad = lab.run(
            "cad_assemble",
            &json!({"doc": "pair", "summary": "x",
            "instances": [{"id": "third", "assembly": "nope"}]}),
        );
        assert!(
            bad["error"].as_str().unwrap_or_default().contains("nope"),
            "{bad}"
        );
    }

    #[test]
    fn dxf_drawings_come_in_as_sketches_and_go_out_as_sections() {
        let (_dir, lab) = lab();
        let made = lab.run("cad_new_part", &json!({"doc": "gasket", "part": "gasket", "summary": "A gasket",
            "features": [{"id": "stub", "op": "solid.box", "params": {"length": 1, "width": 1, "height": 1, "center": [100, 100, 0.5]}}]}));
        assert_eq!(made["saved"], true, "{made}");
        let drawing = "0\nSECTION\n2\nENTITIES\n\
            0\nLINE\n8\n0\n10\n0\n20\n0\n11\n30\n21\n0\n\
            0\nLINE\n8\n0\n10\n30\n20\n0\n11\n30\n21\n20\n\
            0\nLINE\n8\n0\n10\n30\n20\n20\n11\n0\n21\n20\n\
            0\nLINE\n8\n0\n10\n0\n20\n20\n11\n0\n21\n0\n\
            0\nENDSEC\n0\nEOF\n";
        let imported = lab.run(
            "cad_import_dxf",
            &json!({"doc": "gasket", "part": "gasket", "feature": "outline",
            "dxf": drawing, "plane": "XY", "summary": "Outline from DXF"}),
        );
        assert_eq!(imported["version"], 2, "{imported}");
        let (doc2, _) = lab.load("gasket", 2).unwrap();
        let draw = &doc2["parts"]["gasket"]["features"][1]["params"]["draw"];
        assert_eq!(draw[0]["op"], "sketch.polyline", "{draw}");
        assert_eq!(draw[0]["params"]["closed"], true, "{draw}");
        let extruded = lab.run("cad_add_feature", &json!({"doc": "gasket", "part": "gasket", "summary": "Plate",
            "features": [{"id": "plate", "op": "solid.extrude", "params": {"sketch": "outline", "distance": 2, "operation": "new"}}]}));
        assert_eq!(extruded["version"], 3, "{extruded}");
        let sketch = lab.run(
            "cad_export_dxf",
            &json!({"doc": "gasket", "part": "gasket", "sketch": "outline"}),
        );
        assert!(sketch["entities"].as_u64().unwrap_or(0) >= 4, "{sketch}");
        let cut = lab.run(
            "cad_export_dxf",
            &json!({"doc": "gasket", "plane": "XY", "at": 0.25}),
        );
        assert!(cut["entities"].as_u64().unwrap_or(0) >= 4, "{cut}");
        let text = std::fs::read_to_string(cut["path"].as_str().unwrap()).unwrap();
        assert!(text.contains("ENTITIES"));
        // The gasket as STEP, back into another part as a feature.
        let exported = lab.run("cad_export", &json!({"doc": "gasket", "formats": ["step"]}));
        let path = exported["files"][0]["path"].as_str().unwrap().to_owned();
        let imported = lab.run(
            "cad_import_step",
            &json!({"doc": "gasket", "part": "gasket", "feature": "again",
            "path": path, "summary": "The gasket again, from STEP"}),
        );
        assert_eq!(imported["version"], 4, "{imported}");
        assert!(imported["file"].as_str().unwrap().starts_with("sha256:"));
        let bad = lab.run(
            "cad_import_step",
            &json!({"doc": "gasket", "part": "gasket", "feature": "x",
            "step": "not a model", "summary": "x"}),
        );
        assert!(
            bad["error"].as_str().unwrap().contains("isn't a STEP file"),
            "{bad}"
        );
    }

    #[test]
    fn a_linkage_is_driven_and_moved_through_its_range() {
        let (_dir, lab) = lab();
        let bar = |id: &str, length: f64, z: f64| {
            json!({"kind": "part.v1", "id": id, "material": "Aluminum", "features": [{"id": "bar", "op": "solid.box",
                "params": {"length": length + 10.0, "width": 10, "height": 6, "center": [length / 2.0, 0, z + 3.0]}}]})
        };
        let document = json!({
            "parts": {"ground": bar("ground", 100.0, 0.0), "crank": bar("crank", 40.0, 6.0),
                      "coupler": bar("coupler", 120.0, 12.0), "rocker": bar("rocker", 80.0, 6.0)},
            "instances": [
                {"id": "ground", "part": "ground"},
                {"id": "crank", "part": "crank", "parent": "ground"},
                {"id": "coupler", "part": "coupler", "parent": "crank", "at": [40, 0, 0]},
                {"id": "rocker", "part": "rocker", "parent": "ground", "at": [100, 0, 0]}
            ],
            "joints": [
                {"id": "crank", "type": "revolute", "parent": "ground", "child": "crank", "origin": [0, 0, 0], "axis": [0, 0, 1], "value": 0},
                {"id": "pin", "type": "revolute", "parent": "crank", "child": "coupler", "origin": [40, 0, 0], "axis": [0, 0, 1], "value": 36},
                {"id": "rock", "type": "revolute", "parent": "ground", "child": "rocker", "origin": [100, 0, 0], "axis": [0, 0, 1], "value": 63, "limits": [[0, 120]]},
                {"id": "tip", "type": "revolute", "closes": true, "parent": "coupler", "child": "rocker",
                 "origin": [120, 0, 0], "axis": [0, 0, 1], "child_origin": [80, 0, 0]}
            ]
        });
        let made = lab.run(
            "cad_from_json",
            &json!({"doc": "linkage", "document": document, "summary": "A four-bar"}),
        );
        assert_eq!(made["saved"], true, "{made}");
        assert_eq!(made["solver"]["loops"][0]["mobility"], 1, "{made}");
        // Drive the crank to 60°: the rocker follows, and is saved there.
        let driven = lab.run("cad_assemble", &json!({"doc": "linkage", "drive": "{\"joint\":\"crank\",\"value\":60}", "summary": "Crank to 60°"}));
        assert_eq!(driven["saved"], true, "{driven}");
        let rock = driven["solver"]["loops"][0]["joints"]["rock"]
            .as_f64()
            .unwrap();
        let opened = lab.run("cad_open", &json!({"doc": "linkage"}));
        let joints = &opened["document"]["joints"];
        assert_eq!(joints[0]["drive"], true);
        assert!(
            (joints[2]["value"].as_f64().unwrap() - rock).abs() < 1e-9,
            "{joints}"
        );
        // Moving the crank round: the rocker's limit of 120° stops it.
        let moved = lab.run(
            "cad_motion",
            &json!({"doc": "linkage", "joint": "crank", "from": 60, "to": 400, "steps": 34}),
        );
        let reached = moved["reached"].as_f64().unwrap_or(f64::NAN);
        assert!(reached > 60.0 && reached < 400.0, "{moved}");
        assert!(
            moved["stopped"]["why"]
                .as_str()
                .unwrap()
                .contains("rock is at its limit"),
            "{moved}"
        );
        // Driving the rocker where the linkage can't reach is refused, not forced.
        let bad = lab.run(
            "cad_assemble",
            &json!({"doc": "linkage",
            "drive": "{\"joint\":\"rock\",\"value\":10}", "summary": "x"}),
        );
        assert!(
            bad["error"]
                .as_str()
                .unwrap_or_default()
                .contains("can't close"),
            "{bad}"
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
