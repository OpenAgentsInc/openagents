//! Typed queries over a graph. Every answer is a short text of
//! `file:line` rows, never source.

use std::collections::{BTreeSet, HashMap, VecDeque};

use crate::model::{Edge, EdgeKind, Graph, Kind};

/// The most rows one answer lists.
pub const DEFAULT_LIMIT: usize = 40;
/// The most candidates an ambiguous name lists.
pub const MAX_CANDIDATES: usize = 15;

/// A graph with its lookups.
pub struct Index {
    pub graph: Graph,
    incoming: Vec<Vec<u32>>,
    outgoing: Vec<Vec<u32>>,
    children: Vec<Vec<u32>>,
    by_qual: HashMap<String, Vec<u32>>,
    by_name: HashMap<String, Vec<u32>>,
    by_file: HashMap<String, u32>,
}

/// A symbol named by a query.
#[derive(Debug, PartialEq, Eq)]
pub enum Found {
    One(u32),
    /// The name matches several symbols; the caller picks one.
    Many(Vec<u32>),
    None,
}

impl Index {
    #[must_use]
    pub fn new(graph: Graph) -> Index {
        let n = graph.nodes.len();
        let mut incoming = vec![Vec::new(); n];
        let mut outgoing = vec![Vec::new(); n];
        for (at, edge) in graph.edges.iter().enumerate() {
            let at = u32::try_from(at).unwrap_or(u32::MAX);
            incoming[edge.to as usize].push(at);
            outgoing[edge.from as usize].push(at);
        }
        let mut children = vec![Vec::new(); n];
        let mut by_qual: HashMap<String, Vec<u32>> = HashMap::new();
        let mut by_name: HashMap<String, Vec<u32>> = HashMap::new();
        let mut by_file = HashMap::new();
        for (id, node) in graph.nodes.iter().enumerate() {
            let id = u32::try_from(id).unwrap_or(u32::MAX);
            if let Some(parent) = node.parent {
                children[parent as usize].push(id);
            }
            if node.kind == Kind::Module && node.line == 1 {
                by_file
                    .entry(graph.files[node.file as usize].clone())
                    .or_insert(id);
            }
            if node.kind != Kind::Impl {
                by_qual.entry(node.qual.clone()).or_default().push(id);
                by_name.entry(node.name.clone()).or_default().push(id);
            }
        }
        Index {
            graph,
            incoming,
            outgoing,
            children,
            by_qual,
            by_name,
            by_file,
        }
    }

    fn edge(&self, at: u32) -> &Edge {
        &self.graph.edges[at as usize]
    }

    /// The node's one-row description: `qual (kind) file:line`.
    #[must_use]
    pub fn row(&self, id: u32) -> String {
        let node = &self.graph.nodes[id as usize];
        format!(
            "{} ({}) {}",
            node.qual,
            node.kind.code(),
            self.graph.location(id)
        )
    }

    /// Find what `symbol` names: a qualified name (`crate::module::Type::fn`),
    /// any `::`-suffix of one (`Type::fn`, `fn`), a file path (its module),
    /// or `path/file.rs:LINE` (the innermost item on that line).
    #[must_use]
    pub fn find(&self, symbol: &str, kind: Option<Kind>) -> Found {
        let symbol = symbol.trim().trim_start_matches("::");
        if symbol.is_empty() {
            return Found::None;
        }
        let accept = |id: &u32| kind.is_none_or(|k| self.graph.nodes[*id as usize].kind == k);
        if let Some((file, line)) = symbol.rsplit_once(':')
            && !file.ends_with(':')
            && let Ok(line) = line.parse::<u32>()
        {
            return self.at_line(file, line);
        }
        if let Some(id) = self.by_file.get(symbol) {
            return Found::One(*id);
        }
        let pick = |ids: Vec<u32>| match ids.len() {
            0 => Found::None,
            1 => Found::One(ids[0]),
            _ => {
                // Several items share one qualified name (cfg variants):
                // when they are one module's, take the first.
                let first = &self.graph.nodes[ids[0] as usize];
                if ids.iter().all(|id| {
                    let node = &self.graph.nodes[*id as usize];
                    node.qual == first.qual && node.kind == first.kind
                }) {
                    Found::One(ids[0])
                } else {
                    Found::Many(ids)
                }
            }
        };
        if let Some(ids) = self.by_qual.get(symbol) {
            let ids: Vec<u32> = ids.iter().copied().filter(accept).collect();
            if !ids.is_empty() {
                return pick(ids);
            }
        }
        let name = symbol.rsplit("::").next().unwrap_or(symbol);
        let suffix = format!("::{symbol}");
        let ids: Vec<u32> = self
            .by_name
            .get(name)
            .into_iter()
            .flatten()
            .copied()
            .filter(accept)
            .filter(|id| {
                let qual = &self.graph.nodes[*id as usize].qual;
                qual == symbol || qual.ends_with(&suffix)
            })
            .collect();
        pick(ids)
    }

