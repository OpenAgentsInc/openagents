//! What the hosted runner will test: the catalog tools, and chat-made
//! tools built only from a skill and catalog tools.
//!
//! A catalog tool is an extension directory the operator lists
//! (`EVAL_RUNNER_CATALOG`): a package record, the program it pins, and any
//! skills, resolved with `coder::package` exactly as `openagents ext eval`
//! resolves a directory, so a hosted result and a check on a computer name
//! the same subject. A request may name a catalog tool by that
//! DefinitionRef, or by the DefinitionRef of a Wasm guest one of its
//! program's `module` steps runs (the chat catalog's names,
//! `ext_eval::author::catalog`). Anything else is refused `not_admitted`.

use std::path::{Path, PathBuf};

use coder::package::{HeldLock, Package};
use ext_eval::arms::{self, Program, Skill, Subject};
use nostr::cj_conversation::DraftTool;
use nostr::contracts::{DefinitionRef, digest_bytes, jcs, parse_definition};
use serde_json::{Value, json};

use crate::Refusal;

/// The package record file at an extension's root.
pub const PACKAGE_FILE: &str = "package.json";
/// Where an extension's skills live, relative to its root.
pub const SKILLS_DIR: &str = "skills";
/// The key an unpublished extension's definition names, as
/// `openagents ext eval` names it.
pub const LOCAL_KEY: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// One catalog tool, resolved.
#[derive(Clone, Debug)]
pub struct Tool {
    /// Its name, from the package record.
    pub name: String,
    /// Its directory.
    pub root: PathBuf,
    /// The package record.
    pub package: Package,
    /// The package record's exact bytes, which its DefinitionRef digests.
    pub record: Vec<u8>,
    /// The resolved package lock.
    pub lock: coder::package::Lock,
    /// The subject the runner admits for it.
    pub subject: Subject,
    /// Its DefinitionRef.
    pub definition: DefinitionRef,
    /// The DefinitionRefs of the Wasm guests its program runs, which name
    /// the same tool.
    pub aliases: Vec<DefinitionRef>,
}

impl Tool {
    /// Whether `definition` names this tool: its own DefinitionRef or one
    /// of its guests', by ID and artifact digest.
    #[must_use]
    pub fn names(&self, definition: &DefinitionRef) -> bool {
        std::iter::once(&self.definition)
            .chain(&self.aliases)
            .any(|known| {
                known.id == definition.id && known.artifact.digest == definition.artifact.digest
            })
    }

    /// Holds the tool's package lock for a run, so bytes that change under
    /// it are reported.
    #[must_use]
    pub fn hold(&self) -> HeldLock {
        HeldLock::open(&self.lock)
    }

    /// Whether the directory still resolves to the lock the runner loaded.
    #[must_use]
    pub fn unchanged(&self) -> bool {
        Package::resolve(&self.root, &self.package)
            .is_ok_and(|now| now.digest() == self.lock.digest())
    }
}

