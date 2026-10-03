//! Complete source units from an explicit file pool and nearby test files.
use crate::{
    Brief, Components, Evidence, Index, Issue, Result, Source,
    focused::{self, Anchor, Omission, Pack, Role, Selection},
    syntax,
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::Path,
    time::Instant,
};
use tree_sitter::{Node, Parser};

const MAX_READS: usize = 24;
const MAX_UNITS: usize = 1024;
const MAX_NODES: usize = 100_000;
const MAX_DEPENDENCIES: usize = 64;
const MAX_TESTS: usize = 1;
const MAX_DOC_SECTIONS: usize = 2;
const MAX_GROUPS: usize = 16;
const MAX_COVERAGE: usize = 128;
const MAX_RENDERED_COVERAGE: usize = 768;
const SMALL_FILE: usize = 6 * 1024;

#[derive(Clone, Debug)]
struct Unit {
    name: String,
    scope: String,
    qualified: String,
    kind: String,
    start: usize,
    end: usize,
    test: bool,
    public: bool,
    error: bool,
    calls: BTreeSet<String>,
    names: BTreeSet<String>,
    shadowed: BTreeSet<String>,
    unsupported_calls: usize,
}
#[derive(Default)]
struct RustFile {
    units: Vec<Unit>,
    limitations: Vec<String>,
}
struct File<'a> {
    source: &'a Source,
    text: String,
    explicit: bool,
    rust: Option<RustFile>,
}
#[derive(Clone)]
struct Pick {
    file: usize,
    start: usize,
    end: usize,
    role: Role,
    method: String,
    anchor: Option<Anchor>,
    declaration: bool,
    complete_file: bool,
}
struct Bundle {
    picks: Vec<Pick>,
    path: String,
    role: Role,
    description: String,
}

