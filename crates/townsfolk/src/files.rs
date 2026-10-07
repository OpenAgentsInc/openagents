//! The checked-in townsfolk directory and who may change what in it.
//!
//! ```text
//! townsfolk/
//!   town.json          the roster: seed, budgets, admitted IDs and digests
//!   npcs/ID.json       one definition a villager
//!   proposals/ID.json  a staged proposal for the owner to admit
//! ```
//!
//! Anyone, a workshop agent included, may write definitions and run
//! [`propose`], which validates a definition and stages a [`Proposal`]:
//! its digest, the validation result, and the simulated day. Only the
//! owner runs [`admit`], which adds the proposed digest to the roster, and
//! [`remove`]; the command line asks the owner to confirm at a terminal,
//! and a workshop agent's charter never grants either. A change reaches
//! players through a normal commit, which is the owner's review.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use world_tree::Tree;

use crate::routine::{Roster, Villager};
use crate::validate::{self, Checks};
use crate::{Admitted, Code, Npc, PROPOSAL_SCHEMA, Problem, Town, sim};

/// The roster's file name.
pub const TOWN_FILE: &str = "town.json";
/// The definitions' directory.
pub const NPCS_DIR: &str = "npcs";
/// The proposals' directory.
pub const PROPOSALS_DIR: &str = "proposals";

/// A townsfolk directory.
#[derive(Clone, Debug)]
pub struct Dir {
    root: PathBuf,
}

fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Writes `text` to `path` through a temporary file and a rename.
fn write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

impl Dir {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn town_path(&self) -> PathBuf {
        self.root.join(TOWN_FILE)
    }

    #[must_use]
    pub fn npc_path(&self, id: &str) -> PathBuf {
        self.root.join(NPCS_DIR).join(format!("{id}.json"))
    }

    #[must_use]
    pub fn proposal_path(&self, id: &str) -> PathBuf {
        self.root.join(PROPOSALS_DIR).join(format!("{id}.json"))
    }

    /// The roster.
    ///
    /// # Errors
    ///
    /// When it can't be read or parsed.
    pub fn town(&self) -> Result<Town, String> {
        Town::parse(&read(&self.town_path())?).map_err(|e| format!("{TOWN_FILE}: {e}"))
    }

    /// Writes the roster.
    ///
    /// # Errors
    ///
    /// When it can't be written.
    pub fn write_town(&self, town: &Town) -> Result<(), String> {
        write(&self.town_path(), &town.to_json())
    }

    /// The IDs of the definition files, sorted.
    ///
    /// # Errors
    ///
    /// When the directory can't be read.
    pub fn ids(&self) -> Result<Vec<String>, String> {
        let dir = self.root.join(NPCS_DIR);
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("{}: {e}", dir.display())),
        };
        let mut ids: Vec<String> = entries
            .filter_map(Result::ok)
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                name.strip_suffix(".json").map(str::to_owned)
            })
            .collect();
        ids.sort();
        Ok(ids)
    }

    /// The definition in `npcs/ID.json`.
    ///
    /// # Errors
    ///
    /// When it can't be read or parsed, or its ID isn't its file's.
    pub fn npc(&self, id: &str) -> Result<Npc, String> {
        let path = self.npc_path(id);
        let npc = Npc::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
        if npc.id != id {
            return Err(format!(
                "{}: its id is {:?}, not the file's {id:?}",
                path.display(),
                npc.id
            ));
        }
        Ok(npc)
    }

    /// The staged proposal for `id`, if any.
    ///
    /// # Errors
    ///
    /// When one exists but can't be read or parsed.
    pub fn proposal(&self, id: &str) -> Result<Option<Proposal>, String> {
        let path = self.proposal_path(id);
        if !path.exists() {
            return Ok(None);
        }
        serde_json::from_str(&read(&path)?)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The roster as a client loads it from these files.
    ///
    /// # Errors
    ///
    /// When the roster can't be read or is over a budget.
    pub fn roster(&self, tree: &Tree) -> Result<(Roster, Vec<Problem>), String> {
        let town = read(&self.town_path())?;
        let files: Vec<String> = self
            .ids()?
            .iter()
            .map(|id| read(&self.npc_path(id)))
            .collect::<Result<_, _>>()?;
        let files: Vec<&str> = files.iter().map(String::as_str).collect();
        Roster::load(&town, &files, tree)
    }
}