fn is_hex64(text: &str) -> bool {
    text.len() == 64
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// Resolves the extension directory `root` into a catalog tool.
///
/// # Errors
///
/// Names why the package record, its program, or a skill doesn't load.
pub fn resolve(root: &Path) -> Result<Tool, String> {
    let record = root.join(PACKAGE_FILE);
    let bytes = std::fs::read(&record).map_err(|error| format!("{}: {error}", record.display()))?;
    let package = Package::load(&record)?;
    let lock = Package::resolve(root, &package)
        .map_err(|refusal| format!("{}: the package doesn't resolve: {refusal}", root.display()))?;
    let program_path = root.join(&lock.program.found);
    let program = std::fs::read(&program_path)
        .map_err(|error| format!("{}: {error}", program_path.display()))?;
    let skills = arms::skills_in(&root.join(SKILLS_DIR))?;
    let publisher = if is_hex64(&package.publisher) {
        package.publisher.clone()
    } else {
        LOCAL_KEY.to_string()
    };
    let definition_value =
        arms::definition(&publisher, &package.slug, &package.program.name, &bytes);
    let definition = parse_definition(&definition_value).map_err(|error| error.to_string())?;
    let aliases = guest_targets(&program);
    let subject = Subject {
        slug: package.slug.clone(),
        definition: definition_value,
        package_lock: serde_json::to_value(&lock).unwrap_or(Value::Null),
        programs: vec![Program {
            slug: package.program.name.clone(),
            bytes: program,
        }],
        skills,
    };
    Ok(Tool {
        name: package.name.clone(),
        root: root.to_path_buf(),
        package,
        record: bytes,
        lock,
        subject,
        definition,
        aliases,
    })
}

/// The DefinitionRefs of the Wasm guests a program's `module` steps run.
fn guest_targets(program: &[u8]) -> Vec<DefinitionRef> {
    let Ok(value) = serde_json::from_slice::<Value>(program) else {
        return Vec::new();
    };
    value
        .pointer("/definition/steps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|step| step.get("kind").and_then(Value::as_str) == Some("module"))
        .filter_map(|step| step.get("target"))
        .filter_map(|target| parse_definition(target).ok())
        .collect()
}

/// The tools the runner admits.
#[derive(Clone, Debug, Default)]
pub struct Catalog {
    /// The tools, in the order the operator listed them.
    pub tools: Vec<Tool>,
}

impl Catalog {
    /// Resolves every directory in `roots`.
    ///
    /// # Errors
    ///
    /// Names the first directory that doesn't resolve.
    pub fn load(roots: &[PathBuf]) -> Result<Self, String> {
        let tools = roots
            .iter()
            .map(|root| resolve(root))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { tools })
    }

    /// The tool `definition` names.
    #[must_use]
    pub fn find(&self, definition: &DefinitionRef) -> Option<&Tool> {
        self.tools.iter().find(|tool| tool.names(definition))
    }

    /// The tool a qualified ID names: its own or one of its guests'.
    #[must_use]
    pub fn by_id(&self, id: &str) -> Option<&Tool> {
        self.tools.iter().find(|tool| {
            tool.definition.id == id || tool.aliases.iter().any(|alias| alias.id == id)
        })
    }

    /// The subject for a draft's tool: the catalog tool it names, or a
    /// chat-made tool (a skill, with the catalog tools it turns on) that
    /// `requester` made.
    ///
    /// # Errors
    ///
    /// `not_admitted` for a tool outside the catalog or a chat-made tool
    /// that turns on one.
    pub fn draft_subject(&self, tool: &DraftTool, requester: &str) -> Result<Subject, Refusal> {
        if let Some(definition) = &tool.catalog {
            return self
                .find(definition)
                .map(|found| found.subject.clone())
                .ok_or_else(|| {
                    Refusal::not_admitted(format!(
                        "{} isn't a catalog tool, and the hosted runner tests only those",
                        tool.name
                    ))
                });
        }
        let skill = tool
            .skill
            .as_ref()
            .ok_or_else(|| Refusal::not_admitted("a chat-made tool needs its skill"))?;
        let mut programs: Vec<Program> = Vec::new();
        for id in &tool.uses {
            let used = self.by_id(id).ok_or_else(|| {
                Refusal::not_admitted(format!(
                    "{id} isn't a catalog tool, and a chat-made tool may turn on catalog tools only"
                ))
            })?;
            for program in &used.subject.programs {
                if !programs.iter().any(|known| known.slug == program.slug) {
                    programs.push(program.clone());
                }
            }
        }
        let slug = made_slug(&tool.name);
        let record = made_record(tool);
        Ok(Subject {
            slug: slug.clone(),
            definition: json!({
                "id": format!("{requester}:{slug}/skill"),
                "artifact": {
                    "digest": digest_bytes(&record),
                    "size": record.len(),
                    "media_type": "application/json",
                    "schema": nostr::cj_conversation::DRAFT_SCHEMA,
                },
            }),
            package_lock: json!({"made_in_chat": true, "uses": tool.uses}),
            programs,
            skills: vec![Skill {
                name: slug,
                bytes: skill.as_bytes().to_vec(),
            }],
        })
    }
}

/// A chat-made tool's record: its draft `tool` object, canonical, which
/// its DefinitionRef digests.
fn made_record(tool: &DraftTool) -> Vec<u8> {
    jcs(&json!({
        "name": tool.name,
        "summary": tool.summary,
        "skill": tool.skill,
        "uses": tool.uses,
    }))
    .unwrap_or_default()
}

/// A package slug for a name: lowercase letters, digits, and `-`, at most
/// 48 characters, never empty.
#[must_use]
pub fn made_slug(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
        if slug.len() >= 48 {
            break;
        }
    }
    let slug = slug.trim_end_matches('-').to_string();
    if slug.is_empty() {
        "chat-tool".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
    }

    /// The catalog's guests, in the order the operator lists them
    /// (`deploy/eval-runner/catalog`): the three evidence guests, then the
    /// example plugins' tools.
    const GUESTS: [&str; 6] = [
        "repo-map",
        "code-search",
        "test-report",
        "explain-error",
        "release-notes",
        "dependency-check",
    ];

    fn starters() -> Catalog {
        let roots: Vec<PathBuf> = GUESTS
            .iter()
            .map(|guest| root().join(format!("plugin-{guest}")))
            .collect();
        Catalog::load(&roots).expect("the catalog extensions resolve")
    }

    /// The deployed catalog list names exactly these extensions, in order.
    #[test]
    fn the_deployed_catalog_lists_every_catalog_extension() {
        let listed = std::fs::read_to_string(root().join("../deploy/eval-runner/catalog")).unwrap();
        let listed: Vec<&str> = listed
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect();
        let expected: Vec<String> = GUESTS
            .iter()
            .map(|guest| format!("crates/plugin-{guest}"))
            .collect();
        assert_eq!(listed, expected);
    }

    /// Each starter extension resolves, pins the guest the evidence
    /// program runs byte for byte, and answers to the chat catalog's names
    /// for its guest.
    #[test]
    fn the_starter_extensions_resolve_to_their_guests() {
        let catalog = starters();
        let chat = ext_eval::author::catalog::Catalog::starter();
        let names: Vec<&str> = catalog.tools.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Project map",
                "Code finder",
                "Test reader",
                "Explain this error",
                "Release notes",
                "Dependency check"
            ]
        );
        for (tool, guest) in catalog.tools.iter().zip(GUESTS) {
            let wasm = std::fs::read(root().join(format!("plugin/fixtures/{guest}.wasm"))).unwrap();
            assert_eq!(tool.aliases.len(), 1, "{}", tool.name);
            assert_eq!(tool.aliases[0].artifact.digest, digest_bytes(&wasm));
            let chat_tool = chat.by_name(&tool.name).expect("the chat catalog names it");
            let ext_eval::author::catalog::Source::Existing(definition) = &chat_tool.source else {
                panic!("a catalog tool")
            };
            assert!(tool.names(definition), "{} answers to its guest", tool.name);
            assert!(tool.names(&tool.definition));
            assert_eq!(tool.subject.programs.len(), 1);
            assert!(tool.unchanged());
            assert!(
                tool.definition
                    .id
                    .starts_with(&format!("{}:", nostr::eval_ext::hosted::RUNNER))
            );
        }
    }

    /// A chat-made tool is admitted with the programs of the catalog tools
    /// it turns on and its skill; one that turns on anything else, or a
    /// catalog definition that isn't ours, is refused.
    #[test]
    fn a_chat_made_tool_is_built_from_catalog_parts_only() {
        let catalog = starters();
        let requester = "11".repeat(32);
        let map = &catalog.tools[0];
        let tool = DraftTool {
            name: "Brief the layout".into(),
            summary: "Maps first, then says what it found.".into(),
            catalog: None,
            skill: Some("Say which folders matter.".into()),
            uses: vec![map.aliases[0].id.clone()],
        };
        let subject = catalog.draft_subject(&tool, &requester).unwrap();
        assert_eq!(subject.programs.len(), 1);
        assert_eq!(subject.skills[0].name, "brief-the-layout");
        assert!(
            subject.definition["id"]
                .as_str()
                .unwrap()
                .starts_with(&requester)
        );
        let foreign = DraftTool {
            uses: vec![format!("{}:someone/else", "22".repeat(32))],
            ..tool.clone()
        };
        assert_eq!(
            catalog
                .draft_subject(&foreign, &requester)
                .unwrap_err()
                .code,
            "not_admitted"
        );
        let mut outside = map.definition.clone();
        outside.artifact.digest = format!("sha256:{}", "33".repeat(32));
        let named = DraftTool {
            catalog: Some(outside),
            skill: None,
            uses: vec![],
            ..tool
        };
        assert_eq!(
            catalog.draft_subject(&named, &requester).unwrap_err().code,
            "not_admitted"
        );
    }

    /// The starter test sets load as `openagents ext eval run` loads them,
    /// fit the hosted runner, ask for read and write only, have both kinds
    /// of test, and grade should-fire tests on what the run found rather
    /// than the reply.
    #[test]
    fn the_starter_test_sets_load_and_fit_the_hosted_runner() {
        use ext_eval::case::{Grant, Kind};
        for tool in &starters().tools {
            let suite = ext_eval::Suite::load(
                &tool.root.join("evals"),
                ext_eval::case::LoadOptions::default(),
            )
            .unwrap_or_else(|error| panic!("{}: {error}", tool.name));
            assert!(suite.cases.len() as u64 <= nostr::eval_ext::HOSTED_MAX_CASES);
            assert!(
                suite
                    .cases
                    .iter()
                    .any(|case| case.kind == Kind::ShouldNotFire)
            );
            for case in &suite.cases {
                assert_eq!(
                    case.run.allowed_operations,
                    std::collections::BTreeSet::from([Grant::Read, Grant::Write]),
                    "{}: {}",
                    tool.name,
                    case.name
                );
                assert!(
                    case.warnings().is_empty(),
                    "{}: {:?}",
                    case.name,
                    case.warnings()
                );
                if case.kind == Kind::ShouldFire {
                    assert!(
                        case.graders.iter().any(|grader| matches!(
                            grader.check,
                            ext_eval::Check::Regex {
                                target: ext_eval::grader::Focus::Trajectory,
                                ..
                            }
                        )),
                        "{}: {} grades what the run found",
                        tool.name,
                        case.name
                    );
                }
            }
        }
    }

    #[test]
    fn a_slug_is_plain() {
        assert_eq!(made_slug("Brief the Layout!"), "brief-the-layout");
        assert_eq!(made_slug("   "), "chat-tool");
        assert!(made_slug(&"a".repeat(90)).len() <= 48);
    }
}
