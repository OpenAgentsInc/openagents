//! Grounding a workshop agent's plan in a place
//! (`docs/verse/generative-agents.md`, item 3): the live Jev chooser for a
//! walk down the world tree, and the agent's known subgraph on disk.
//!
//! A plan names a world-tree node, never coordinates. To pick one, code
//! walks down from the zone ([`world_tree::descend`]), asking a
//! [`Choose`] for one child at a time among the children the agent knows
//! that fit the activity and are free. [`JevChooser`] asks the
//! `questions/world-place.json` set; tests use the fakes in
//! [`world_tree::choose`]. Alice keeps what she knows of the tree in
//! `agents/NAME/known.json` ([`known_path`]); the tree itself is the
//! checked-in snapshot ([`world_tree::everglade`]), so this crate never
//! links the zone.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use jev::Answer;
use serde_json::json;
use world_tree::{Ask, Choose, Known, Node, Tree};

use crate::questions::{Fill, Set};

const SET_JSON: &str = include_str!("../../../../questions/world-place.json");

static SET: LazyLock<Set> = LazyLock::new(|| {
    let set: Set = serde_json::from_str(SET_JSON).expect("the world-place set parses");
    set.validate()
        .expect("the world-place set is one this host asks");
    set
});

/// The place-choice question set.
#[must_use]
pub fn place_set() -> &'static Set {
    &SET
}

/// The known subgraph of the agent whose store is `dir` (`agents/NAME`).
#[must_use]
pub fn known_path(dir: &Path) -> PathBuf {
    dir.join("known.json")
}

/// The agent's known subgraph of `tree` from `dir`, moved to `tree` when it
/// was made against another, or a new one knowing only the root.
///
/// # Errors
///
/// When the file exists and can't be read.
pub fn load_known(dir: &Path, agent: &str, tree: &Tree) -> Result<Known, String> {
    let mut known = Known::load(&known_path(dir))?.unwrap_or_else(|| Known::new(agent, tree));
    if known.tree != tree.digest() {
        known.rebase(tree);
    }
    Ok(known)
}

/// An option's description: its name, kind, and what it offers.
fn describe(node: &Node) -> String {
    let kind = match node.object {
        Some(object) => object.as_str().replace('-', " "),
        None => node.kind.as_str().to_owned(),
    };
    let offers: Vec<&str> = node.affordances.iter().map(|a| a.as_str()).collect();
    if offers.is_empty() {
        format!("{} ({kind})", node.name)
    } else {
        format!("{} ({kind}), offers {}", node.name, offers.join(", "))
    }
}

/// The request one [`Ask`] makes of Jev.
///
/// # Errors
///
/// When the ask offers nothing or its state is over the set's bound.
pub fn place_request(ask: &Ask<'_>) -> Result<jev::SystemOneRequest, String> {
    let state = json!({
        "agent": ask.agent,
        "activity": ask.activity,
        "at": {"name": ask.at.name, "kind": ask.at.kind.as_str()},
    });
    let size = serde_json::to_vec(&state).map_or(usize::MAX, |b| b.len());
    if let Some(max) = SET.policy.state_max_bytes
        && size as u64 > max
    {
        return Err(format!("the state is {size} bytes, over the set's {max}"));
    }
    let options = ask
        .options
        .iter()
        .map(|n| (n.slug().to_owned(), describe(n)))
        .collect();
    let questions = SET.build(&Fill::Options(options))?;
    Ok(jev::SystemOneRequest::new(state, questions))
}

/// The chosen slug from Jev's answer to the gate question, or `None` for
/// `none`.
///
/// # Errors
///
/// When there is no answer or it isn't a choice.
pub fn read_answer(answer: Option<&Answer>) -> Result<Option<String>, String> {
    match answer {
        Some(Answer::Choice(answer)) if answer.choice == "none" => Ok(None),
        Some(Answer::Choice(answer)) => Ok(Some(answer.choice.clone())),
        _ => Err("Jev didn't answer the place question".into()),
    }
}

