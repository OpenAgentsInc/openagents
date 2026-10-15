//! Ship Lab: Lagrange 2's spacecraft document (`spacecraft.v1`,
//! [`verse_lagrange2::ship`]) edited by talking (#11651).
//!
//! Each ship is kept as numbered versions on this computer
//! (`ROOT/<id>/vN.json`, with `vN.meta.json` holding the one-line summary
//! and the content hash). Every change goes through
//! [`Spacecraft::edit`], so it is accepted only if the ship still passes
//! every check (mass budget, thrust line, test burn, latch loads, the
//! assembly tree), and a refusal names the requirement that failed. The
//! chat's `ship_*` tools ([`tools`]) call [`Lab::run`] through
//! `ship-lab serve`, as CAD Lab's `cad_*` tools call CAD Lab.
//!
//! An accepted version of the sample tug is also written to
//! `ROOT/.verse/lagrange2.json`, which the Lagrange 2 zone builds its next
//! ship from on a computer that has it.

use std::path::{Path, PathBuf};

use glam::DVec3;
use serde_json::{Value, json};
use verse_lagrange2::ship::{Edit, PartKind, Spacecraft};

/// The card kind a tool result carries for the chat.
pub const CARD: &str = "ship_lab";
/// The ship every Lab starts with: the Lagrange 2 sample tug.
pub const SAMPLE: &str = "tug";
/// Where the Lagrange 2 zone reads the tug's latest accepted version,
/// under the Lab's root.
pub const ZONE_FILE: &str = ".verse/lagrange2.json";

/// The Lab: its documents' folder.
pub struct Lab {
    pub root: PathBuf,
}

impl Lab {
    /// `SHIP_LAB_DIR`, else `~/work/film-out/lab/ship`, beside CAD Lab's
    /// documents.
    #[must_use]
    pub fn default_root() -> PathBuf {
        if let Some(dir) = std::env::var_os("SHIP_LAB_DIR") {
            return PathBuf::from(dir);
        }
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("work/film-out/lab/ship")
    }

