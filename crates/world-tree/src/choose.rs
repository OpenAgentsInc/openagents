//! Grounding an action in a place: where the paper asks a model to walk
//! down the tree, an agent here asks a [`Choose`] for one child of the
//! current node at a time, from the children it knows that offer what the
//! activity needs and that nobody occupies, with `none` always an answer.
//! The live chooser is a Jev `choice` over supplied options
//! (`questions/world-place.json`, in `coder`); tests and townsfolk use
//! [`First`] and [`Scripted`]. Code then resolves the chosen node to its
//! standing point ([`Tree::stand`]) and the zone's navigation routes there.

use std::collections::{BTreeSet, VecDeque};

use crate::{Affordance, Kind, Known, Node, Tree};

/// One question to a chooser: which of `options`, children of `at`, suits
/// `activity`, or none of them.
#[derive(Clone, Debug)]
pub struct Ask<'t> {
    pub agent: &'t str,
    /// The activity in the agent's words, such as `buy bread for supper`.
    pub activity: &'t str,
    pub at: &'t Node,
    pub options: Vec<&'t Node>,
}

/// Picks one option of an [`Ask`], by its slug, or none.
pub trait Choose {
    /// The chosen option's slug ([`Node::slug`]), or `None` when no option
    /// suits the activity.
    ///
    /// # Errors
    ///
    /// When the chooser can't answer, such as a failed call.
    fn choose(&mut self, ask: &Ask<'_>) -> Result<Option<String>, String>;
}

/// Always the first option: a deterministic stand-in for tests.
#[derive(Clone, Copy, Debug, Default)]
pub struct First;

impl Choose for First {
    fn choose(&mut self, ask: &Ask<'_>) -> Result<Option<String>, String> {
        Ok(ask.options.first().map(|n| n.slug().to_owned()))
    }
}

/// Answers in order from a script; `None` is the `none` answer. Records
/// every ask's option slugs.
#[derive(Clone, Debug, Default)]
pub struct Scripted {
    pub answers: VecDeque<Option<String>>,
    pub asked: Vec<Vec<String>>,
}

impl Scripted {
    #[must_use]
    pub fn new<I: IntoIterator<Item = Option<&'static str>>>(answers: I) -> Self {
        Self {
            answers: answers.into_iter().map(|a| a.map(str::to_owned)).collect(),
            asked: Vec::new(),
        }
    }
}

impl Choose for Scripted {
    fn choose(&mut self, ask: &Ask<'_>) -> Result<Option<String>, String> {
        self.asked
            .push(ask.options.iter().map(|n| n.slug().to_owned()).collect());
        self.answers
            .pop_front()
            .ok_or_else(|| "the script has no more answers".to_owned())
    }
}

/// Whether `node` or anything under it that `known` knows offers `need`.
fn serves(tree: &Tree, known: Option<&Known>, node: &Node, need: Affordance) -> bool {
    node.offers(need)
        || tree
            .descendants(&node.id)
            .into_iter()
            .any(|n| n.offers(need) && known.is_none_or(|k| k.knows(&n.id)))
}

/// The children of `at` an agent may choose: those `known` knows (every
/// child when `known` is `None`), that offer `need` themselves or through
/// something under them, and that aren't an exclusive object in
/// `occupied`.
#[must_use]
pub fn options<'t>(
    tree: &'t Tree,
    known: Option<&Known>,
    at: &str,
    need: Option<Affordance>,
    occupied: &BTreeSet<String>,
) -> Vec<&'t Node> {
    tree.children(at)
        .filter(|n| known.is_none_or(|k| k.knows(&n.id)))
        .filter(|n| !(n.exclusive && occupied.contains(&n.id)))
        .filter(|n| need.is_none_or(|need| serves(tree, known, n, need)))
        .collect()
}

/// Where a walk down the tree ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Descent {
    /// The nodes chosen, from the first step down.
    pub path: Vec<String>,
    /// How many questions the chooser answered.
    pub asked: usize,
}

impl Descent {
    /// The deepest chosen node, which the agent walks to; `None` when the
    /// first answer was `none` or nothing was offered.
    #[must_use]
    pub fn chosen(&self) -> Option<&str> {
        self.path.last().map(String::as_str)
    }
}

/// Walks down from node `from`, asking `chooser` for one child at a time
/// among [`options`], until it picks an object, answers `none`, or nothing
/// is left to offer.
///
/// # Errors
///
/// When `from` isn't in the tree, the chooser fails, or it names
/// something it wasn't offered.
#[allow(clippy::too_many_arguments)]
pub fn descend(
    tree: &Tree,
    known: Option<&Known>,
    chooser: &mut dyn Choose,
    agent: &str,
    activity: &str,
    need: Option<Affordance>,
    occupied: &BTreeSet<String>,
    from: &str,
) -> Result<Descent, String> {
    let mut at = tree
        .node(from)
        .ok_or_else(|| format!("{from} isn't in the tree"))?;
    let mut descent = Descent {
        path: Vec::new(),
        asked: 0,
    };
    while at.kind != Kind::Object {
        let offered = options(tree, known, &at.id, need, occupied);
        if offered.is_empty() {
            break;
        }
        let ask = Ask {
            agent,
            activity,
            at,
            options: offered,
        };
        descent.asked += 1;
        let Some(slug) = chooser.choose(&ask)? else {
            break;
        };
        let next = ask
            .options
            .iter()
            .find(|n| n.slug() == slug)
            .ok_or_else(|| format!("the chooser named {slug}, which wasn't offered"))?;
        descent.path.push(next.id.clone());
        at = next;
    }
    Ok(descent)
}
