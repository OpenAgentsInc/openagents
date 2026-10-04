//! Subagent tabs in a seat panel.
//!
//! When an engine spawns subagents of its own, the seat panel shows each one
//! as a read-only transcript tab beside the parent's transcript. This module
//! is that panel's state, shared by the desktop and phone adapters: it finds
//! the spawns in a seat's ATIF document ([`spawns`]) and keeps the tab strip
//! ([`Tabs`]). It opens nothing on the network and sends nothing to an
//! engine; a tab has no composer, so a subagent is never steered from here.
//!
//! The tab behavior reimplements Zeron's right-surface subagent tabs
//! (`RightSurface::Subagent` in `crates/ui/src/shell.rs`, public MIT
//! zeronsh/zeron at `9e1a1115`): opening a spawn that already has a tab
//! focuses that tab, a spawn inside a subagent's transcript gets a tab of its
//! own, a running subagent follows its newest step while a finished one reads
//! from the top, and closing the active tab falls back to a neighbor. Unlike
//! Zeron, which opens a tab only from a spawn chip, [`Tabs::sync`] adds a tab
//! for every spawn the trace shows, without moving focus, and a tab the
//! person closed stays closed.
//!
//! # Which engines' steps identify a spawn
//!
//! A spawn is a tool call named [`SPAWN_TOOLS`], or any call whose
//! observation carries ATIF's `subagent_trajectory_ref`. The subagent's own
//! steps are the trajectory that reference names in the document's
//! `subagent_trajectories`; without one, the tab shows the spawn's prompt
//! and result only ([`Spawn::steps`] is `None`). What each route records
//! today:
//!
//! - **Coder chat delegation** (`openagents_chat::thread::trajectory_with`):
//!   writes the reference and embeds the task's trajectories, so the tab has
//!   every step.
//! - **OpenCode, Grok Build, Devin** (ACP, `microcoder::repository::devin`'s
//!   recorder): a completed tool call is a step named by the tool the agent
//!   reports in `_meta`, else by its ACP kind. A spawn is identified when
//!   that name is `task` or `spawn_subagent`; an agent that reports only the
//!   kind (`other`) leaves its spawn unidentified. The child's steps are not
//!   in the parent's trace: OpenCode keeps them in a child session (listed by
//!   `coder-history` as a subagent chat), and Devin streams them into the
//!   parent's session untagged, marking only their usage.
//! - **Claude Code and Codex sessions** (`coder_delegate::stream`): the
//!   normalized stream keeps commands, file changes, and replies, and drops
//!   the `Task`/`Agent` and `spawn_agent` calls, so no spawn reaches the
//!   trace. Claude Code's stream does tag a child's messages with
//!   `parent_tool_use_id`; recording that is the gap to close.
//! - **Microcoder loop**: spawns no subagents.

use serde_json::Value;
use std::collections::BTreeSet;

/// Tool names engines give a subagent spawn: Claude Code's `Task` (renamed
/// `Agent`), OpenCode's `task`, Codex's `spawn_agent`, and Grok Build's
/// `spawn_subagent`. A name of the form `Agent: description` is also a spawn.
pub const SPAWN_TOOLS: [&str; 5] = ["Task", "Agent", "task", "spawn_agent", "spawn_subagent"];

/// Spawn-input keys that name the child's model, in precedence order;
/// engines spell the key differently.
pub const MODEL_KEYS: [&str; 4] = ["model", "modelId", "model_id", "subagent_model"];

/// The title of a spawn that names no description.
pub const UNTITLED: &str = "Subagent";

/// Where a subagent stands, from its spawn call's observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// No result yet.
    Running,
    Done,
    Failed,
    Stopped,
}

impl State {
    /// The state's word, as a tab shows it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            State::Running => "running",
            State::Done => "done",
            State::Failed => "failed",
            State::Stopped => "stopped",
        }
    }
}

