//! The SDK reference the website's `/docs/sdk` pages draw (#12007).
//!
//! This test reads the crate's own source with `syn`, lists every public
//! module, item, and method with its doc comment, signature, and the HTTP
//! operations its body calls, and compares the result with the committed
//! `docs/sdk/sdk-reference.json`. It fails when that file is stale and when
//! a public item has no doc comment.
//!
//! Regenerate the file after changing the SDK's public items:
//!
//! ```text
//! SDK_REFERENCE_WRITE=1 cargo test -p openagents-sdk --test reference
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use syn::visit::Visit;
use syn::{Attribute, Expr, ImplItem, Item, Lit, UseTree, Visibility};

const REFERENCE: &str = "../../docs/sdk/sdk-reference.json";

/// Modules whose items are generated from a contract; they are listed by
/// name only and need no hand-written doc comments.
const GENERATED: &[&str] = &["generated", "generated::computers"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// One parsed module: its path from the crate root, whether every
/// ancestor is `pub mod`, and its file.
struct Module {
    path: String,
    public: bool,
    file: syn::File,
    items: Vec<Item>,
}

fn module_file(dir: &Path, name: &str) -> PathBuf {
    let flat = dir.join(format!("{name}.rs"));
    if flat.exists() {
        flat
    } else {
        dir.join(name).join("mod.rs")
    }
}

fn is_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg")
            && attr
                .parse_args::<syn::Meta>()
                .is_ok_and(|meta| meta.path().is_ident("test"))
    })
}

fn load(modules: &mut Vec<Module>, path: &str, public: bool, file: &Path, children: &Path) {
    let source = std::fs::read_to_string(file).unwrap_or_else(|_| panic!("read {file:?}"));
    let parsed = syn::parse_file(&source).unwrap_or_else(|e| panic!("parse {file:?}: {e}"));
    let items = parsed.items.clone();
    for item in &items {
        if let Item::Mod(module) = item {
            if is_test(&module.attrs) || module.content.is_some() {
                continue;
            }
            let name = module.ident.to_string();
            let child = if path.is_empty() {
                name.clone()
            } else {
                format!("{path}::{name}")
            };
            let pub_mod = matches!(module.vis, Visibility::Public(_));
            load(
                modules,
                &child,
                public && pub_mod,
                &module_file(children, &name),
                &children.join(&name),
            );
        }
    }
    modules.push(Module {
        path: path.to_owned(),
        public,
        file: parsed,
        items,
    });
}

fn docs(attrs: &[Attribute]) -> String {
    let mut lines = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        if let syn::Meta::NameValue(value) = &attr.meta
            && let Expr::Lit(lit) = &value.value
            && let Lit::Str(text) = &lit.lit
        {
            let text = text.value();
            lines.push(
                text.strip_prefix(' ')
                    .unwrap_or(&text)
                    .trim_end()
                    .to_owned(),
            );
        }
    }
    lines.join("\n").trim().to_owned()
}

fn feature(attrs: &[Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| {
        if !attr.path().is_ident("cfg") {
            return None;
        }
        let meta = attr.parse_args::<syn::MetaNameValue>().ok()?;
        if !meta.path.is_ident("feature") {
            return None;
        }
        match meta.value {
            Expr::Lit(syn::ExprLit {
                lit: Lit::Str(s), ..
            }) => Some(s.value()),
            _ => None,
        }
    })
}

/// Resolves a `use` path written in module `here` to a crate path, or
/// `None` for another crate.
fn resolve(here: &str, segments: &[String]) -> Option<String> {
    let mut base: Vec<String> = if here.is_empty() {
        Vec::new()
    } else {
        here.split("::").map(str::to_owned).collect()
    };
    let mut rest = segments;
    match rest.first().map(String::as_str) {
        Some("crate") => {
            base.clear();
            rest = &rest[1..];
        }
        Some("self") => rest = &rest[1..],
        Some("super") => {
            while rest.first().map(String::as_str) == Some("super") {
                base.pop();
                rest = &rest[1..];
            }
        }
        Some(first) if first.is_empty() => return None,
        _ => {}
    }
    base.extend(rest.iter().cloned());
    Some(base.join("::"))
}

