//! Validation: statements into a typed tree, against the catalog.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::catalog::{self, Kind};
use crate::lex::{self, Expr, Statement};

/// A button's look.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonStyle {
    #[default]
    Primary,
    Secondary,
}

/// Who sees a button: a surface that knows whether the reader is signed in
/// draws only the buttons for them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Audience {
    #[default]
    Everyone,
    SignedIn,
    SignedOut,
}

impl Audience {
    /// Whether a reader who is (`Some(true)`), is not, or may be (`None`)
    /// signed in sees it. When it is not known, the signed-in button shows.
    #[must_use]
    pub fn shows(self, signed_in: Option<bool>) -> bool {
        match self {
            Audience::Everyone => true,
            Audience::SignedIn => signed_in != Some(false),
            Audience::SignedOut => signed_in == Some(false),
        }
    }
}

/// One numbered step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
}

/// One tab.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tab {
    pub label: String,
    pub children: Vec<Node>,
}

/// A validated component. Every link is safe ([`safe_href`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Node {
    Stack {
        children: Vec<Node>,
    },
    Columns {
        children: Vec<Node>,
    },
    Card {
        title: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        children: Vec<Node>,
    },
    Text {
        text: String,
    },
    Link {
        label: String,
        href: String,
    },
    Button {
        label: String,
        href: String,
        #[serde(default)]
        style: ButtonStyle,
        #[serde(default)]
        show: Audience,
    },
    CodeBlock {
        code: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        language: Option<String>,
    },
    Command {
        unix: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        windows: Option<String>,
    },
    Steps {
        steps: Vec<Step>,
    },
    Tabs {
        tabs: Vec<Tab>,
    },
    LinkCard {
        title: String,
        description: String,
        href: String,
    },
}

impl Node {
    /// The component's name in the catalog.
    #[must_use]
    pub fn component(&self) -> &'static str {
        match self {
            Node::Stack { .. } => "Stack",
            Node::Columns { .. } => "Columns",
            Node::Card { .. } => "Card",
            Node::Text { .. } => "Text",
            Node::Link { .. } => "Link",
            Node::Button { .. } => "Button",
            Node::CodeBlock { .. } => "CodeBlock",
            Node::Command { .. } => "Command",
            Node::Steps { .. } => "Steps",
            Node::Tabs { .. } => "Tabs",
            Node::LinkCard { .. } => "LinkCard",
        }
    }
}

/// Something the parser fixed or dropped, for logs and for feeding back to
/// a model; never shown to a reader.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// The statement it is about, when one is.
    pub statement: Option<String>,
    pub message: String,
}

/// A parsed program: the tree from `root`, when there is one to draw, and
/// what was dropped on the way.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Document {
    pub root: Option<Node>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Whether `href` may be a link target: an `https://` URL with no spaces or
/// quotes, or a site path (`/download`, not `//host`).
#[must_use]
pub fn safe_href(href: &str) -> bool {
    let clean = !href.is_empty()
        && !href.chars().any(|c| {
            c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '<' | '>' | '\\')
        });
    let https = href
        .get(..8)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
        && href.len() > 8;
    let site = href.starts_with('/') && !href.starts_with("//");
    clean && (https || site)
}

/// The statements, by name; a later definition replaces an earlier one
/// (merge by name).
pub(crate) type Table = BTreeMap<String, Statement>;

/// Resolves `root` in `table` into a tree. `complete` says the program is
/// whole, so a missing required argument, an unknown choice, or a name
/// never defined is a diagnostic instead of something still to come.
pub(crate) fn materialize(
    table: &Table,
    complete: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Node> {
    let Some(root) = table.get("root") else {
        if complete && !table.is_empty() {
            diagnostics.push(Diagnostic {
                statement: None,
                message: "there is no `root` statement".into(),
            });
        }
        return None;
    };
    let mut walk = Walk {
        table,
        complete,
        diagnostics,
        stack: Vec::new(),
    };
    walk.stack.push("root".into());
    let node = walk.node(&root.expr, "root");
    match node {
        Some(Found::Node(node)) => Some(node),
        Some(Found::Inner(_)) => {
            walk.note("root", "`root` is a Step or a Tab outside its list".into());
            None
        }
        None => None,
    }
}

enum Found {
    Node(Node),
    /// A `Step` or `Tab`, which only its list may hold.
    Inner(Inner),
}

enum Inner {
    Step(Step),
    Tab(Tab),
}

struct Walk<'a> {
    table: &'a Table,
    complete: bool,
    diagnostics: &'a mut Vec<Diagnostic>,
    /// The statements being resolved, to refuse a cycle.
    stack: Vec<String>,
}