/// One subagent a trace shows a spawn of.
#[derive(Clone, Debug, PartialEq)]
pub struct Spawn {
    /// The spawn call's IDs from the seat's own trace down to this one, so a
    /// nested spawn is told apart from a same-named call in another child.
    pub path: Vec<String>,
    pub title: String,
    /// The subagent's role, such as `general-purpose`, when the spawn named
    /// one.
    pub agent: Option<String>,
    /// The child's model, when the spawn named one; otherwise it inherits
    /// the parent's.
    pub model: Option<String>,
    /// What the parent asked the subagent.
    pub prompt: Option<String>,
    /// What the subagent returned to the parent.
    pub result: Option<String>,
    pub state: State,
    /// The subagent's own ATIF steps, when the document embeds its
    /// trajectory; `None` when the trace does not hold them.
    pub steps: Option<Vec<Value>>,
}

impl Spawn {
    /// The spawn call's ID in the trace that made it.
    #[must_use]
    pub fn call_id(&self) -> &str {
        self.path.last().map_or("", String::as_str)
    }
}

/// Every spawn in `document`, an ATIF document, in step order, each one's
/// nested spawns right after it.
#[must_use]
pub fn spawns(document: &Value) -> Vec<Spawn> {
    let mut found = Vec::new();
    collect(document, document, &[], &mut found);
    found
}

/// Whether a tool named `name` spawns a subagent.
#[must_use]
pub fn is_spawn_tool(name: &str) -> bool {
    SPAWN_TOOLS.contains(&name) || name.starts_with("Agent: ")
}

/// The most levels of nested spawns read, so a self-referencing document
/// cannot recurse without end.
const MAX_DEPTH: usize = 8;

fn collect(root: &Value, document: &Value, path: &[String], found: &mut Vec<Spawn>) {
    if path.len() >= MAX_DEPTH {
        return;
    }
    for step in document["steps"].as_array().into_iter().flatten() {
        let results = step["observation"]["results"].as_array();
        for call in step["tool_calls"].as_array().into_iter().flatten() {
            let id = call["tool_call_id"].as_str().unwrap_or_default();
            let result = results
                .into_iter()
                .flatten()
                .find(|result| result["source_call_id"].as_str() == Some(id));
            let reference = result
                .and_then(|result| result["subagent_trajectory_ref"].as_array())
                .and_then(|refs| refs.first());
            let name = call["function_name"].as_str().unwrap_or_default();
            if !is_spawn_tool(name) && reference.is_none() {
                continue;
            }
            let mut spawn_path = path.to_vec();
            spawn_path.push(id.to_owned());
            let child = reference.and_then(|reference| trajectory(root, reference));
            let spawn = spawn(name, call, step, result, child, spawn_path.clone());
            found.push(spawn);
            if let Some(child) = child {
                collect(root, child, &spawn_path, found);
            }
        }
    }
}

/// The embedded trajectory `reference` names, by trajectory ID, else by
/// session ID.
fn trajectory<'a>(root: &'a Value, reference: &Value) -> Option<&'a Value> {
    let embedded = root["subagent_trajectories"].as_array()?;
    let by = move |key: &str| {
        let wanted = reference[key].as_str()?;
        embedded
            .iter()
            .find(|document| document[key].as_str() == Some(wanted))
    };
    by("trajectory_id").or_else(|| by("session_id"))
}

fn text(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn spawn(
    name: &str,
    call: &Value,
    step: &Value,
    result: Option<&Value>,
    child: Option<&Value>,
    path: Vec<String>,
) -> Spawn {
    let arguments = &call["arguments"];
    let title = text(arguments, &["description", "title", "task_name"])
        .or_else(|| name.strip_prefix("Agent: ").map(str::to_owned))
        .or_else(|| text(&step["extra"], &["purpose"]))
        .or_else(|| text(arguments, &["subagent_type", "agent_type"]))
        .unwrap_or_else(|| UNTITLED.to_owned());
    let state = match result {
        None => State::Running,
        Some(result) => match result["extra"]["status"].as_str() {
            Some("failed") => State::Failed,
            Some("cancelled" | "stopped") => State::Stopped,
            _ => State::Done,
        },
    };
    Spawn {
        path,
        title,
        agent: text(arguments, &["subagent_type", "agent_type"]),
        model: text(arguments, &MODEL_KEYS),
        prompt: text(arguments, &["prompt", "message", "instructions"]),
        result: result.and_then(|result| text(result, &["content"])),
        state,
        steps: child.and_then(|child| child["steps"].as_array().cloned()),
    }
}

/// What the seat panel shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// The seat's own transcript.
    Parent,
    /// The subagent tab with this ID.
    Subagent(u64),
}

