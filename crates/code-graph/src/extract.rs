//! One Rust file's declarations, references and imports, from its syntax
//! tree alone. Names are left as written; [`crate::build`] resolves them
//! across files.

use tree_sitter::{Node as TsNode, Parser};

use crate::model::Kind;

/// The longest signature kept, in chars.
pub const MAX_SIGNATURE: usize = 200;
/// The deepest syntax nesting walked; deeper subtrees are skipped.
const MAX_DEPTH: usize = 400;

/// A declaration in the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decl {
    pub kind: Kind,
    pub name: String,
    /// Inline modules, types and functions between the file's module and
    /// the name (`["tests", "Foo"]` for `mod tests { impl Foo { fn x } }`).
    pub scope: Vec<String>,
    /// The inline modules the declaration sits in (a prefix of `scope`).
    pub module: Vec<String>,
    pub line: u32,
    pub end_line: u32,
    pub start_byte: u32,
    pub public: bool,
    /// The enclosing declaration, by index into [`Facts::decls`].
    pub parent: Option<usize>,
    pub signature: String,
    /// An impl's type and trait paths, as written (generics dropped).
    pub impl_type: Vec<String>,
    pub impl_trait: Vec<String>,
    /// The `Self` type of a method or other item inside an impl or trait.
    pub self_type: Vec<String>,
}

/// How a name is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RefKind {
    /// `path(...)`.
    Call,
    /// `receiver.name(...)`; `on_self` when the receiver is `self`.
    Method { on_self: bool },
    /// A path in type position.
    Type,
    /// A path in value position (`Kind::Module`, `MAX`).
    Value,
    /// `name!(...)`.
    Macro,
}

/// A use of a name, from a declaration (or the file's top level).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ref {
    pub from: Option<usize>,
    pub kind: RefKind,
    pub path: Vec<String>,
}

/// A `use` import: `alias` names `path` inside the inline modules
/// `module`. A glob import has the alias `*`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Use {
    pub module: Vec<String>,
    pub path: Vec<String>,
    pub alias: String,
    /// The declaration the `use` sits in (`None` at a module's top level).
    pub from: Option<usize>,
}

/// Everything one file says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub decls: Vec<Decl>,
    pub refs: Vec<Ref>,
    pub uses: Vec<Use>,
    pub lines: u32,
    pub parse_errors: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Container {
    None,
    Impl { of_trait: bool },
    Trait { public: bool },
}

#[derive(Clone)]
struct Ctx {
    owner: Option<usize>,
    module: Vec<String>,
    scope: Vec<String>,
    self_type: Vec<String>,
    container: Container,
}

struct Walker<'a> {
    text: &'a str,
    facts: Facts,
}

/// Parse `text` and extract its facts.
pub fn extract(text: &str) -> Result<Facts, String> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .map_err(|error| error.to_string())?;
    let tree = parser
        .parse(text, None)
        .ok_or("Tree-sitter did not return a syntax tree.")?;
    let root = tree.root_node();
    let mut walker = Walker {
        text,
        facts: Facts {
            lines: u32::try_from(text.lines().count()).unwrap_or(u32::MAX),
            parse_errors: root.has_error(),
            ..Facts::default()
        },
    };
    let ctx = Ctx {
        owner: None,
        module: Vec::new(),
        scope: Vec::new(),
        self_type: Vec::new(),
        container: Container::None,
    };
    walker.children(root, &ctx, 0);
    Ok(walker.facts)
}

fn line(position: tree_sitter::Point) -> u32 {
    u32::try_from(position.row + 1).unwrap_or(u32::MAX)
}

