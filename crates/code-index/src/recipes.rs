//! Typed extension recipes and verification against a frozen Git tree.

use code_graph::{
    Index,
    build::{Source, build},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Component, Path};
use std::process::Command;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub repository: String,
    pub commit: String,
    pub workspace: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub path: String,
    pub symbol: Option<String>,
    pub start: u32,
    pub end: u32,
    pub content_digest: String,
    pub excerpt: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Role {
    pub id: String,
    pub purpose: String,
    pub required: bool,
    pub when: Option<String>,
    pub references: Vec<Reference>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Condition {
    pub id: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewFile {
    pub directory: String,
    pub purpose: String,
    pub integration: Reference,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub package: String,
    pub manifest: String,
    pub filter: Option<String>,
    pub establishes: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceClass {
    SourceReviewed,
    SourceVerified,
    ReplayVerified,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub id: String,
    pub capability: String,
    pub operation: String,
    pub description: String,
    pub aliases: Vec<String>,
    pub exclusions: Vec<String>,
    pub conditions: Vec<Condition>,
    pub roles: Vec<Role>,
    pub examples: Vec<Reference>,
    pub new_files: Vec<NewFile>,
    pub checks: Vec<Check>,
    pub evidence: EvidenceClass,
    pub limitations: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependency_scopes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub schema: String,
    pub snapshot: Snapshot,
    pub recipes: Vec<Recipe>,
}

pub(crate) fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("The repository could not provide the requested revision or source.".into());
    }
    Ok(output.stdout)
}

pub fn snapshot(root: &Path, rev: &str, workspace: &str) -> Result<Snapshot, String> {
    if workspace.trim().is_empty() {
        return Err("Choose a workspace name.".into());
    }
    let commit = String::from_utf8(git(
        root,
        &["rev-parse", "--verify", &format!("{rev}^{{commit}}")],
    )?)
    .map_err(|e| e.to_string())?
    .trim()
    .to_owned();
    let roots = git(root, &["rev-list", "--max-parents=0", &commit])?;
    let remote = git(root, &["config", "--get", "remote.origin.url"]).unwrap_or_default();
    let remote = normalize_remote(&String::from_utf8_lossy(&remote));
    Ok(Snapshot {
        repository: crate::digest(&[roots, remote.into_bytes()].concat()),
        commit,
        workspace: workspace.into(),
    })
}

pub fn normalize_remote(remote: &str) -> String {
    let remote = remote.trim();
    let normalized = if let Some(path) = remote.strip_prefix("git@github.com:") {
        format!("github.com/{path}")
    } else if let Ok(url) = reqwest::Url::parse(remote) {
        format!("{}{}", url.host_str().unwrap_or(""), url.path())
    } else {
        remote.to_owned()
    };
    let normalized = normalized.trim_end_matches('/').trim_end_matches(".git");
    if normalized.starts_with("github.com/") {
        normalized.to_ascii_lowercase()
    } else {
        normalized.into()
    }
}

pub fn relative(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.contains(['\\', '\0', ':'])
        || !Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err("Use a path inside the repository.".into());
    }
    Ok(())
}

pub fn source(root: &Path, snapshot: &Snapshot, path: &str) -> Result<String, String> {
    relative(path)?;
    let object = format!("{}:{path}", snapshot.commit);
    let size = String::from_utf8(git(root, &["cat-file", "-s", &object])?)
        .map_err(|e| e.to_string())?
        .trim()
        .parse::<usize>()
        .map_err(|_| "The source size is invalid.")?;
    if size > 2 * 1024 * 1024 {
        return Err("Choose a source file smaller than 2 MiB.".into());
    }
    let mode = String::from_utf8(git(root, &["ls-tree", &snapshot.commit, "--", path])?)
        .map_err(|e| e.to_string())?;
    if !(mode.starts_with("100644 ") || mode.starts_with("100755 ")) {
        return Err("Choose a regular source file.".into());
    }
    String::from_utf8(git(root, &["cat-file", "blob", &object])?)
        .map_err(|_| "The source is not UTF-8 text.".into())
}

pub struct Context {
    pub root: std::path::PathBuf,
    pub snapshot: Snapshot,
    pub graph: Index,
}

impl Context {
    pub fn open(root: &Path, rev: &str, workspace: &str) -> Result<Self, String> {
        let snapshot = snapshot(root, rev, workspace)?;
        let graph = Index::new(
            build(&Source::Commit {
                root: root.to_path_buf(),
                rev: snapshot.commit.clone(),
            })?
            .graph,
        );
        Ok(Self {
            root: root.to_path_buf(),
            snapshot,
            graph,
        })
    }