/// One subagent tab.
#[derive(Clone, Debug, PartialEq)]
pub struct Tab {
    /// Unique within its strip, never reused.
    pub id: u64,
    pub spawn: Spawn,
}

impl Tab {
    /// Whether the transcript follows its newest step, as a running
    /// subagent's does; a finished one reads from the top.
    #[must_use]
    pub fn follows_end(&self) -> bool {
        self.spawn.state == State::Running
    }

    /// The tab's label: the title, and the child's model when the spawn
    /// named one.
    #[must_use]
    pub fn label(&self) -> String {
        match &self.spawn.model {
            Some(model) => format!("{} · {model}", self.spawn.title),
            None => self.spawn.title.clone(),
        }
    }
}

/// The tab strip of one seat panel: the parent's transcript, then one tab
/// per subagent, in the order they opened unless the person moved them.
#[derive(Clone, Debug)]
pub struct Tabs {
    tabs: Vec<Tab>,
    active: Surface,
    next: u64,
    /// Spawn paths whose tab the person closed; [`Tabs::sync`] leaves them
    /// closed and only [`Tabs::open`] reopens one.
    closed: BTreeSet<Vec<String>>,
}

impl Default for Tabs {
    fn default() -> Self {
        Tabs {
            tabs: Vec::new(),
            active: Surface::Parent,
            next: 1,
            closed: BTreeSet::new(),
        }
    }
}

impl Tabs {
    /// The open tabs, in strip order.
    #[must_use]
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// What the panel shows.
    #[must_use]
    pub fn active(&self) -> Surface {
        self.active
    }

    /// The tab with `id`.
    #[must_use]
    pub fn tab(&self, id: u64) -> Option<&Tab> {
        self.tabs.iter().find(|tab| tab.id == id)
    }

    /// The active subagent tab, when a subagent is showing.
    #[must_use]
    pub fn active_tab(&self) -> Option<&Tab> {
        match self.active {
            Surface::Parent => None,
            Surface::Subagent(id) => self.tab(id),
        }
    }

    /// Adds a tab for every spawn in `document` that has none and that the
    /// person did not close, and refreshes the open tabs, without moving
    /// focus. Returns the IDs of the tabs it added.
    pub fn sync(&mut self, document: &Value) -> Vec<u64> {
        let mut added = Vec::new();
        for spawn in spawns(document) {
            if let Some(tab) = self
                .tabs
                .iter_mut()
                .find(|tab| tab.spawn.path == spawn.path)
            {
                tab.spawn = spawn;
            } else if !self.closed.contains(&spawn.path) {
                added.push(self.push(spawn));
            }
        }
        added
    }

    /// Shows `spawn`'s tab, opening one when it has none (a closed tab
    /// reopens), and returns its ID.
    pub fn open(&mut self, spawn: Spawn) -> u64 {
        self.closed.remove(&spawn.path);
        let id = match self
            .tabs
            .iter_mut()
            .find(|tab| tab.spawn.path == spawn.path)
        {
            Some(tab) => {
                tab.spawn = spawn;
                tab.id
            }
            None => self.push(spawn),
        };
        self.active = Surface::Subagent(id);
        id
    }

    fn push(&mut self, spawn: Spawn) -> u64 {
        let id = self.next;
        self.next += 1;
        self.tabs.push(Tab { id, spawn });
        id
    }

    /// Shows `surface`; a tab that is not open leaves the panel as it was.
    /// Returns whether the panel shows `surface`.
    pub fn select(&mut self, surface: Surface) -> bool {
        if let Surface::Subagent(id) = surface
            && self.tab(id).is_none()
        {
            return false;
        }
        self.active = surface;
        true
    }