impl Walker<'_> {
    fn text(&self, node: TsNode<'_>) -> &str {
        self.text.get(node.byte_range()).unwrap_or("")
    }

    fn children(&mut self, node: TsNode<'_>, ctx: &Ctx, depth: usize) {
        let mut cursor = node.walk();
        let children: Vec<TsNode<'_>> = node.named_children(&mut cursor).collect();
        for child in children {
            self.walk(child, ctx, depth + 1);
        }
    }

    fn signature(&self, node: TsNode<'_>, end_at: Option<TsNode<'_>>) -> String {
        let end = end_at.map_or(node.end_byte(), |body| body.start_byte());
        let raw = self.text.get(node.start_byte()..end).unwrap_or("");
        let mut out = String::new();
        for word in raw.split_whitespace() {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(word);
        }
        let out = out.trim_end_matches(['{', '=', ';', ' ']).to_owned();
        if out.chars().count() > MAX_SIGNATURE {
            let mut cut: String = out.chars().take(MAX_SIGNATURE - 1).collect();
            cut.push('…');
            cut
        } else {
            out
        }
    }

    fn public(node: TsNode<'_>) -> bool {
        let mut cursor = node.walk();
        let found = node
            .children(&mut cursor)
            .any(|child| child.kind() == "visibility_modifier");
        found
    }

    #[allow(clippy::too_many_arguments)]
    fn declare(
        &mut self,
        node: TsNode<'_>,
        kind: Kind,
        name: String,
        ctx: &Ctx,
        public: bool,
        signature: String,
    ) -> usize {
        self.facts.decls.push(Decl {
            kind,
            name,
            scope: ctx.scope.clone(),
            module: ctx.module.clone(),
            line: line(node.start_position()),
            end_line: line(node.end_position()),
            start_byte: u32::try_from(node.start_byte()).unwrap_or(u32::MAX),
            public,
            parent: ctx.owner,
            signature,
            impl_type: Vec::new(),
            impl_trait: Vec::new(),
            self_type: ctx.self_type.clone(),
        });
        self.facts.decls.len() - 1
    }

    fn reference(&mut self, ctx: &Ctx, kind: RefKind, path: Vec<String>) {
        if !path.is_empty() && path.iter().all(|segment| !segment.is_empty()) {
            self.facts.refs.push(Ref {
                from: ctx.owner,
                kind,
                path,
            });
        }
    }

    /// A path's segments as written, generics dropped; empty when it is
    /// not a plain path (`<T as Trait>::x`).
    fn path(&self, node: TsNode<'_>) -> Vec<String> {
        let mut out = Vec::new();
        self.path_into(node, &mut out, 0);
        out
    }

    fn path_into(&self, node: TsNode<'_>, out: &mut Vec<String>, depth: usize) {
        if depth > 64 {
            out.clear();
            return;
        }
        match node.kind() {
            "identifier" | "type_identifier" | "field_identifier" | "crate" | "self" | "super"
            | "metavariable" | "primitive_type" => out.push(self.text(node).to_owned()),
            "scoped_identifier" | "scoped_type_identifier" => {
                if let Some(path) = node.child_by_field_name("path") {
                    self.path_into(path, out, depth + 1);
                    if out.is_empty() {
                        return;
                    }
                }
                match node.child_by_field_name("name") {
                    Some(name) => out.push(self.text(name).to_owned()),
                    None => out.clear(),
                }
            }
            "generic_type" | "generic_function" => {
                match node
                    .child_by_field_name("type")
                    .or_else(|| node.child_by_field_name("function"))
                {
                    Some(inner) => self.path_into(inner, out, depth + 1),
                    None => out.clear(),
                }
            }
            "reference_type" | "pointer_type" => match node.child_by_field_name("type") {
                Some(inner) => self.path_into(inner, out, depth + 1),
                None => out.clear(),
            },
            _ => out.clear(),
        }
    }

    fn walk(&mut self, node: TsNode<'_>, ctx: &Ctx, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        match node.kind() {
            "line_comment"
            | "block_comment"
            | "attribute_item"
            | "inner_attribute_item"
            | "string_literal"
            | "raw_string_literal"
            | "char_literal" => {}
            "function_item" | "function_signature_item" => {
                let Some(name) = node.child_by_field_name("name") else {
                    return;
                };
                let name = self.text(name).to_owned();
                let (kind, public) = match ctx.container {
                    Container::None => (Kind::Function, Self::public(node)),
                    Container::Impl { of_trait } => (Kind::Method, of_trait || Self::public(node)),
                    Container::Trait { public } => (Kind::Method, public),
                };
                let body = node.child_by_field_name("body");
                let signature = self.signature(node, body);
                let id = self.declare(node, kind, name.clone(), ctx, public, signature);
                let mut inner = ctx.clone();
                inner.owner = Some(id);
                inner.scope.push(name);
                inner.self_type = Vec::new();
                inner.container = Container::None;
                self.children(node, &inner, depth);
            }
            "struct_item" | "enum_item" | "union_item" | "type_item" | "const_item"
            | "static_item" | "trait_item" => {
                let Some(name) = node.child_by_field_name("name") else {
                    return;
                };
                let name = self.text(name).to_owned();
                let kind = match node.kind() {
                    "struct_item" => Kind::Struct,
                    "enum_item" => Kind::Enum,
                    "union_item" => Kind::Union,
                    "type_item" => Kind::TypeAlias,
                    "const_item" => Kind::Const,
                    "static_item" => Kind::Static,
                    _ => Kind::Trait,
                };
                let public = Self::public(node)
                    || (matches!(ctx.container, Container::Impl { of_trait: true })
                        && matches!(kind, Kind::Const | Kind::TypeAlias));
                let body = node
                    .child_by_field_name("body")
                    .or_else(|| node.child_by_field_name("value"));
                let signature = self.signature(node, body);
                let id = self.declare(node, kind, name.clone(), ctx, public, signature);
                let mut inner = ctx.clone();
                inner.owner = Some(id);
                if kind == Kind::Trait {
                    inner.scope.push(name.clone());
                    inner.self_type = vec![name];
                    inner.container = Container::Trait { public };
                } else {
                    inner.container = Container::None;
                }
                self.children(node, &inner, depth);
            }
            "macro_definition" => {
                if let Some(name) = node.child_by_field_name("name") {
                    let name = self.text(name).to_owned();
                    let signature = format!("macro_rules! {name}");
                    self.declare(node, Kind::Macro, name, ctx, true, signature);
                }
            }
            "mod_item" => {
                let (Some(name), Some(body)) = (
                    node.child_by_field_name("name"),
                    node.child_by_field_name("body"),
                ) else {
                    return;
                };
                let name = self.text(name).to_owned();
                let id = self.declare(
                    node,
                    Kind::Module,
                    name.clone(),
                    ctx,
                    Self::public(node),
                    String::new(),
                );
                let inner = Ctx {
                    owner: Some(id),
                    module: {
                        let mut module = ctx.module.clone();
                        module.push(name.clone());
                        module
                    },
                    scope: {
                        let mut scope = ctx.scope.clone();
                        scope.push(name);
                        scope
                    },
                    self_type: Vec::new(),
                    container: Container::None,
                };
                self.children(body, &inner, depth);
            }
            "impl_item" => {
                let Some(ty) = node.child_by_field_name("type") else {
                    return;
                };
                let ty_path = self.path(ty);
                let trait_path = node
                    .child_by_field_name("trait")
                    .map(|t| self.path(t))
                    .unwrap_or_default();
                let ty_name = ty_path.last().cloned().unwrap_or_else(|| {
                    self.text(ty)
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join("")
                });
                let name = match trait_path.last() {
                    Some(t) => format!("impl {t} for {ty_name}"),
                    None => format!("impl {ty_name}"),
                };
                let body = node.child_by_field_name("body");
                let signature = self.signature(node, body);
                let id = self.declare(node, Kind::Impl, name, ctx, false, signature);
                self.facts.decls[id].impl_type = ty_path.clone();
                self.facts.decls[id].impl_trait = trait_path.clone();
                if let Some(body) = body {
                    let mut inner = ctx.clone();
                    inner.owner = Some(id);
                    inner.scope.push(ty_name);
                    inner.self_type = ty_path;
                    inner.container = Container::Impl {
                        of_trait: !trait_path.is_empty(),
                    };
                    self.children(body, &inner, depth);
                }
                // Generic bounds and where clauses name types too.
                let mut inner = ctx.clone();
                inner.owner = Some(id);
                for field in ["type_parameters", "where_clause"] {
                    if let Some(part) = node.child_by_field_name(field) {
                        self.walk(part, &inner, depth + 1);
                    }
                }
                let mut cursor = node.walk();
                let clauses: Vec<TsNode<'_>> = node
                    .named_children(&mut cursor)
                    .filter(|child| child.kind() == "where_clause")
                    .collect();
                for clause in clauses {
                    self.walk(clause, &inner, depth + 1);
                }
            }
            "use_declaration" => {
                if let Some(argument) = node.child_by_field_name("argument") {
                    let mut found = Vec::new();
                    self.use_tree(argument, &[], &mut found, 0);
                    for (path, alias) in found {
                        self.facts.uses.push(Use {
                            module: ctx.module.clone(),
                            path,
                            alias,
                            from: ctx.owner,
                        });
                    }
                }
            }
            "call_expression" => {
                if let Some(function) = node.child_by_field_name("function") {
                    let function = if function.kind() == "generic_function" {
                        function.child_by_field_name("function").unwrap_or(function)
                    } else {
                        function
                    };
                    match function.kind() {
                        "identifier" | "scoped_identifier" => {
                            let path = self.path(function);
                            self.reference(ctx, RefKind::Call, path);
                        }
                        "field_expression" => {
                            if let Some(field) = function.child_by_field_name("field") {
                                let on_self = function
                                    .child_by_field_name("value")
                                    .is_some_and(|value| value.kind() == "self");
                                let name = self.text(field).to_owned();
                                self.reference(ctx, RefKind::Method { on_self }, vec![name]);
                            }
                            if let Some(value) = function.child_by_field_name("value") {
                                self.walk(value, ctx, depth + 1);
                            }
                        }
                        _ => self.walk(function, ctx, depth + 1),
                    }
                }
                if let Some(arguments) = node.child_by_field_name("arguments") {
                    self.walk(arguments, ctx, depth + 1);
                }
            }
            "macro_invocation" => {
                if let Some(name) = node.child_by_field_name("macro") {
                    let path = self.path(name);
                    if let Some(last) = path.last() {
                        self.reference(ctx, RefKind::Macro, vec![last.clone()]);
                    }
                }
                let mut cursor = node.walk();
                let trees: Vec<TsNode<'_>> = node
                    .named_children(&mut cursor)
                    .filter(|child| child.kind() == "token_tree")
                    .collect();
                for tree in trees {
                    self.tokens(tree, ctx, depth + 1);
                }
            }
            "type_identifier" => {
                let name = self.text(node).to_owned();
                self.reference(ctx, RefKind::Type, vec![name]);
            }
            "scoped_type_identifier" => {
                let path = self.path(node);
                self.reference(ctx, RefKind::Type, path);
            }
            "scoped_identifier" => {
                let path = self.path(node);
                if path.len() >= 2 {
                    self.reference(ctx, RefKind::Value, path);
                }
            }
            "identifier" => {
                let name = self.text(node);
                if name.len() > 1
                    && name
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
                    && name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                {
                    let name = name.to_owned();
                    self.reference(ctx, RefKind::Value, vec![name]);
                }
            }
            _ => self.children(node, ctx, depth),
        }
    }

    /// Expand a use tree into `(path, alias)` pairs.
    fn use_tree(
        &self,
        node: TsNode<'_>,
        prefix: &[String],
        out: &mut Vec<(Vec<String>, String)>,
        depth: usize,
    ) {
        if depth > 32 {
            return;
        }
        let joined = |tail: Vec<String>| {
            let mut path = prefix.to_vec();
            path.extend(tail);
            path
        };
        match node.kind() {
            "identifier" | "scoped_identifier" | "crate" | "super" | "metavariable" => {
                let path = joined(self.path(node));
                if let Some(alias) = path.last().cloned() {
                    out.push((path, alias));
                }
            }
            "self" => {
                if let Some(alias) = prefix.last().cloned() {
                    out.push((prefix.to_vec(), alias));
                }
            }
            "use_as_clause" => {
                if let (Some(path), Some(alias)) = (
                    node.child_by_field_name("path"),
                    node.child_by_field_name("alias"),
                ) {
                    let path = joined(self.path(path));
                    let alias = self.text(alias).to_owned();
                    if !path.is_empty() && alias != "_" {
                        out.push((path, alias));
                    }
                }
            }
            "use_wildcard" => {
                let mut cursor = node.walk();
                let inner = node.named_children(&mut cursor).next();
                let path = match inner {
                    Some(inner) => joined(self.path(inner)),
                    None => prefix.to_vec(),
                };
                if !path.is_empty() {
                    out.push((path, "*".into()));
                }
            }
            "use_list" => {
                let mut cursor = node.walk();
                let items: Vec<TsNode<'_>> = node.named_children(&mut cursor).collect();
                for item in items {
                    self.use_tree(item, prefix, out, depth + 1);
                }
            }
            "scoped_use_list" => {
                let base = match node.child_by_field_name("path") {
                    Some(path) => joined(self.path(path)),
                    None => prefix.to_vec(),
                };
                if let Some(list) = node.child_by_field_name("list") {
                    self.use_tree(list, &base, out, depth + 1);
                }
            }
            _ => {}
        }
    }

    /// Calls inside a macro's arguments: `a::b(` and `.m(` token patterns.
    fn tokens(&mut self, tree: TsNode<'_>, ctx: &Ctx, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        let mut cursor = tree.walk();
        let tokens: Vec<TsNode<'_>> = tree.children(&mut cursor).collect();
        let mut path: Vec<String> = Vec::new();
        let mut method = false;
        let mut on_self = false;
        let mut prev = "";
        let mut prev2 = "";
        for token in tokens {
            let kind = token.kind();
            match kind {
                "identifier" | "crate" | "self" | "super" => {
                    let text = self.text(token).to_owned();
                    if prev == "::" && !path.is_empty() {
                        path.push(text);
                    } else {
                        self.flush_value(ctx, &mut path);
                        method = prev == ".";
                        on_self = method && prev2 == "self";
                        path = vec![text];
                    }
                }
                "::" => {}
                "token_tree" => {
                    let opens_call = self.text(token).starts_with('(');
                    if opens_call && !path.is_empty() {
                        let taken = std::mem::take(&mut path);
                        if method && taken.len() == 1 {
                            self.reference(ctx, RefKind::Method { on_self }, taken);
                        } else if !method {
                            self.reference(ctx, RefKind::Call, taken);
                        }
                    } else {
                        self.flush_value(ctx, &mut path);
                    }
                    self.tokens(token, ctx, depth + 1);
                }
                _ => self.flush_value(ctx, &mut path),
            }
            prev2 = prev;
            prev = match kind {
                "identifier" | "crate" | "super" => "ident",
                "self" => "self",
                "::" => "::",
                "." => ".",
                _ => "",
            };
        }
        self.flush_value(ctx, &mut path);
    }

    fn flush_value(&mut self, ctx: &Ctx, path: &mut Vec<String>) {
        let taken = std::mem::take(path);
        if taken.len() >= 2 {
            self.reference(ctx, RefKind::Value, taken);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarations_references_and_imports() {
        let facts = extract(
            "use crate::a::{b, c as d, self};\nuse super::*;\n\npub struct Foo { x: Bar }\nimpl Foo {\n    pub fn new() -> Self { helper(); Self::other(); d::run() }\n    fn other() { self.go(); assert!(check(1)); }\n}\nimpl Display for Foo {\n    fn fmt(&self) {}\n}\nmod inner { fn deep() { crate::x::y(); } }\nfn helper() {}\n",
        )
        .unwrap();
        let names: Vec<(Kind, &str, String)> = facts
            .decls
            .iter()
            .map(|d| (d.kind, d.name.as_str(), d.scope.join("::")))
            .collect();
        assert_eq!(
            names,
            vec![
                (Kind::Struct, "Foo", String::new()),
                (Kind::Impl, "impl Foo", String::new()),
                (Kind::Method, "new", "Foo".into()),
                (Kind::Method, "other", "Foo".into()),
                (Kind::Impl, "impl Display for Foo", String::new()),
                (Kind::Method, "fmt", "Foo".into()),
                (Kind::Module, "inner", String::new()),
                (Kind::Function, "deep", "inner".into()),
                (Kind::Function, "helper", String::new()),
            ]
        );
        assert!(facts.decls[5].public, "a trait impl's method is public");
        assert!(!facts.decls[3].public);
        let uses: Vec<(String, String)> = facts
            .uses
            .iter()
            .map(|u| (u.path.join("::"), u.alias.clone()))
            .collect();
        assert_eq!(
            uses,
            vec![
                ("crate::a::b".into(), "b".into()),
                ("crate::a::c".into(), "d".into()),
                ("crate::a".into(), "a".into()),
                ("super".into(), "*".into()),
            ]
        );
        let refs: Vec<(RefKind, String)> = facts
            .refs
            .iter()
            .map(|r| (r.kind, r.path.join("::")))
            .collect();
        for expected in [
            (RefKind::Type, "Bar".to_owned()),
            (RefKind::Call, "helper".into()),
            (RefKind::Call, "Self::other".into()),
            (RefKind::Call, "d::run".into()),
            (RefKind::Method { on_self: true }, "go".into()),
            (RefKind::Macro, "assert".into()),
            (RefKind::Call, "check".into()),
            (RefKind::Call, "crate::x::y".into()),
        ] {
            assert!(refs.contains(&expected), "{expected:?} in {refs:?}");
        }
        assert_eq!(facts.decls[2].signature, "pub fn new() -> Self");
    }
}