/// A staged proposal, `openagents.verse-town-proposal.v1`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub schema: String,
    pub id: String,
    /// The definition's digest when it was proposed.
    pub digest: String,
    /// Unix seconds.
    pub proposed_at: i64,
    /// Whether every leg was routed over the zone's blockers.
    pub routes_checked: bool,
    /// Whether it passed every check.
    pub valid: bool,
    pub problems: Vec<Problem>,
    /// Its routine and the meetings it would have with the admitted town on
    /// town day zero, as text.
    pub day: Vec<String>,
}

/// Checks the definition `id` in `dir` against `checks`, with the roster's
/// budgets, and against the admitted town: the roster's villager budget and
/// the exclusive objects.
///
/// # Errors
///
/// When the roster or the definition can't be read.
pub fn check(dir: &Dir, id: &str, checks: Checks) -> Result<(Npc, Vec<Problem>), String> {
    let town = dir.town()?;
    let npc = dir.npc(id)?;
    let checks = checks.with_budgets(town.budgets);
    let mut problems = town.problems();
    problems.extend(validate::npc(&npc, &checks));
    if town.entry(id).is_none() && town.admitted.len() >= town.budgets.villagers as usize {
        problems.push(Problem::new(
            "id",
            Code::Budget,
            format!(
                "the town already admits {} villagers, its budget",
                town.budgets.villagers
            ),
        ));
    }
    if problems.is_empty() {
        let (roster, _) = dir.roster(checks.tree)?;
        let mut town_now: Vec<Villager> = roster
            .villagers
            .into_iter()
            .filter(|v| v.id() != id)
            .collect();
        town_now.push(Villager::compiled(npc.clone(), checks.tree));
        problems.extend(
            validate::exclusive(&town_now)
                .into_iter()
                .filter(|p| p.field.starts_with(&format!("{id}:"))),
        );
    }
    Ok((npc, problems))
}

/// Validates the definition `id` and stages a proposal for it in
/// `proposals/ID.json`, valid or not. Anyone may propose.
///
/// # Errors
///
/// When a file can't be read or written.
pub fn propose(dir: &Dir, id: &str, checks: Checks, now_unix: i64) -> Result<Proposal, String> {
    let routes_checked = checks.router.is_some();
    let tree = checks.tree;
    let (npc, problems) = check(dir, id, checks)?;
    let mut day = Vec::new();
    if problems.is_empty() {
        let town = dir.town()?;
        let (roster, _) = dir.roster(tree)?;
        let mut all: Vec<Villager> = roster
            .villagers
            .into_iter()
            .filter(|v| v.id() != id)
            .collect();
        let me = Villager::compiled(npc.clone(), tree);
        day.extend(sim::schedule(&me, tree));
        all.push(me);
        for m in sim::meetings(&all, town.seed, 0, 300)
            .into_iter()
            .filter(|m| m.ids.iter().any(|i| i == id))
        {
            let place = tree
                .node(&m.node)
                .map_or(m.node.as_str(), |n| n.name.as_str());
            day.push(format!(
                "  meets {} at {place}, {}-{}",
                m.ids
                    .iter()
                    .filter(|i| *i != id)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", "),
                m.from,
                m.to
            ));
        }
    }
    let proposal = Proposal {
        schema: PROPOSAL_SCHEMA.into(),
        id: id.to_owned(),
        digest: npc.digest(),
        proposed_at: now_unix,
        routes_checked,
        valid: problems.is_empty() && routes_checked,
        problems,
        day,
    };
    let mut text = serde_json::to_string_pretty(&proposal).expect("plain data serializes");
    text.push('\n');
    write(&dir.proposal_path(id), &text)?;
    Ok(proposal)
}

