//! The tree as text, one node a line, indented by depth: its slug, kind,
//! name, standing point, affordances, and state. The authoring tools and
//! the captures print it.

use std::fmt::Write as _;

use crate::{Known, Node, States, Tree};

fn line(out: &mut String, node: &Node, states: Option<&States>) {
    let depth = node.id.matches('/').count();
    let kind = match node.object {
        Some(object) => format!("{}:{}", node.kind.as_str(), object.as_str()),
        None => node.kind.as_str().to_owned(),
    };
    let _ = write!(
        out,
        "{:indent$}{}  [{kind}]  {:?}  @ {:.1}, {:.1}",
        "",
        node.slug(),
        node.name,
        node.stand[0],
        node.stand[1],
        indent = depth * 2
    );
    if !node.affordances.is_empty() {
        let names: Vec<&str> = node.affordances.iter().map(|a| a.as_str()).collect();
        let _ = write!(out, "  does {}", names.join(", "));
    }
    if node.exclusive {
        out.push_str("  exclusive");
    }
    if let Some(state) = states.and_then(|s| s.get(&node.id)) {
        let _ = write!(out, "  is {}", state.describe());
    }
    out.push('\n');
}

fn header(out: &mut String, tree: &Tree) {
    let _ = writeln!(out, "{} {}", crate::SCHEMA, tree.zone());
    let _ = writeln!(out, "digest {}", tree.digest());
}

/// Every node of `tree`, with `states` when given.
#[must_use]
pub fn dump(tree: &Tree, states: Option<&States>) -> String {
    let mut out = String::new();
    header(&mut out, tree);
    let _ = writeln!(out, "{} nodes", tree.nodes().len());
    for node in tree.walk() {
        line(&mut out, node, states);
    }
    out
}

/// The nodes `known` knows of `tree`, with `states` when given.
#[must_use]
pub fn dump_known(tree: &Tree, known: &Known, states: Option<&States>) -> String {
    let mut out = String::new();
    header(&mut out, tree);
    let nodes: Vec<_> = tree
        .walk()
        .into_iter()
        .filter(|n| known.knows(&n.id))
        .collect();
    let _ = writeln!(
        out,
        "{} knows {} of {} nodes",
        known.agent,
        nodes.len(),
        tree.nodes().len()
    );
    for node in nodes {
        line(&mut out, node, states);
    }
    out
}
