//! The checked-in townsfolk directory and who may change what in it.
//!
//! ```text
//! townsfolk/
//!   town.json          the roster: seed, budgets, admitted IDs and digests
//!   npcs/ID.json       one definition a villager
//!   proposals/ID.json  a staged proposal for the owner to admit
//!   rumors/ID.json     one rumor (phase E2)
//!   proposals/rumors/ID.json  a staged rumor proposal
//! ```
//!
//! A rumor goes through the same flow: [`propose_rumor`] validates it,
//! sets its repeat score once through a [`Scorer`], and stages a proposal
//! with its diffusion; the owner's [`admit_rumor`] adds its digest to the
//! roster's `rumors`, within `rumors_in_flight`.
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
use crate::rumor::{self, Against, Rumor, Scorer};
use crate::validate::{self, Checks, Screen};
use crate::{Admitted, Code, Npc, PROPOSAL_SCHEMA, Problem, Town, diffusion, sim};

/// The roster's file name.
pub const TOWN_FILE: &str = "town.json";
/// The definitions' directory.
pub const NPCS_DIR: &str = "npcs";
/// The proposals' directory.
pub const PROPOSALS_DIR: &str = "proposals";
/// The rumors' directory.
pub const RUMORS_DIR: &str = "rumors";

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

    #[must_use]
    pub fn rumor_path(&self, id: &str) -> PathBuf {
        self.root.join(RUMORS_DIR).join(format!("{id}.json"))
    }

    #[must_use]
    pub fn rumor_proposal_path(&self, id: &str) -> PathBuf {
        self.root
            .join(PROPOSALS_DIR)
            .join(RUMORS_DIR)
            .join(format!("{id}.json"))
    }

    /// The IDs of the definition files, sorted.
    ///
    /// # Errors
    ///
    /// When the directory can't be read.
    pub fn ids(&self) -> Result<Vec<String>, String> {
        self.json_ids(NPCS_DIR)
    }

    /// The IDs of the rumor files, sorted.
    ///
    /// # Errors
    ///
    /// When the directory can't be read.
    pub fn rumor_ids(&self) -> Result<Vec<String>, String> {
        self.json_ids(RUMORS_DIR)
    }

    /// The rumor in `rumors/ID.json`.
    ///
    /// # Errors
    ///
    /// When it can't be read or parsed, or its ID isn't its file's.
    pub fn rumor(&self, id: &str) -> Result<Rumor, String> {
        let path = self.rumor_path(id);
        let rumor = Rumor::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
        if rumor.id != id {
            return Err(format!(
                "{}: its id is {:?}, not the file's {id:?}",
                path.display(),
                rumor.id
            ));
        }
        Ok(rumor)
    }

    /// Writes a rumor to its file.
    ///
    /// # Errors
    ///
    /// When it can't be written.
    pub fn write_rumor(&self, rumor: &Rumor) -> Result<(), String> {
        write(&self.rumor_path(&rumor.id), &rumor.to_json())
    }

    /// The staged proposal for rumor `id`, if any.
    ///
    /// # Errors
    ///
    /// When one exists but can't be read or parsed.
    pub fn rumor_proposal(&self, id: &str) -> Result<Option<Proposal>, String> {
        let path = self.rumor_proposal_path(id);
        if !path.exists() {
            return Ok(None);
        }
        serde_json::from_str(&read(&path)?)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The admitted rumors as a client loads them against `roster`.
    ///
    /// # Errors
    ///
    /// When a rumor file can't be read.
    pub fn rumors(
        &self,
        roster: &Roster,
        tree: &Tree,
    ) -> Result<(Vec<Rumor>, Vec<Problem>), String> {
        let files: Vec<String> = self
            .rumor_ids()?
            .iter()
            .map(|id| read(&self.rumor_path(id)))
            .collect::<Result<_, _>>()?;
        let files: Vec<&str> = files.iter().map(String::as_str).collect();
        Ok(rumor::load(roster, &files, tree))
    }

    fn json_ids(&self, sub: &str) -> Result<Vec<String>, String> {
        let dir = self.root.join(sub);
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
        out.push(Status {
            state: state(file.as_ref(), admitted.as_ref(), proposal.as_ref()),
            id,
            file_digest: file,
            admitted_digest: admitted,
        });
    }
    Ok(out)
}

