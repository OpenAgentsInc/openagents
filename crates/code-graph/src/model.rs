//! The graph and its file format.
//!
//! A graph is one JSON-lines file. The first line is the [`Header`]; then
//! one line per file (`["f",id,path]`), per node
//! (`["n",id,kind,name,qual,file,line,end_line,pub,parent,signature]`,
//! `parent` is -1 for none) and per edge (`["e",from,to,kind,exact]`), in
//! that order and sorted, so the same input gives the same bytes.

use std::io::{BufRead, Write};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The format's name and version.
pub const SCHEMA: &str = "openagents.code-graph.v1";
/// This builder and its extraction rules; bump the number when the rules
/// change, so two graphs from different rules never look alike. It is not
/// the workspace version, so a release alone does not change a graph.
pub const BUILDER: &str = "code-graph extractor-1";
/// The parser, pinned exactly in `Cargo.toml`.
pub const PARSER: &str = "tree-sitter 0.27.0";
/// The grammar, pinned exactly in `Cargo.toml`.
pub const GRAMMAR: &str = "tree-sitter-rust 0.24.2";

/// What a node is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Module,
    Function,
    Method,
    Struct,
    Enum,
    Union,
    Trait,
    Impl,
    TypeAlias,
    Const,
    Static,
    Macro,
}

impl Kind {
    pub const ALL: [Kind; 12] = [
        Kind::Module,
        Kind::Function,
        Kind::Method,
        Kind::Struct,
        Kind::Enum,
        Kind::Union,
        Kind::Trait,
        Kind::Impl,
        Kind::TypeAlias,
        Kind::Const,
        Kind::Static,
        Kind::Macro,
    ];

    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Kind::Module => "mod",
            Kind::Function => "fn",
            Kind::Method => "method",
            Kind::Struct => "struct",
            Kind::Enum => "enum",
            Kind::Union => "union",
            Kind::Trait => "trait",
            Kind::Impl => "impl",
            Kind::TypeAlias => "type",
            Kind::Const => "const",
            Kind::Static => "static",
            Kind::Macro => "macro",
        }
    }

    #[must_use]
    pub fn parse(code: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|kind| kind.code() == code)
    }

    /// A type a path can name in type position.
    #[must_use]
    pub fn is_type(self) -> bool {
        matches!(
            self,
            Kind::Struct | Kind::Enum | Kind::Union | Kind::Trait | Kind::TypeAlias
        )
    }

    /// Something a call expression can call.
    #[must_use]
    pub fn is_callable(self) -> bool {
        matches!(self, Kind::Function | Kind::Method | Kind::Struct)
    }
}

/// What an edge means.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeKind {
    /// `from` calls `to` (a function, method, tuple-struct constructor or
    /// macro).
    Calls,
    /// A module imports `to` with `use`.
    Uses,
    /// An `impl` block implements the trait `to`.
    Implements,
    /// An `impl` block is for the type `to`.
    ImplFor,
    /// `from` names the type, constant or static `to`.
    References,
}

impl EdgeKind {
    pub const ALL: [EdgeKind; 5] = [
        EdgeKind::Calls,
        EdgeKind::Uses,
        EdgeKind::Implements,
        EdgeKind::ImplFor,
        EdgeKind::References,
    ];

    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            EdgeKind::Calls => "calls",
            EdgeKind::Uses => "uses",
            EdgeKind::Implements => "impl",
            EdgeKind::ImplFor => "impl_for",
            EdgeKind::References => "refs",
        }
    }

    #[must_use]
    pub fn parse(code: &str) -> Option<EdgeKind> {
        EdgeKind::ALL.into_iter().find(|kind| kind.code() == code)
    }
}

/// One item of the code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub kind: Kind,
    /// The item's own name (`of`), or for an impl `impl Trait for Type`.
    pub name: String,
    /// The qualified name: crate, modules, then the type for a method
    /// (`coder_new::plugin_tools::Family::of`).
    pub qual: String,
    /// Index into [`Graph::files`].
    pub file: u32,
    /// One-based first and last line.
    pub line: u32,
    pub end_line: u32,
    /// Declared `pub` (any `pub(...)` counts), or a trait's or trait
    /// impl's method.
    pub public: bool,
    /// The enclosing node (a method's impl or trait, an item's module).
    pub parent: Option<u32>,
    /// The declaration up to its body, on one line, at most 200 chars;
    /// empty for modules and impls.
    pub signature: String,
}

/// One relation between two nodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Edge {
    pub from: u32,
    pub to: u32,
    pub kind: EdgeKind,
    /// Resolved to exactly this target; `false` when the name had a few
    /// candidates and each got an edge.
    pub exact: bool,
}