    /// Opens the Lab at `root`, starting it with the sample tug.
    ///
    /// # Errors
    /// The folder can't be made or written.
    pub fn open(root: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&root)
            .map_err(|e| format!("can't make {}: {e}", root.display()))?;
        let lab = Self { root };
        if lab.latest(SAMPLE).is_none() {
            lab.save(SAMPLE, &Spacecraft::sample(), "The sample tug")?;
        }
        Ok(lab)
    }

    fn dir(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }

    /// The versions of `id`, oldest first.
    #[must_use]
    pub fn versions(&self, id: &str) -> Vec<u64> {
        if !valid_id(id) {
            return Vec::new();
        }
        let mut out: Vec<u64> = std::fs::read_dir(self.dir(id))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                name.strip_prefix('v')?.strip_suffix(".json")?.parse().ok()
            })
            .collect();
        out.sort_unstable();
        out
    }

    fn latest(&self, id: &str) -> Option<u64> {
        self.versions(id).last().copied()
    }

    /// A version of `id` and its summary.
    #[must_use]
    pub fn load(&self, id: &str, n: u64) -> Option<(Spacecraft, String)> {
        if !valid_id(id) {
            return None;
        }
        let text = std::fs::read_to_string(self.dir(id).join(format!("v{n}.json"))).ok()?;
        let ship: Spacecraft = serde_json::from_str(&text).ok()?;
        let summary = std::fs::read_to_string(self.dir(id).join(format!("v{n}.meta.json")))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|m| m["summary"].as_str().map(str::to_owned))
            .unwrap_or_default();
        Some((ship, summary))
    }

    fn save(&self, id: &str, ship: &Spacecraft, summary: &str) -> Result<u64, String> {
        let n = self.latest(id).unwrap_or(0) + 1;
        let dir = self.dir(id);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(ship).map_err(|e| e.to_string())?;
        write(&dir.join(format!("v{n}.json")), &text)?;
        let meta =
            json!({"summary": summary, "content": ship.version(), "revision": ship.revision});
        write(&dir.join(format!("v{n}.meta.json")), &meta.to_string())?;
        if id == SAMPLE {
            write(&self.root.join(ZONE_FILE), &text)?;
        }
        Ok(n)
    }

    /// Every ship: id, name, versions.
    #[must_use]
    pub fn documents(&self) -> Vec<Value> {
        let mut out = Vec::new();
        let mut ids: Vec<String> = std::fs::read_dir(&self.root)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|id| valid_id(id))
            .collect();
        ids.sort();
        for id in ids {
            let versions = self.versions(&id);
            let Some(&n) = versions.last() else { continue };
            let Some((ship, _)) = self.load(&id, n) else {
                continue;
            };
            out.push(json!({"id": id, "title": ship.name, "kind": "spacecraft.v1", "versions": versions.len()}));
        }
        out
    }

    /// Runs one tool: its JSON result, with `"error"` when it refuses.
    #[must_use]
    pub fn run(&self, name: &str, args: &Value) -> Value {
        let result = match name {
            "ship_list" => Ok(json!({"documents": self.documents()})),
            "ship_open" => self.open_doc(args),
            "ship_edit" => self.edit(args),
            "ship_versions" => self.list_versions(args),
            "ship_compare" => self.compare(args),
            _ => Err(format!("{name} isn't a Ship Lab tool.")),
        };
        result.unwrap_or_else(|why| json!({"error": why}))
    }

    /// The document and version a tool names, loaded.
    fn pick(&self, args: &Value) -> Result<(String, u64, Spacecraft, String), String> {
        let id = args["doc"].as_str().unwrap_or(SAMPLE).trim().to_owned();
        let versions = self.versions(&id);
        let Some(&latest) = versions.last() else {
            return Err(format!("There's no ship named {id}."));
        };
        let n = args["version"].as_u64().unwrap_or(latest);
        let (ship, summary) = self
            .load(&id, n)
            .ok_or_else(|| format!("{id} has versions 1 to {latest}; there's no version {n}."))?;
        Ok((id, n, ship, summary))
    }

    fn open_doc(&self, args: &Value) -> Result<Value, String> {
        let (id, n, ship, summary) = self.pick(args)?;
        Ok(json!({
            "doc": id, "version": n, "summary": summary,
            "ship": numbers(&ship),
            "parts": parts(&ship),
            "card": card(&id, n, &ship, &summary, None),
        }))
    }

    fn edit(&self, args: &Value) -> Result<Value, String> {
        let (id, n, ship, _) = self.pick(&json!({"doc": args["doc"]}))?;
        let edit = parse_edit(args)?;
        let summary = args["summary"]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("An edit")
            .to_owned();
        match ship.edit(&edit) {
            Ok(next) => {
                let v = self.save(&id, &next, &summary)?;
                Ok(json!({
                    "doc": id, "version": v, "accepted": true, "summary": summary,
                    "before": numbers(&ship), "after": numbers(&next),
                    "card": card(&id, v, &next, &summary, None),
                }))
            }
            Err(finding) => Ok(json!({
                "doc": id, "version": n, "accepted": false,
                "error": format!(
                    "Refused by the {} check: {}. The ship stays at version {n}.",
                    finding.requirement, finding.detail
                ),
                "requirement": finding.requirement,
                "card": card(&id, n, &ship, &summary, Some(&finding.to_string())),
            })),
        }
    }

    fn list_versions(&self, args: &Value) -> Result<Value, String> {
        let (id, _, _, _) = self.pick(&json!({"doc": args["doc"]}))?;
        let list: Vec<Value> = self
            .versions(&id)
            .into_iter()
            .filter_map(|n| {
                let (ship, summary) = self.load(&id, n)?;
                let report = ship.report();
                Some(json!({"version": n, "summary": summary,
                            "budget_mass_kg": report.budget_mass_kg.round(),
                            "delta_v_m_s": round1(report.delta_v_m_s)}))
            })
            .collect();
        Ok(json!({"doc": id, "versions": list}))
    }

    fn compare(&self, args: &Value) -> Result<Value, String> {
        let (id, b, ship_b, summary_b) =
            self.pick(&json!({"doc": args["doc"], "version": args["b"]}))?;
        let a = args["a"].as_u64().unwrap_or(b.saturating_sub(1).max(1));
        if a == b {
            return Err(format!(
                "That's version {b} with itself; name two versions."
            ));
        }
        let (ship_a, _) = self
            .load(&id, a)
            .ok_or_else(|| format!("{id} has no version {a}."))?;
        let mut changed = Vec::new();
        for p in &ship_b.parts {
            match ship_a.part(&p.id) {
                None => changed.push(format!("added {}", p.id)),
                Some(old) if old != p => changed.push(format!(
                    "{}: {:.0} kg to {:.0} kg",
                    p.id,
                    old.mass(),
                    p.mass()
                )),
                Some(_) => {}
            }
        }
        for p in &ship_a.parts {
            if ship_b.part(&p.id).is_none() {
                changed.push(format!("removed {}", p.id));
            }
        }
        Ok(json!({
            "doc": id, "a": a, "b": b, "parts_changed": changed,
            "a_numbers": numbers(&ship_a), "b_numbers": numbers(&ship_b),
            "card": card(&id, b, &ship_b, &summary_b, None),
        }))
    }
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, text).map_err(|e| format!("can't write {}: {e}", path.display()))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 48
        && id.starts_with(|c: char| c.is_ascii_lowercase())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// The typed edit a `ship_edit` call names. Its fields are bounded: an