/// A `pub use`: the crate module and name it re-exports (or the other
/// crate), and the name it is exposed under.
struct Export {
    source: Option<String>,
    external: String,
    name: String,
    alias: String,
}

fn exports(here: &str, tree: &UseTree, prefix: &mut Vec<String>, out: &mut Vec<Export>) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            exports(here, &path.tree, prefix, out);
            prefix.pop();
        }
        UseTree::Name(name) => push_export(here, prefix, &name.ident.to_string(), None, out),
        UseTree::Rename(rename) => push_export(
            here,
            prefix,
            &rename.ident.to_string(),
            Some(rename.rename.to_string()),
            out,
        ),
        UseTree::Glob(_) => push_export(here, prefix, "*", None, out),
        UseTree::Group(group) => {
            for tree in &group.items {
                exports(here, tree, prefix, out);
            }
        }
    }
}

fn push_export(
    here: &str,
    prefix: &[String],
    name: &str,
    alias: Option<String>,
    out: &mut Vec<Export>,
) {
    let local = matches!(
        prefix.first().map(String::as_str),
        Some("crate" | "self" | "super")
    ) || (!prefix.is_empty() && is_child(here, &prefix[0]));
    let (source, external) = if name == "self" {
        (None, prefix.join("::"))
    } else if local {
        (resolve(here, prefix), String::new())
    } else {
        (None, prefix.join("::"))
    };
    let name = if name == "self" {
        prefix.last().cloned().unwrap_or_default()
    } else {
        name.to_owned()
    };
    out.push(Export {
        source,
        external,
        alias: alias.unwrap_or_else(|| name.clone()),
        name,
    });
}

thread_local! {
    static CHILDREN: std::cell::RefCell<BTreeSet<String>> = const { std::cell::RefCell::new(BTreeSet::new()) };
}

fn is_child(here: &str, name: &str) -> bool {
    let path = if here.is_empty() {
        name.to_owned()
    } else {
        format!("{here}::{name}")
    };
    CHILDREN.with(|c| c.borrow().contains(&path))
}

/// The HTTP calls a function body makes: a `Method::X` argument beside a
/// `&[...]` path-segment argument, or a generated operation constant.
struct Calls<'a> {
    found: BTreeSet<(String, String)>,
    operations: &'a BTreeMap<String, (String, String)>,
}

fn method_of(expr: &Expr) -> Option<String> {
    let Expr::Path(path) = expr else { return None };
    let segments: Vec<String> = path
        .path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect();
    let [.., kind, verb] = segments.as_slice() else {
        return None;
    };
    (kind == "Method" && matches!(verb.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE"))
        .then(|| verb.clone())
}

fn segments_of(expr: &Expr) -> Option<String> {
    let Expr::Reference(reference) = expr else {
        return None;
    };
    let Expr::Array(array) = &*reference.expr else {
        return None;
    };
    // An empty list is the query (`&[]`), not a path.
    if array.elems.is_empty() {
        return None;
    }
    let mut out = String::from("/v1");
    for element in &array.elems {
        out.push('/');
        match element {
            Expr::Lit(syn::ExprLit {
                lit: Lit::Str(s), ..
            }) => out.push_str(&s.value()),
            _ => out.push('*'),
        }
    }
    Some(out)
}

impl Calls<'_> {
    fn arguments<'b>(&mut self, args: impl Iterator<Item = &'b Expr> + Clone) {
        let methods: Vec<String> = args.clone().filter_map(method_of).collect();
        let paths: Vec<String> = args.filter_map(segments_of).collect();
        if let ([method], [path]) = (methods.as_slice(), paths.as_slice()) {
            self.found.insert((method.clone(), path.clone()));
        }
    }
}

