use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    CompiledModuleManifest, GroundedAnswerSignature, IntentRouteSignature, Signature,
    ToolArgumentsSignature, ToolPolicySignature, TypedModule, VerifySignature,
};

/// Authority posture for a selected graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphAuthority {
    /// Use promoted modules as the source of truth.
    Promoted,
    /// Use candidate modules as the source of truth.
    Candidate,
}

/// Candidate evaluation posture for a compiled-agent run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowMode {
    /// Run only promoted modules.
    Disabled,
    /// Evaluate a candidate graph in shadow while keeping promoted output authoritative.
    EvaluateCandidate { label: String },
    /// Route authority to a candidate graph while still retaining traces.
    CandidateAuthority { label: String },
}

/// Stored compiled module artifact.
#[derive(Clone)]
pub struct ModuleVariant<S: Signature> {
    /// Manifest describing the compiled artifact.
    pub manifest: CompiledModuleManifest,
    /// Executable compiled module.
    pub module: Arc<dyn TypedModule<S>>,
}

impl<S: Signature> ModuleVariant<S> {
    /// Build a variant from an executable compiled module.
    #[must_use]
    pub fn new(module: Arc<dyn TypedModule<S>>) -> Self {
        Self {
            manifest: module.manifest().clone(),
            module,
        }
    }
}

/// Hub for one module slot across promoted and candidate artifacts.
#[derive(Clone)]
pub struct ModuleFamilyHub<S: Signature> {
    promoted: ModuleVariant<S>,
    candidates: BTreeMap<String, ModuleVariant<S>>,
}

impl<S: Signature> ModuleFamilyHub<S> {
    /// Create a new hub around a promoted artifact.
    #[must_use]
    pub fn new(promoted: Arc<dyn TypedModule<S>>) -> Self {
        Self {
            promoted: ModuleVariant::new(promoted),
            candidates: BTreeMap::new(),
        }
    }

    /// Insert a named candidate artifact.
    pub fn insert_candidate(&mut self, label: impl Into<String>, candidate: Arc<dyn TypedModule<S>>) {
        self.candidates
            .insert(label.into(), ModuleVariant::new(candidate));
    }

    /// Return the promoted artifact.
    #[must_use]
    pub fn promoted(&self) -> &ModuleVariant<S> {
        &self.promoted
    }

    /// Return a candidate artifact by label.
    #[must_use]
    pub fn candidate(&self, label: &str) -> Option<&ModuleVariant<S>> {
        self.candidates.get(label)
    }

    fn select(&self, authority: GraphAuthority, label: Option<&str>) -> &ModuleVariant<S> {
        match authority {
            GraphAuthority::Promoted => &self.promoted,
            GraphAuthority::Candidate => label
                .and_then(|candidate_label| self.candidate(candidate_label))
                .unwrap_or(&self.promoted),
        }
    }
}

/// Selected executable compiled-agent graph.
#[derive(Clone)]
pub struct SelectedGraph {
    /// Which graph provides user-visible authority.
    pub authority: GraphAuthority,
    /// Candidate label when candidate modules participate.
    pub candidate_label: Option<String>,
    /// Route module.
    pub intent_route: ModuleVariant<IntentRouteSignature>,
    /// Tool policy module.
    pub tool_policy: ModuleVariant<ToolPolicySignature>,
    /// Tool arguments module.
    pub tool_arguments: ModuleVariant<ToolArgumentsSignature>,
    /// Grounded answer module.
    pub grounded_answer: ModuleVariant<GroundedAnswerSignature>,
    /// Verify module.
    pub verify: ModuleVariant<VerifySignature>,
}

/// Bundle of compiled module hubs for the first graph.
#[derive(Clone)]
pub struct FirstGraphModuleHub {
    /// Route module family.
    pub intent_route: ModuleFamilyHub<IntentRouteSignature>,
    /// Tool policy module family.
    pub tool_policy: ModuleFamilyHub<ToolPolicySignature>,
    /// Tool arguments module family.
    pub tool_arguments: ModuleFamilyHub<ToolArgumentsSignature>,
    /// Grounded answer module family.
    pub grounded_answer: ModuleFamilyHub<GroundedAnswerSignature>,
    /// Verify module family.
    pub verify: ModuleFamilyHub<VerifySignature>,
}

impl FirstGraphModuleHub {
    /// Select the primary graph for a given shadow posture.
    #[must_use]
    pub fn primary_graph(&self, mode: &ShadowMode) -> SelectedGraph {
        match mode {
            ShadowMode::CandidateAuthority { label } => {
                self.select_graph(GraphAuthority::Candidate, Some(label.as_str()))
            }
            ShadowMode::Disabled | ShadowMode::EvaluateCandidate { .. } => {
                self.select_graph(GraphAuthority::Promoted, None)
            }
        }
    }

    /// Select the shadow graph for a given posture, if any.
    #[must_use]
    pub fn shadow_graph(&self, mode: &ShadowMode) -> Option<SelectedGraph> {
        match mode {
            ShadowMode::EvaluateCandidate { label }
            | ShadowMode::CandidateAuthority { label } => {
                Some(self.select_graph(GraphAuthority::Candidate, Some(label.as_str())))
            }
            ShadowMode::Disabled => None,
        }
    }

    fn select_graph(&self, authority: GraphAuthority, label: Option<&str>) -> SelectedGraph {
        SelectedGraph {
            authority,
            candidate_label: label.map(ToOwned::to_owned),
            intent_route: self.intent_route.select(authority, label).clone(),
            tool_policy: self.tool_policy.select(authority, label).clone(),
            tool_arguments: self.tool_arguments.select(authority, label).clone(),
            grounded_answer: self.grounded_answer.select(authority, label).clone(),
            verify: self.verify.select(authority, label).clone(),
        }
    }
}