/// edit kind from a fixed list, part ids, and numbers.
fn parse_edit(args: &Value) -> Result<Edit, String> {
    let kind = args["edit"].as_str().unwrap_or_default();
    let parts: Vec<String> = match &args["parts"] {
        Value::Array(list) => list
            .iter()
            .filter_map(|p| p.as_str().map(str::to_owned))
            .collect(),
        Value::String(one) => vec![one.clone()],
        _ => Vec::new(),
    };
    if parts.is_empty() {
        return Err("Name the parts to edit (parts), e.g. [\"tank-port\"].".into());
    }
    let number = |key: &str| {
        args[key]
            .as_f64()
            .ok_or_else(|| format!("{kind} needs {key}."))
    };
    let mut edits = Vec::new();
    for part in parts {
        edits.push(match kind {
            "scale" => Edit::ScalePart {
                part,
                factor: number("factor")?,
            },
            "set_dry_mass" => Edit::SetDryMass {
                part,
                kg: number("kg")?,
            },
            "set_propellant" => Edit::SetPropellant {
                part,
                kg: number("kg")?,
            },
            "move" => {
                let at: Vec<f64> = args["position"]
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_f64).collect())
                    .unwrap_or_default();
                let [x, y, z] = at[..] else {
                    return Err("move needs position [x, y, z] in metres.".into());
                };
                Edit::MovePart {
                    part,
                    position: DVec3::new(x, y, z),
                }
            }
            "remove" => Edit::RemovePart { part },
            other => {
                return Err(format!(
                    "There's no edit called {other}; use scale, set_dry_mass, set_propellant, move or remove."
                ));
            }
        });
    }
    Ok(if edits.len() == 1 {
        edits.remove(0)
    } else {
        Edit::Batch { edits }
    })
}

/// The ship's derived numbers, rounded for reading.
fn numbers(ship: &Spacecraft) -> Value {
    let r = ship.report();
    let worst = r
        .latch_loads
        .iter()
        .min_by(|a, b| a.margin.total_cmp(&b.margin));
    json!({
        "mass_kg": r.mass.mass.round(),
        "budget_mass_kg": r.budget_mass_kg.round(),
        "max_mass_kg": ship.requirements.max_mass_kg,
        "mass_margin_percent": round1(r.mass_margin * 100.0),
        "propellant_kg": r.propellant_kg.round(),
        "delta_v_m_s": round1(r.delta_v_m_s),
        "test_burn_m_s": ship.requirements.test_burn_m_s,
        "thrust_offset_m": (r.thrust_offset_m * 1000.0).round() / 1000.0,
        "max_thrust_offset_m": ship.requirements.max_thrust_offset_m,
        "tightest_latch": worst.map(|l| json!({"part": l.part, "margin_percent": round1(l.margin * 100.0)})),
    })
}

fn parts(ship: &Spacecraft) -> Vec<Value> {
    ship.parts
        .iter()
        .map(|p| {
            let kind = match p.kind {
                PartKind::Truss => "truss",
                PartKind::Module => "module",
                PartKind::Tank { .. } => "tank",
                PartKind::Engine { .. } => "engine",
            };
            json!({"id": p.id, "kind": kind, "mass_kg": p.mass().round(),
                   "propellant_kg": p.propellant().round(),
                   "parent": p.attach.parent})
        })
        .collect()
}

/// The chat's card: the version and its numbers, and a refusal's reason.
fn card(id: &str, n: u64, ship: &Spacecraft, summary: &str, refused: Option<&str>) -> Value {
    json!({
        "card": CARD, "doc": id, "title": ship.name, "version": n,
        "summary": summary, "numbers": numbers(ship), "refused": refused,
    })
}