fn owner_manifest<'a>(path: &str, index: &'a Index) -> Option<&'a Source> {
    index
        .files
        .iter()
        .filter(|f| {
            f.path == "Cargo.toml"
                || f.path
                    .strip_suffix("/Cargo.toml")
                    .is_some_and(|dir| path.starts_with(&format!("{dir}/")))
        })
        .max_by_key(|f| f.path.len())
}
fn node_text<'a>(node: Node<'_>, text: &'a str) -> &'a str {
    text.get(node.byte_range()).unwrap_or("")
}
fn lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}
fn excerpt(file: &File<'_>, start: usize, end: usize) -> String {
    lines(&file.text)[start - 1..end].concat()
}
fn last_line(node: Node<'_>) -> usize {
    node.end_position().row + usize::from(node.end_position().column > 0)
}
fn declaration_kind(kind: &str) -> bool {
    matches!(
        kind,
        "function_item"
            | "struct_item"
            | "enum_item"
            | "impl_item"
            | "trait_item"
            | "type_item"
            | "const_item"
            | "static_item"
            | "macro_definition"
            | "use_declaration"
    )
}
fn attached(node: Node<'_>, text: &str) -> (usize, bool) {
    let mut start = node.start_position().row + 1;
    let mut previous = node.prev_named_sibling();
    let mut test = false;
    while let Some(item) = previous {
        let item_text = node_text(item, text);
        let attached_across_space = item.kind() == "attribute_item"
            || item_text.starts_with("///")
            || item_text.starts_with("/**");
        if !matches!(
            item.kind(),
            "attribute_item" | "line_comment" | "block_comment"
        ) || (!attached_across_space && last_line(item) + 1 < start)
        {
            break;
        }
        if item.kind() == "attribute_item" {
            let compact: String = node_text(item, text)
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            let inner = compact
                .strip_prefix("#[")
                .and_then(|s| s.strip_suffix(']'))
                .unwrap_or("");
            // cfg_attr and test_case require macro evaluation and are not treated as tests.
            test |= inner == "test" || (inner.ends_with("::test") && !inner.contains('('));
        }
        start = item.start_position().row + 1;
        previous = item.prev_named_sibling();
    }
    (start, test)
}
fn identifiers_in(node: Node<'_>, text: &str, limit: &mut usize) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        if *limit == 0 {
            break;
        }
        *limit -= 1;
        if matches!(
            n.kind(),
            "identifier" | "type_identifier" | "shorthand_field_identifier"
        ) {
            result.insert(node_text(n, text).to_owned());
        }
        let mut cursor = n.walk();
        stack.extend(n.named_children(&mut cursor));
    }
    result
}
fn references(
    node: Node<'_>,
    text: &str,
    remaining: &mut usize,
) -> (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>, usize) {
    let mut calls = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut shadowed = BTreeSet::new();
    let mut unsupported = 0;
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        if *remaining == 0 {
            break;
        }
        *remaining -= 1;
        match n.kind() {
            "call_expression" => {
                if let Some(callee) = n.child_by_field_name("function") {
                    if matches!(callee.kind(), "identifier" | "scoped_identifier") {
                        calls.insert(node_text(callee, text).split_whitespace().collect());
                    } else {
                        unsupported += 1;
                    }
                }
            }
            "type_identifier" | "identifier" => {
                names.insert(node_text(n, text).to_owned());
            }
            "let_declaration" | "parameter" => {
                if let Some(pattern) = n.child_by_field_name("pattern") {
                    shadowed.extend(identifiers_in(pattern, text, remaining));
                }
            }
            _ => {}
        }
        // Macro token trees are not parsed call expressions. Record only their limitation.
        if n.kind() == "macro_invocation" {
            unsupported += 1;
            continue;
        }
        let mut cursor = n.walk();
        stack.extend(n.named_children(&mut cursor));
    }
    (calls, names, shadowed, unsupported)
}
fn parse_rust(text: &str) -> Result<RustFile> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
    let tree = parser
        .parse(text, None)
        .ok_or("Rust syntax parsing returned no tree.")?;
    let mut result = RustFile::default();
    let mut remaining = MAX_NODES;
    let mut stack = vec![(tree.root_node(), String::new())];
    while let Some((node, scope)) = stack.pop() {
        if remaining == 0 || result.units.len() >= MAX_UNITS {
            result.limitations.push("Syntax extraction reached its node or declaration bound; omitted units are unavailable.".into());
            break;
        }
        remaining -= 1;
        let kind = node.kind();
        let name = if kind == "impl_item" {
            node.child_by_field_name("type").map(|n| {
                node_text(n, text)
                    .split('<')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_owned()
            })
        } else if kind == "use_declaration" {
            Some(format!("import@{}", node.start_position().row + 1))
        } else {
            node.child_by_field_name("name")
                .map(|n| node_text(n, text).to_owned())
        };
        let mut next_scope = scope.clone();
        if let Some(name) = name.filter(|s| !s.is_empty()) {
            let qualified = if scope.is_empty() {
                name.clone()
            } else {
                format!("{scope}::{name}")
            };
            if declaration_kind(kind) {
                let (start, test) = attached(node, text);
                let (calls, names, shadowed, unsupported_calls) =
                    references(node, text, &mut remaining);
                let public = node_text(node, text).trim_start().starts_with("pub ")
                    || node_text(node, text).trim_start().starts_with("pub(");
                result.units.push(Unit {
                    name: name.clone(),
                    scope: scope.clone(),
                    qualified: qualified.clone(),
                    kind: kind.into(),
                    start,
                    end: last_line(node),
                    test: test && kind == "function_item",
                    public,
                    error: node.has_error(),
                    calls,
                    names,
                    shadowed,
                    unsupported_calls,
                });
            }
            if matches!(
                kind,
                "impl_item" | "mod_item" | "trait_item" | "function_item"
            ) {
                next_scope = qualified;
            }
        }
        let mut cursor = node.walk();
        let children: Vec<_> = node.named_children(&mut cursor).collect();
        for child in children.into_iter().rev() {
            stack.push((child, next_scope.clone()));
        }
    }
    if tree.root_node().has_error() {
        result
            .limitations
            .push("Rust syntax contains errors; affected declarations are excluded.".into());
    }
    result.units.sort_by_key(|u| (u.start, u.end));
    Ok(result)
}
fn local_links(unit: &Unit, rust: &RustFile) -> (BTreeSet<usize>, Vec<String>) {
    let mut links = BTreeSet::new();
    let mut warnings = Vec::new();
    let mut unresolved_calls = Vec::new();
    for call in &unit.calls {
        if unit.shadowed.contains(call) {
            warnings.push(format!(
                "Call `{call}` may be shadowed by a local binding; its target is unresolved."
            ));
            continue;
        }
        let qualified = if unit.scope.is_empty() {
            call.clone()
        } else {
            format!("{}::{call}", unit.scope)
        };
        let exact: Vec<_> = rust
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| {
                u.kind == "function_item"
                    && (u.qualified == qualified || call.contains("::") && u.qualified == *call)
            })
            .map(|(i, _)| i)
            .collect();
        if exact.len() == 1 {
            links.insert(exact[0]);
            continue;
        }
        if exact.len() > 1 {
            warnings.push(format!(
                "Call `{call}` has multiple same-file declarations; no target was selected."
            ));
            continue;
        }
        if let Some((owner, _)) = call.rsplit_once("::") {
            let qualified_owner = if unit.scope.is_empty() {
                owner.to_owned()
            } else {
                format!("{}::{owner}", unit.scope)
            };
            let owners: Vec<_> = rust
                .units
                .iter()
                .enumerate()
                .filter(|(_, u)| {
                    matches!(u.kind.as_str(), "impl_item" | "struct_item" | "enum_item")
                        && (u.qualified == qualified_owner || u.qualified == owner)
                })
                .map(|(i, _)| i)
                .collect();
            if !owners.is_empty() {
                links.extend(owners);
                continue;
            }
        }
        unresolved_calls.push(call.clone());
    }
    if !unresolved_calls.is_empty() {
        warnings.push(format!("{} calls have no supported same-file target (examples: {}); external and relative paths were not expanded.", unresolved_calls.len(), unresolved_calls.iter().take(3).cloned().collect::<Vec<_>>().join(", ")));
    }
    for name in unit.names.difference(&unit.shadowed) {
        links.extend(
            rust.units
                .iter()
                .enumerate()
                .filter(|(_, u)| {
                    u.scope == unit.scope
                        && u.name == *name
                        && matches!(
                            u.kind.as_str(),
                            "struct_item"
                                | "enum_item"
                                | "impl_item"
                                | "const_item"
                                | "static_item"
                                | "type_item"
                        )
                })
                .map(|(i, _)| i),
        );
    }
    (links, warnings)
}
fn closure(
    file_index: usize,
    root: usize,
    files: &[File<'_>],
    role: Role,
    anchor: Option<Anchor>,
    method: String,
    omissions: &mut Vec<Omission>,
) -> Vec<Pick> {
    let file = &files[file_index];
    let rust = file.rust.as_ref().unwrap();
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::from([root]);
    let mut picks = Vec::new();
    let mut unsupported = 0;
    while let Some(index) = queue.pop_front() {
        if visited.contains(&index) {
            continue;
        }
        if visited.len() >= MAX_DEPENDENCIES {
            omissions.push(Omission{path:file.source.path.clone(),role,reason:"Same-file dependency traversal reached 64 declarations; the closure is incomplete.".into()});
            break;
        }
        visited.insert(index);
        let unit = &rust.units[index];
        if unit.error {
            omissions.push(Omission {
                path: file.source.path.clone(),
                role,
                reason: format!(
                    "Declaration `{}` contains parse errors and was omitted.",
                    unit.qualified
                ),
            });
            continue;
        }
        let dependency_role = if role == Role::NearbyTest {
            Role::TestFixtureHelper
        } else {
            Role::StructuralDependency
        };
        picks.push(Pick {
            file: file_index,
            start: unit.start,
            end: unit.end,
            role: if index == root { role } else { dependency_role },
            method: if index == root {
                method.clone()
            } else {
                format!("Same-file syntax dependency candidate `{}`", unit.qualified)
            },
            anchor: if index == root { anchor } else { None },
            declaration: true,
            complete_file: false,
        });
        let (links, warnings) = local_links(unit, rust);
        for reason in warnings {
            omissions.push(Omission {
                path: file.source.path.clone(),
                role: dependency_role,
                reason,
            });
        }
        unsupported += unit.unsupported_calls;
        queue.extend(links);
    }
    // Imports explain the local fixture's external names without chasing other files.
    for unit in &rust.units {
        if unit.kind == "use_declaration" && !unit.error && unit.scope == rust.units[root].scope {
            picks.push(Pick {
                file: file_index,
                start: unit.start,
                end: unit.end,
                role: Role::StructuralDependency,
                method: "Same-scope imports; imported definitions were not expanded".into(),
                anchor: None,
                declaration: true,
                complete_file: false,
            });
        }
    }
    if unsupported > 0 {
        omissions.push(Omission{path:file.source.path.clone(),role,reason:format!("{unsupported} method, macro, or indirect call occurrences are unresolved; this is a syntax dependency candidate set, not a complete call graph.")});
    }
    picks
}

#[derive(Clone)]
struct Section {
    start: usize,
    end: usize,
    heading: String,
    leaf: bool,
}
fn markdown_sections(text: &str) -> (Vec<Section>, bool) {
    let lines = lines(text);
    let mut heads = Vec::<(usize, usize, String)>::new();
    let mut fence: Option<(char, usize)> = None;
    let mut unsupported = false;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let indentation = line.len() - trimmed.len();
        let fence_char = trimmed.chars().next().filter(|c| matches!(c, '`' | '~'));
        if let Some((c, count)) = fence {
            let run = trimmed.chars().take_while(|x| *x == c).count();
            if indentation <= 3
                && fence_char == Some(c)
                && run >= count
                && trimmed
                    .get(run..)
                    .is_some_and(|tail| tail.trim().is_empty())
            {
                fence = None;
            }
            continue;
        }
        if indentation <= 3 {
            if let Some(c) = fence_char {
                let n = trimmed.chars().take_while(|x| *x == c).count();
                if n >= 3 {
                    fence = Some((c, n));
                    continue;
                }
            }
            let level = trimmed.bytes().take_while(|b| *b == b'#').count();
            if (1..=6).contains(&level)
                && trimmed
                    .get(level..)
                    .is_some_and(|s| s.is_empty() || s.starts_with(char::is_whitespace))
            {
                heads.push((
                    i + 1,
                    level,
                    trimmed[level..]
                        .trim()
                        .trim_end_matches('#')
                        .trim()
                        .to_owned(),
                ));
            } else if i > 0
                && !lines[i - 1].trim().is_empty()
                && !lines[i - 1].trim_start().starts_with(['`', '~', '#'])
                && (trimmed.trim().chars().all(|c| c == '=')
                    || trimmed.trim().chars().all(|c| c == '-'))
                && !trimmed.trim().is_empty()
            {
                heads.push((
                    i,
                    if trimmed.starts_with('=') { 1 } else { 2 },
                    lines[i - 1].trim().into(),
                ));
            }
        }
        if indentation >= 4 && trimmed.starts_with('#') {
            unsupported = true;
        }
    }
    if fence.is_some() {
        unsupported = true;
    }
    let mut sections = Vec::new();
    for (i, (start, level, heading)) in heads.iter().enumerate() {
        let leaf = !heads.get(i + 1).is_some_and(|(_, next, _)| next > level);
        let end = heads
            .iter()
            .skip(i + 1)
            .find(|(_, next, _)| next <= level)
            .map_or(lines.len(), |(line, _, _)| line - 1);
        sections.push(Section {
            start: *start,
            end,
            heading: heading.clone(),
            leaf,
        });
    }
    (sections, unsupported)
}
fn unit_terms(file: &File<'_>, unit: &Unit) -> BTreeSet<String> {
    crate::terms(&excerpt(file, unit.start, unit.end))
}
fn score(
    query: &BTreeSet<String>,
    terms: &BTreeSet<String>,
    documents: &[BTreeSet<String>],
) -> usize {
    query
        .intersection(terms)
        .map(|term| documents.len() + 1 - documents.iter().filter(|d| d.contains(term)).count())
        .sum()
}
fn pick_range(
    file: usize,
    start: usize,
    end: usize,
    role: Role,
    method: String,
    anchor: Option<Anchor>,
    declaration: bool,
) -> Pick {
    Pick {
        file,
        start,
        end,
        role,
        method,
        anchor,
        declaration,
        complete_file: false,
    }
}