/// The first line: what built the graph, from what.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub schema: String,
    pub builder: String,
    pub parser: String,
    pub grammar: String,
    /// The commit the graph was built from.
    pub commit: String,
    /// Built from a working tree with uncommitted code changes.
    pub dirty: bool,
    pub files: usize,
    pub nodes: usize,
    pub edges: usize,
}

/// A whole graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Graph {
    pub header: Header,
    pub files: Vec<String>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Graph {
    /// The node's `file:line`.
    #[must_use]
    pub fn location(&self, node: u32) -> String {
        let node = &self.nodes[node as usize];
        format!("{}:{}", self.files[node.file as usize], node.line)
    }

    /// Write the JSON-lines form.
    pub fn write(&self, out: &mut impl Write) -> std::io::Result<()> {
        writeln!(out, "{}", serde_json::to_string(&self.header)?)?;
        for (id, path) in self.files.iter().enumerate() {
            writeln!(out, "{}", json!(["f", id, path]))?;
        }
        for (id, node) in self.nodes.iter().enumerate() {
            let parent = node.parent.map_or(-1, i64::from);
            writeln!(
                out,
                "{}",
                json!([
                    "n",
                    id,
                    node.kind.code(),
                    node.name,
                    node.qual,
                    node.file,
                    node.line,
                    node.end_line,
                    u8::from(node.public),
                    parent,
                    node.signature
                ])
            )?;
        }
        for edge in &self.edges {
            writeln!(
                out,
                "{}",
                json!([
                    "e",
                    edge.from,
                    edge.to,
                    edge.kind.code(),
                    u8::from(edge.exact)
                ])
            )?;
        }
        Ok(())
    }

    /// The JSON-lines form as bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.write(&mut out).expect("writing to a Vec cannot fail");
        out
    }

    /// Read the JSON-lines form.
    pub fn read(input: impl BufRead) -> Result<Graph, String> {
        let mut lines = input.lines();
        let header = lines
            .next()
            .ok_or("The graph file is empty.")?
            .map_err(|error| error.to_string())?;
        let header: Header = serde_json::from_str(&header)
            .map_err(|error| format!("The graph's header is unreadable: {error}"))?;
        if header.schema != SCHEMA {
            return Err(format!(
                "The graph is {}, and this reader takes {SCHEMA}.",
                header.schema
            ));
        }
        let mut graph = Graph {
            files: Vec::with_capacity(header.files),
            nodes: Vec::with_capacity(header.nodes),
            edges: Vec::with_capacity(header.edges),
            header,
        };
        for (number, line) in lines.enumerate() {
            let line = line.map_err(|error| error.to_string())?;
            let bad = || format!("The graph's line {} is unreadable.", number + 2);
            let row: Value = serde_json::from_str(&line).map_err(|_| bad())?;
            let row = row.as_array().ok_or_else(bad)?;
            let int = |at: usize| row.get(at).and_then(Value::as_i64).ok_or_else(bad);
            let text = |at: usize| {
                row.get(at)
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(bad)
            };
            match row.first().and_then(Value::as_str) {
                Some("f") => graph.files.push(text(2)?),
                Some("n") => {
                    let parent = int(9)?;
                    graph.nodes.push(Node {
                        kind: Kind::parse(&text(2)?).ok_or_else(bad)?,
                        name: text(3)?,
                        qual: text(4)?,
                        file: u32::try_from(int(5)?).map_err(|_| bad())?,
                        line: u32::try_from(int(6)?).map_err(|_| bad())?,
                        end_line: u32::try_from(int(7)?).map_err(|_| bad())?,
                        public: int(8)? == 1,
                        parent: u32::try_from(parent).ok(),
                        signature: text(10)?,
                    });
                }
                Some("e") => graph.edges.push(Edge {
                    from: u32::try_from(int(1)?).map_err(|_| bad())?,
                    to: u32::try_from(int(2)?).map_err(|_| bad())?,
                    kind: EdgeKind::parse(&text(3)?).ok_or_else(bad)?,
                    exact: int(4)? == 1,
                }),
                _ => return Err(bad()),
            }
        }
        let nodes = graph.nodes.len();
        if graph
            .nodes
            .iter()
            .any(|node| node.file as usize >= graph.files.len())
            || graph
                .edges
                .iter()
                .any(|edge| edge.from as usize >= nodes || edge.to as usize >= nodes)
        {
            return Err("The graph names a file or node it does not have.".into());
        }
        Ok(graph)
    }
}