impl<'ast> Visit<'ast> for Calls<'_> {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        self.arguments(call.args.iter());
        syn::visit::visit_expr_call(self, call);
    }
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.arguments(call.args.iter());
        syn::visit::visit_expr_method_call(self, call);
    }
    fn visit_expr_struct(&mut self, value: &'ast syn::ExprStruct) {
        if let Some(last) = value.path.segments.last()
            && let Some((method, route)) = self.operations.get(&last.ident.to_string())
        {
            self.found.insert((method.clone(), route.clone()));
        }
        syn::visit::visit_expr_struct(self, value);
    }
    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if let Some(last) = path.path.segments.last()
            && let Some((method, route)) = self.operations.get(&last.ident.to_string())
        {
            self.found.insert((method.clone(), route.clone()));
        }
        syn::visit::visit_expr_path(self, path);
    }
}

fn calls(block: &syn::Block, operations: &BTreeMap<String, (String, String)>) -> Vec<Value> {
    let mut visitor = Calls {
        found: BTreeSet::new(),
        operations,
    };
    visitor.visit_block(block);
    visitor
        .found
        .into_iter()
        .map(|(method, path)| json!({"method": method, "path": path}))
        .collect()
}

/// The generated operation constants: name to (method, path).
fn generated_operations(modules: &[Module]) -> BTreeMap<String, (String, String)> {
    let mut out = BTreeMap::new();
    for module in modules.iter().filter(|m| m.path == "generated::computers") {
        for item in &module.items {
            let Item::Const(constant) = item else {
                continue;
            };
            let Expr::Struct(value) = &*constant.expr else {
                continue;
            };
            let field = |name: &str| {
                value
                    .fields
                    .iter()
                    .find_map(|f| match (&f.member, &f.expr) {
                        (
                            syn::Member::Named(id),
                            Expr::Lit(syn::ExprLit {
                                lit: Lit::Str(s), ..
                            }),
                        ) if id == name => Some(s.value()),
                        _ => None,
                    })
            };
            if let (Some(method), Some(path)) = (field("method"), field("path")) {
                out.insert(constant.ident.to_string(), (method, path));
            }
        }
        // The SDK builds generated requests by type (`generated::ListComputers
        // { .. }.into_request()`), so map each request type to the operation
        // its `into_request` names.
        let mut by_type = Vec::new();
        for item in &module.items {
            let Item::Impl(block) = item else { continue };
            let Some(ty) = self_type(&block.self_ty) else {
                continue;
            };
            for entry in &block.items {
                let ImplItem::Fn(f) = entry else { continue };
                if f.sig.ident != "into_request" {
                    continue;
                }
                let mut named = OperationField(None);
                named.visit_block(&f.block);
                if let Some(name) = named.0 {
                    by_type.push((ty.clone(), name));
                }
            }
        }
        for (ty, name) in by_type {
            if let Some(operation) = out.get(&name).cloned() {
                out.insert(ty, operation);
            }
        }
    }
    out
}

fn signature(sig: &syn::Signature, vis: &Visibility) -> String {
    let item = syn::ItemFn {
        attrs: Vec::new(),
        vis: vis.clone(),
        sig: sig.clone(),
        block: Box::new(syn::parse_quote!({})),
    };
    let text = prettyplease::unparse(&syn::File {
        shebang: None,
        attrs: Vec::new(),
        items: vec![Item::Fn(item)],
    });
    text.trim()
        .trim_end_matches("{}")
        .trim()
        .trim_end_matches('{')
        .trim()
        .to_owned()
}

fn declaration(item: &Item) -> String {
    let mut item = item.clone();
    // Keep the declaration short: no attributes, bodies, or private fields.
    match &mut item {
        Item::Struct(s) => {
            s.attrs.clear();
            match &mut s.fields {
                syn::Fields::Named(fields) => {
                    fields.named = fields
                        .named
                        .iter()
                        .filter(|f| matches!(f.vis, Visibility::Public(_)))
                        .cloned()
                        .map(|mut f| {
                            f.attrs.clear();
                            f
                        })
                        .collect();
                }
                syn::Fields::Unnamed(fields) => {
                    for field in fields.unnamed.iter_mut() {
                        field.attrs.clear();
                    }
                }
                syn::Fields::Unit => {}
            }
        }
        Item::Enum(e) => {
            e.attrs.clear();
            for variant in e.variants.iter_mut() {
                variant.attrs.clear();
                for field in variant.fields.iter_mut() {
                    field.attrs.clear();
                }
            }
        }
        Item::Trait(t) => {
            t.attrs.clear();
            for entry in t.items.iter_mut() {
                if let syn::TraitItem::Fn(f) = entry {
                    f.attrs.clear();
                    f.default = None;
                    f.semi_token = Some(Default::default());
                }
            }
        }
        Item::Type(t) => t.attrs.clear(),
        Item::Const(c) => c.attrs.clear(),
        Item::Static(s) => s.attrs.clear(),
        _ => {}
    }
    prettyplease::unparse(&syn::File {
        shebang: None,
        attrs: Vec::new(),
        items: vec![item],
    })
    .trim()
    .to_owned()
}

