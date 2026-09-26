//! Explicit frozen context inputs; provenance is declared, not remote attestation.
use super::*;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeInput {
    pub id: String,
    pub version: u32,
    pub digest: String,
    pub path: PathBuf,
    pub sources: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenKnowledge {
    pub input: KnowledgeInput,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckLineage {
    pub check: String,
    pub sources: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Lineage {
    pub task_sources: Vec<String>,
    pub source_exclusions: Vec<String>,
    pub checks: Vec<CheckLineage>,
}

fn sources(values: &[String], required: bool) -> Result<BTreeSet<String>, Error> {
    if (required && values.is_empty()) || values.len() > 256 {
        return Err(Error::InvalidCommand(
            "source identities are missing or exceed their bound",
        ));
    }
    let mut original = BTreeSet::new();
    let mut normalized = BTreeSet::new();
    for value in values {
        if !text(value, 1024, false) || !original.insert(value) {
            return Err(Error::InvalidCommand(
                "source identities must be unique nonempty text",
            ));
        }
        normalized.insert(value.clone());
        normalized.insert(knowledge::evidence::task_of(value));
    }
    Ok(normalized)
}

pub(super) fn validate(
    requirements: &Requirements,
    plan: &verification::Plan,
) -> Result<(), Error> {
    let mut excluded = sources(&requirements.task_sources, true)?;
    excluded.extend(sources(&requirements.source_exclusions, false)?);
    if requirements.knowledge.len() > 16 || requirements.check_lineage.len() != plan.checks.len() {
        return Err(Error::InvalidCommand(
            "every independent check needs source lineage",
        ));
    }
    let mut checks = BTreeSet::new();
    for lineage in &requirements.check_lineage {
        if !checks.insert(&lineage.check)
            || !plan.checks.iter().any(|check| check.id == lineage.check)
            || !sources(&lineage.sources, true)?.is_disjoint(&excluded)
        {
            return Err(Error::InvalidCommand(
                "check lineage is missing, repeated, or source-excluded",
            ));
        }
    }
    let mut ids = BTreeSet::new();
    for input in &requirements.knowledge {
        if !identifier(&input.id, true)
            || !ids.insert(&input.id)
            || input.version == 0
            || !is_digest(&input.digest)
            || input.path.as_os_str().is_empty()
            || input
                .path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
            || !sources(&input.sources, true)?.is_disjoint(&excluded)
        {
            return Err(Error::InvalidCommand(
                "knowledge identity or source provenance is invalid",
            ));
        }
    }
    Ok(())
}

impl FrozenKnowledge {
    fn valid(&self) -> bool {
        let Ok(entry) = knowledge::Entry::parse(&self.text) else {
            return false;
        };
        entry.id == self.input.id
            && entry.version == self.input.version
            && entry.digest == self.input.digest
            && entry.status != knowledge::Status::Withdrawn
            && entry.written_from.iter().collect::<BTreeSet<_>>()
                == self.input.sources.iter().collect::<BTreeSet<_>>()
            && self.text.len() <= MAX_COMMAND_BYTES
    }
}

impl Lineage {
    pub(super) fn is_empty(&self) -> bool {
        self.task_sources.is_empty() && self.source_exclusions.is_empty() && self.checks.is_empty()
    }
    pub(super) fn from_requirements(requirements: Option<&Requirements>) -> Self {
        requirements.map_or_else(Self::default, |r| Self {
            task_sources: r.task_sources.clone(),
            source_exclusions: r.source_exclusions.clone(),
            checks: r.check_lineage.clone(),
        })
    }

    fn excludes_current(&self, knowledge: &[FrozenKnowledge], task_id: &str) -> bool {
        let current = sources(&[task_id.to_string()], true).unwrap_or_default();
        self.checks
            .iter()
            .flat_map(|check| &check.sources)
            .chain(knowledge.iter().flat_map(|entry| &entry.input.sources))
            .all(|source| {
                !current.contains(source)
                    && !current.contains(&knowledge::evidence::task_of(source))
            })
    }
}

pub(super) fn capture(
    requirements: Option<&Requirements>,
    workspace: &Path,
    task_id: &str,
) -> Result<(Lineage, Vec<FrozenKnowledge>), Error> {
    let lineage = Lineage::from_requirements(requirements);
    let mut entries = Vec::new();
    let mut total = 0;
    for input in requirements.into_iter().flat_map(|r| &r.knowledge) {
        let mut bytes = Vec::new();
        artifact::confined_file(workspace, &input.path)?
            .take(MAX_COMMAND_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        total += bytes.len();
        if total > 256 * 1024 {
            return Err(Error::LimitExceeded);
        }
        let entry = FrozenKnowledge {
            input: input.clone(),
            text: String::from_utf8(bytes)
                .map_err(|_| Error::InvalidCommand("knowledge must be UTF-8"))?,
        };
        if !entry.valid() {
            return Err(Error::InvalidCommand(
                "knowledge document differs from its exact grant",
            ));
        }
        entries.push(entry);
    }
    if !lineage.excludes_current(&entries, task_id) {
        return Err(Error::InvalidCommand(
            "knowledge or checks derive from the current task",
        ));
    }
    Ok((lineage, entries))
}

pub(super) fn matches(
    context: &Context,
    requirements: Option<&Requirements>,
    task_id: Option<&str>,
) -> bool {
    context.lineage == Lineage::from_requirements(requirements)
        && context
            .knowledge
            .iter()
            .map(|entry| &entry.input)
            .eq(requirements.into_iter().flat_map(|r| &r.knowledge))
        && context.knowledge.iter().all(FrozenKnowledge::valid)
        && context
            .knowledge
            .iter()
            .map(|entry| entry.text.len())
            .sum::<usize>()
            <= 256 * 1024
        && task_id.is_none_or(|id| context.lineage.excludes_current(&context.knowledge, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> String {
        "---\nid: fixture.reference\nversion: 1\nkind: method\ntitle: Reference\nsummary: A synthetic reference.\ntags: [fixture]\napplies_when: A fixture needs context.\nstatus: candidate\nauthor: Fixture\nprovenance:\n  written_from: [reference]\n  cites: [Fixture specification]\nevidence: []\n---\n\nExact frozen reference bytes.\n".into()
    }

    fn add_input(requirements: &mut Requirements, workspace: &Path) -> String {
        let text = document();
        std::fs::write(workspace.join("reference.md"), &text).unwrap();
        requirements.knowledge = vec![KnowledgeInput {
            id: "fixture.reference".into(),
            version: 1,
            digest: knowledge::digest(text.as_bytes()),
            path: "reference.md".into(),
            sources: vec!["reference".into()],
        }];
        text
    }

    #[test]
    fn exact_context_survives_replay_and_changed_bytes_refuse_new_admission() {
        let (_host, workspace, mut requirements, mut context, _) = super::super::tests::fixture();
        let text = add_input(&mut requirements, workspace.path());
        requirements.validate().unwrap();
        let (lineage, entries) =
            capture(Some(&requirements), workspace.path(), "fixture-task").unwrap();
        assert_eq!(entries[0].text, text);
        context.lineage = lineage;
        context.knowledge = entries;
        context.digest = context.expected_digest();
        assert!(matches(&context, Some(&requirements), Some("fixture-task")));
        std::fs::write(workspace.path().join("reference.md"), "changed").unwrap();
        assert!(capture(Some(&requirements), workspace.path(), "fixture-task").is_err());
        assert!(matches(&context, Some(&requirements), Some("fixture-task")));
        context.knowledge[0].text.push_str("altered");
        assert!(!matches(
            &context,
            Some(&requirements),
            Some("fixture-task")
        ));
    }

    #[test]
    fn unknown_mismatched_and_excluded_provenance_refuses() {
        let (_host, workspace, mut requirements, _, _) = super::super::tests::fixture();
        add_input(&mut requirements, workspace.path());
        requirements.check_lineage.clear();
        assert!(requirements.validate().is_err());
        requirements.check_lineage = vec![CheckLineage {
            check: "content".into(),
            sources: vec!["independent".into()],
        }];
        requirements.source_exclusions = vec!["reference-12345".into()];
        assert!(requirements.validate().is_err());
        requirements.source_exclusions.clear();
        requirements.task_sources = vec!["independent-9876".into()];
        assert!(requirements.validate().is_err());
        requirements.task_sources = vec!["target".into()];
        assert!(capture(Some(&requirements), workspace.path(), "reference-123").is_err());
        requirements.knowledge[0].sources = vec!["different-declaration".into()];
        assert!(capture(Some(&requirements), workspace.path(), "target").is_err());
    }

    #[test]
    fn private_paths_symlinks_and_changed_versions_refuse() {
        use std::os::unix::fs::symlink;
        let (_host, workspace, mut requirements, _, _) = super::super::tests::fixture();
        add_input(&mut requirements, workspace.path());
        requirements.knowledge[0].version = 2;
        assert!(capture(Some(&requirements), workspace.path(), "target").is_err());
        requirements.knowledge[0].version = 1;
        requirements.knowledge[0].path = "../outside.md".into();
        assert!(requirements.validate().is_err());
        symlink(
            workspace.path().join("reference.md"),
            workspace.path().join("link.md"),
        )
        .unwrap();
        requirements.knowledge[0].path = "link.md".into();
        assert!(capture(Some(&requirements), workspace.path(), "target").is_err());
    }
}
