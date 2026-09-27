//! SpecDb: canonical facade for content specs (spells/classes/monsters).
//!
//! Provides in-memory indexes and simple normalization so callers don't need
//! to guess file names or embed heuristics.

use crate::class::ClassSpec;
use crate::monster::MonsterSpec;
use crate::{loader, spell::SpellSpec};
use std::collections::HashMap;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Prefer workspace root (two levels up) if it contains data/
    let ws = here.join("..").join("..");
    if ws.join("data").is_dir() { ws } else { here }
}

#[derive(Default)]
pub struct SpecDb {
    spells: HashMap<String, SpellSpec>,
    classes: HashMap<String, ClassSpec>,
    monsters: HashMap<String, MonsterSpec>,
}

impl SpecDb {
    pub fn load_default() -> Self {
        let mut db = SpecDb::default();
        let root = workspace_root();
        let spells_dir = root.join("data/spells");
        if spells_dir.is_dir()
            && let Ok(rd) = std::fs::read_dir(&spells_dir)
        {
            for ent in rd.flatten() {
                let path = ent.path();
                if path.extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }
                let rel = format!("spells/{}", path.file_name().unwrap().to_string_lossy());
                if let Ok(spec) = loader::load_spell_spec(&rel) {
                    Self::index_spell(&mut db.spells, spec);
                }
            }
        }
        let classes_dir = root.join("data/classes");
        if classes_dir.is_dir()
            && let Ok(rd) = std::fs::read_dir(&classes_dir)
        {
            for ent in rd.flatten() {
                let path = ent.path();
                if path.extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }
                let rel = format!("classes/{}", path.file_name().unwrap().to_string_lossy());
                if let Ok(spec) = loader::load_class_spec(rel) {
                    db.classes.insert(spec.id.clone(), spec);
                }
            }
        }
        let monsters_dir = root.join("data/monsters");
        if monsters_dir.is_dir()
            && let Ok(rd) = std::fs::read_dir(&monsters_dir)
        {
            for ent in rd.flatten() {
                let path = ent.path();
                if path.extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }
                let rel = format!("monsters/{}", path.file_name().unwrap().to_string_lossy());
                if let Ok(spec) = loader::load_monster_spec(rel) {
                    db.monsters.insert(spec.id.clone(), spec);
                }
            }
        }
        db
    }

    fn index_spell(map: &mut HashMap<String, SpellSpec>, spec: SpellSpec) {
        let canon = spec.id.clone();
        let name_key = spec.name.to_ascii_lowercase().replace(' ', "_");
        let last = canon.rsplit('.').next().unwrap_or(&canon).to_string();
        map.insert(canon, spec.clone());
        map.insert(last, spec.clone());
        map.insert(name_key, spec);
    }

    pub fn get_spell(&self, id: &str) -> Option<&SpellSpec> {
        if let Some(s) = self.spells.get(id) {
            return Some(s);
        }
        let last = id.rsplit('.').next().unwrap_or(id);
        if let Some(s) = self.spells.get(last) {
            return Some(s);
        }
        let name_key = id.to_ascii_lowercase().replace(' ', "_");
        self.spells.get(&name_key)
    }

    pub fn get_class(&self, id: &str) -> Option<&ClassSpec> {
        self.classes.get(id)
    }

    pub fn get_monster(&self, id: &str) -> Option<&MonsterSpec> {
        self.monsters.get(id)
    }
}