fn kind(item: &Item) -> Option<(&'static str, String, &[Attribute], &Visibility)> {
    Some(match item {
        Item::Fn(f) => ("fn", f.sig.ident.to_string(), &f.attrs, &f.vis),
        Item::Struct(s) => ("struct", s.ident.to_string(), &s.attrs, &s.vis),
        Item::Enum(e) => ("enum", e.ident.to_string(), &e.attrs, &e.vis),
        Item::Trait(t) => ("trait", t.ident.to_string(), &t.attrs, &t.vis),
        Item::Type(t) => ("type", t.ident.to_string(), &t.attrs, &t.vis),
        Item::Const(c) => ("const", c.ident.to_string(), &c.attrs, &c.vis),
        Item::Static(s) => ("static", s.ident.to_string(), &s.attrs, &s.vis),
        _ => return None,
    })
}

fn self_type(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(path) => path.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    }
}

fn build() -> (Value, Vec<String>) {
    let mut modules = Vec::new();
    let src = root();
    // Record every module path first, so `use child::X` resolves locally.
    let mut first = Vec::new();
    load(&mut first, "", true, &src.join("lib.rs"), &src);
    CHILDREN.with(|c| *c.borrow_mut() = first.iter().map(|m| m.path.clone()).collect());
    load(&mut modules, "", true, &src.join("lib.rs"), &src);
    let operations = generated_operations(&modules);
    let public: BTreeMap<&str, bool> = modules
        .iter()
        .map(|m| (m.path.as_str(), m.public))
        .collect();

    // Where each crate item is exposed: (module, name) -> (module, alias).
    let mut exposed_at: BTreeMap<(String, String), (String, String)> = BTreeMap::new();
    let mut external: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for module in &modules {
        if !module.public {
            continue;
        }
        for item in &module.items {
            let Item::Use(use_item) = item else { continue };
            if !matches!(use_item.vis, Visibility::Public(_)) {
                continue;
            }
            let mut found = Vec::new();
            exports(&module.path, &use_item.tree, &mut Vec::new(), &mut found);
            for export in found {
                match export.source {
                    Some(source) if public.get(source.as_str()) == Some(&false) => {
                        exposed_at
                            .insert((source, export.name), (module.path.clone(), export.alias));
                    }
                    Some(_) => {}
                    None => external.entry(module.path.clone()).or_default().push((
                        export.external,
                        if export.alias == export.name {
                            export.name
                        } else {
                            format!("{} as {}", export.name, export.alias)
                        },
                    )),
                }
            }
        }
    }
    // A private module's own `pub use` passes its exposure on.
    for _ in 0..4 {
        for module in modules.iter().filter(|m| !m.public) {
            for item in &module.items {
                let Item::Use(use_item) = item else { continue };
                if !matches!(use_item.vis, Visibility::Public(_)) {
                    continue;
                }
                let mut found = Vec::new();
                exports(&module.path, &use_item.tree, &mut Vec::new(), &mut found);
                for export in found {
                    let Some(source) = export.source else {
                        continue;
                    };
                    if let Some(at) = exposed_at
                        .get(&(module.path.clone(), export.alias.clone()))
                        .cloned()
                    {
                        exposed_at.insert((source, export.name), at);
                    }
                }
            }
        }
    }
    let exposure = |module: &Module, name: &str| -> Option<(String, String)> {
        if module.public {
            return Some((module.path.clone(), name.to_owned()));
        }
        exposed_at
            .get(&(module.path.clone(), name.to_owned()))
            .or_else(|| exposed_at.get(&(module.path.clone(), "*".to_owned())))
            .map(|(at, alias)| {
                (
                    at.clone(),
                    if alias == "*" {
                        name.to_owned()
                    } else {
                        alias.clone()
                    },
                )
            })
    };

    // Public types by name, where they are exposed.
    let mut types: BTreeMap<(String, String), (String, String)> = BTreeMap::new();
    for module in &modules {
        for item in &module.items {
            if let Some((k, name, _, Visibility::Public(_))) = kind(item)
                && matches!(k, "struct" | "enum" | "trait" | "type")
                && let Some(at) = exposure(module, &name)
            {
                types.insert((module.path.clone(), name), at);
            }
        }
    }
    let find_type = |module: &str, name: &str| {
        types
            .get(&(module.to_owned(), name.to_owned()))
            .cloned()
            .or_else(|| {
                types
                    .iter()
                    .find(|((_, n), _)| n == name)
                    .map(|(_, at)| at.clone())
            })
    };

    let mut out: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut module_docs: BTreeMap<String, String> = BTreeMap::new();
    let mut missing = Vec::new();
    for module in &modules {
        if module.public {
            module_docs.insert(module.path.clone(), docs(&module.file.attrs));
        }
        let generated = GENERATED.contains(&module.path.as_str());
        for item in &module.items {
            if let Some((k, name, attrs, Visibility::Public(_))) = kind(item) {
                let Some((at, alias)) = exposure(module, &name) else {
                    continue;
                };
                let doc = docs(attrs);
                if doc.is_empty() && !generated {
                    missing.push(format!("{}::{name}", module.path));
                }
                if generated {
                    // Listed in the module's summary, not one by one.
                    continue;
                }
                let mut entry = json!({
                    "kind": k,
                    "name": alias,
                    "doc": doc,
                    "declaration": match item {
                        Item::Fn(f) => signature(&f.sig, &f.vis),
                        other => declaration(other),
                    },
                });
                if let Item::Fn(f) = item {
                    entry["calls"] = Value::Array(calls(&f.block, &operations));
                }
                if let Some(feature) = feature(attrs) {
                    entry["feature"] = json!(feature);
                }
                if let Item::Trait(t) = item {
                    let mut methods = Vec::new();
                    for entry in &t.items {
                        if let syn::TraitItem::Fn(f) = entry {
                            let doc = docs(&f.attrs);
                            if doc.is_empty() {
                                missing.push(format!("{}::{name}::{}", module.path, f.sig.ident));
                            }
                            methods.push(json!({
                                "name": f.sig.ident.to_string(),
                                "doc": doc,
                                "declaration": signature(&f.sig, &Visibility::Inherited),
                                "calls": [],
                            }));
                        }
                    }
                    entry["methods"] = Value::Array(methods);
                }
                out.entry(at).or_default().push(entry);
            }
        }
    }
    // Inherent methods, attached to their type.
    let mut methods: BTreeMap<(String, String), Vec<Value>> = BTreeMap::new();
    for module in &modules {
        if GENERATED.contains(&module.path.as_str()) {
            continue;
        }
        for item in &module.items {
            let Item::Impl(block) = item else { continue };
            if block.trait_.is_some() || is_test(&block.attrs) {
                continue;
            }
            let Some(ty) = self_type(&block.self_ty) else {
                continue;
            };
            let Some(at) = find_type(&module.path, &ty) else {
                continue;
            };
            for entry in &block.items {
                let ImplItem::Fn(f) = entry else { continue };
                if !matches!(f.vis, Visibility::Public(_)) {
                    continue;
                }
                let doc = docs(&f.attrs);
                if doc.is_empty() {
                    missing.push(format!("{}::{ty}::{}", module.path, f.sig.ident));
                }
                let mut value = json!({
                    "name": f.sig.ident.to_string(),
                    "doc": doc,
                    "declaration": signature(&f.sig, &f.vis),
                    "calls": calls(&f.block, &operations),
                });
                if let Some(feature) = feature(&f.attrs) {
                    value["feature"] = json!(feature);
                }
                methods.entry(at.clone()).or_default().push(value);
            }
        }
    }
    for (module, items) in out.iter_mut() {
        for item in items.iter_mut() {
            let name = item["name"].as_str().unwrap_or_default().to_owned();
            if let Some(list) = methods.remove(&(module.clone(), name)) {
                let existing = item
                    .get("methods")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                item["methods"] = Value::Array(existing.into_iter().chain(list).collect());
            }
        }
        items.sort_by(|a, b| {
            let rank = |v: &Value| match v["kind"].as_str() {
                Some("struct" | "enum" | "trait" | "type") => 0,
                Some("fn") => 1,
                _ => 2,
            };
            (rank(a), a["name"].as_str()).cmp(&(rank(b), b["name"].as_str()))
        });
    }
    // Methods left over belong to types outside the public API.

    let mut list = Vec::new();
    for (path, doc) in &module_docs {
        let generated = GENERATED.contains(&path.as_str());
        if doc.is_empty() && !generated {
            missing.push(format!(
                "module {}",
                if path.is_empty() { "crate" } else { path }
            ));
        }
        let mut entry = json!({
            "path": path,
            "doc": doc,
            "items": out.remove(path).unwrap_or_default(),
            "reexports": external.remove(path).unwrap_or_default().into_iter()
                .map(|(from, name)| json!({"from": from, "name": name})).collect::<Vec<_>>(),
        });
        if generated {
            entry["generated"] = json!(true);
        }
        list.push(entry);
    }
    assert!(out.is_empty(), "items in unknown modules: {:?}", out.keys());
    let manifest =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    let description = manifest
        .lines()
        .find_map(|l| l.strip_prefix("description = "))
        .map(|d| d.trim_matches('"').to_owned())
        .unwrap_or_default();
    (
        json!({
            "schema": "openagents.sdk-reference.v1",
            "crate": "openagents-sdk",
            "description": description,
            "regenerate": "SDK_REFERENCE_WRITE=1 cargo test -p openagents-sdk --test reference",
            "modules": list,
        }),
        missing,
    )
}