/// Admits the proposed definition `id`: adds or replaces its roster entry
/// with the proposed digest. The owner's action only.
///
/// # Errors
///
/// When there is no valid, route-checked proposal for the definition as it
/// is now, the roster is full, or it would double-book an exclusive object.
pub fn admit(dir: &Dir, id: &str, tree: &Tree) -> Result<Admitted, String> {
    let mut town = dir.town()?;
    let npc = dir.npc(id)?;
    let proposal = dir
        .proposal(id)?
        .ok_or_else(|| format!("{id} has no proposal; run propose first"))?;
    let digest = npc.digest();
    if proposal.digest != digest {
        return Err(format!(
            "{id} changed since it was proposed ({} then, {digest} now); propose it again",
            proposal.digest
        ));
    }
    if !proposal.routes_checked {
        return Err(format!(
            "{id}'s proposal didn't route its legs; propose it again where the zone's pack is"
        ));
    }
    if !proposal.valid {
        let first = proposal
            .problems
            .first()
            .map_or_else(String::new, |p| format!(": {p}"));
        return Err(format!("{id}'s proposal failed validation{first}"));
    }
    let entry = Admitted {
        id: id.to_owned(),
        digest,
    };
    match town.admitted.iter_mut().find(|a| a.id == id) {
        Some(a) => *a = entry.clone(),
        None => town.admitted.push(entry.clone()),
    }
    if let Some(p) = town.problems().first() {
        return Err(format!("the roster would be invalid: {p}"));
    }
    let (roster, _) = dir.roster(tree)?;
    let mut all: Vec<Villager> = roster
        .villagers
        .into_iter()
        .filter(|v| v.id() != id)
        .collect();
    all.push(Villager::compile(npc, tree).map_err(|p| {
        p.first()
            .map_or_else(String::new, |p| format!("{id} no longer validates: {p}"))
    })?);
    if let Some(p) = validate::exclusive(&all).first() {
        return Err(format!("admitting {id} would double-book: {p}"));
    }
    dir.write_town(&town)?;
    Ok(entry)
}

/// Removes `id` from the roster; its file stays. The owner's action only.
/// Returns whether it was admitted.
///
/// # Errors
///
/// When the roster can't be read or written.
pub fn remove(dir: &Dir, id: &str) -> Result<bool, String> {
    let mut town = dir.town()?;
    let before = town.admitted.len();
    town.admitted.retain(|a| a.id != id);
    if town.admitted.len() == before {
        return Ok(false);
    }
    dir.write_town(&town)?;
    Ok(true)
}

/// Where one villager stands in the admission flow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Status {
    pub id: String,
    /// `admitted` (the file is the admitted digest), `changed` (admitted,
    /// but the file differs), `missing` (admitted, no file), `proposed`
    /// (a valid proposal matches the file), `refused` (the proposal for
    /// this file failed), or `draft`.
    pub state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admitted_digest: Option<String>,
}

/// Every definition file and admitted entry, by ID.
///
/// # Errors
///
/// When the roster can't be read, or a definition or proposal can't be
/// parsed.
pub fn list(dir: &Dir) -> Result<Vec<Status>, String> {
    let town = dir.town()?;
    let mut ids = dir.ids()?;
    for a in &town.admitted {
        if !ids.contains(&a.id) {
            ids.push(a.id.clone());
        }
    }
    ids.sort();
    let mut out = Vec::new();
    for id in ids {
        let file = if dir.npc_path(&id).exists() {
            Some(dir.npc(&id)?.digest())
        } else {
            None
        };
        let admitted = town.entry(&id).map(|a| a.digest.clone());
        let proposal = dir.proposal(&id)?;
        let state = match (&file, &admitted, &proposal) {
            (None, Some(_), _) => "missing",
            (Some(f), Some(a), _) if f == a => "admitted",
            (Some(_), Some(_), _) => "changed",
            (Some(f), None, Some(p)) if &p.digest == f && p.valid => "proposed",
            (Some(f), None, Some(p)) if &p.digest == f => "refused",
            _ => "draft",
        };
        out.push(Status {
            id,
            state,
            file_digest: file,
            admitted_digest: admitted,
        });
    }
    Ok(out)
}
