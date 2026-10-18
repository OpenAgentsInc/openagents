//! Shared request shapes for the Labs service and its clients.
use super::*;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveDocument {
    pub request_id: String,
    pub expected_revision: u64,
    pub content: Content,
    pub summary: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartRun {
    pub request_id: String,
    pub version: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunChange {
    pub request_id: String,
    pub expected_revision: u64,
    pub action: RunAction,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum RunAction {
    Stop,
    Archive,
    Review {
        trial: usize,
        passed: bool,
        note: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Promote {
    pub request_id: String,
    pub version: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostUpdate {
    pub request_id: String,
    pub host: String,
    pub action: HostAction,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum HostAction {
    Advance,
    Settled,
    Ready { trial: usize, evidence: Evidence },
    Unavailable { trial: usize, reason: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Page<T> {
    pub data: Vec<T>,
    pub next: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolOffer {
    pub id: String,
    pub name: String,
    pub project: String,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Host {
    pub id: String,
    pub name: String,
    pub projects: Vec<Project>,
    pub runtimes: Vec<String>,
    pub runners: Vec<String>,
    pub tools: Vec<ToolOffer>,
    pub seen_ms: u64,
    /// A headless engine on a computer that stays up without anyone at its
    /// screen (#12050), so an agent that wakes on a schedule can run there.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub always_on: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteDocument {
    pub request_id: String,
    pub expected_revision: u64,
}

/// The published ID of a first publish sent with `request_id`. Keep that
/// request ID private: this ID can be shared before publishing, and nobody
/// without the request ID can publish under it.
pub fn published_id(request_id: &str) -> String {
    lab_kit::digest(&json!({ "published_agent": request_id }))
}
/// An agent definition published for anyone to start from (#12048): the
/// Agent Lab template it was made from, with its skills and tests, and the
/// publisher's saved definition it matches. Revisions start at 1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishedAgent {
    pub id: String,
    pub revision: u64,
    pub definition: AgentDefinition,
    pub template: Experiment,
}
/// The request body that publishes one of your saved agents under a new
/// published ID ([`published_id`]).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishAgent {
    pub request_id: String,
    pub definition: Pin,
    pub template: Experiment,
}
/// The request body that adds a revision to an agent you published.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdatePublishedAgent {
    pub request_id: String,
    pub expected_revision: u64,
    pub definition: Pin,
    pub template: Experiment,
}
/// Checks that `template` may be published as `definition`: it names no
/// computer, project or saved agent, and the agent it saves is exactly the
/// definition, so a copy starts from what runs.
pub fn check_published(template: &Experiment, definition: &AgentDefinition) -> Result<(), Refusal> {
    template.validate()?;
    if template.host != "choose"
        || template.project != "choose"
        || template.draft.source.is_some()
        || template.compare.is_some()
    {
        return Err(invalid(
            "Clear the computer, project and saved agent before publishing.",
        ));
    }
    if definition.deleted || template.draft.agent_spec()? != definition.spec {
        return Err(invalid("Publish the agent this experiment saved."));
    }
    Ok(())
}

/// The request body that forks a published agent into your own agents
/// (#12049). The fork's ID is the request ID.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForkAgent {
    pub request_id: String,
    pub name: String,
}
/// A new fork: your own agent, where it came from, and the Agent Lab
/// experiment to test and change it in. The experiment saves the fork.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Forked {
    pub definition: AgentDefinition,
    pub forked_from: agent_contract::ForkedFrom,
    pub template: Experiment,
}
/// The request body that publishes one of your agents, or a new revision of
/// it once it is published. Without a template, a fork publishes with the
/// tests it was forked with.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublishDefinition {
    pub request_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<Experiment>,
}
/// A fork's original: the revision it copied and, when the original has
/// published a newer revision since, that revision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Upstream {
    pub forked_from: agent_contract::ForkedFrom,
    pub newer: Option<PublishedAgent>,
}

/// The fork of `published` named `name`: its spec, and the experiment that
/// saves it, pinned to the fork's own definition at revision 1.
pub fn fork(published: &PublishedAgent, id: &str, name: &str) -> (AgentSpec, Experiment) {
    let mut spec = published.definition.spec.clone();
    spec.name = name.into();
    let mut template = published.template.clone();
    template.title = name.chars().take(100).collect();
    template.draft.spec.name = name.into();
    template.draft.source = Some(Pin {
        id: id.into(),
        revision: 1,
    });
    (spec, template)
}
/// The experiment that publishes `definition` when none is sent: the
/// fork's experiment with the definition's current settings, so it saves
/// exactly that definition.
pub fn publish_template(forked: &Experiment, definition: &AgentDefinition) -> Experiment {
    let mut t = forked.clone();
    t.draft.source = None;
    t.compare = None;
    t.host = "choose".into();
    t.project = "choose".into();
    if t.draft.agent_spec().ok().as_ref() != Some(&definition.spec) {
        t.draft.spec = definition.spec.clone();
        t.draft.components.clear();
    }
    t.title = definition.spec.name.chars().take(100).collect();
    t
}