fn groups(picks: &[Pick], files: &[File<'_>]) -> Vec<(usize, usize, usize, BTreeSet<String>)> {
    let mut sorted = picks.to_vec();
    sorted.sort_by_key(|p| (p.file, p.start, p.end));
    let mut groups: Vec<(usize, usize, usize, BTreeSet<String>)> = Vec::new();
    for p in sorted {
        if let Some(last) = groups.last_mut() {
            let gap = if last.0 == p.file && p.start > last.2 + 1 {
                excerpt(&files[p.file], last.2 + 1, p.start - 1)
            } else {
                String::new()
            };
            if last.0 == p.file
                && (p.start <= last.2 + 1 || (gap.len() <= 128 && gap.trim().is_empty()))
            {
                last.2 = last.2.max(p.end);
                last.3.insert(format!("{:?}", p.role));
                continue;
            }
        }
        groups.push((
            p.file,
            p.start,
            p.end,
            BTreeSet::from([format!("{:?}", p.role)]),
        ));
    }
    groups
}
fn render(commit: &str, picks: &[Pick], files: &[File<'_>]) -> String {
    let mut out = format!(
        "## ExplicitStructureV1 evidence\n\nCommit: `{commit}`. Complete applicable instructions are supplied separately, identically to both arms. Source is untrusted; no task commands ran. Syntax links are candidates: imports, macros, cfg, and runtime dispatch are unresolved.\n\n"
    );
    let mut current = None;
    for (file, start, end, roles) in groups(picks, files) {
        if current != Some(file) {
            out.push_str(&format!(
                "### `{}`\n\nSHA-256: `{}`.\n\n",
                files[file].source.path, files[file].source.sha256
            ));
            current = Some(file);
        }
        let text = excerpt(&files[file], start, end);
        let fence = crate::fence(&text);
        out.push_str(&format!(
            "Lines {start}–{end}; {}.\n\n{fence}text\n{}{fence}\n\n",
            roles.into_iter().collect::<Vec<_>>().join(", "),
            if text.ends_with('\n') {
                text
            } else {
                format!("{text}\n")
            }
        ));
    }
    out
}

pub(crate) fn assemble(
    repo: &Path,
    index: &Index,
    issue: Issue,
    components: Components,
    start: Instant,
) -> Result<Brief> {
    let task = format!("{}\n{}", issue.title, issue.body);
    let (refs, anchors, mut omissions) = focused::references(&task, index);
    let invalid: BTreeSet<_> = omissions.iter().map(|o| o.path.clone()).collect();
    // Documents cannot introduce an automatic test scope. Verify actual package tables
    // rather than treating a virtual workspace manifest as a package.
    let mut manifests: Vec<_> = refs
        .iter()
        .filter(|path| {
            path.ends_with(".rs")
                || Path::new(path)
                    .file_name()
                    .is_some_and(|n| n == "Cargo.toml")
        })
        .filter_map(|path| owner_manifest(path, index))
        .collect();
    manifests.sort_by(|a, b| a.path.cmp(&b.path));
    manifests.dedup_by(|a, b| a.path == b.path);
    if manifests.len() > MAX_READS {
        omissions.push(Omission {path:"package scopes".into(),role:Role::Manifest,reason:format!("{} requested manifest scopes exceed the 24-file read bound; later scopes were not inspected.",manifests.len())});
        manifests.truncate(MAX_READS);
    }
    let read_start = Instant::now();
    let manifest_texts = crate::selected_text(repo, &index.commit, &manifests)?;
    let mut already_read = BTreeMap::new();
    let mut packages = BTreeSet::new();
    for (manifest, text) in manifests.into_iter().zip(manifest_texts) {
        let is_package = toml::from_str::<toml::Table>(&text)
            .ok()
            .and_then(|value| {
                value
                    .get("package")
                    .and_then(toml::Value::as_table)
                    .map(|_| ())
            })
            .is_some();
        if is_package {
            packages.insert(manifest.path.clone());
        } else {
            omissions.push(Omission{path:manifest.path.clone(),role:Role::Manifest,reason:"The nearest manifest does not establish a package table; it grants no automatic test scope.".into()});
        }
        already_read.insert(manifest.path.clone(), text);
    }
    let mut clean_task = task.clone();
    for path in &refs {
        clean_task = clean_task.replace(path, "");
    }
    let query = if components.lexical {
        crate::terms(&clean_task)
    } else {
        BTreeSet::new()
    };
    let identifiers = if components.symbols {
        syntax::identifiers(&clean_task)
    } else {
        BTreeSet::new()
    };
    for path in &refs {
        let reason = if focused::instruction(path) {
            Some(
                "Complete mandatory instructions are supplied separately; this pack omits instruction files.",
            )
        } else if !index.files.iter().any(|f| f.path == *path) {
            Some("Explicit path is absent or excluded from the bounded source index.")
        } else {
            None
        };
        if let Some(reason) = reason {
            omissions.push(Omission {
                path: path.clone(),
                role: focused::role(path),
                reason: reason.into(),
            });
        }
    }
    let mut sources: Vec<_> = index
        .files
        .iter()
        .filter(|f| {
            !focused::instruction(&f.path)
                && !invalid.contains(&f.path)
                && (refs.contains(&f.path)
                    || (focused::test_path(&f.path)
                        && owner_manifest(&f.path, index)
                            .is_some_and(|p| packages.contains(&p.path))))
        })
        .collect();
    sources.sort_by_key(|s| {
        (
            !refs.contains(&s.path),
            !anchors.contains_key(&s.path),
            s.path.clone(),
        )
    });
    let candidate_files = sources.len();
    let mut admitted = Vec::new();
    let mut available = MAX_READS.saturating_sub(already_read.len());
    let mut omitted_nearby = 0;
    for source in sources {
        if already_read.contains_key(&source.path) {
            admitted.push(source);
        } else if available > 0 {
            admitted.push(source);
            available -= 1;
        } else if refs.contains(&source.path) {
            omissions.push(Omission {path:source.path.clone(),role:focused::role(&source.path),reason:"The 24-file read bound omitted this explicit path after package-scope inspection.".into()});
        } else {
            omitted_nearby += 1;
        }
    }
    if omitted_nearby > 0 {
        omissions.push(Omission{path:"nearby tests".into(),role:Role::NearbyTest,reason:format!("{omitted_nearby} admitted test files were omitted by the 24-file read bound after package-scope inspection.")});
    }
    let unread: Vec<_> = admitted
        .iter()
        .copied()
        .filter(|source| !already_read.contains_key(&source.path))
        .collect();
    let texts = crate::selected_text(repo, &index.commit, &unread)?;
    for (source, text) in unread.into_iter().zip(texts) {
        already_read.insert(source.path.clone(), text);
    }
    let read_ms = read_start.elapsed().as_secs_f64() * 1000.0;
    let mut files = Vec::new();
    for source in admitted {
        let text = already_read
            .remove(&source.path)
            .ok_or("Admitted source was not read.")?;
        let rust = if source.path.ends_with(".rs") {
            Some(parse_rust(&text)?)
        } else {
            None
        };
        if let Some(rust) = &rust {
            for reason in &rust.limitations {
                omissions.push(Omission {
                    path: source.path.clone(),
                    role: focused::role(&source.path),
                    reason: reason.clone(),
                });
            }
        }
        files.push(File {
            source,
            explicit: refs.contains(&source.path),
            text,
            rust,
        });
    }
    let mut bundles = Vec::new();
    let mut public_links = BTreeSet::new();
    for (file_index, file) in files
        .iter()
        .enumerate()
        .filter(|(_, f)| f.explicit && f.rust.is_some())
    {
        let rust = file.rust.as_ref().unwrap();
        if let Some(module) = Path::new(&file.source.path)
            .file_stem()
            .and_then(|s| s.to_str())
        {
            public_links.extend(
                rust.units
                    .iter()
                    .filter(|u| u.public && u.kind == "function_item")
                    .map(|u| (module.to_owned(), u.name.clone())),
            );
        }
        let targets: Vec<_> = anchors
            .get(&file.source.path)
            .map_or(vec![None], |a| a.iter().copied().map(Some).collect());
        for anchor in targets {
            let eligible: Vec<_> = rust
                .units
                .iter()
                .enumerate()
                .filter(|(_, u)| {
                    !u.error && (!u.test || anchor.is_some()) && u.kind != "use_declaration"
                })
                .collect();
            let selected = if let Some(a) = anchor {
                let matches: Vec<_> = eligible
                    .iter()
                    .copied()
                    .filter(|(_, u)| u.start <= a.start_line && u.end >= a.end_line)
                    .collect();
                let smallest = matches.iter().map(|(_, u)| u.end - u.start).min();
                let tied: Vec<_> = matches
                    .iter()
                    .filter(|(_, u)| Some(u.end - u.start) == smallest)
                    .collect();
                tied.first()
                    .map(|&&(i, _)| (i, tied.len(), "containing explicit line anchor"))
            } else {
                let exact: Vec<_> = eligible
                    .iter()
                    .copied()
                    .filter(|(_, u)| {
                        identifiers.contains(&u.qualified) || identifiers.contains(&u.name)
                    })
                    .collect();
                if let Some((i, _)) = exact.first() {
                    Some((*i, exact.len(), "exact declaration identifier"))
                } else {
                    let corpus: Vec<_> =
                        eligible.iter().map(|(_, u)| unit_terms(file, u)).collect();
                    let mut scored: Vec<_> = eligible
                        .iter()
                        .enumerate()
                        .map(|(n, (i, _))| (*i, score(&query, &corpus[n], &corpus)))
                        .filter(|(_, s)| *s > 0)
                        .collect();
                    scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                    scored.first().map(|(i, s)| {
                        (
                            *i,
                            scored.iter().filter(|(_, other)| other == s).count(),
                            "term match within explicit file",
                        )
                    })
                }
            };
            if let Some((root, ties, method)) = selected {
                let unit = &rust.units[root];
                let method = format!(
                    "Complete declaration `{}`; {method}; {ties} equally ranked candidate(s)",
                    unit.qualified
                );
                if ties > 1 {
                    omissions.push(Omission{path:file.source.path.clone(),role:Role::ExplicitSource,reason:format!("{ties} declarations tie for this selection; source order chooses `{}`.",unit.qualified)});
                }
                let picks = closure(
                    file_index,
                    root,
                    &files,
                    Role::ExplicitSource,
                    anchor,
                    method,
                    &mut omissions,
                );
                bundles.push(Bundle {
                    picks,
                    path: file.source.path.clone(),
                    role: Role::ExplicitSource,
                    description: format!(
                        "Declaration `{}` and its same-file dependency candidates",
                        unit.qualified
                    ),
                });
            } else if anchor.is_none() && file.text.len() <= SMALL_FILE {
                bundles.push(Bundle {
                    picks: vec![Pick {
                        file: file_index,
                        start: 1,
                        end: file.source.line_count,
                        role: Role::ExplicitSource,
                        method: "Complete small explicit file; no declaration target established"
                            .into(),
                        anchor: None,
                        declaration: false,
                        complete_file: true,
                    }],
                    path: file.source.path.clone(),
                    role: Role::ExplicitSource,
                    description: "Complete small explicit file".into(),
                });
            } else {
                omissions.push(Omission{path:file.source.path.clone(),role:Role::ExplicitSource,reason:"No complete declaration resolves the explicit anchor or supported issue terms; no partial source was substituted.".into()});
            }
        }
    }
    // Tests are ranked only inside explicit Rust files and admitted same-package test files.
    let test_units: Vec<_> = files
        .iter()
        .enumerate()
        .flat_map(|(f, file)| {
            file.rust.iter().flat_map(move |r| {
                r.units
                    .iter()
                    .enumerate()
                    .filter(|(_, u)| u.test && !u.error)
                    .map(move |(u, _)| (f, u))
            })
        })
        .collect();
    let test_corpus: Vec<_> = test_units
        .iter()
        .map(|(f, u)| unit_terms(&files[*f], &files[*f].rust.as_ref().unwrap().units[*u]))
        .collect();
    let mut tests = Vec::new();
    for (position, (f, u)) in test_units.iter().copied().enumerate() {
        let file = &files[f];
        let unit = &file.rust.as_ref().unwrap().units[u];
        let direct = unit.calls.iter().any(|call| {
            if unit.shadowed.contains(call) {
                return false;
            }
            let parts: Vec<_> = call.split("::").collect();
            parts.len() >= 2
                && public_links.contains(&(
                    parts[parts.len() - 2].to_owned(),
                    parts[parts.len() - 1].to_owned(),
                ))
                || (file.explicit
                    && parts.len() == 1
                    && file
                        .rust
                        .as_ref()
                        .unwrap()
                        .units
                        .iter()
                        .any(|u| u.public && u.kind == "function_item" && u.name == *call))
        });
        let lexical = score(&query, &test_corpus[position], &test_corpus)
            + 3 * score(&query, &crate::terms(&unit.name), &test_corpus);
        if direct || lexical > 0 {
            tests.push((f, u, direct, lexical));
        }
    }
    tests.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then(b.3.cmp(&a.3))
            .then(a.0.cmp(&b.0))
            .then(a.1.cmp(&b.1))
    });
    if test_units.is_empty() {
        omissions.push(Omission{path:"nearby tests".into(),role:Role::NearbyTest,reason:"No supported #[test] or namespaced test attribute was found in the admitted files; inline tests in other files were not searched.".into()});
    } else if tests.is_empty() {
        omissions.push(Omission {
            path: "nearby tests".into(),
            role: Role::NearbyTest,
            reason: "No test has a supported source reference or positive scoped term match."
                .into(),
        });
    }
    if tests.len() > MAX_TESTS {
        omissions.push(Omission{path:"nearby tests".into(),role:Role::NearbyTest,reason:format!("{} additional ranked tests were omitted by the one automatically ranked test bound.",tests.len()-MAX_TESTS)});
    }
    for (position, (f, u, direct, lexical)) in tests.iter().copied().enumerate() {
        let file = &files[f];
        let unit = &file.rust.as_ref().unwrap().units[u];
        if position >= MAX_TESTS {
            continue;
        }
        let ties = tests
            .iter()
            .filter(|(_, _, d, s)| *d == direct && *s == lexical)
            .count();
        let method = if direct {
            "Syntactic module/public-function call candidate"
        } else {
            "Scope-limited lexical fallback; no direct source call was established"
        };
        let description = format!(
            "Complete test `{}`; {method}; {ties} equally ranked candidate(s)",
            unit.qualified
        );
        let picks = closure(
            f,
            u,
            &files,
            Role::NearbyTest,
            None,
            description.clone(),
            &mut omissions,
        );
        if ties > 1 {
            omissions.push(Omission {
                path: file.source.path.clone(),
                role: Role::NearbyTest,
                reason: format!(
                    "{ties} test candidates tie; source order selects `{}`.",
                    unit.qualified
                ),
            });
        }
        bundles.push(Bundle {
            picks,
            path: file.source.path.clone(),
            role: Role::NearbyTest,
            description,
        });
    }
    for (f, file) in files
        .iter()
        .enumerate()
        .filter(|(_, f)| f.explicit && f.rust.is_none())
    {
        let role = focused::role(&file.source.path);
        if anchors
            .get(&file.source.path)
            .is_some_and(|targets| targets.iter().any(|a| a.end_line > file.source.line_count))
        {
            omissions.push(Omission {path:file.source.path.clone(),role,reason:"An explicit line anchor is outside the pinned file; no substitute content was selected.".into()});
            continue;
        }
        if !file.source.path.ends_with(".md") {
            if file.text.len() <= SMALL_FILE {
                bundles.push(Bundle {
                    picks: vec![Pick {
                        file: f,
                        start: 1,
                        end: file.source.line_count,
                        role,
                        method: "Complete small explicit text file".into(),
                        anchor: None,
                        declaration: false,
                        complete_file: true,
                    }],
                    path: file.source.path.clone(),
                    role,
                    description: "Complete small text file".into(),
                });
            } else {
                omissions.push(Omission{path:file.source.path.clone(),role,reason:"No complete-unit extractor supports this large file; no partial excerpt was substituted.".into()});
            }
            continue;
        }
        let (sections, limited) = markdown_sections(&file.text);
        if limited {
            omissions.push(Omission {
                path: file.source.path.clone(),
                role,
                reason:
                    "An unclosed fence or indented heading limits Markdown section interpretation."
                        .into(),
            });
        }
        if sections.is_empty() {
            if file.text.len() <= SMALL_FILE {
                bundles.push(Bundle {
                    picks: vec![Pick {
                        file: f,
                        start: 1,
                        end: file.source.line_count,
                        role,
                        method: "Complete small document without supported headings".into(),
                        anchor: None,
                        declaration: false,
                        complete_file: true,
                    }],
                    path: file.source.path.clone(),
                    role,
                    description: "Complete document".into(),
                });
            } else {
                omissions.push(Omission {
                    path: file.source.path.clone(),
                    role,
                    reason: "No supported complete Markdown section was found.".into(),
                });
            }
            continue;
        }
        let corpus: Vec<_> = sections
            .iter()
            .map(|s| crate::terms(&excerpt(file, s.start, s.end)))
            .collect();
        let mut ranked: Vec<_> = sections
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let anchored = anchors.get(&file.source.path).is_some_and(|targets| {
                    targets.iter().any(|a| {
                        sections
                            .iter()
                            .enumerate()
                            .filter(|(_, candidate)| {
                                candidate.start <= a.start_line && candidate.end >= a.end_line
                            })
                            .min_by_key(|(_, candidate)| candidate.end - candidate.start)
                            .is_some_and(|(selected, _)| selected == i)
                    })
                });
                let value = score(&query, &corpus[i], &corpus);
                (anchored || (s.leaf && value > 0)).then_some((i, anchored, value))
            })
            .collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)).then(a.0.cmp(&b.0)));
        if let Some(targets) = anchors.get(&file.source.path) {
            for a in targets {
                if !sections
                    .iter()
                    .any(|s| s.start <= a.start_line && s.end >= a.end_line)
                {
                    omissions.push(Omission{path:file.source.path.clone(),role,reason:format!("Lines {}–{} do not resolve to one supported complete Markdown section.",a.start_line,a.end_line)});
                }
            }
        }
        if ranked.is_empty() {
            omissions.push(Omission {
                path: file.source.path.clone(),
                role,
                reason: "No complete Markdown section matches an anchor or scoped issue terms."
                    .into(),
            });
        }
        if ranked.len() > MAX_DOC_SECTIONS {
            omissions.push(Omission {
                path: file.source.path.clone(),
                role,
                reason: format!(
                    "{} additional ranked document sections were omitted by the two-section bound.",
                    ranked.len() - MAX_DOC_SECTIONS
                ),
            });
        }
        for (position, (i, anchored, value)) in ranked.iter().copied().enumerate() {
            let section = &sections[i];
            if position >= MAX_DOC_SECTIONS {
                continue;
            }
            let ties = ranked
                .iter()
                .filter(|(_, a, v)| *a == anchored && *v == value)
                .count();
            let method = format!(
                "Complete Markdown section `{}`; {}; {ties} equally ranked candidate(s)",
                section.heading,
                if anchored {
                    "explicit anchor"
                } else {
                    "term match within named document"
                }
            );
            bundles.push(Bundle {
                picks: vec![pick_range(
                    f,
                    section.start,
                    section.end,
                    role,
                    method.clone(),
                    None,
                    false,
                )],
                path: file.source.path.clone(),
                role,
                description: method,
            });
        }
    }
    let mut picks = Vec::new();
    for bundle in bundles {
        let mut candidate = picks.clone();
        candidate.extend(bundle.picks);
        if groups(&candidate, &files).len() > MAX_GROUPS
            || render(&index.commit, &candidate, &files).len() + 512 > focused::BYTE_BUDGET
        {
            omissions.push(Omission{path:bundle.path,role:bundle.role,reason:format!("{} did not fit the 16 KiB/16-range bound; its complete bundle was omitted without clipping.",bundle.description)});
        } else {
            picks = candidate;
        }
    }
    let mut evidence = Vec::new();
    for (f, start, end, roles) in groups(&picks, &files) {
        let file = &files[f];
        let text = excerpt(file, start, end);
        evidence.push(Evidence {
            path: file.source.path.clone(),
            blob: file.source.blob.clone(),
            file_sha256: file.source.sha256.clone(),
            excerpt_sha256: crate::sha256(text.as_bytes()),
            start_line: start,
            end_line: end,
            total_lines: file.source.line_count,
            reasons: roles.into_iter().collect(),
            text,
            syntax_selection: None,
        });
    }
    let selections = picks
        .iter()
        .map(|p| Selection {
            path: files[p.file].source.path.clone(),
            role: p.role,
            anchor: p.anchor,
            method: p.method.clone(),
            complete_file: p.complete_file,
            complete_declaration: p.declaration,
            start_line: p.start,
            end_line: p.end,
        })
        .collect();
    let mut markdown = render(&index.commit, &picks, &files);
    let omitted_coverage_records = omissions.len().saturating_sub(MAX_COVERAGE);
    omissions.truncate(MAX_COVERAGE);
    let mut shown = 0;
    let mut coverage_bytes = 0;
    for omission in &omissions {
        let line = format!("Coverage `{}`: {}\n", omission.path, omission.reason);
        if coverage_bytes + line.len() <= MAX_RENDERED_COVERAGE
            && markdown.len() + line.len() + 256 <= focused::BYTE_BUDGET
        {
            coverage_bytes += line.len();
            markdown.push_str(&line);
            shown += 1;
        }
    }
    markdown.push_str(&format!("\nCoverage: {} complete ranges; {shown}/{} retained warnings shown; {omitted_coverage_records} additional warnings omitted. Missing, ambiguous, and budget-omitted evidence remains unresolved. Full selection provenance is in briefing.json.\n",evidence.len(),omissions.len()));
    if markdown.len() > focused::BYTE_BUDGET {
        return Err("ExplicitStructureV1 payload exceeds 16 KiB.".into());
    }
    let mut provenance = syntax::Provenance::default();
    provenance.extractor_version = "briefing-lab-explicit-structure-v1".into();
    let pack=Pack{schema:"openagents.briefing-lab.explicit-structure.v1".into(),byte_budget:focused::BYTE_BUDGET,packed_bytes:markdown.len(),source_bytes:evidence.iter().map(|e|e.text.len()).sum(),selections,omissions,omitted_coverage_records,syntax:provenance.clone(),instructions:"Complete AGENTS.md, CLAUDE.md, and applicable skills must be supplied separately and identically to both arms.".into(),markdown};
    let selected_files: BTreeSet<_> = evidence.iter().map(|e| e.path.clone()).collect();
    let omitted_candidates = candidate_files.saturating_sub(selected_files.len());
    Ok(Brief{schema:"openagents.briefing-lab.preview.v1".into(),commit:index.commit.clone(),issue,components,evidence,history:vec![],notes:vec!["ExplicitStructureV1 admits named files and test filenames under verified package manifests derived from explicit Rust or Cargo paths only. Documents do not introduce automatic test scopes. Nearby inline tests in other source files are not searched. Syntax dependency candidates stay in the same file; external calls, dispatch, macro expansion, cfg, and shadowing beyond simple local bindings are unresolved.".into(),"Complete declarations include attached attributes/comments. Markdown term matching uses complete leaf ATX or Setext sections; anchors select the smallest complete containing section. Fenced headings are skipped; this is not a full Markdown parser. The pack automatically selects at most one test and two sections per named document; explicit anchors can select additional tests. Dependency bundles are omitted atomically when they cannot fit.".into(),"The exact optional payload is focused.md. It is at most 16 KiB including coverage text. Rendered warning details are capped at 768 bytes plus a summary; bounded full records remain in briefing.json. Instructions and the original issue remain separate. Source validation, fresh syntax parsing, and rendering are included in assembly time.".into()],candidate_files,omitted_candidates,index_omissions:index.omissions.clone(),timings_ms:BTreeMap::from([("selected_git_validation_and_read".into(),read_ms),("assembly".into(),start.elapsed().as_secs_f64()*1000.0)]),syntax:Some(provenance),execution:None,focused:Some(pack)})
}