/// Checks rumor `id` in `dir` against the admitted town, and the roster's
/// `rumors_in_flight` on town day `today`.
///
/// # Errors
///
/// When the roster or the rumor can't be read.
pub fn check_rumor(
    dir: &Dir,
    id: &str,
    tree: &Tree,
    screen: &dyn Screen,
    today: i64,
) -> Result<(Rumor, Vec<Problem>), String> {
    let town = dir.town()?;
    let rumor = dir.rumor(id)?;
    let (roster, _) = dir.roster(tree)?;
    let mut problems = town.problems();
    problems.extend(rumor::check(
        &rumor,
        &Against {
            tree,
            villagers: &roster.villagers,
            seed: town.seed,
            screen,
            scored: false,
        },
    ));
    if dir.npc_path(id).exists() {
        problems.push(Problem::new(
            "id",
            Code::Duplicate,
            format!("{id} is also a villager's file"),
        ));
    }
    let flying = in_flight(dir, &town, id, today);
    if rumor.in_flight(today) && flying >= town.budgets.rumors_in_flight as usize {
        problems.push(Problem::new(
            "id",
            Code::Budget,
            format!(
                "{flying} admitted rumors are in flight on day {today}, the budget of {}",
                town.budgets.rumors_in_flight
            ),
        ));
    }
    Ok((rumor, problems))
}

/// The admitted rumors other than `except` in flight on `today`; one whose
/// file is missing or unreadable counts.
fn in_flight(dir: &Dir, town: &Town, except: &str, today: i64) -> usize {
    town.rumors
        .iter()
        .filter(|a| a.id != except)
        .filter(|a| dir.rumor(&a.id).map_or(true, |r| r.in_flight(today)))
        .count()
}

/// Validates rumor `id`, sets its repeat score through `scorer` when it
/// has none (rewriting its file, so the digest covers the score), and
/// stages `proposals/rumors/ID.json` with its diffusion over its days.
/// Anyone may propose.
///
/// # Errors
///
/// When a file can't be read or written, or the scorer fails.
pub fn propose_rumor(
    dir: &Dir,
    id: &str,
    tree: &Tree,
    screen: &dyn Screen,
    scorer: &mut dyn Scorer,
    now_unix: i64,
    today: i64,
) -> Result<Proposal, String> {
    let (mut rumor, problems) = check_rumor(dir, id, tree, screen, today)?;
    let mut day = Vec::new();
    if problems.is_empty() {
        let (roster, _) = dir.roster(tree)?;
        if rumor.repeat.is_none() {
            let source = roster
                .villager(&rumor.source)
                .map(|v| v.npc.clone())
                .ok_or_else(|| format!("{} isn't in the town", rumor.source))?;
            rumor.repeat = Some(scorer.score(&rumor, &source)?);
            dir.write_rumor(&rumor)?;
        }
        let spread = diffusion::spread(&roster, &rumor);
        let name = |id: &str| {
            roster
                .villager(id)
                .map_or(id.to_owned(), |v| v.npc.name.clone())
        };
        if let Some(r) = &rumor.repeat {
            day.push(format!("repeat score {:.2} ({})", r.probability, r.basis));
        }
        day.extend(spread.render(tree, name));
    }
    let proposal = Proposal {
        schema: PROPOSAL_SCHEMA.into(),
        id: id.to_owned(),
        digest: rumor.digest(),
        proposed_at: now_unix,
        // A rumor walks no legs.
        routes_checked: true,
        valid: problems.is_empty(),
        problems,
        day,
    };
    let mut text = serde_json::to_string_pretty(&proposal).expect("plain data serializes");
    text.push('\n');
    write(&dir.rumor_proposal_path(id), &text)?;
    Ok(proposal)
}