    /// Closes the tab with `id`. Closing the active tab shows the tab after
    /// it, else the one before, else the parent. Returns whether a tab
    /// closed.
    pub fn close(&mut self, id: u64) -> bool {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return false;
        };
        let tab = self.tabs.remove(index);
        self.closed.insert(tab.spawn.path);
        if self.active == Surface::Subagent(id) {
            self.active = self
                .tabs
                .get(index)
                .or_else(|| {
                    index
                        .checked_sub(1)
                        .and_then(|before| self.tabs.get(before))
                })
                .map_or(Surface::Parent, |tab| Surface::Subagent(tab.id));
        }
        true
    }

    /// Moves the tab with `id` to strip position `to`, clamped to the strip.
    /// Returns whether it was open.
    pub fn reorder(&mut self, id: u64, to: usize) -> bool {
        let Some(from) = self.tabs.iter().position(|tab| tab.id == id) else {
            return false;
        };
        let tab = self.tabs.remove(from);
        self.tabs.insert(to.min(self.tabs.len()), tab);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const FIXTURE: &str = include_str!("../fixtures/subagent-steps.atif.json");

    fn fixture() -> Value {
        serde_json::from_str(FIXTURE).unwrap()
    }

    fn path(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    #[test]
    fn the_fixture_spawns_one_tab_per_subagent_with_nested_ones_after_their_parent() {
        let found = spawns(&fixture());
        let paths: Vec<_> = found.iter().map(|spawn| spawn.path.clone()).collect();
        assert_eq!(
            paths,
            [
                path(&["toolu_audit"]),
                path(&["toolu_audit", "toolu_nested"]),
                path(&["call_flaky"]),
                path(&["call_docs"]),
            ]
        );
        let audit = &found[0];
        assert_eq!(audit.title, "Audit the auth flow");
        assert_eq!(audit.agent.as_deref(), Some("general-purpose"));
        assert_eq!(audit.model.as_deref(), Some("haiku"));
        assert_eq!(audit.state, State::Done);
        assert_eq!(audit.steps.as_ref().map(Vec::len), Some(3));
        assert_eq!(
            audit.result.as_deref(),
            Some("Grants are checked in three places.")
        );
        // OpenCode-style: the trace holds the spawn, not the child's steps.
        assert_eq!(found[2].state, State::Failed);
        assert_eq!(found[2].steps, None);
        // No observation yet: still running, Grok's `model_id` read.
        assert_eq!(found[3].state, State::Running);
        assert_eq!(found[3].model.as_deref(), Some("grok-code"));
    }

    #[test]
    fn an_ordinary_call_with_a_model_argument_is_not_a_spawn() {
        assert!(
            spawns(&fixture())
                .iter()
                .all(|spawn| spawn.call_id() != "call_ls")
        );
        assert!(is_spawn_tool("Agent: scan the repository"));
        assert!(!is_spawn_tool("Agents"));
        assert!(!is_spawn_tool("Bash"));
    }

    #[test]
    fn a_trajectory_reference_alone_marks_a_spawn_and_falls_back_to_the_session_id() {
        let document = json!({
            "steps": [{"tool_calls": [{"tool_call_id": "d1", "function_name": "delegate",
                                        "arguments": {}}],
                       "extra": {"purpose": "Hand the task to Codex"},
                       "observation": {"results": [{"source_call_id": "d1", "content": "ok",
                           "subagent_trajectory_ref": [{"session_id": "task-1"}]}]}}],
            "subagent_trajectories": [{"session_id": "task-1", "steps": [{"message": "hi"}]}],
        });
        let found = spawns(&document);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Hand the task to Codex");
        assert_eq!(found[0].steps.as_ref().map(Vec::len), Some(1));
    }

    #[test]
    fn a_self_referencing_document_stops_at_the_depth_bound() {
        let document = json!({
            "session_id": "loop",
            "steps": [{"tool_calls": [{"tool_call_id": "a", "function_name": "Task", "arguments": {}}],
                       "observation": {"results": [{"source_call_id": "a",
                           "subagent_trajectory_ref": [{"session_id": "loop"}]}]}}],
            "subagent_trajectories": [{"session_id": "loop",
                "steps": [{"tool_calls": [{"tool_call_id": "a", "function_name": "Task", "arguments": {}}],
                           "observation": {"results": [{"source_call_id": "a",
                               "subagent_trajectory_ref": [{"session_id": "loop"}]}]}}]}],
        });
        assert_eq!(spawns(&document).len(), MAX_DEPTH);
    }

    #[test]
    fn sync_adds_tabs_without_moving_focus_and_refreshes_open_ones() {
        let mut tabs = Tabs::default();
        let mut document = fixture();
        assert_eq!(tabs.sync(&document), [1, 2, 3, 4]);
        assert_eq!(tabs.active(), Surface::Parent);
        let docs = tabs.tab(4).unwrap();
        assert!(docs.follows_end());
        assert_eq!(docs.label(), "Check the docs · grok-code");
        assert!(!tabs.tab(1).unwrap().follows_end());

        // The running spawn finishes: the same tab updates, none is added.
        document["steps"][4]["observation"] = json!({"results": [{"source_call_id": "call_docs", "content": "Docs agree.",
                                "extra": {"status": "completed"}}]});
        assert!(tabs.sync(&document).is_empty());
        let docs = tabs.tab(4).unwrap();
        assert_eq!(docs.spawn.state, State::Done);
        assert!(!docs.follows_end());
    }

    #[test]
    fn opening_a_spawn_with_a_tab_focuses_it_and_a_closed_tab_stays_closed_until_opened() {
        let mut tabs = Tabs::default();
        let document = fixture();
        tabs.sync(&document);
        let flaky = spawns(&document)[2].clone();
        assert_eq!(tabs.open(flaky.clone()), 3);
        assert_eq!(tabs.tabs().len(), 4);
        assert_eq!(
            tabs.active_tab().unwrap().spawn.title,
            "Find the flaky relay test"
        );

        assert!(tabs.close(3));
        assert!(tabs.sync(&document).is_empty());
        assert!(tabs.tab(3).is_none());
        // Reopening makes a new tab; IDs are never reused.
        assert_eq!(tabs.open(flaky), 5);
        assert_eq!(tabs.active(), Surface::Subagent(5));
    }

    #[test]
    fn closing_the_active_tab_falls_back_to_the_next_then_the_previous_then_the_parent() {
        let mut tabs = Tabs::default();
        tabs.sync(&fixture());
        assert!(tabs.select(Surface::Subagent(2)));
        tabs.close(2);
        assert_eq!(tabs.active(), Surface::Subagent(3));
        tabs.select(Surface::Subagent(4));
        tabs.close(4);
        assert_eq!(tabs.active(), Surface::Subagent(3));
        // Closing an inactive tab leaves focus alone.
        tabs.close(1);
        assert_eq!(tabs.active(), Surface::Subagent(3));
        tabs.close(3);
        assert_eq!(tabs.active(), Surface::Parent);
        assert!(!tabs.close(3));
    }

    #[test]
    fn selecting_a_missing_tab_changes_nothing_and_tabs_reorder() {
        let mut tabs = Tabs::default();
        tabs.sync(&fixture());
        assert!(!tabs.select(Surface::Subagent(99)));
        assert_eq!(tabs.active(), Surface::Parent);
        assert!(tabs.reorder(4, 0));
        assert!(tabs.reorder(1, 99));
        let order: Vec<u64> = tabs.tabs().iter().map(|tab| tab.id).collect();
        assert_eq!(order, [4, 2, 3, 1]);
        assert!(!tabs.reorder(99, 0));
    }

    #[test]
    fn a_document_without_spawns_has_no_tabs() {
        let mut tabs = Tabs::default();
        assert!(
            tabs.sync(&json!({"steps": [{"source": "user", "message": "hi"}]}))
                .is_empty()
        );
        assert!(tabs.sync(&json!({})).is_empty());
        assert!(tabs.tabs().is_empty());
    }
}