/// Jev picks the place, one level at a time.
pub struct JevChooser {
    client: jev::Client,
    runtime: tokio::runtime::Runtime,
}

impl JevChooser {
    /// # Errors
    /// When the runtime doesn't start.
    pub fn new(client: jev::Client) -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("cannot start a runtime: {e}"))?;
        Ok(Self { client, runtime })
    }
}

impl Choose for JevChooser {
    fn choose(&mut self, ask: &Ask<'_>) -> Result<Option<String>, String> {
        let request = place_request(ask)?;
        let response = self
            .runtime
            .block_on(self.client.system_one(request))
            .map_err(|e| format!("Jev: {e}"))?;
        read_answer(response.answers.get(SET.gate.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use world_tree::{Affordance, Kind, choose::Scripted};

    use super::*;

    #[test]
    fn the_request_offers_the_known_free_children_and_none() {
        let tree = world_tree::everglade();
        let hall = "everglade/commons/workshop-hall/hall";
        let mut known = Known::new("alice", tree);
        known.enter(tree, hall);
        let occupied = BTreeSet::from([format!("{hall}/desk-2")]);
        let options = world_tree::options(
            tree,
            Some(&known),
            hall,
            Some(Affordance::RunCommands),
            &occupied,
        );
        let ask = Ask {
            agent: "alice",
            activity: "run the failing test again",
            at: tree.node(hall).unwrap(),
            options,
        };
        let request = serde_json::Value::Object(place_request(&ask).unwrap().body("jev").unwrap());
        let criteria = &request["questions"]["place"]["criteria"];
        assert!(criteria["none"].is_string(), "{request}");
        assert_eq!(
            criteria["desk-1"],
            "desk 1 (workstation), offers work, run-commands"
        );
        assert!(criteria.get("desk-2").is_none(), "occupied: {criteria}");
        assert!(criteria.get("library").is_none(), "no commands: {criteria}");
        assert_eq!(request["state"]["at"]["kind"], "room");
    }

    #[test]
    fn a_walk_with_a_fake_chooser_ends_at_a_desk_and_the_subgraph_persists() {
        let tree = world_tree::everglade();
        let dir = tempfile::tempdir().unwrap();
        let mut known = load_known(dir.path(), "alice", tree).unwrap();
        assert_eq!(known.of_kind(tree, Kind::Object).len(), 0);
        known.enter(tree, "everglade/knowledge-district/owners-house/great-room");
        known.save(&known_path(dir.path())).unwrap();
        let known = load_known(dir.path(), "alice", tree).unwrap();
        let mut chooser = Scripted::new([
            Some("knowledge-district"),
            Some("owners-house"),
            Some("great-room"),
            Some("console"),
        ]);
        let walk = world_tree::descend(
            tree,
            Some(&known),
            &mut chooser,
            "alice",
            "run the build",
            Some(Affordance::RunCommands),
            &BTreeSet::new(),
            tree.root().id.as_str(),
        )
        .unwrap();
        assert_eq!(
            walk.chosen(),
            Some("everglade/knowledge-district/owners-house/great-room/console")
        );
        // She knew only the great room, so each level offered one place.
        assert!(chooser.asked.iter().take(3).all(|a| a.len() == 1));
    }

    #[test]
    fn the_answer_reads_none_as_staying() {
        let choice = |choice: &str| {
            Answer::Choice(jev::ChoiceAnswer {
                choice: choice.into(),
                confidence: 0.9,
                probabilities: [(choice.to_owned(), 0.9)].into_iter().collect(),
            })
        };
        assert_eq!(read_answer(Some(&choice("none"))), Ok(None));
        assert_eq!(read_answer(Some(&choice("hall"))), Ok(Some("hall".into())));
        assert!(read_answer(None).is_err());
    }
}