    pub fn reference(
        &self,
        path: &str,
        symbol: Option<&str>,
        start: u32,
        end: u32,
    ) -> Result<Reference, String> {
        let text = source(&self.root, &self.snapshot, path)?;
        let (start, end) = if let Some(symbol) = symbol {
            let id = self.graph.resolve(symbol, None)?;
            let node = &self.graph.graph.nodes[id as usize];
            if self.graph.graph.files[node.file as usize] != path {
                return Err("The symbol belongs to a different file.".into());
            }
            (node.line, node.end_line.min(node.line + 23))
        } else {
            (start, end)
        };
        let excerpt = excerpt(&text, start, end)?;
        Ok(Reference {
            path: path.into(),
            symbol: symbol.map(str::to_owned),
            start,
            end,
            content_digest: crate::digest(text.as_bytes()),
            excerpt,
        })
    }

    pub fn verify_reference(&self, r: &Reference) -> Result<(), String> {
        let text = source(&self.root, &self.snapshot, &r.path)?;
        if crate::digest(text.as_bytes()) != r.content_digest
            || excerpt(&text, r.start, r.end)? != r.excerpt
        {
            return Err("The source reference does not match this revision.".into());
        }
        if let Some(symbol) = &r.symbol {
            let id = self.graph.resolve(symbol, None)?;
            let node = &self.graph.graph.nodes[id as usize];
            if self.graph.graph.files[node.file as usize] != r.path
                || r.start < node.line
                || r.end > node.end_line
            {
                return Err("The source span does not belong to the symbol.".into());
            }
        }
        Ok(())
    }

    pub fn verify(&self, artifact: &Artifact) -> Result<Artifact, String> {
        if artifact.schema != "openagents.task-index.v1"
            || artifact.snapshot != self.snapshot
            || artifact.recipes.len() > 4096
        {
            return Err("The task index does not match this repository and workspace.".into());
        }
        let mut verified = artifact.clone();
        let mut ids = BTreeSet::new();
        for recipe in &mut verified.recipes {
            for scope in &recipe.dependency_scopes {
                relative(scope)?;
                if git(
                    &self.root,
                    &[
                        "cat-file",
                        "-t",
                        &format!("{}:{scope}", self.snapshot.commit),
                    ],
                )? != b"tree\n"
                {
                    return Err(
                        "A recipe discovery scope must be an existing repository directory.".into(),
                    );
                }
            }
            if recipe.id.trim().is_empty()
                || recipe.description.trim().is_empty()
                || !ids.insert(recipe.id.clone())
                || recipe.roles.is_empty()
                || recipe.checks.is_empty()
            {
                return Err("Each recipe needs a unique name, change roles, and checks.".into());
            }
            let conditions: BTreeSet<_> = recipe.conditions.iter().map(|c| c.id.as_str()).collect();
            let mut roles = BTreeSet::new();
            for role in &recipe.roles {
                if !roles.insert(&role.id)
                    || role.references.is_empty()
                    || role
                        .when
                        .as_ref()
                        .is_some_and(|c| !conditions.contains(c.as_str()))
                {
                    return Err("Every change role needs evidence and a declared condition.".into());
                }
                for r in &role.references {
                    self.verify_reference(r)?;
                }
            }
            for r in &recipe.examples {
                self.verify_reference(r)?;
            }
            for new in &recipe.new_files {
                relative(&new.directory)?;
                let kind = git(
                    &self.root,
                    &[
                        "cat-file",
                        "-t",
                        &format!("{}:{}", self.snapshot.commit, new.directory),
                    ],
                )?;
                if kind != b"tree\n" {
                    return Err("The proposed file directory does not exist.".into());
                }
                self.verify_reference(&new.integration)?;
            }
            for check in &recipe.checks {
                if check.package.is_empty()
                    || !check
                        .package
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                {
                    return Err("Choose a Cargo package name.".into());
                }
                let manifest = source(&self.root, &self.snapshot, &check.manifest)?;
                if !manifest
                    .lines()
                    .any(|line| line.trim() == format!("name = \"{}\"", check.package))
                {
                    return Err("The check package does not match its manifest.".into());
                }
            }
            // Source validation cannot turn a teacher's claim into a behavior result.
            recipe.evidence = EvidenceClass::SourceVerified;
        }
        verified.recipes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(verified)
    }
}

fn excerpt(text: &str, start: u32, end: u32) -> Result<String, String> {
    if start == 0 || end < start || end - start >= 96 || end as usize > text.lines().count() {
        return Err("Choose an existing source span of at most 96 lines.".into());
    }
    Ok(text
        .lines()
        .skip(start as usize - 1)
        .take((end - start + 1) as usize)
        .collect::<Vec<_>>()
        .join("\n"))
}