    fn at_line(&self, file: &str, line: u32) -> Found {
        let Some(module) = self.by_file.get(file) else {
            return Found::None;
        };
        let file_id = self.graph.nodes[*module as usize].file;
        let best = self
            .graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.file == file_id && node.line <= line && line <= node.end_line)
            .min_by_key(|(_, node)| node.end_line - node.line)
            .map(|(id, _)| u32::try_from(id).unwrap_or(u32::MAX));
        best.map_or(Found::None, Found::One)
    }

    /// The answer for a name that matched nothing or several symbols.
    fn unresolved(&self, symbol: &str, found: Found) -> Result<u32, String> {
        match found {
            Found::One(id) => Ok(id),
            Found::None => Err(format!(
                "No symbol `{symbol}` in the graph. Give a qualified name (crate::module::Item), a Type::method, a file path, or file.rs:LINE."
            )),
            Found::Many(mut ids) => {
                let total = ids.len();
                ids.sort_by_key(|id| {
                    let node = &self.graph.nodes[*id as usize];
                    (u8::from(node.kind == Kind::Module), node.qual.len(), *id)
                });
                let mut out = format!(
                    "ambiguous: {total} symbols match `{symbol}`; ask again with one qualified name:\n"
                );
                for id in ids.iter().take(MAX_CANDIDATES) {
                    out.push_str("  ");
                    out.push_str(&self.row(*id));
                    out.push('\n');
                }
                if total > MAX_CANDIDATES {
                    out.push_str(&format!("  … {} more\n", total - MAX_CANDIDATES));
                }
                Err(out)
            }
        }
    }

    /// Resolve `symbol`, or say why not.
    pub fn resolve(&self, symbol: &str, kind: Option<Kind>) -> Result<u32, String> {
        self.unresolved(symbol, self.find(symbol, kind))
    }

    fn mark(edge: &Edge) -> &'static str {
        if edge.exact { "" } else { " ?" }
    }

    /// What calls or names `symbol`: calls for a function, references and
    /// impls for a type, imports for anything.
    pub fn callers(&self, symbol: &str, limit: usize) -> Result<String, String> {
        let id = self.resolve(symbol, None)?;
        let mut rows: BTreeSet<(String, String, &'static str, &'static str)> = BTreeSet::new();
        for at in &self.incoming[id as usize] {
            let edge = self.edge(*at);
            let from = &self.graph.nodes[edge.from as usize];
            rows.insert((
                self.graph.location(edge.from),
                from.qual.clone(),
                edge.kind.code(),
                Self::mark(edge),
            ));
        }
        Ok(self.listing(
            &format!("callers of {}", self.row(id)),
            rows.into_iter()
                .map(|(location, qual, kind, mark)| format!("{location}  {qual} [{kind}]{mark}")),
            limit,
        ))
    }

    /// What `symbol` calls and names.
    pub fn callees(&self, symbol: &str, limit: usize) -> Result<String, String> {
        let id = self.resolve(symbol, None)?;
        let mut rows: BTreeSet<(String, String, &'static str, &'static str)> = BTreeSet::new();
        for at in &self.outgoing[id as usize] {
            let edge = self.edge(*at);
            let to = &self.graph.nodes[edge.to as usize];
            rows.insert((
                to.qual.clone(),
                self.graph.location(edge.to),
                edge.kind.code(),
                Self::mark(edge),
            ));
        }
        Ok(self.listing(
            &format!("callees of {}", self.row(id)),
            rows.into_iter()
                .map(|(qual, location, kind, mark)| format!("{location}  {qual} [{kind}]{mark}")),
            limit,
        ))
    }

    /// Everything that depends on `symbol` up to `depth` steps back
    /// (callers, referrers, impls), nearest first, grouped by depth.
    /// Inexact edges are followed unless `exact_only`.
    pub fn impact(
        &self,
        symbol: &str,
        depth: usize,
        limit: usize,
        exact_only: bool,
    ) -> Result<String, String> {
        let id = self.resolve(symbol, None)?;
        let depth = depth.clamp(1, 8);
        let mut seen: HashMap<u32, usize> = HashMap::from([(id, 0)]);
        let mut queue = VecDeque::from([id]);
        let mut found: Vec<(usize, u32, bool)> = Vec::new();
        while let Some(at) = queue.pop_front() {
            let level = seen[&at];
            if level >= depth {
                continue;
            }
            let mut sources: Vec<(u32, bool)> = Vec::new();
            for e in &self.incoming[at as usize] {
                let edge = self.edge(*e);
                if edge.kind == EdgeKind::Uses || (exact_only && !edge.exact) {
                    continue;
                }
                sources.push((edge.from, edge.exact));
            }
            // Methods of an impl for a changed type are its dependents too,
            // and so are the callers of the methods of a changed type.
            let node = &self.graph.nodes[at as usize];
            if node.kind.is_type() {
                for e in &self.incoming[at as usize] {
                    let edge = self.edge(*e);
                    if matches!(edge.kind, EdgeKind::ImplFor | EdgeKind::Implements) {
                        for child in &self.children[edge.from as usize] {
                            sources.push((*child, edge.exact));
                        }
                    }
                }
            }
            sources.sort_unstable();
            sources.dedup_by_key(|(from, _)| *from);
            for (from, exact) in sources {
                let from_node = &self.graph.nodes[from as usize];
                // A module is not a dependent; the items in it are.
                if from_node.kind == Kind::Module {
                    continue;
                }
                if let std::collections::hash_map::Entry::Vacant(slot) = seen.entry(from) {
                    slot.insert(level + 1);
                    found.push((level + 1, from, exact));
                    queue.push_back(from);
                }
            }
        }
        found.sort_by(|a, b| {
            (a.0, &self.graph.location(a.1)).cmp(&(b.0, &self.graph.location(b.1)))
        });
        let files: BTreeSet<u32> = found
            .iter()
            .map(|(_, id, _)| self.graph.nodes[*id as usize].file)
            .collect();
        let crates: BTreeSet<&str> = found
            .iter()
            .filter_map(|(_, id, _)| self.graph.nodes[*id as usize].qual.split("::").next())
            .collect();
        let title = format!(
            "impact of {} (depth {depth}): {} dependents in {} files, crates: {}",
            self.row(id),
            found.len(),
            files.len(),
            crates.into_iter().collect::<Vec<_>>().join(", ")
        );
        Ok(self.listing(
            &title,
            found.iter().map(|(level, id, exact)| {
                format!(
                    "d{level} {}  {}{}",
                    self.graph.location(*id),
                    self.graph.nodes[*id as usize].qual,
                    if *exact { "" } else { " ?" }
                )
            }),
            limit,
        ))
    }

    /// The shortest chain of calls and references from `from` to `to`.
    pub fn path(&self, from: &str, to: &str, max_depth: usize) -> Result<String, String> {
        let start = self.resolve(from, None)?;
        let goal = self.resolve(to, None)?;
        let max_depth = max_depth.clamp(1, 12);
        let mut back: HashMap<u32, (u32, u32)> = HashMap::new();
        let mut level: HashMap<u32, usize> = HashMap::from([(start, 0)]);
        let mut queue = VecDeque::from([start]);
        while let Some(at) = queue.pop_front() {
            if at == goal {
                break;
            }
            let depth = level[&at];
            if depth >= max_depth {
                continue;
            }
            for e in &self.outgoing[at as usize] {
                let edge = self.edge(*e);
                if edge.kind == EdgeKind::Uses {
                    continue;
                }
                if let std::collections::hash_map::Entry::Vacant(slot) = level.entry(edge.to) {
                    slot.insert(depth + 1);
                    back.insert(edge.to, (at, *e));
                    queue.push_back(edge.to);
                }
            }
        }
        if !level.contains_key(&goal) {
            return Ok(format!(
                "no path of calls or references from {} to {} within {max_depth} steps",
                self.row(start),
                self.row(goal)
            ));
        }
        let mut chain = vec![(goal, None)];
        let mut at = goal;
        while let Some((prev, e)) = back.get(&at) {
            chain.push((*prev, Some(*e)));
            at = *prev;
        }
        chain.reverse();
        let mut out = format!("path, {} steps:\n", chain.len() - 1);
        let mut via: Option<u32> = None;
        for (id, edge) in chain {
            let arrow = via.map_or(String::new(), |e| {
                let edge = self.edge(e);
                format!("  -[{}{}]-> ", edge.kind.code(), Self::mark(edge))
            });
            out.push_str(&format!("{arrow}{}\n", self.row(id)));
            via = edge;
        }
        Ok(out)
    }

    /// Where `symbol` is defined, its signature, and its neighborhood in
    /// counts.
    pub fn symbol(&self, symbol: &str, kind: Option<Kind>) -> Result<String, String> {
        let id = self.resolve(symbol, kind)?;
        let node = &self.graph.nodes[id as usize];
        let mut out = format!(
            "{} ({}{})\n{}-{}\n",
            node.qual,
            if node.public { "pub " } else { "" },
            node.kind.code(),
            self.graph.location(id),
            node.end_line
        );
        if !node.signature.is_empty() {
            out.push_str(&format!("{}\n", node.signature));
        }
        if let Some(parent) = node.parent {
            out.push_str(&format!("in {}\n", self.row(parent)));
        }
        let count = |edges: &[u32], kinds: &[EdgeKind]| {
            edges
                .iter()
                .filter(|e| kinds.contains(&self.edge(**e).kind))
                .count()
        };
        let incoming = &self.incoming[id as usize];
        let outgoing = &self.outgoing[id as usize];
        out.push_str(&format!(
            "callers {}, referrers {}, importers {}, calls {}, names {}\n",
            count(incoming, &[EdgeKind::Calls]),
            count(incoming, &[EdgeKind::References]),
            count(incoming, &[EdgeKind::Uses]),
            count(outgoing, &[EdgeKind::Calls]),
            count(outgoing, &[EdgeKind::References]),
        ));
        if node.kind.is_type() {
            let impls: Vec<u32> = incoming
                .iter()
                .map(|e| self.edge(*e))
                .filter(|edge| matches!(edge.kind, EdgeKind::ImplFor | EdgeKind::Implements))
                .map(|edge| edge.from)
                .collect();
            if !impls.is_empty() {
                out.push_str("impls:\n");
                for impl_id in impls.iter().take(DEFAULT_LIMIT) {
                    let methods: Vec<&str> = self.children[*impl_id as usize]
                        .iter()
                        .map(|c| self.graph.nodes[*c as usize].name.as_str())
                        .collect();
                    out.push_str(&format!(
                        "  {} {}{}\n",
                        self.graph.location(*impl_id),
                        self.graph.nodes[*impl_id as usize].name,
                        if methods.is_empty() {
                            String::new()
                        } else {
                            format!(": {}", methods.join(", "))
                        }
                    ));
                }
                if impls.len() > DEFAULT_LIMIT {
                    out.push_str(&format!("  … {} more\n", impls.len() - DEFAULT_LIMIT));
                }
            }
        }
        if node.kind == Kind::Trait {
            let methods: Vec<String> = self.children[id as usize]
                .iter()
                .map(|c| {
                    let child = &self.graph.nodes[*c as usize];
                    format!("{}:{}", child.name, child.line)
                })
                .collect();
            if !methods.is_empty() {
                out.push_str(&format!("items: {}\n", methods.join(", ")));
            }
        }
        Ok(out)
    }

    /// A module's public surface: its public items with signatures, its
    /// submodules, and a count of private items.
    pub fn module(&self, module: &str, limit: usize) -> Result<String, String> {
        let id = self.resolve(module, Some(Kind::Module))?;
        let node = &self.graph.nodes[id as usize];
        let mut rows = Vec::new();
        let mut private = 0;
        let mut submodules = Vec::new();
        // A file module's children and the children of its impls (methods
        // on its types are part of its surface).
        let mut stack: Vec<u32> = self.children[id as usize].clone();
        stack.reverse();
        while let Some(child) = stack.pop() {
            let item = &self.graph.nodes[child as usize];
            match item.kind {
                Kind::Module => submodules.push(child),
                Kind::Impl => {
                    let mut inner = self.children[child as usize].clone();
                    inner.reverse();
                    stack.extend(inner);
                }
                _ if item.public => rows.push(format!(
                    "{}  {}",
                    item.line,
                    if item.signature.is_empty() {
                        format!("{} {}", item.kind.code(), item.name)
                    } else {
                        item.signature.clone()
                    }
                )),
                _ => private += 1,
            }
        }
        // Submodules declared by file: modules one level down.
        let prefix = format!("{}::", node.qual);
        let mut seen: BTreeSet<String> = submodules
            .iter()
            .map(|s| self.graph.nodes[*s as usize].qual.clone())
            .collect();
        for (qual, ids) in &self.by_qual {
            if let Some(rest) = qual.strip_prefix(&prefix)
                && !rest.contains("::")
                && let Some(sub) = ids
                    .iter()
                    .find(|i| self.graph.nodes[**i as usize].kind == Kind::Module)
                && seen.insert(qual.clone())
            {
                submodules.push(*sub);
            }
        }
        submodules.sort_by_key(|s| self.graph.nodes[*s as usize].qual.clone());
        let mut out = format!(
            "module {} {} ({} public items, {} private)\n",
            node.qual,
            self.graph.files[node.file as usize],
            rows.len(),
            private
        );
        if !submodules.is_empty() {
            out.push_str(&format!(
                "submodules: {}\n",
                submodules
                    .iter()
                    .map(|s| {
                        let sub = &self.graph.nodes[*s as usize];
                        format!("{} ({})", sub.name, self.graph.files[sub.file as usize])
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        let total = rows.len();
        for row in rows.into_iter().take(limit) {
            out.push_str(&row);
            out.push('\n');
        }
        if total > limit {
            out.push_str(&format!("… {} more\n", total - limit));
        }
        Ok(out)
    }

    fn listing(&self, title: &str, rows: impl Iterator<Item = String>, limit: usize) -> String {
        let rows: Vec<String> = rows.collect();
        let mut out = format!("{title}\n");
        if rows.is_empty() {
            out.push_str("(none found)\n");
        }
        for row in rows.iter().take(limit) {
            out.push_str(row);
            out.push('\n');
        }
        if rows.len() > limit {
            out.push_str(&format!("… {} more (raise limit)\n", rows.len() - limit));
        }
        if rows.iter().any(|row| row.ends_with(" ?")) {
            out.push_str("? = matched by name, not pinned to one definition\n");
        }
        out
    }
}