/// The tools' declarations, for the chat's model.
#[must_use]
pub fn tools() -> Value {
    let doc = json!({"type": "string", "description": "the ship's id, e.g. tug"});
    let version =
        json!({"type": "integer", "description": "a version number; the latest when left out"});
    json!({"tools": [
        {"name": "ship_list", "description": "List the ships in Ship Lab on this computer (Lagrange 2's spacecraft documents).",
         "parameters": {"type": "object", "properties": {}}},
        {"name": "ship_open", "description": "Open a ship: its parts (ids, kinds, masses) and its numbers (mass with contingency against the budget, delta-v against the test burn, thrust-line offset, the tightest latch).",
         "parameters": {"type": "object", "properties": {"doc": doc, "version": version}, "required": ["doc"]}},
        {"name": "ship_edit", "description": "Change a ship. Every edit is checked (mass budget, thrust line, test burn, latch loads, assembly tree); a refused edit leaves the ship as it was and names the check. Edits: scale (a part's volume times factor: 1.2 is 20% bigger; a tank's propellant and dry mass grow with it), set_dry_mass (kg), set_propellant (kg, tanks only), move (position [x,y,z] m, ship frame), remove. Naming several parts applies the edit to each, checked together as one version.",
         "parameters": {"type": "object", "properties": {
            "doc": doc,
            "edit": {"type": "string", "enum": ["scale", "set_dry_mass", "set_propellant", "move", "remove"]},
            "parts": {"type": "array", "items": {"type": "string"}, "description": "part ids from ship_open, e.g. [\"tank-port\", \"tank-starboard\"]"},
            "factor": {"type": "number", "description": "for scale: the new volume over the old"},
            "kg": {"type": "number"},
            "position": {"type": "array", "items": {"type": "number"}},
            "summary": {"type": "string", "description": "one line in plain words saying what this edit changes"}},
          "required": ["doc", "edit", "parts", "summary"]}},
        {"name": "ship_versions", "description": "A ship's versions with their summaries and numbers.",
         "parameters": {"type": "object", "properties": {"doc": doc}, "required": ["doc"]}},
        {"name": "ship_compare", "description": "Compare two versions of a ship (a = the earlier, b = the later; the latest and the one before by default).",
         "parameters": {"type": "object", "properties": {"doc": doc, "a": {"type": "integer"}, "b": {"type": "integer"}}, "required": ["doc"]}},
    ]})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lab() -> (tempfile::TempDir, Lab) {
        let dir = tempfile::tempdir().unwrap();
        let lab = Lab::open(dir.path().to_path_buf()).unwrap();
        (dir, lab)
    }

    #[test]
    fn a_new_lab_has_the_sample_tug() {
        let (_dir, lab) = lab();
        let docs = lab.run("ship_list", &json!({}));
        assert_eq!(docs["documents"][0]["id"], "tug", "{docs}");
        let open = lab.run("ship_open", &json!({"doc": "tug"}));
        assert_eq!(open["version"], 1);
        assert_eq!(open["card"]["card"], CARD);
        assert!(!open["parts"].as_array().unwrap().is_empty());
    }

    #[test]
    fn every_edit_is_checked_and_a_refusal_names_the_requirement() {
        let (dir, lab) = lab();
        // One tank 20% bigger: the checks decide, and either way the
        // result says which.
        let one = lab.run(
            "ship_edit",
            &json!({"doc": "tug", "edit": "scale", "parts": ["tank-port"], "factor": 1.2, "summary": "A bigger port tank"}),
        );
        let ship = Spacecraft::sample();
        let expected = ship.edit(&Edit::ScalePart {
            part: "tank-port".into(),
            factor: 1.2,
        });
        assert_eq!(one["accepted"], expected.is_ok(), "{one}");
        // An edit far over the budget is refused by name, and nothing
        // is saved.
        let before = lab.versions("tug").len();
        let heavy = lab.run(
            "ship_edit",
            &json!({"doc": "tug", "edit": "set_dry_mass", "parts": ["habitat"], "kg": 60000.0, "summary": "Heavy"}),
        );
        assert_eq!(heavy["accepted"], false, "{heavy}");
        assert_eq!(heavy["requirement"], "mass budget");
        assert!(heavy["error"].as_str().unwrap().contains("mass budget"));
        assert_eq!(lab.versions("tug").len(), before);
        // Accepted versions reach the zone's file.
        if expected.is_ok() {
            let zone: Spacecraft =
                serde_json::from_str(&std::fs::read_to_string(dir.path().join(ZONE_FILE)).unwrap())
                    .unwrap();
            assert!(zone.part("tank-port").unwrap().propellant() > 22_000.0 * 1.19);
        }
        // Unknown parts and edits are refused.
        assert!(
            lab.run(
                "ship_edit",
                &json!({"doc": "tug", "edit": "scale", "parts": ["nope"], "factor": 2.0})
            )["error"]
                .as_str()
                .unwrap()
                .contains("no part named nope")
        );
        assert!(
            lab.run(
                "ship_edit",
                &json!({"doc": "tug", "edit": "paint", "parts": ["engine"]})
            )["error"]
                .is_string()
        );
    }

    #[test]
    fn both_tanks_are_checked_together_as_one_version() {
        let (_dir, lab) = lab();
        let both = lab.run(
            "ship_edit",
            &json!({"doc": "tug", "edit": "scale", "parts": ["tank-port", "tank-starboard"], "factor": 1.05, "summary": "Both tanks 5% bigger"}),
        );
        assert_eq!(both["accepted"], true, "{both}");
        assert_eq!(both["version"], 2);
        let compared = lab.run("ship_compare", &json!({"doc": "tug"}));
        assert_eq!(
            compared["parts_changed"].as_array().unwrap().len(),
            2,
            "{compared}"
        );
        let versions = lab.run("ship_versions", &json!({"doc": "tug"}));
        assert_eq!(versions["versions"][1]["summary"], "Both tanks 5% bigger");
    }
}