/// Admits the proposed rumor `id`: adds or replaces its roster entry with
/// the proposed digest. The owner's action only.
///
/// # Errors
///
/// When there is no valid proposal for the rumor as it is now, it no
/// longer checks, or it would pass `rumors_in_flight` on `today`.
pub fn admit_rumor(dir: &Dir, id: &str, tree: &Tree, today: i64) -> Result<Admitted, String> {
    let proposal = dir
        .rumor_proposal(id)?
        .ok_or_else(|| format!("{id} has no proposal; run rumor propose first"))?;
    let (rumor, problems) = check_rumor(dir, id, tree, &validate::NoScreen, today)?;
    let digest = rumor.digest();
    if proposal.digest != digest {
        return Err(format!(
            "{id} changed since it was proposed ({} then, {digest} now); propose it again",
            proposal.digest
        ));
    }
    if !proposal.valid {
        let first = proposal
            .problems
            .first()
            .map_or_else(String::new, |p| format!(": {p}"));
        return Err(format!("{id}'s proposal failed validation{first}"));
    }
    if rumor.repeat.is_none() {
        return Err(format!("{id} has no repeat score; propose it again"));
    }
    if let Some(p) = problems.first() {
        return Err(format!("{id} no longer checks: {p}"));
    }
    let mut town = dir.town()?;
    let entry = Admitted {
        id: id.to_owned(),
        digest,
    };
    match town.rumors.iter_mut().find(|a| a.id == id) {
        Some(a) => *a = entry.clone(),
        None => town.rumors.push(entry.clone()),
    }
    if let Some(p) = town.problems().first() {
        return Err(format!("the roster would be invalid: {p}"));
    }
    dir.write_town(&town)?;
    Ok(entry)
}

/// Removes rumor `id` from the roster; its file stays. The owner's action
/// only. Returns whether it was admitted.
///
/// # Errors
///
/// When the roster can't be read or written.
pub fn remove_rumor(dir: &Dir, id: &str) -> Result<bool, String> {
    let mut town = dir.town()?;
    let before = town.rumors.len();
    town.rumors.retain(|a| a.id != id);
    if town.rumors.len() == before {
        return Ok(false);
    }
    dir.write_town(&town)?;
    Ok(true)
}

/// Every rumor file and admitted rumor, by ID, in the states [`list`]
/// uses.
///
/// # Errors
///
/// When the roster can't be read, or a rumor or proposal can't be parsed.
pub fn list_rumors(dir: &Dir) -> Result<Vec<Status>, String> {
    let town = dir.town()?;
    let mut ids = dir.rumor_ids()?;
    for a in &town.rumors {
        if !ids.contains(&a.id) {
            ids.push(a.id.clone());
        }
    }
    ids.sort();
    let mut out = Vec::new();
    for id in ids {
        let file = if dir.rumor_path(&id).exists() {
            Some(dir.rumor(&id)?.digest())
        } else {
            None
        };
        let admitted = town.rumor(&id).map(|a| a.digest.clone());
        let proposal = dir.rumor_proposal(&id)?;
        out.push(Status {
            state: state(file.as_ref(), admitted.as_ref(), proposal.as_ref()),
            id,
            file_digest: file,
            admitted_digest: admitted,
        });
    }
    Ok(out)
}

fn state(
    file: Option<&String>,
    admitted: Option<&String>,
    proposal: Option<&Proposal>,
) -> &'static str {
    match (file, admitted, proposal) {
        (None, Some(_), _) => "missing",
        (Some(f), Some(a), _) if f == a => "admitted",
        (Some(_), Some(_), _) => "changed",
        (Some(f), None, Some(p)) if &p.digest == f && p.valid => "proposed",
        (Some(f), None, Some(p)) if &p.digest == f => "refused",
        _ => "draft",
    }
}