impl<'a> Walk<'a> {
    fn note(&mut self, statement: &str, message: String) {
        self.diagnostics.push(Diagnostic {
            statement: Some(statement.to_owned()),
            message,
        });
    }

    /// Follows references to the expression they name; `None` for a name
    /// not defined (yet), or a cycle.
    fn resolve<'e>(&mut self, expr: &'e Expr, at: &str) -> Option<(&'e Expr, Option<String>)>
    where
        'a: 'e,
    {
        let Expr::Ref(name) = expr else {
            return Some((expr, None));
        };
        if self.stack.contains(name) {
            self.note(at, format!("`{name}` refers to itself"));
            return None;
        }
        match self.table.get(name) {
            Some(statement) => Some((&statement.expr, Some(name.clone()))),
            None => {
                if self.complete {
                    self.note(at, format!("`{name}` is never defined"));
                }
                None
            }
        }
    }

    fn node(&mut self, expr: &Expr, at: &str) -> Option<Found> {
        let (expr, named) = self.resolve(expr, at)?;
        let at = named.clone().unwrap_or_else(|| at.to_owned());
        if let Some(name) = &named {
            self.stack.push(name.clone());
        }
        let found = match expr {
            Expr::Call { name, args, named } => self.call(name, args, named, &at),
            other => {
                self.note(
                    &at,
                    format!("expected a component, found {}", describe(other)),
                );
                None
            }
        };
        if named.is_some() {
            self.stack.pop();
        }
        found
    }

    fn text(&mut self, expr: &Expr, at: &str, prop: &str) -> Option<String> {
        let (expr, _) = self.resolve(expr, at)?;
        match expr {
            Expr::Str(text) => Some(text.clone()),
            Expr::Num(n) => Some(n.to_string()),
            other => {
                self.note(
                    at,
                    format!("{prop} should be text, not {}", describe(other)),
                );
                None
            }
        }
    }

    fn list(
        &mut self,
        expr: &Expr,
        at: &str,
        prop: &str,
        want: Option<&str>,
    ) -> Option<Vec<Found>> {
        let (expr, _) = self.resolve(expr, at)?;
        let items = match expr {
            Expr::Array(items) => items.as_slice(),
            // One component where a list belongs is a list of one.
            call @ Expr::Call { .. } => std::slice::from_ref(call),
            other => {
                self.note(
                    at,
                    format!("{prop} should be a list, not {}", describe(other)),
                );
                return None;
            }
        };
        let mut out = Vec::new();
        for item in items {
            match self.node(item, at) {
                Some(Found::Node(node)) if want.is_none() => out.push(Found::Node(node)),
                Some(Found::Inner(Inner::Step(step))) if want == Some("Step") => {
                    out.push(Found::Inner(Inner::Step(step)));
                }
                Some(Found::Inner(Inner::Tab(tab))) if want == Some("Tab") => {
                    out.push(Found::Inner(Inner::Tab(tab)));
                }
                // An item of the wrong kind is pruned; the rest stay.
                Some(_) => self.note(
                    at,
                    format!(
                        "{prop} holds {}, so an item was dropped",
                        want.unwrap_or("blocks")
                    ),
                ),
                None => {}
            }
        }
        Some(out)
    }

    fn call(
        &mut self,
        name: &str,
        args: &[Expr],
        named: &[(String, Expr)],
        at: &str,
    ) -> Option<Found> {
        let Some(spec) = catalog::component(name) else {
            self.note(at, format!("`{name}` is not in the catalog"));
            return None;
        };
        let mut given: Vec<(&catalog::Prop, &Expr)> = Vec::new();
        for (index, arg) in args.iter().enumerate() {
            match spec.props.get(index) {
                Some(prop) => given.push((prop, arg)),
                None => self.note(
                    at,
                    format!(
                        "{name} takes {} arguments; the rest were dropped",
                        spec.props.len()
                    ),
                ),
            }
        }
        for (key, arg) in named {
            match spec.props.iter().find(|p| p.name == key) {
                Some(prop) => {
                    given.retain(|(p, _)| p.name != prop.name);
                    given.push((prop, arg));
                }
                None => self.note(
                    at,
                    format!("{name} has no argument `{key}`; it was dropped"),
                ),
            }
        }
        let mut texts: BTreeMap<&str, String> = BTreeMap::new();
        let mut lists: BTreeMap<&str, Vec<Found>> = BTreeMap::new();
        for (prop, arg) in given {
            if matches!(arg, Expr::Null) {
                continue;
            }
            match prop.kind {
                Kind::Text => {
                    if let Some(text) = self.text(arg, at, prop.name) {
                        texts.insert(prop.name, text);
                    }
                }
                Kind::Href => {
                    if let Some(href) = self.text(arg, at, prop.name) {
                        if safe_href(&href) {
                            texts.insert(prop.name, href);
                        } else {
                            self.note(
                                at,
                                format!(
                                    "{name}'s {} `{href}` is not an https:// URL or a site path",
                                    prop.name
                                ),
                            );
                        }
                    }
                }
                Kind::Choice(words) => {
                    if let Some(word) = self.text(arg, at, prop.name) {
                        if words.contains(&word.as_str()) {
                            texts.insert(prop.name, word);
                        } else if self.complete {
                            // A partial word may still become valid while
                            // streaming; once whole, it falls back.
                            self.note(at, format!("{name}'s {} `{word}` is not one of {words:?}; the default is used", prop.name));
                        }
                    }
                }
                Kind::Children => {
                    if let Some(items) = self.list(arg, at, prop.name, None) {
                        lists.insert(prop.name, items);
                    }
                }
                Kind::Items(item) => {
                    if let Some(items) = self.list(arg, at, prop.name, Some(item)) {
                        lists.insert(prop.name, items);
                    }
                }
            }
        }
        for prop in spec.props.iter().filter(|p| p.required) {
            let present = texts.contains_key(prop.name)
                || lists.get(prop.name).is_some_and(|items| !items.is_empty());
            if !present {
                if self.complete {
                    self.note(at, format!("{name} needs {}; it was dropped", prop.name));
                }
                return None;
            }
        }
        let mut text = |key: &str| texts.remove(key).unwrap_or_default();
        let mut nodes = |key: &str| -> Vec<Node> {
            lists
                .remove(key)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|found| match found {
                    Found::Node(node) => Some(node),
                    Found::Inner(_) => None,
                })
                .collect()
        };
        let node = match name {
            "Stack" => Node::Stack {
                children: nodes("children"),
            },
            "Columns" => Node::Columns {
                children: nodes("children"),
            },
            "Card" => Node::Card {
                title: text("title"),
                children: nodes("children"),
            },
            "Text" => Node::Text { text: text("text") },
            "Link" => Node::Link {
                label: text("label"),
                href: text("href"),
            },
            "Button" => Node::Button {
                label: text("label"),
                href: text("href"),
                style: match text("style").as_str() {
                    "secondary" => ButtonStyle::Secondary,
                    _ => ButtonStyle::Primary,
                },
                show: match text("show").as_str() {
                    "signed_in" => Audience::SignedIn,
                    "signed_out" => Audience::SignedOut,
                    _ => Audience::Everyone,
                },
            },
            "CodeBlock" => Node::CodeBlock {
                code: text("code"),
                language: Some(text("language")).filter(|l| !l.is_empty()),
            },
            "Command" => Node::Command {
                unix: text("unix"),
                windows: Some(text("windows")).filter(|w| !w.is_empty()),
            },
            "Steps" => Node::Steps {
                steps: lists
                    .remove("steps")
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|found| match found {
                        Found::Inner(Inner::Step(step)) => Some(step),
                        _ => None,
                    })
                    .collect(),
            },
            "Tabs" => Node::Tabs {
                tabs: lists
                    .remove("tabs")
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|found| match found {
                        Found::Inner(Inner::Tab(tab)) => Some(tab),
                        _ => None,
                    })
                    .collect(),
            },
            "LinkCard" => Node::LinkCard {
                title: text("title"),
                description: text("description"),
                href: text("href"),
            },
            "Step" => {
                return Some(Found::Inner(Inner::Step(Step {
                    title: text("title"),
                    children: nodes("children"),
                })));
            }
            "Tab" => {
                return Some(Found::Inner(Inner::Tab(Tab {
                    label: text("label"),
                    children: nodes("children"),
                })));
            }
            _ => return None,
        };
        Some(Found::Node(node))
    }
}

fn describe(expr: &Expr) -> &'static str {
    match expr {
        Expr::Str(_) => "text",
        Expr::Num(_) => "a number",
        Expr::Bool(_) => "true or false",
        Expr::Null => "null",
        Expr::Ref(_) => "a name",
        Expr::Array(_) => "a list",
        Expr::Object(_) => "an object",
        Expr::Call { .. } => "a component",
    }
}

/// Parses statement texts into the table, noting the ones that fail.
pub(crate) fn admit(texts: &[&str], table: &mut Table, diagnostics: &mut Vec<Diagnostic>) {
    for text in texts.iter().filter(|text| !lex::blank(text)) {
        match lex::statement(text) {
            Ok(statement) => {
                table.insert(statement.name.clone(), statement);
            }
            Err(message) => diagnostics.push(Diagnostic {
                statement: lex::head(text).map(str::to_owned),
                message: format!("a line was dropped: {message}"),
            }),
        }
    }
}