#[test]
fn the_committed_reference_matches_the_source_and_every_public_item_is_described() {
    let (reference, missing) = build();
    let text = serde_json::to_string_pretty(&reference).unwrap() + "\n";
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(REFERENCE);
    if std::env::var_os("SDK_REFERENCE_WRITE").is_some() {
        std::fs::write(&path, &text).unwrap();
    }
    assert!(
        missing.is_empty(),
        "{} public SDK items have no doc comment:\n{}",
        missing.len(),
        missing.join("\n")
    );
    let committed = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == text,
        "docs/sdk/sdk-reference.json is stale; regenerate with \
SDK_REFERENCE_WRITE=1 cargo test -p openagents-sdk --test reference"
    );
}

#[test]
fn every_module_and_its_calls_are_found() {
    let (reference, _) = build();
    let modules: Vec<&str> = reference["modules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap())
        .collect();
    for expected in [
        "",
        "chats",
        "computers",
        "labs",
        "agents",
        "history",
        "work",
    ] {
        assert!(modules.contains(&expected), "{expected} in {modules:?}");
    }
    let text = reference.to_string();
    assert!(
        text.contains("\"path\":\"/v1/threads/*/messages\""),
        "chat calls"
    );
    assert!(
        text.contains("\"path\":\"/v1/computers\""),
        "generated calls"
    );
}

/// The constant a generated `into_request` puts in its `operation` field.
struct OperationField(Option<String>);

impl<'ast> Visit<'ast> for OperationField {
    fn visit_field_value(&mut self, field: &'ast syn::FieldValue) {
        if let syn::Member::Named(id) = &field.member
            && id == "operation"
            && let Expr::Reference(reference) = &field.expr
            && let Expr::Path(path) = &*reference.expr
            && let Some(last) = path.path.segments.last()
        {
            self.0 = Some(last.ident.to_string());
        }
        syn::visit::visit_field_value(self, field);
    }
}
