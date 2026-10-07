//! Provider-neutral model adapter contracts for legal benchmark agents.
//!
//! The adapter layer normalizes hosted Google Vertex Gemini providers,
//! OpenAI-compatible local serving endpoints, Anthropic parity routes, and
//! deterministic CI mocks into one benchmark model surface. It deliberately
//! records route and secret reference identity without carrying raw credentials
//! into run artifacts.

use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Digest;

use crate::{
    LegalBenchmarkToolName, Metadata, ScoreReport, score_report_digest, stable_json_digest,
};

pub const LEGAL_BENCHMARK_PROVIDER_SCHEMA_VERSION: u16 = 1;
pub const QWEN_LEGAL_CANDIDATE_SCHEMA_VERSION: u16 = 1;
pub const QWEN_LEGAL_PROMOTED_PROVIDER_ROUTE_SCHEMA_VERSION: u16 = 1;
pub const QWEN_LEGAL_BASE_MODEL_ID: &str = "Qwen/Qwen3.5-4B";
pub const QWEN_LEGAL_SERVED_MODEL_ID: &str = "qwen3.5-4b";
pub const QWEN_LEGAL_MODEL_FAMILY_ACCEPTANCE_LABEL: &str = "qwen35";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelProviderFamily {
    GoogleVertexGemini,
    OpenAiCompatible,
    Anthropic,
    PsionicCompatible,
    Mock,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkModelCandidateKind {
    Base,
    TunedAdapter,
    MockLocalSmoke,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkScoreClaimKind {
    NoPublicClaim,
    MockLocalSmokeOnly,
    RetainedScoreClaim,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkCandidateScoreClass {
    BaseModel,
    TunedAdapter,
    MockLocalSmoke,
    RetainedClaim,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalBasePlusAdapterCandidateIdentity {
    pub schema_version: u16,
    pub candidate_id: String,
    pub candidate_kind: LegalBenchmarkModelCandidateKind,
    pub serving_model_id: String,
    pub base_model_id: String,
    pub served_model_id: String,
    pub model_family_acceptance_label: String,
    pub base_served_artifact_digest: String,
    pub tokenizer_digest: String,
    pub tokenizer_contract_digest: String,
    pub prompt_template_digest: String,
    pub dataset_digest: String,
    pub eval_pack_digest: String,
    pub serving_revision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_artifact_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_identity_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_known_good_adapter_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback_adapter_revision: Option<String>,
    pub score_claim_kind: LegalBenchmarkScoreClaimKind,
}

impl QwenLegalBasePlusAdapterCandidateIdentity {
    pub fn base(
        candidate_id: impl Into<String>,
        serving_model_id: impl Into<String>,
        base_served_artifact_digest: impl Into<String>,
        tokenizer_digest: impl Into<String>,
        tokenizer_contract_digest: impl Into<String>,
        prompt_template_digest: impl Into<String>,
        dataset_digest: impl Into<String>,
        eval_pack_digest: impl Into<String>,
        serving_revision: impl Into<String>,
    ) -> Result<Self, ModelAdapterError> {
        let candidate = Self {
            schema_version: QWEN_LEGAL_CANDIDATE_SCHEMA_VERSION,
            candidate_id: candidate_id.into(),
            candidate_kind: LegalBenchmarkModelCandidateKind::Base,
            serving_model_id: serving_model_id.into(),
            base_model_id: String::from(QWEN_LEGAL_BASE_MODEL_ID),
            served_model_id: String::from(QWEN_LEGAL_SERVED_MODEL_ID),
            model_family_acceptance_label: String::from(QWEN_LEGAL_MODEL_FAMILY_ACCEPTANCE_LABEL),
            base_served_artifact_digest: base_served_artifact_digest.into(),
            tokenizer_digest: tokenizer_digest.into(),
            tokenizer_contract_digest: tokenizer_contract_digest.into(),
            prompt_template_digest: prompt_template_digest.into(),
            dataset_digest: dataset_digest.into(),
            eval_pack_digest: eval_pack_digest.into(),
            serving_revision: serving_revision.into(),
            adapter_id: None,
            adapter_revision: None,
            adapter_artifact_digest: None,
            adapter_identity_digest: None,
            last_known_good_adapter_revision: None,
            rollback_adapter_revision: None,
            score_claim_kind: LegalBenchmarkScoreClaimKind::NoPublicClaim,
        };
        candidate.validate()?;
        Ok(candidate)
    }

    pub fn tuned_adapter(
        candidate_id: impl Into<String>,
        serving_model_id: impl Into<String>,
        base_served_artifact_digest: impl Into<String>,
        tokenizer_digest: impl Into<String>,
        tokenizer_contract_digest: impl Into<String>,
        prompt_template_digest: impl Into<String>,
        dataset_digest: impl Into<String>,
        eval_pack_digest: impl Into<String>,
        serving_revision: impl Into<String>,
        adapter_id: impl Into<String>,
        adapter_revision: impl Into<String>,
        adapter_artifact_digest: impl Into<String>,
        adapter_identity_digest: impl Into<String>,
        last_known_good_adapter_revision: Option<String>,
        rollback_adapter_revision: Option<String>,
    ) -> Result<Self, ModelAdapterError> {
        let candidate = Self {
            schema_version: QWEN_LEGAL_CANDIDATE_SCHEMA_VERSION,
            candidate_id: candidate_id.into(),
            candidate_kind: LegalBenchmarkModelCandidateKind::TunedAdapter,
            serving_model_id: serving_model_id.into(),
            base_model_id: String::from(QWEN_LEGAL_BASE_MODEL_ID),
            served_model_id: String::from(QWEN_LEGAL_SERVED_MODEL_ID),
            model_family_acceptance_label: String::from(QWEN_LEGAL_MODEL_FAMILY_ACCEPTANCE_LABEL),
            base_served_artifact_digest: base_served_artifact_digest.into(),
            tokenizer_digest: tokenizer_digest.into(),
            tokenizer_contract_digest: tokenizer_contract_digest.into(),
            prompt_template_digest: prompt_template_digest.into(),
            dataset_digest: dataset_digest.into(),
            eval_pack_digest: eval_pack_digest.into(),
            serving_revision: serving_revision.into(),
            adapter_id: Some(adapter_id.into()),
            adapter_revision: Some(adapter_revision.into()),
            adapter_artifact_digest: Some(adapter_artifact_digest.into()),
            adapter_identity_digest: Some(adapter_identity_digest.into()),
            last_known_good_adapter_revision,
            rollback_adapter_revision,
            score_claim_kind: LegalBenchmarkScoreClaimKind::NoPublicClaim,
        };
        candidate.validate()?;
        Ok(candidate)
    }

    pub fn mock_local_smoke(
        mut candidate: Self,
    ) -> Result<QwenLegalBasePlusAdapterCandidateIdentity, ModelAdapterError> {
        candidate.candidate_kind = LegalBenchmarkModelCandidateKind::MockLocalSmoke;
        candidate.score_claim_kind = LegalBenchmarkScoreClaimKind::MockLocalSmokeOnly;
        candidate.validate()?;
        Ok(candidate)
    }

    pub fn validate(&self) -> ModelAdapterResult<()> {
        require_candidate_field(self.candidate_id.as_str(), "candidate_id")?;
        require_candidate_field(self.serving_model_id.as_str(), "serving_model_id")?;
        require_candidate_field(
            self.base_served_artifact_digest.as_str(),
            "base_served_artifact_digest",
        )?;
        require_candidate_field(self.tokenizer_digest.as_str(), "tokenizer_digest")?;
        require_candidate_field(
            self.tokenizer_contract_digest.as_str(),
            "tokenizer_contract_digest",
        )?;
        require_candidate_field(
            self.prompt_template_digest.as_str(),
            "prompt_template_digest",
        )?;
        require_candidate_field(self.dataset_digest.as_str(), "dataset_digest")?;
        require_candidate_field(self.eval_pack_digest.as_str(), "eval_pack_digest")?;
        require_candidate_field(self.serving_revision.as_str(), "serving_revision")?;
        if self.schema_version != QWEN_LEGAL_CANDIDATE_SCHEMA_VERSION {
            return Err(candidate_error("candidate schema version drifted"));
        }
        if self.base_model_id != QWEN_LEGAL_BASE_MODEL_ID
            || self.served_model_id != QWEN_LEGAL_SERVED_MODEL_ID
            || self.model_family_acceptance_label != QWEN_LEGAL_MODEL_FAMILY_ACCEPTANCE_LABEL
        {
            return Err(candidate_error(
                "candidate does not target the admitted Qwen3.5-4B qwen35 lane",
            ));
        }
        match self.candidate_kind {
            LegalBenchmarkModelCandidateKind::Base => {
                if self.adapter_id.is_some()
                    || self.adapter_revision.is_some()
                    || self.adapter_artifact_digest.is_some()
                    || self.adapter_identity_digest.is_some()
                {
                    return Err(candidate_error(
                        "base candidate cannot carry adapter identity",
                    ));
                }
                if self.score_claim_kind == LegalBenchmarkScoreClaimKind::RetainedScoreClaim {
                    return Err(candidate_error(
                        "base candidate cannot emit a tuned retained score claim",
                    ));
                }
            }
            LegalBenchmarkModelCandidateKind::TunedAdapter => {
                require_candidate_option(self.adapter_id.as_deref(), "adapter_id")?;
                require_candidate_option(self.adapter_revision.as_deref(), "adapter_revision")?;
                require_candidate_option(
                    self.adapter_artifact_digest.as_deref(),
                    "adapter_artifact_digest",
                )?;
                require_candidate_option(
                    self.adapter_identity_digest.as_deref(),
                    "adapter_identity_digest",
                )?;
            }
            LegalBenchmarkModelCandidateKind::MockLocalSmoke => {
                if self.score_claim_kind != LegalBenchmarkScoreClaimKind::MockLocalSmokeOnly {
                    return Err(candidate_error(
                        "mock/local smoke candidates cannot emit public retained score claims",
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn identity_digest(&self) -> ModelAdapterResult<String> {
        stable_json_digest("psionic.legal_benchmark.qwen_candidate_identity.v1", self).map_err(
            |err| {
                ModelAdapterError::new(
                    ModelAdapterFailureKind::InternalError,
                    format!("failed to hash Qwen candidate identity: {err}"),
                )
            },
        )
    }

    pub fn receipt_metadata(&self) -> ModelAdapterResult<Metadata> {
        self.validate()?;
        let mut metadata = Metadata::new();
        metadata.insert(
            String::from("legal_candidate_id"),
            Value::String(self.candidate_id.clone()),
        );
        metadata.insert(
            String::from("legal_candidate_kind"),
            json!(self.candidate_kind),
        );
        metadata.insert(
            String::from("legal_candidate_identity_digest"),
            Value::String(self.identity_digest()?),
        );
        metadata.insert(
            String::from("base_model_id"),
            Value::String(self.base_model_id.clone()),
        );
        metadata.insert(
            String::from("served_model_id"),
            Value::String(self.served_model_id.clone()),
        );
        metadata.insert(
            String::from("base_served_artifact_digest"),
            Value::String(self.base_served_artifact_digest.clone()),
        );
        metadata.insert(
            String::from("tokenizer_digest"),
            Value::String(self.tokenizer_digest.clone()),
        );
        metadata.insert(
            String::from("tokenizer_contract_digest"),
            Value::String(self.tokenizer_contract_digest.clone()),
        );
        metadata.insert(
            String::from("prompt_template_digest"),
            Value::String(self.prompt_template_digest.clone()),
        );
        metadata.insert(
            String::from("dataset_digest"),
            Value::String(self.dataset_digest.clone()),
        );
        metadata.insert(
            String::from("eval_pack_digest"),
            Value::String(self.eval_pack_digest.clone()),
        );
        metadata.insert(
            String::from("serving_revision"),
            Value::String(self.serving_revision.clone()),
        );
        metadata.insert(
            String::from("score_claim_kind"),
            json!(self.score_claim_kind),
        );
        if let Some(value) = &self.adapter_id {
            metadata.insert(String::from("adapter_id"), Value::String(value.clone()));
        }
        if let Some(value) = &self.adapter_revision {
            metadata.insert(
                String::from("adapter_revision"),
                Value::String(value.clone()),
            );
        }
        if let Some(value) = &self.adapter_artifact_digest {
            metadata.insert(
                String::from("adapter_artifact_digest"),
                Value::String(value.clone()),
            );
        }
        if let Some(value) = &self.adapter_identity_digest {
            metadata.insert(
                String::from("adapter_identity_digest"),
                Value::String(value.clone()),
            );
        }
        if let Some(value) = &self.last_known_good_adapter_revision {
            metadata.insert(
                String::from("last_known_good_adapter_revision"),
                Value::String(value.clone()),
            );
        }
        if let Some(value) = &self.rollback_adapter_revision {
            metadata.insert(
                String::from("rollback_adapter_revision"),
                Value::String(value.clone()),
            );
        }
        Ok(metadata)
    }

    pub fn openai_compatible_route(
        &self,
        route_id: impl Into<String>,
        base_url: impl Into<String>,
        secret_reference_id: Option<String>,
    ) -> ModelAdapterResult<ModelProviderRoute> {
        let mut route = ModelProviderRoute::openai_compatible(
            route_id,
            base_url,
            self.serving_model_id.clone(),
            secret_reference_id,
        );
        route.metadata = self.receipt_metadata()?;
        route.redacted_headers.insert(
            String::from("x-psionic-base-artifact-digest"),
            self.base_served_artifact_digest.clone(),
        );
        route.redacted_headers.insert(
            String::from("x-psionic-template-digest"),
            self.prompt_template_digest.clone(),
        );
        route.redacted_headers.insert(
            String::from("x-psionic-dataset-digest"),
            self.dataset_digest.clone(),
        );
        route.redacted_headers.insert(
            String::from("x-psionic-eval-pack-digest"),
            self.eval_pack_digest.clone(),
        );
        if let Some(adapter_digest) = &self.adapter_artifact_digest {
            route.redacted_headers.insert(
                String::from("x-psionic-adapter-artifact-digest"),
                adapter_digest.clone(),
            );
        }
        Ok(route)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkCandidateScoreEntry {
    pub score_class: LegalBenchmarkCandidateScoreClass,
    pub candidate_id: String,
    pub candidate_identity_digest: String,
    pub score_report_id: String,
    pub score_report_hash: String,
    pub criterion_pass_rate_bps: u32,
    pub all_pass: bool,
    pub public_retained_claim: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkQwenAdapterScoreBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub base_model_score: LegalBenchmarkCandidateScoreEntry,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tuned_adapter_score: Option<LegalBenchmarkCandidateScoreEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mock_local_smoke_score: Option<LegalBenchmarkCandidateScoreEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retained_score_claim: Option<LegalBenchmarkCandidateScoreEntry>,
    pub bundle_digest: String,
}

impl LegalBenchmarkQwenAdapterScoreBundle {
    pub fn stable_digest(&self) -> ModelAdapterResult<String> {
        let mut clone = self.clone();
        clone.bundle_digest.clear();
        stable_json_digest(
            "psionic.legal_benchmark.qwen_adapter_score_bundle.v1",
            &clone,
        )
        .map_err(|err| {
            ModelAdapterError::new(
                ModelAdapterFailureKind::InternalError,
                format!("failed to hash Qwen adapter score bundle: {err}"),
            )
        })
    }
}

pub fn qwen_legal_base_vs_adapter_score_bundle(
    bundle_id: impl Into<String>,
    base_candidate: &QwenLegalBasePlusAdapterCandidateIdentity,
    base_score: &ScoreReport,
    tuned_candidate: Option<&QwenLegalBasePlusAdapterCandidateIdentity>,
    tuned_score: Option<&ScoreReport>,
) -> ModelAdapterResult<LegalBenchmarkQwenAdapterScoreBundle> {
    base_candidate.validate()?;
    if base_candidate.candidate_kind != LegalBenchmarkModelCandidateKind::Base {
        return Err(candidate_error(
            "base score entry requires a base candidate",
        ));
    }
    let base_entry = candidate_score_entry(
        LegalBenchmarkCandidateScoreClass::BaseModel,
        base_candidate,
        base_score,
        false,
    )?;
    let tuned_entry = match (tuned_candidate, tuned_score) {
        (Some(candidate), Some(score)) => {
            ensure_qwen_candidate_pair_compatible(base_candidate, candidate)?;
            let score_class = match candidate.candidate_kind {
                LegalBenchmarkModelCandidateKind::MockLocalSmoke => {
                    LegalBenchmarkCandidateScoreClass::MockLocalSmoke
                }
                LegalBenchmarkModelCandidateKind::TunedAdapter => {
                    LegalBenchmarkCandidateScoreClass::TunedAdapter
                }
                LegalBenchmarkModelCandidateKind::Base => {
                    return Err(candidate_error(
                        "tuned score entry cannot be a base candidate",
                    ));
                }
            };
            Some(candidate_score_entry(score_class, candidate, score, false)?)
        }
        (None, None) => None,
        _ => {
            return Err(candidate_error(
                "tuned candidate and tuned score must be provided together",
            ));
        }
    };
    let retained_score_claim = match (&tuned_entry, tuned_candidate) {
        (Some(_), Some(candidate))
            if candidate.score_claim_kind == LegalBenchmarkScoreClaimKind::RetainedScoreClaim =>
        {
            Some(candidate_score_entry(
                LegalBenchmarkCandidateScoreClass::RetainedClaim,
                candidate,
                tuned_score.expect("checked above"),
                true,
            )?)
        }
        _ => None,
    };
    if let Some(candidate) = tuned_candidate
        && candidate.candidate_kind == LegalBenchmarkModelCandidateKind::MockLocalSmoke
        && retained_score_claim.is_some()
    {
        return Err(candidate_error(
            "mock/local smoke scores cannot be emitted as retained score claims",
        ));
    }
    let mut bundle = LegalBenchmarkQwenAdapterScoreBundle {
        schema_version: QWEN_LEGAL_CANDIDATE_SCHEMA_VERSION,
        bundle_id: bundle_id.into(),
        base_model_score: base_entry,
        tuned_adapter_score: tuned_entry
            .clone()
            .filter(|entry| entry.score_class == LegalBenchmarkCandidateScoreClass::TunedAdapter),
        mock_local_smoke_score: tuned_entry
            .filter(|entry| entry.score_class == LegalBenchmarkCandidateScoreClass::MockLocalSmoke),
        retained_score_claim,
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = bundle.stable_digest()?;
    Ok(bundle)
}

pub fn ensure_qwen_candidate_pair_compatible(
    base: &QwenLegalBasePlusAdapterCandidateIdentity,
    candidate: &QwenLegalBasePlusAdapterCandidateIdentity,
) -> ModelAdapterResult<()> {
    base.validate()?;
    candidate.validate()?;
    let checks = [
        (
            "base_model_id",
            base.base_model_id.as_str(),
            candidate.base_model_id.as_str(),
        ),
        (
            "served_model_id",
            base.served_model_id.as_str(),
            candidate.served_model_id.as_str(),
        ),
        (
            "base_served_artifact_digest",
            base.base_served_artifact_digest.as_str(),
            candidate.base_served_artifact_digest.as_str(),
        ),
        (
            "tokenizer_digest",
            base.tokenizer_digest.as_str(),
            candidate.tokenizer_digest.as_str(),
        ),
        (
            "tokenizer_contract_digest",
            base.tokenizer_contract_digest.as_str(),
            candidate.tokenizer_contract_digest.as_str(),
        ),
        (
            "prompt_template_digest",
            base.prompt_template_digest.as_str(),
            candidate.prompt_template_digest.as_str(),
        ),
        (
            "dataset_digest",
            base.dataset_digest.as_str(),
            candidate.dataset_digest.as_str(),
        ),
        (
            "eval_pack_digest",
            base.eval_pack_digest.as_str(),
            candidate.eval_pack_digest.as_str(),
        ),
    ];
    for (field, expected, actual) in checks {
        if expected != actual {
            return Err(candidate_error(format!(
                "candidate {field} drifted from the base route"
            )));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModelProviderRoute {
    pub schema_version: u16,
    pub route_id: String,
    pub family: ModelProviderFamily,
    pub base_url: String,
    pub model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_reference_id: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub redacted_headers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

impl ModelProviderRoute {
    pub fn google_vertex_gemini(
        route_id: impl Into<String>,
        project_id: impl Into<String>,
        location: impl Into<String>,
        model_id: impl Into<String>,
        secret_reference_id: Option<String>,
    ) -> Self {
        let project_id = project_id.into();
        let location = location.into();
        let model_id = model_id.into();
        let api_endpoint = if location == "global" {
            String::from("https://aiplatform.googleapis.com")
        } else {
            format!("https://{location}-aiplatform.googleapis.com")
        };
        let base_url = format!(
            "{api_endpoint}/v1/projects/{project_id}/locations/{location}/publishers/google/models/{model_id}:generateContent"
        );
        let secret_reference_id = secret_reference_id.filter(|value| !value.trim().is_empty());
        let mut redacted_headers = BTreeMap::new();
        redacted_headers.insert(
            String::from("content-type"),
            String::from("application/json"),
        );
        redacted_headers.insert(String::from("x-goog-user-project"), project_id.clone());
        if let Some(secret_ref) = &secret_reference_id {
            redacted_headers.insert(
                String::from("authorization"),
                format!("Bearer <secret_ref:{secret_ref}>"),
            );
        }
        let mut metadata = Metadata::new();
        metadata.insert(String::from("project_id"), Value::String(project_id));
        metadata.insert(String::from("location"), Value::String(location));
        metadata.insert(
            String::from("endpoint_kind"),
            Value::String(String::from("generateContent")),
        );
        Self {
            schema_version: LEGAL_BENCHMARK_PROVIDER_SCHEMA_VERSION,
            route_id: route_id.into(),
            family: ModelProviderFamily::GoogleVertexGemini,
            base_url: trim_url(base_url),
            model_id,
            endpoint_path: None,
            secret_reference_id,
            redacted_headers,
            metadata,
        }
    }

    pub fn openai_compatible(
        route_id: impl Into<String>,
        base_url: impl Into<String>,
        model_id: impl Into<String>,
        secret_reference_id: Option<String>,
    ) -> Self {
        let secret_reference_id = secret_reference_id.filter(|value| !value.trim().is_empty());
        let mut redacted_headers = BTreeMap::new();
        if let Some(secret_ref) = &secret_reference_id {
            redacted_headers.insert(
                String::from("authorization"),
                format!("Bearer <secret_ref:{secret_ref}>"),
            );
        }
        Self {
            schema_version: LEGAL_BENCHMARK_PROVIDER_SCHEMA_VERSION,
            route_id: route_id.into(),
            family: ModelProviderFamily::OpenAiCompatible,
            base_url: trim_url(base_url.into()),
            model_id: model_id.into(),
            endpoint_path: Some(String::from("/chat/completions")),
            secret_reference_id,
            redacted_headers,
            metadata: Metadata::new(),
        }
    }

    pub fn anthropic(
        route_id: impl Into<String>,
        base_url: impl Into<String>,
        model_id: impl Into<String>,
        secret_reference_id: Option<String>,
    ) -> Self {
        let secret_reference_id = secret_reference_id.filter(|value| !value.trim().is_empty());
        let mut redacted_headers = BTreeMap::new();
        redacted_headers.insert(
            String::from("anthropic-version"),
            String::from("2023-06-01"),
        );
        if let Some(secret_ref) = &secret_reference_id {
            redacted_headers.insert(
                String::from("x-api-key"),
                format!("<secret_ref:{secret_ref}>"),
            );
        }
        Self {
            schema_version: LEGAL_BENCHMARK_PROVIDER_SCHEMA_VERSION,
            route_id: route_id.into(),
            family: ModelProviderFamily::Anthropic,
            base_url: trim_url(base_url.into()),
            model_id: model_id.into(),
            endpoint_path: Some(String::from("/messages")),
            secret_reference_id,
            redacted_headers,
            metadata: Metadata::new(),
        }
    }

    pub fn psionic_compatible(
        route_id: impl Into<String>,
        base_url: impl Into<String>,
        model_id: impl Into<String>,
        secret_reference_id: Option<String>,
    ) -> Self {
        let secret_reference_id = secret_reference_id.filter(|value| !value.trim().is_empty());
        let mut redacted_headers = BTreeMap::new();
        redacted_headers.insert(
            String::from("content-type"),
            String::from("application/json"),
        );
        if let Some(secret_ref) = &secret_reference_id {
            redacted_headers.insert(
                String::from("authorization"),
                format!("Bearer <secret_ref:{secret_ref}>"),
            );
        }
        Self {
            schema_version: LEGAL_BENCHMARK_PROVIDER_SCHEMA_VERSION,
            route_id: route_id.into(),
            family: ModelProviderFamily::PsionicCompatible,
            base_url: trim_url(base_url.into()),
            model_id: model_id.into(),
            endpoint_path: Some(String::from("/v1/chat/completions")),
            secret_reference_id,
            redacted_headers,
            metadata: Metadata::new(),
        }
    }

    pub fn mock(route_id: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self {
            schema_version: LEGAL_BENCHMARK_PROVIDER_SCHEMA_VERSION,
            route_id: route_id.into(),
            family: ModelProviderFamily::Mock,
            base_url: String::from("mock://legal-benchmark"),
            model_id: model_id.into(),
            endpoint_path: None,
            secret_reference_id: None,
            redacted_headers: BTreeMap::new(),
            metadata: Metadata::new(),
        }
    }

    pub fn endpoint_url(&self) -> String {
        match &self.endpoint_path {
            Some(path) if path.starts_with('/') => format!("{}{}", self.base_url, path),
            Some(path) => format!("{}/{}", self.base_url, path),
            None => self.base_url.clone(),
        }
    }

    pub fn config_hash(&self) -> Result<String, ModelAdapterError> {
        stable_json_digest("psionic.legal_benchmark.provider_route.v1", self).map_err(|err| {
            ModelAdapterError::new(
                ModelAdapterFailureKind::InternalError,
                format!("failed to hash provider route: {err}"),
            )
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalPromotedRouteRefusalKind {
    MissingAdapterArtifact,
    MissingCheckpointArtifact,
    BaseHashMismatch,
    BackendUnavailable,
    MissingPromotionMetadata,
    FallbackNotExplicit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QwenLegalPromotedRouteTrafficMode {
    Primary,
    Canary,
    Rollback,
    ExplicitFallbackOnly,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalPromotedRouteArtifactMetadata {
    pub base_model_id: String,
    pub served_model_id: String,
    pub base_model_hash: String,
    pub loaded_base_model_hash: String,
    pub adapter_artifact_hash: String,
    pub checkpoint_artifact_hash: String,
    pub corpus_manifest_hash: String,
    pub promotion_id: String,
    pub eval_gate_id: String,
    pub serving_backend: String,
    pub adapter_artifact_present: bool,
    pub checkpoint_artifact_present: bool,
    pub backend_available: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalPromotedRouteCanaryPolicy {
    pub traffic_mode: QwenLegalPromotedRouteTrafficMode,
    pub canary_traffic_bps: u32,
    pub rollback_served_model_id: String,
    pub operator_command: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalPromotedRouteFallback {
    pub route: ModelProviderRoute,
    pub explicit_fallback_only: bool,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalPromotedRouteSpec {
    pub schema_version: u16,
    pub route_id: String,
    pub base_url: String,
    pub secret_reference_id: Option<String>,
    pub artifacts: QwenLegalPromotedRouteArtifactMetadata,
    pub canary_policy: QwenLegalPromotedRouteCanaryPolicy,
    pub fallback_routes: Vec<QwenLegalPromotedRouteFallback>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalPromotedRouteReceipt {
    pub schema_version: u16,
    pub route_id: String,
    pub served_model_id: String,
    pub route_config_hash: String,
    pub base_model_hash: String,
    pub adapter_artifact_hash: String,
    pub checkpoint_artifact_hash: String,
    pub corpus_manifest_hash: String,
    pub promotion_id: String,
    pub eval_gate_id: String,
    pub serving_backend: String,
    pub traffic_mode: QwenLegalPromotedRouteTrafficMode,
    pub receipt_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalPromotedRouteCommandReceipt {
    pub schema_version: u16,
    pub command_kind: QwenLegalPromotedRouteTrafficMode,
    pub served_model_id: String,
    pub command: Vec<String>,
    pub target_route_id: String,
    pub rollback_served_model_id: Option<String>,
    pub traffic_bps: u32,
    pub receipt_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QwenLegalPromotedProviderRouteRegistration {
    pub schema_version: u16,
    pub served_model_id: String,
    pub route: ModelProviderRoute,
    pub serving_receipt: QwenLegalPromotedRouteReceipt,
    pub canary_receipt: QwenLegalPromotedRouteCommandReceipt,
    pub rollback_receipt: QwenLegalPromotedRouteCommandReceipt,
    pub fallback_routes: Vec<QwenLegalPromotedRouteFallback>,
    pub blueprint_route_metadata: Metadata,
}

pub fn register_qwen_legal_promoted_provider_route(
    spec: QwenLegalPromotedRouteSpec,
) -> ModelAdapterResult<QwenLegalPromotedProviderRouteRegistration> {
    validate_qwen_legal_promoted_route_spec(&spec)?;
    let artifacts = &spec.artifacts;
    let mut route = ModelProviderRoute::psionic_compatible(
        spec.route_id.clone(),
        spec.base_url.clone(),
        artifacts.served_model_id.clone(),
        spec.secret_reference_id.clone(),
    );
    route.metadata = qwen_legal_promoted_route_metadata(artifacts);
    route.redacted_headers.insert(
        String::from("x-psionic-qwen-base-model-hash"),
        artifacts.base_model_hash.clone(),
    );
    route.redacted_headers.insert(
        String::from("x-psionic-qwen-adapter-hash"),
        artifacts.adapter_artifact_hash.clone(),
    );
    route.redacted_headers.insert(
        String::from("x-psionic-qwen-checkpoint-hash"),
        artifacts.checkpoint_artifact_hash.clone(),
    );
    route.redacted_headers.insert(
        String::from("x-psionic-qwen-promotion-id"),
        artifacts.promotion_id.clone(),
    );
    let mut serving_receipt = QwenLegalPromotedRouteReceipt {
        schema_version: QWEN_LEGAL_PROMOTED_PROVIDER_ROUTE_SCHEMA_VERSION,
        route_id: route.route_id.clone(),
        served_model_id: route.model_id.clone(),
        route_config_hash: route.config_hash()?,
        base_model_hash: artifacts.base_model_hash.clone(),
        adapter_artifact_hash: artifacts.adapter_artifact_hash.clone(),
        checkpoint_artifact_hash: artifacts.checkpoint_artifact_hash.clone(),
        corpus_manifest_hash: artifacts.corpus_manifest_hash.clone(),
        promotion_id: artifacts.promotion_id.clone(),
        eval_gate_id: artifacts.eval_gate_id.clone(),
        serving_backend: artifacts.serving_backend.clone(),
        traffic_mode: spec.canary_policy.traffic_mode,
        receipt_hash: String::new(),
    };
    serving_receipt.receipt_hash = stable_json_digest(
        "psionic.legal_benchmark.qwen_promoted_route_receipt.v1",
        &serving_receipt,
    )
    .map_err(|err| {
        ModelAdapterError::new(
            ModelAdapterFailureKind::InternalError,
            format!("failed to hash promoted route receipt: {err}"),
        )
    })?;
    let canary_receipt =
        qwen_legal_promoted_route_command_receipt(&spec, QwenLegalPromotedRouteTrafficMode::Canary)?;
    let rollback_receipt = qwen_legal_promoted_route_command_receipt(
        &spec,
        QwenLegalPromotedRouteTrafficMode::Rollback,
    )?;
    Ok(QwenLegalPromotedProviderRouteRegistration {
        schema_version: QWEN_LEGAL_PROMOTED_PROVIDER_ROUTE_SCHEMA_VERSION,
        served_model_id: artifacts.served_model_id.clone(),
        route,
        serving_receipt,
        canary_receipt,
        rollback_receipt,
        fallback_routes: spec.fallback_routes,
        blueprint_route_metadata: qwen_legal_promoted_route_metadata(artifacts),
    })
}

pub fn qwen_legal_promoted_smoke_route_spec() -> QwenLegalPromotedRouteSpec {
    QwenLegalPromotedRouteSpec {
        schema_version: QWEN_LEGAL_PROMOTED_PROVIDER_ROUTE_SCHEMA_VERSION,
        route_id: String::from("psionic.qwen36_legal.promoted.grpo_001"),
        base_url: String::from("https://psionic.local"),
        secret_reference_id: Some(String::from("secret.psionic.local.benchmark")),
        artifacts: QwenLegalPromotedRouteArtifactMetadata {
            base_model_id: String::from("Qwen/Qwen3.6-35B-A3B"),
            served_model_id: String::from("qwen36-legal-grpo-001"),
            base_model_hash: digest("qwen36-35b-a3b-base"),
            loaded_base_model_hash: digest("qwen36-35b-a3b-base"),
            adapter_artifact_hash: digest("qwen36-legal-grpo-001-adapter"),
            checkpoint_artifact_hash: digest("qwen36-legal-grpo-001-checkpoint"),
            corpus_manifest_hash: digest("harvey-legal-corpus-locked-v1"),
            promotion_id: String::from("qwen.legal.full_artifact_promotion.001"),
            eval_gate_id: String::from("qwen.legal.full_artifact_gate.001"),
            serving_backend: String::from("psionic-compatible"),
            adapter_artifact_present: true,
            checkpoint_artifact_present: true,
            backend_available: true,
        },
        canary_policy: QwenLegalPromotedRouteCanaryPolicy {
            traffic_mode: QwenLegalPromotedRouteTrafficMode::Canary,
            canary_traffic_bps: 500,
            rollback_served_model_id: String::from("qwen36-legal-prior-champion-000"),
            operator_command: vec![
                String::from("psionic-train"),
                String::from("qwen-legal-artifact-promotion"),
                String::from("--out"),
                String::from("target/legal/qwen_promotion_gate/full-artifact-001"),
            ],
        },
        fallback_routes: vec![QwenLegalPromotedRouteFallback {
            route: ModelProviderRoute::openai_compatible(
                "fallback.retired.openai.compat",
                "https://retired-openai-compatible.invalid",
                "retired-non-production-legal-fallback",
                Some(String::from("secret.retired.openai.compat")),
            ),
            explicit_fallback_only: true,
            reason: String::from(
                "retired provider path kept for explicit emergency comparison only",
            ),
        }],
    }
}

fn validate_qwen_legal_promoted_route_spec(
    spec: &QwenLegalPromotedRouteSpec,
) -> ModelAdapterResult<()> {
    if spec.schema_version != QWEN_LEGAL_PROMOTED_PROVIDER_ROUTE_SCHEMA_VERSION {
        return Err(candidate_error("promoted route schema version drifted"));
    }
    require_candidate_field(spec.route_id.as_str(), "route_id")?;
    require_candidate_field(spec.base_url.as_str(), "base_url")?;
    let artifacts = &spec.artifacts;
    for (field, value) in [
        ("base_model_id", artifacts.base_model_id.as_str()),
        ("served_model_id", artifacts.served_model_id.as_str()),
        ("promotion_id", artifacts.promotion_id.as_str()),
        ("eval_gate_id", artifacts.eval_gate_id.as_str()),
        ("serving_backend", artifacts.serving_backend.as_str()),
    ] {
        require_candidate_field(value, field)?;
    }
    for (field, value) in [
        ("base_model_hash", artifacts.base_model_hash.as_str()),
        (
            "loaded_base_model_hash",
            artifacts.loaded_base_model_hash.as_str(),
        ),
        (
            "adapter_artifact_hash",
            artifacts.adapter_artifact_hash.as_str(),
        ),
        (
            "checkpoint_artifact_hash",
            artifacts.checkpoint_artifact_hash.as_str(),
        ),
        (
            "corpus_manifest_hash",
            artifacts.corpus_manifest_hash.as_str(),
        ),
    ] {
        if !digest_is_complete(value) {
            return Err(qwen_promoted_route_refusal(
                QwenLegalPromotedRouteRefusalKind::MissingPromotionMetadata,
                format!("missing or invalid {field}"),
            ));
        }
    }
    if !artifacts.adapter_artifact_present {
        return Err(qwen_promoted_route_refusal(
            QwenLegalPromotedRouteRefusalKind::MissingAdapterArtifact,
            "promoted Qwen route is missing the adapter artifact",
        ));
    }
    if !artifacts.checkpoint_artifact_present {
        return Err(qwen_promoted_route_refusal(
            QwenLegalPromotedRouteRefusalKind::MissingCheckpointArtifact,
            "promoted Qwen route is missing the checkpoint artifact",
        ));
    }
    if artifacts.loaded_base_model_hash != artifacts.base_model_hash {
        return Err(qwen_promoted_route_refusal(
            QwenLegalPromotedRouteRefusalKind::BaseHashMismatch,
            "loaded base model hash does not match the promoted registry hash",
        ));
    }
    if !artifacts.backend_available {
        return Err(qwen_promoted_route_refusal(
            QwenLegalPromotedRouteRefusalKind::BackendUnavailable,
            "Psionic backend cannot serve the promoted model",
        ));
    }
    if spec
        .fallback_routes
        .iter()
        .any(|fallback| !fallback.explicit_fallback_only)
    {
        return Err(qwen_promoted_route_refusal(
            QwenLegalPromotedRouteRefusalKind::FallbackNotExplicit,
            "non-production fallback routes must be explicit fallback only",
        ));
    }
    if spec.canary_policy.canary_traffic_bps > 10_000 {
        return Err(candidate_error("canary traffic must be <= 10000 bps"));
    }
    require_candidate_field(
        spec.canary_policy.rollback_served_model_id.as_str(),
        "rollback_served_model_id",
    )?;
    Ok(())
}

fn qwen_legal_promoted_route_metadata(
    artifacts: &QwenLegalPromotedRouteArtifactMetadata,
) -> Metadata {
    let mut metadata = Metadata::new();
    metadata.insert(
        String::from("served_model_id"),
        Value::String(artifacts.served_model_id.clone()),
    );
    metadata.insert(
        String::from("base_model_id"),
        Value::String(artifacts.base_model_id.clone()),
    );
    metadata.insert(
        String::from("base_model_hash"),
        Value::String(artifacts.base_model_hash.clone()),
    );
    metadata.insert(
        String::from("adapter_artifact_hash"),
        Value::String(artifacts.adapter_artifact_hash.clone()),
    );
    metadata.insert(
        String::from("checkpoint_artifact_hash"),
        Value::String(artifacts.checkpoint_artifact_hash.clone()),
    );
    metadata.insert(
        String::from("corpus_manifest_hash"),
        Value::String(artifacts.corpus_manifest_hash.clone()),
    );
    metadata.insert(
        String::from("promotion_id"),
        Value::String(artifacts.promotion_id.clone()),
    );
    metadata.insert(
        String::from("eval_gate_id"),
        Value::String(artifacts.eval_gate_id.clone()),
    );
    metadata.insert(
        String::from("serving_backend"),
        Value::String(artifacts.serving_backend.clone()),
    );
    metadata.insert(
        String::from("provider_route_owner"),
        Value::String(String::from("psionic")),
    );
    metadata
}

fn qwen_legal_promoted_route_command_receipt(
    spec: &QwenLegalPromotedRouteSpec,
    command_kind: QwenLegalPromotedRouteTrafficMode,
) -> ModelAdapterResult<QwenLegalPromotedRouteCommandReceipt> {
    let traffic_bps = match command_kind {
        QwenLegalPromotedRouteTrafficMode::Canary => spec.canary_policy.canary_traffic_bps,
        QwenLegalPromotedRouteTrafficMode::Rollback => 10_000,
        QwenLegalPromotedRouteTrafficMode::Primary
        | QwenLegalPromotedRouteTrafficMode::ExplicitFallbackOnly => 0,
    };
    let mut command = match command_kind {
        QwenLegalPromotedRouteTrafficMode::Canary => vec![
            String::from("psionic-route"),
            String::from("canary"),
            spec.artifacts.served_model_id.clone(),
            format!("{traffic_bps}bps"),
        ],
        QwenLegalPromotedRouteTrafficMode::Rollback => vec![
            String::from("psionic-route"),
            String::from("rollback"),
            spec.artifacts.served_model_id.clone(),
            spec.canary_policy.rollback_served_model_id.clone(),
        ],
        QwenLegalPromotedRouteTrafficMode::Primary
        | QwenLegalPromotedRouteTrafficMode::ExplicitFallbackOnly => Vec::new(),
    };
    if command_kind == QwenLegalPromotedRouteTrafficMode::Canary {
        command.extend(spec.canary_policy.operator_command.clone());
    }
    let mut receipt = QwenLegalPromotedRouteCommandReceipt {
        schema_version: QWEN_LEGAL_PROMOTED_PROVIDER_ROUTE_SCHEMA_VERSION,
        command_kind,
        served_model_id: spec.artifacts.served_model_id.clone(),
        command,
        target_route_id: spec.route_id.clone(),
        rollback_served_model_id: (command_kind == QwenLegalPromotedRouteTrafficMode::Rollback)
            .then(|| spec.canary_policy.rollback_served_model_id.clone()),
        traffic_bps,
        receipt_hash: String::new(),
    };
    receipt.receipt_hash = stable_json_digest(
        "psionic.legal_benchmark.qwen_promoted_route_command_receipt.v1",
        &receipt,
    )
    .map_err(|err| {
        ModelAdapterError::new(
            ModelAdapterFailureKind::InternalError,
            format!("failed to hash promoted route command receipt: {err}"),
        )
    })?;
    Ok(receipt)
}

fn qwen_promoted_route_refusal(
    refusal: QwenLegalPromotedRouteRefusalKind,
    detail: impl Into<String>,
) -> ModelAdapterError {
    let mut error = candidate_error(format!("{refusal:?}: {}", detail.into()));
    error.retryable = refusal == QwenLegalPromotedRouteRefusalKind::BackendUnavailable;
    error
}

fn digest(seed: impl AsRef<[u8]>) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(seed.as_ref());
    hex::encode(hasher.finalize())
}

fn digest_is_complete(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl ModelMessageRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelMessage {
    pub role: ModelMessageRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ModelToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

impl ModelMessage {
    pub fn new(role: ModelMessageRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: Some(content.into()),
            tool_calls: Vec::new(),
            tool_call_id: None,
            tool_name: None,
            metadata: Metadata::new(),
        }
    }

    pub fn assistant_response(content: Option<String>, tool_calls: Vec<ModelToolCall>) -> Self {
        Self {
            role: ModelMessageRole::Assistant,
            content,
            tool_calls,
            tool_call_id: None,
            tool_name: None,
            metadata: Metadata::new(),
        }
    }

    pub fn tool_result(result: ToolResultMessage) -> Self {
        Self {
            role: ModelMessageRole::Tool,
            content: Some(result.content),
            tool_calls: Vec::new(),
            tool_call_id: Some(result.tool_call_id),
            tool_name: Some(result.tool_name),
            metadata: result.metadata,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolResultMessage {
    pub tool_call_id: String,
    pub tool_name: String,
    pub content: String,
    pub is_error: bool,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelSamplingConfig {
    pub max_output_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub random_seed: Option<u64>,
}

impl Default for ModelSamplingConfig {
    fn default() -> Self {
        Self {
            max_output_tokens: 4096,
            temperature: Some(0.0),
            top_p: None,
            random_seed: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelRetryPolicy {
    pub max_retries: u32,
    pub timeout_ms: u64,
    pub retry_on_rate_limit: bool,
    pub retry_on_timeout: bool,
    pub retry_on_server_error: bool,
}

impl Default for ModelRetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            timeout_ms: 120_000,
            retry_on_rate_limit: true,
            retry_on_timeout: true,
            retry_on_server_error: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelRequest {
    pub schema_version: u16,
    pub request_id: String,
    pub messages: Vec<ModelMessage>,
    pub tools: Vec<ModelToolSpec>,
    pub sampling: ModelSamplingConfig,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

impl ModelRequest {
    pub fn new(request_id: impl Into<String>, messages: Vec<ModelMessage>) -> Self {
        Self {
            schema_version: LEGAL_BENCHMARK_PROVIDER_SCHEMA_VERSION,
            request_id: request_id.into(),
            messages,
            tools: legal_benchmark_model_tool_specs(),
            sampling: ModelSamplingConfig::default(),
            metadata: Metadata::new(),
        }
    }

    pub fn config_hash(&self, route: &ModelProviderRoute) -> Result<String, ModelAdapterError> {
        let value = json!({
            "route": route,
            "sampling": self.sampling,
            "tools": self.tools,
            "metadata": self.metadata,
        });
        stable_json_digest("psionic.legal_benchmark.model_config.v1", &value).map_err(|err| {
            ModelAdapterError::new(
                ModelAdapterFailureKind::InternalError,
                format!("failed to hash model config: {err}"),
            )
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModelToolCall {
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelStopReason {
    Stop,
    ToolCalls,
    MaxTokens,
    SafetyRefusal,
    ProviderError,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModelUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cached_input_tokens: u64,
    pub estimated_cost_micro_usd: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelResponse {
    pub schema_version: u16,
    pub response_id: String,
    pub request_id: String,
    pub route_id: String,
    pub provider_family: ModelProviderFamily,
    pub model_id: String,
    pub model_config_hash: String,
    pub secret_reference_id: Option<String>,
    pub final_text: Option<String>,
    pub tool_calls: Vec<ModelToolCall>,
    pub stop_reason: ModelStopReason,
    pub usage: ModelUsage,
    pub elapsed_ms: u64,
    pub retry_count: u32,
    pub raw_response_hash: String,
    pub created_at_ms: u64,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelAdapterFailureKind {
    Timeout,
    RateLimited,
    ContextOverflow,
    SafetyRefusal,
    ProviderError,
    TransportError,
    ParseError,
    InvalidRequest,
    InternalError,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModelAdapterError {
    pub kind: ModelAdapterFailureKind,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_response_hash: Option<String>,
}

impl ModelAdapterError {
    pub fn new(kind: ModelAdapterFailureKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
            status_code: None,
            retryable: false,
            route_id: None,
            model_id: None,
            elapsed_ms: None,
            raw_response_hash: None,
        }
    }

    fn with_route(mut self, route: &ModelProviderRoute) -> Self {
        self.route_id = Some(route.route_id.clone());
        self.model_id = Some(route.model_id.clone());
        self
    }
}

pub type ModelAdapterResult<T> = Result<T, ModelAdapterError>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProviderHttpRequest {
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Value,
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProviderHttpResponse {
    pub status: u16,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    pub body: Value,
    pub elapsed_ms: u64,
}

pub trait ProviderHttpTransport {
    fn send(&mut self, request: ProviderHttpRequest) -> ModelAdapterResult<ProviderHttpResponse>;
}

pub trait ModelAdapter {
    fn route(&self) -> &ModelProviderRoute;

    fn complete(&mut self, request: &ModelRequest) -> ModelAdapterResult<ModelResponse>;
}

#[derive(Clone, Debug)]
pub struct OpenAiCompatibleAdapter<T> {
    route: ModelProviderRoute,
    retry_policy: ModelRetryPolicy,
    transport: T,
}

impl<T> OpenAiCompatibleAdapter<T> {
    pub fn new(route: ModelProviderRoute, transport: T, retry_policy: ModelRetryPolicy) -> Self {
        Self {
            route,
            retry_policy,
            transport,
        }
    }

    pub fn into_transport(self) -> T {
        self.transport
    }
}

impl<T> ModelAdapter for OpenAiCompatibleAdapter<T>
where
    T: ProviderHttpTransport,
{
    fn route(&self) -> &ModelProviderRoute {
        &self.route
    }

    fn complete(&mut self, request: &ModelRequest) -> ModelAdapterResult<ModelResponse> {
        let http_request = build_openai_request(&self.route, &self.retry_policy, request);
        let (http_response, retry_count) = send_with_retries(
            &mut self.transport,
            http_request,
            &self.route,
            &self.retry_policy,
        )?;
        parse_openai_response(&self.route, request, &http_response, retry_count)
    }
}

#[derive(Clone, Debug)]
pub struct AnthropicAdapter<T> {
    route: ModelProviderRoute,
    retry_policy: ModelRetryPolicy,
    transport: T,
}

impl<T> AnthropicAdapter<T> {
    pub fn new(route: ModelProviderRoute, transport: T, retry_policy: ModelRetryPolicy) -> Self {
        Self {
            route,
            retry_policy,
            transport,
        }
    }

    pub fn into_transport(self) -> T {
        self.transport
    }
}

impl<T> ModelAdapter for AnthropicAdapter<T>
where
    T: ProviderHttpTransport,
{
    fn route(&self) -> &ModelProviderRoute {
        &self.route
    }

    fn complete(&mut self, request: &ModelRequest) -> ModelAdapterResult<ModelResponse> {
        let http_request = build_anthropic_request(&self.route, &self.retry_policy, request);
        let (http_response, retry_count) = send_with_retries(
            &mut self.transport,
            http_request,
            &self.route,
            &self.retry_policy,
        )?;
        parse_anthropic_response(&self.route, request, &http_response, retry_count)
    }
}

#[derive(Clone, Debug)]
pub struct GoogleVertexGeminiAdapter<T> {
    route: ModelProviderRoute,
    retry_policy: ModelRetryPolicy,
    transport: T,
}

impl<T> GoogleVertexGeminiAdapter<T> {
    pub fn new(route: ModelProviderRoute, transport: T, retry_policy: ModelRetryPolicy) -> Self {
        Self {
            route,
            retry_policy,
            transport,
        }
    }

    pub fn into_transport(self) -> T {
        self.transport
    }
}

impl<T> ModelAdapter for GoogleVertexGeminiAdapter<T>
where
    T: ProviderHttpTransport,
{
    fn route(&self) -> &ModelProviderRoute {
        &self.route
    }

    fn complete(&mut self, request: &ModelRequest) -> ModelAdapterResult<ModelResponse> {
        let http_request =
            build_google_vertex_gemini_request(&self.route, &self.retry_policy, request);
        let (http_response, retry_count) = send_with_retries(
            &mut self.transport,
            http_request,
            &self.route,
            &self.retry_policy,
        )?;
        parse_google_vertex_gemini_response(&self.route, request, &http_response, retry_count)
    }
}

#[derive(Clone, Debug)]
pub struct MockModelAdapter {
    route: ModelProviderRoute,
    responses: VecDeque<ModelAdapterResult<ModelResponse>>,
}

impl MockModelAdapter {
    pub fn new(
        route: ModelProviderRoute,
        responses: Vec<ModelAdapterResult<ModelResponse>>,
    ) -> Self {
        Self {
            route,
            responses: responses.into(),
        }
    }
}

impl ModelAdapter for MockModelAdapter {
    fn route(&self) -> &ModelProviderRoute {
        &self.route
    }

    fn complete(&mut self, _request: &ModelRequest) -> ModelAdapterResult<ModelResponse> {
        match self.responses.pop_front() {
            Some(Ok(mut response)) => {
                merge_route_metadata(&self.route, &mut response.metadata);
                Ok(response)
            }
            Some(Err(error)) => Err(error),
            None => Err(ModelAdapterError::new(
                ModelAdapterFailureKind::ProviderError,
                "mock adapter has no queued response",
            )
            .with_route(&self.route)),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct MockHttpTransport {
    pub requests: Vec<ProviderHttpRequest>,
    pub responses: VecDeque<ModelAdapterResult<ProviderHttpResponse>>,
}

impl MockHttpTransport {
    pub fn new(responses: Vec<ModelAdapterResult<ProviderHttpResponse>>) -> Self {
        Self {
            requests: Vec::new(),
            responses: responses.into(),
        }
    }
}

impl ProviderHttpTransport for MockHttpTransport {
    fn send(&mut self, request: ProviderHttpRequest) -> ModelAdapterResult<ProviderHttpResponse> {
        self.requests.push(request);
        match self.responses.pop_front() {
            Some(response) => response,
            None => Err(ModelAdapterError::new(
                ModelAdapterFailureKind::TransportError,
                "mock HTTP transport has no queued response",
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReqwestBlockingHttpTransport {
    client: reqwest::blocking::Client,
}

impl ReqwestBlockingHttpTransport {
    pub fn new() -> ModelAdapterResult<Self> {
        let client = reqwest::blocking::Client::builder()
            .build()
            .map_err(|error| transport_error(format!("failed to build HTTP client: {error}")))?;
        Ok(Self { client })
    }
}

impl Default for ReqwestBlockingHttpTransport {
    fn default() -> Self {
        match Self::new() {
            Ok(transport) => transport,
            Err(_) => Self {
                client: reqwest::blocking::Client::new(),
            },
        }
    }
}

impl ProviderHttpTransport for ReqwestBlockingHttpTransport {
    fn send(&mut self, request: ProviderHttpRequest) -> ModelAdapterResult<ProviderHttpResponse> {
        if request
            .headers
            .values()
            .any(|value| value.contains("<secret_ref:"))
        {
            return Err(transport_error(
                "reqwest legal benchmark transport refuses unresolved secret references",
            ));
        }

        let method = match request.method.as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            other => {
                return Err(transport_error(format!(
                    "unsupported HTTP method for legal benchmark transport: {other}"
                )));
            }
        };
        let mut builder = self
            .client
            .request(method, request.url.clone())
            .timeout(Duration::from_millis(request.timeout_ms.max(1)))
            .header(reqwest::header::CONTENT_TYPE, "application/json");
        for (name, value) in &request.headers {
            if name.eq_ignore_ascii_case("content-type") {
                continue;
            }
            builder = builder.header(name.as_str(), value.as_str());
        }
        if request.method == "POST" {
            builder = builder.json(&request.body);
        }

        let started = SystemTime::now();
        let response = builder
            .send()
            .map_err(|error| transport_error(format!("HTTP send failed: {error}")))?;
        let elapsed_ms = started
            .elapsed()
            .map(|elapsed| elapsed.as_millis().min(u128::from(u64::MAX)) as u64)
            .unwrap_or(0);
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    value.to_str().unwrap_or("<non_utf8_header>").to_owned(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let body = response.json::<Value>().map_err(|error| {
            transport_error(format!("HTTP response JSON parse failed: {error}"))
        })?;

        Ok(ProviderHttpResponse {
            status,
            headers,
            body,
            elapsed_ms,
        })
    }
}

pub fn legal_benchmark_model_tool_specs() -> Vec<ModelToolSpec> {
    vec![
        tool_spec(
            LegalBenchmarkToolName::Shell,
            "Run a sandboxed command with argv, timeout, stdout, stderr, and exit-code receipt.",
            json!({
                "type": "object",
                "required": ["command"],
                "properties": {
                    "command": {"type": "array", "items": {"type": "string"}, "minItems": 1},
                    "timeout_ms": {"type": "integer", "minimum": 1}
                },
                "additionalProperties": false
            }),
        ),
        tool_spec(
            LegalBenchmarkToolName::Read,
            "Read a file from documents, workspace, or output roots, preferring extracted text when requested.",
            json!({
                "type": "object",
                "required": ["root", "relative_path", "prefer_extracted"],
                "properties": {
                    "root": {"type": "string", "enum": ["documents", "workspace", "output"]},
                    "relative_path": {"type": "string"},
                    "prefer_extracted": {"type": "boolean"}
                },
                "additionalProperties": false
            }),
        ),
        tool_spec(
            LegalBenchmarkToolName::Write,
            "Write a UTF-8 output file under the workspace or output root.",
            json!({
                "type": "object",
                "required": ["root", "relative_path", "content", "overwrite"],
                "properties": {
                    "root": {"type": "string", "enum": ["workspace", "output"]},
                    "relative_path": {"type": "string"},
                    "content": {"type": "string"},
                    "overwrite": {"type": "boolean"}
                },
                "additionalProperties": false
            }),
        ),
        tool_spec(
            LegalBenchmarkToolName::Edit,
            "Replace exact text in an existing workspace or output file.",
            json!({
                "type": "object",
                "required": ["root", "relative_path", "find", "replace"],
                "properties": {
                    "root": {"type": "string", "enum": ["workspace", "output"]},
                    "relative_path": {"type": "string"},
                    "find": {"type": "string"},
                    "replace": {"type": "string"},
                    "expected_replacements": {"type": "integer", "minimum": 0}
                },
                "additionalProperties": false
            }),
        ),
        tool_spec(
            LegalBenchmarkToolName::Glob,
            "List files matching a simple wildcard pattern under one allowed root.",
            json!({
                "type": "object",
                "required": ["root", "pattern", "max_results", "include_hidden"],
                "properties": {
                    "root": {"type": "string", "enum": ["documents", "workspace", "output"]},
                    "pattern": {"type": "string"},
                    "max_results": {"type": "integer", "minimum": 1},
                    "include_hidden": {"type": "boolean"}
                },
                "additionalProperties": false
            }),
        ),
        tool_spec(
            LegalBenchmarkToolName::Grep,
            "Search text files under one allowed root and return deterministic line matches.",
            json!({
                "type": "object",
                "required": ["root", "pattern", "case_sensitive", "max_results", "include_hidden"],
                "properties": {
                    "root": {"type": "string", "enum": ["documents", "workspace", "output"]},
                    "pattern": {"type": "string"},
                    "case_sensitive": {"type": "boolean"},
                    "max_results": {"type": "integer", "minimum": 1},
                    "include_hidden": {"type": "boolean"}
                },
                "additionalProperties": false
            }),
        ),
        tool_spec(
            LegalBenchmarkToolName::Inventory,
            "Inventory files with size, media type, hash, extraction availability, and document-count hints.",
            json!({
                "type": "object",
                "required": ["root", "max_results", "include_hidden", "include_hashes"],
                "properties": {
                    "root": {"type": "string", "enum": ["documents", "workspace", "output"]},
                    "max_results": {"type": "integer", "minimum": 1},
                    "include_hidden": {"type": "boolean"},
                    "include_hashes": {"type": "boolean"}
                },
                "additionalProperties": false
            }),
        ),
        tool_spec(
            LegalBenchmarkToolName::EmailSummary,
            "Summarize an EML-style message with headers, body preview, and attachment count.",
            json!({
                "type": "object",
                "required": ["root", "relative_path", "max_body_chars"],
                "properties": {
                    "root": {"type": "string", "enum": ["documents", "workspace", "output"]},
                    "relative_path": {"type": "string"},
                    "max_body_chars": {"type": "integer", "minimum": 1}
                },
                "additionalProperties": false
            }),
        ),
        tool_spec(
            LegalBenchmarkToolName::SpreadsheetSummary,
            "Summarize CSV/TSV or extracted spreadsheet text with row, column, formula, and preview data.",
            json!({
                "type": "object",
                "required": ["root", "relative_path", "max_preview_rows"],
                "properties": {
                    "root": {"type": "string", "enum": ["documents", "workspace", "output"]},
                    "relative_path": {"type": "string"},
                    "max_preview_rows": {"type": "integer", "minimum": 1}
                },
                "additionalProperties": false
            }),
        ),
        tool_spec(
            LegalBenchmarkToolName::PdfSearch,
            "Search extracted PDF text by page and return page-scoped snippets with span hashes.",
            json!({
                "type": "object",
                "required": ["root", "relative_path", "query", "max_matches"],
                "properties": {
                    "root": {"type": "string", "enum": ["documents", "workspace", "output"]},
                    "relative_path": {"type": "string"},
                    "query": {"type": "string"},
                    "page": {"type": "integer", "minimum": 1},
                    "max_matches": {"type": "integer", "minimum": 1}
                },
                "additionalProperties": false
            }),
        ),
        tool_spec(
            LegalBenchmarkToolName::EvidenceTable,
            "Build a receipt-backed evidence table from source refs, locators, quotes, and notes.",
            json!({
                "type": "object",
                "required": ["entries"],
                "properties": {
                    "entries": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["source_ref", "quote"],
                            "properties": {
                                "source_ref": {"type": "string"},
                                "locator": {"type": "string"},
                                "quote": {"type": "string"},
                                "note": {"type": "string"}
                            },
                            "additionalProperties": false
                        }
                    }
                },
                "additionalProperties": false
            }),
        ),
        tool_spec(
            LegalBenchmarkToolName::ValidateDeliverables,
            "Check required workspace/output deliverables for existence, readability, media type, and hash.",
            json!({
                "type": "object",
                "required": ["root", "required_paths", "max_results"],
                "properties": {
                    "root": {"type": "string", "enum": ["workspace", "output"]},
                    "required_paths": {"type": "array", "items": {"type": "string"}, "minItems": 1},
                    "max_results": {"type": "integer", "minimum": 1}
                },
                "additionalProperties": false
            }),
        ),
    ]
}

fn tool_spec(
    name: LegalBenchmarkToolName,
    description: &str,
    input_schema: Value,
) -> ModelToolSpec {
    ModelToolSpec {
        name: name.as_str().to_owned(),
        description: description.to_owned(),
        input_schema,
    }
}

fn build_openai_request(
    route: &ModelProviderRoute,
    retry_policy: &ModelRetryPolicy,
    request: &ModelRequest,
) -> ProviderHttpRequest {
    let messages = request
        .messages
        .iter()
        .map(openai_message_json)
        .collect::<Vec<_>>();
    let tools = request
        .tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema,
                }
            })
        })
        .collect::<Vec<_>>();
    let mut body = json!({
        "model": route.model_id,
        "messages": messages,
        "max_completion_tokens": request.sampling.max_output_tokens,
    });
    if !tools.is_empty() {
        body["tools"] = json!(tools);
        body["tool_choice"] = Value::String(String::from("auto"));
        if let Some(tool_choice) = request.metadata.get("tool_choice").and_then(Value::as_str) {
            if tool_choice == "auto" {
                body["tool_choice"] = Value::String(String::from("auto"));
            } else {
                body["tool_choice"] = json!({
                    "type": "function",
                    "function": {
                        "name": tool_choice
                    }
                });
            }
        }
    }
    if let Some(temperature) = request.sampling.temperature {
        body["temperature"] = json!(temperature);
    }
    if let Some(top_p) = request.sampling.top_p {
        body["top_p"] = json!(top_p);
    }
    if let Some(seed) = request.sampling.random_seed {
        body["seed"] = json!(seed);
    }
    ProviderHttpRequest {
        method: String::from("POST"),
        url: route.endpoint_url(),
        headers: route.redacted_headers.clone(),
        body,
        timeout_ms: retry_policy.timeout_ms,
    }
}

fn openai_message_json(message: &ModelMessage) -> Value {
    match message.role {
        ModelMessageRole::Tool => json!({
            "role": "tool",
            "tool_call_id": message.tool_call_id,
            "content": message.content.clone().unwrap_or_default(),
        }),
        ModelMessageRole::Assistant if !message.tool_calls.is_empty() => json!({
            "role": "assistant",
            "content": message.content.clone().unwrap_or_default(),
            "tool_calls": message.tool_calls.iter().map(openai_tool_call_message_json).collect::<Vec<_>>(),
        }),
        _ => json!({
            "role": message.role.as_str(),
            "content": message.content.clone().unwrap_or_default(),
        }),
    }
}

fn openai_tool_call_message_json(tool_call: &ModelToolCall) -> Value {
    json!({
        "id": tool_call.tool_call_id.clone(),
        "type": "function",
        "function": {
            "name": tool_call.tool_name.clone(),
            "arguments": tool_call.arguments.to_string(),
        }
    })
}

fn build_anthropic_request(
    route: &ModelProviderRoute,
    retry_policy: &ModelRetryPolicy,
    request: &ModelRequest,
) -> ProviderHttpRequest {
    let system = request
        .messages
        .iter()
        .filter(|message| message.role == ModelMessageRole::System)
        .filter_map(|message| message.content.clone())
        .collect::<Vec<_>>()
        .join("\n\n");
    let messages = request
        .messages
        .iter()
        .filter(|message| message.role != ModelMessageRole::System)
        .map(anthropic_message_json)
        .collect::<Vec<_>>();
    let tools = request
        .tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.input_schema,
            })
        })
        .collect::<Vec<_>>();
    let mut body = json!({
        "model": route.model_id,
        "max_tokens": request.sampling.max_output_tokens,
        "messages": messages,
        "tools": tools,
    });
    if !system.is_empty() {
        body["system"] = json!(system);
    }
    if let Some(temperature) = request.sampling.temperature {
        body["temperature"] = json!(temperature);
    }
    if let Some(top_p) = request.sampling.top_p {
        body["top_p"] = json!(top_p);
    }
    ProviderHttpRequest {
        method: String::from("POST"),
        url: route.endpoint_url(),
        headers: route.redacted_headers.clone(),
        body,
        timeout_ms: retry_policy.timeout_ms,
    }
}

fn anthropic_message_json(message: &ModelMessage) -> Value {
    match message.role {
        ModelMessageRole::Tool => json!({
            "role": "user",
            "content": [{
                "type": "tool_result",
                "tool_use_id": message.tool_call_id,
                "content": message.content.clone().unwrap_or_default(),
                "is_error": message
                    .metadata
                    .get("is_error")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            }]
        }),
        ModelMessageRole::Assistant if !message.tool_calls.is_empty() => json!({
            "role": "assistant",
            "content": message.tool_calls.iter().map(anthropic_tool_use_message_json).collect::<Vec<_>>(),
        }),
        ModelMessageRole::Assistant => json!({
            "role": "assistant",
            "content": message.content.clone().unwrap_or_default(),
        }),
        _ => json!({
            "role": "user",
            "content": message.content.clone().unwrap_or_default(),
        }),
    }
}

fn anthropic_tool_use_message_json(tool_call: &ModelToolCall) -> Value {
    json!({
        "type": "tool_use",
        "id": tool_call.tool_call_id.clone(),
        "name": tool_call.tool_name.clone(),
        "input": tool_call.arguments.clone(),
    })
}

fn build_google_vertex_gemini_request(
    route: &ModelProviderRoute,
    retry_policy: &ModelRetryPolicy,
    request: &ModelRequest,
) -> ProviderHttpRequest {
    let system = request
        .messages
        .iter()
        .filter(|message| message.role == ModelMessageRole::System)
        .filter_map(|message| message.content.clone())
        .collect::<Vec<_>>()
        .join("\n\n");
    let contents = request
        .messages
        .iter()
        .filter(|message| message.role != ModelMessageRole::System)
        .map(google_vertex_gemini_content_json)
        .collect::<Vec<_>>();
    let function_declarations = request
        .tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.input_schema,
            })
        })
        .collect::<Vec<_>>();
    let mut generation_config = json!({
        "maxOutputTokens": request.sampling.max_output_tokens,
    });
    if let Some(temperature) = request.sampling.temperature {
        generation_config["temperature"] = json!(temperature);
    }
    if let Some(top_p) = request.sampling.top_p {
        generation_config["topP"] = json!(top_p);
    }
    if let Some(thinking_level) = route
        .metadata
        .get("thinking_level")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        generation_config["thinkingConfig"] = json!({
            "thinkingLevel": thinking_level.to_ascii_uppercase(),
        });
    }

    let mut body = json!({
        "contents": contents,
        "generationConfig": generation_config,
    });
    if !system.is_empty() {
        body["systemInstruction"] = json!({
            "parts": [{"text": system}],
        });
    }
    if !function_declarations.is_empty() {
        body["tools"] = json!([{
            "functionDeclarations": function_declarations,
        }]);
        body["toolConfig"] = json!({
            "functionCallingConfig": {"mode": "AUTO"},
        });
    }
    ProviderHttpRequest {
        method: String::from("POST"),
        url: route.endpoint_url(),
        headers: route.redacted_headers.clone(),
        body,
        timeout_ms: retry_policy.timeout_ms,
    }
}

fn google_vertex_gemini_content_json(message: &ModelMessage) -> Value {
    let role = match message.role {
        ModelMessageRole::Assistant => "model",
        ModelMessageRole::System | ModelMessageRole::User | ModelMessageRole::Tool => "user",
    };
    let mut parts = Vec::new();
    if let Some(content) = message
        .content
        .as_ref()
        .filter(|content| !content.trim().is_empty())
    {
        if message.role == ModelMessageRole::Tool {
            parts.push(json!({
                "functionResponse": {
                    "name": message.tool_name.as_deref().unwrap_or_else(|| {
                        message.tool_call_id.as_deref().unwrap_or("tool")
                    }),
                    "response": {
                        "result": content,
                        "is_error": message
                            .metadata
                            .get("is_error")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                    }
                }
            }));
        } else {
            parts.push(json!({"text": content}));
        }
    }
    for tool_call in &message.tool_calls {
        parts.push(google_vertex_gemini_function_call_json(tool_call));
    }
    if parts.is_empty() {
        parts.push(json!({"text": ""}));
    }
    json!({
        "role": role,
        "parts": parts,
    })
}

fn google_vertex_gemini_function_call_json(tool_call: &ModelToolCall) -> Value {
    json!({
        "functionCall": {
            "name": tool_call.tool_name.clone(),
            "args": tool_call.arguments.clone(),
        }
    })
}

fn send_with_retries<T>(
    transport: &mut T,
    request: ProviderHttpRequest,
    route: &ModelProviderRoute,
    retry_policy: &ModelRetryPolicy,
) -> ModelAdapterResult<(ProviderHttpResponse, u32)>
where
    T: ProviderHttpTransport,
{
    let mut attempts = 0;
    loop {
        let response = transport.send(request.clone());
        match response {
            Ok(http_response) if http_response.status < 400 => {
                return Ok((http_response, attempts));
            }
            Ok(http_response) => {
                let mut err = classify_http_failure(route, &http_response)?;
                let retryable = should_retry(err.kind, retry_policy);
                err.retryable = retryable;
                if retryable && attempts < retry_policy.max_retries {
                    attempts += 1;
                    continue;
                }
                return Err(err);
            }
            Err(mut err) => {
                err = err.with_route(route);
                let retryable = should_retry(err.kind, retry_policy);
                err.retryable = retryable;
                if retryable && attempts < retry_policy.max_retries {
                    attempts += 1;
                    continue;
                }
                return Err(err);
            }
        }
    }
}

fn classify_http_failure(
    route: &ModelProviderRoute,
    response: &ProviderHttpResponse,
) -> ModelAdapterResult<ModelAdapterError> {
    let raw_response_hash = raw_response_hash(&response.body)?;
    let kind = match response.status {
        408 | 504 => ModelAdapterFailureKind::Timeout,
        429 => ModelAdapterFailureKind::RateLimited,
        400 if json_contains_context_overflow(&response.body) => {
            ModelAdapterFailureKind::ContextOverflow
        }
        400 | 401 | 403 | 404 => ModelAdapterFailureKind::ProviderError,
        500..=599 => ModelAdapterFailureKind::ProviderError,
        _ => ModelAdapterFailureKind::ProviderError,
    };
    Ok(ModelAdapterError {
        kind,
        detail: response
            .body
            .get("error")
            .and_then(|value| {
                value
                    .get("message")
                    .and_then(Value::as_str)
                    .or_else(|| value.as_str())
            })
            .unwrap_or("provider returned an error status")
            .to_owned(),
        status_code: Some(response.status),
        retryable: false,
        route_id: Some(route.route_id.clone()),
        model_id: Some(route.model_id.clone()),
        elapsed_ms: Some(response.elapsed_ms),
        raw_response_hash: Some(raw_response_hash),
    })
}

fn should_retry(kind: ModelAdapterFailureKind, retry_policy: &ModelRetryPolicy) -> bool {
    match kind {
        ModelAdapterFailureKind::Timeout => retry_policy.retry_on_timeout,
        ModelAdapterFailureKind::RateLimited => retry_policy.retry_on_rate_limit,
        ModelAdapterFailureKind::ProviderError => retry_policy.retry_on_server_error,
        _ => false,
    }
}

fn parse_openai_response(
    route: &ModelProviderRoute,
    request: &ModelRequest,
    response: &ProviderHttpResponse,
    retry_count: u32,
) -> ModelAdapterResult<ModelResponse> {
    let choice = response
        .body
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .ok_or_else(|| parse_error(route, "OpenAI-compatible response missing first choice"))?;
    let message = choice
        .get("message")
        .ok_or_else(|| parse_error(route, "OpenAI-compatible response missing message"))?;
    let final_text = message
        .get("content")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let tool_calls = message
        .get("tool_calls")
        .and_then(Value::as_array)
        .map(|calls| {
            calls
                .iter()
                .filter_map(openai_tool_call)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let finish_reason = choice
        .get("finish_reason")
        .and_then(Value::as_str)
        .unwrap_or("stop");
    let stop_reason = match finish_reason {
        "tool_calls" => ModelStopReason::ToolCalls,
        "length" => ModelStopReason::MaxTokens,
        "content_filter" => ModelStopReason::SafetyRefusal,
        "stop" => ModelStopReason::Stop,
        _ if !tool_calls.is_empty() => ModelStopReason::ToolCalls,
        _ => ModelStopReason::ProviderError,
    };
    Ok(ModelResponse {
        schema_version: LEGAL_BENCHMARK_PROVIDER_SCHEMA_VERSION,
        response_id: response
            .body
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("openai-compatible-response")
            .to_owned(),
        request_id: request.request_id.clone(),
        route_id: route.route_id.clone(),
        provider_family: route.family,
        model_id: route.model_id.clone(),
        model_config_hash: request.config_hash(route)?,
        secret_reference_id: route.secret_reference_id.clone(),
        final_text,
        tool_calls,
        stop_reason,
        usage: parse_openai_usage(response),
        elapsed_ms: response.elapsed_ms,
        retry_count,
        raw_response_hash: raw_response_hash(&response.body)?,
        created_at_ms: now_ms(),
        metadata: response_metadata_for_route(route),
    })
}

fn openai_tool_call(value: &Value) -> Option<ModelToolCall> {
    let function = value.get("function")?;
    Some(ModelToolCall {
        tool_call_id: value.get("id")?.as_str()?.to_owned(),
        tool_name: function.get("name")?.as_str()?.to_owned(),
        arguments: parse_tool_arguments(function.get("arguments")),
    })
}

fn parse_openai_usage(response: &ProviderHttpResponse) -> ModelUsage {
    let usage = response.body.get("usage").unwrap_or(&Value::Null);
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("completion_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(input_tokens + output_tokens);
    let cached_input_tokens = usage
        .get("prompt_tokens_details")
        .and_then(|details| details.get("cached_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    ModelUsage {
        input_tokens,
        output_tokens,
        total_tokens,
        cached_input_tokens,
        estimated_cost_micro_usd: 0,
    }
}

fn parse_anthropic_response(
    route: &ModelProviderRoute,
    request: &ModelRequest,
    response: &ProviderHttpResponse,
    retry_count: u32,
) -> ModelAdapterResult<ModelResponse> {
    let content = response
        .body
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| parse_error(route, "Anthropic response missing content blocks"))?;
    let mut text_blocks = Vec::new();
    let mut tool_calls = Vec::new();
    for block in content {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    text_blocks.push(text.to_owned());
                }
            }
            Some("tool_use") => {
                let Some(tool_call_id) = block.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let Some(tool_name) = block.get("name").and_then(Value::as_str) else {
                    continue;
                };
                tool_calls.push(ModelToolCall {
                    tool_call_id: tool_call_id.to_owned(),
                    tool_name: tool_name.to_owned(),
                    arguments: block.get("input").cloned().unwrap_or_else(|| json!({})),
                });
            }
            _ => {}
        }
    }
    let stop_reason_raw = response
        .body
        .get("stop_reason")
        .and_then(Value::as_str)
        .unwrap_or("end_turn");
    let stop_reason = match stop_reason_raw {
        "tool_use" => ModelStopReason::ToolCalls,
        "max_tokens" => ModelStopReason::MaxTokens,
        "refusal" => ModelStopReason::SafetyRefusal,
        "end_turn" | "stop_sequence" => ModelStopReason::Stop,
        _ if !tool_calls.is_empty() => ModelStopReason::ToolCalls,
        _ => ModelStopReason::ProviderError,
    };
    Ok(ModelResponse {
        schema_version: LEGAL_BENCHMARK_PROVIDER_SCHEMA_VERSION,
        response_id: response
            .body
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("anthropic-response")
            .to_owned(),
        request_id: request.request_id.clone(),
        route_id: route.route_id.clone(),
        provider_family: route.family,
        model_id: route.model_id.clone(),
        model_config_hash: request.config_hash(route)?,
        secret_reference_id: route.secret_reference_id.clone(),
        final_text: (!text_blocks.is_empty()).then(|| text_blocks.join("\n")),
        tool_calls,
        stop_reason,
        usage: parse_anthropic_usage(response),
        elapsed_ms: response.elapsed_ms,
        retry_count,
        raw_response_hash: raw_response_hash(&response.body)?,
        created_at_ms: now_ms(),
        metadata: response_metadata_for_route(route),
    })
}

fn parse_anthropic_usage(response: &ProviderHttpResponse) -> ModelUsage {
    let usage = response.body.get("usage").unwrap_or(&Value::Null);
    let input_tokens = usage
        .get("input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cached_input_tokens = usage
        .get("cache_read_input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    ModelUsage {
        input_tokens,
        output_tokens,
        total_tokens: input_tokens + output_tokens,
        cached_input_tokens,
        estimated_cost_micro_usd: 0,
    }
}

fn parse_google_vertex_gemini_response(
    route: &ModelProviderRoute,
    request: &ModelRequest,
    response: &ProviderHttpResponse,
    retry_count: u32,
) -> ModelAdapterResult<ModelResponse> {
    let candidate = response
        .body
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first())
        .ok_or_else(|| parse_error(route, "Gemini response missing first candidate"))?;
    let parts = candidate
        .get("content")
        .and_then(|content| content.get("parts"))
        .and_then(Value::as_array)
        .ok_or_else(|| parse_error(route, "Gemini response missing content parts"))?;
    let mut text_blocks = Vec::new();
    let mut tool_calls = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        if let Some(text) = part.get("text").and_then(Value::as_str) {
            if !text.is_empty() {
                text_blocks.push(text.to_owned());
            }
        }
        if let Some(function_call) = part
            .get("functionCall")
            .or_else(|| part.get("function_call"))
        {
            if let Some(tool_call) = google_vertex_gemini_tool_call(function_call, index) {
                tool_calls.push(tool_call);
            }
        }
    }
    let finish_reason = candidate
        .get("finishReason")
        .or_else(|| candidate.get("finish_reason"))
        .and_then(Value::as_str)
        .unwrap_or("STOP");
    let stop_reason = match finish_reason {
        "MAX_TOKENS" => ModelStopReason::MaxTokens,
        "SAFETY" | "RECITATION" | "SPII" => ModelStopReason::SafetyRefusal,
        "STOP" if !tool_calls.is_empty() => ModelStopReason::ToolCalls,
        "STOP" => ModelStopReason::Stop,
        "MALFORMED_FUNCTION_CALL" => ModelStopReason::ProviderError,
        _ if !tool_calls.is_empty() => ModelStopReason::ToolCalls,
        _ => ModelStopReason::ProviderError,
    };
    Ok(ModelResponse {
        schema_version: LEGAL_BENCHMARK_PROVIDER_SCHEMA_VERSION,
        response_id: response
            .body
            .get("responseId")
            .or_else(|| response.body.get("response_id"))
            .and_then(Value::as_str)
            .unwrap_or("gemini-response")
            .to_owned(),
        request_id: request.request_id.clone(),
        route_id: route.route_id.clone(),
        provider_family: route.family,
        model_id: route.model_id.clone(),
        model_config_hash: request.config_hash(route)?,
        secret_reference_id: route.secret_reference_id.clone(),
        final_text: (!text_blocks.is_empty()).then(|| text_blocks.join("\n")),
        tool_calls,
        stop_reason,
        usage: parse_google_vertex_gemini_usage(response),
        elapsed_ms: response.elapsed_ms,
        retry_count,
        raw_response_hash: raw_response_hash(&response.body)?,
        created_at_ms: now_ms(),
        metadata: response_metadata_for_route(route),
    })
}

fn google_vertex_gemini_tool_call(value: &Value, index: usize) -> Option<ModelToolCall> {
    let tool_name = value.get("name")?.as_str()?.to_owned();
    Some(ModelToolCall {
        tool_call_id: value
            .get("id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| {
                if index == 0 {
                    tool_name.clone()
                } else {
                    format!("{}_{}", tool_name, index)
                }
            }),
        tool_name,
        arguments: value
            .get("args")
            .or_else(|| value.get("arguments"))
            .cloned()
            .unwrap_or_else(|| json!({})),
    })
}

fn parse_google_vertex_gemini_usage(response: &ProviderHttpResponse) -> ModelUsage {
    let usage = response.body.get("usageMetadata").unwrap_or(&Value::Null);
    let input_tokens = usage
        .get("promptTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = usage
        .get("candidatesTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total_tokens = usage
        .get("totalTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(input_tokens + output_tokens);
    let cached_input_tokens = usage
        .get("cachedContentTokenCount")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    ModelUsage {
        input_tokens,
        output_tokens,
        total_tokens,
        cached_input_tokens,
        estimated_cost_micro_usd: 0,
    }
}

fn parse_tool_arguments(value: Option<&Value>) -> Value {
    match value {
        Some(Value::String(arguments)) if arguments.trim().is_empty() => json!({}),
        Some(Value::String(arguments)) => {
            serde_json::from_str(arguments).unwrap_or_else(|_| json!({"raw_arguments": arguments}))
        }
        Some(other) => other.clone(),
        None => json!({}),
    }
}

fn parse_error(route: &ModelProviderRoute, detail: &str) -> ModelAdapterError {
    ModelAdapterError::new(ModelAdapterFailureKind::ParseError, detail).with_route(route)
}

fn json_contains_context_overflow(value: &Value) -> bool {
    let needle = value.to_string().to_ascii_lowercase();
    needle.contains("context") && (needle.contains("overflow") || needle.contains("length"))
}

fn raw_response_hash(value: &Value) -> ModelAdapterResult<String> {
    stable_json_digest("psionic.legal_benchmark.provider_raw_response.v1", value).map_err(|err| {
        ModelAdapterError::new(
            ModelAdapterFailureKind::InternalError,
            format!("failed to hash provider response: {err}"),
        )
    })
}

fn candidate_score_entry(
    score_class: LegalBenchmarkCandidateScoreClass,
    candidate: &QwenLegalBasePlusAdapterCandidateIdentity,
    score: &ScoreReport,
    public_retained_claim: bool,
) -> ModelAdapterResult<LegalBenchmarkCandidateScoreEntry> {
    if candidate.candidate_kind == LegalBenchmarkModelCandidateKind::MockLocalSmoke
        && public_retained_claim
    {
        return Err(candidate_error(
            "mock/local smoke scores cannot be emitted as retained score claims",
        ));
    }
    let score_report_hash = score_report_digest(score).map_err(|err| {
        ModelAdapterError::new(
            ModelAdapterFailureKind::InternalError,
            format!("failed to hash score report: {err}"),
        )
    })?;
    Ok(LegalBenchmarkCandidateScoreEntry {
        score_class,
        candidate_id: candidate.candidate_id.clone(),
        candidate_identity_digest: candidate.identity_digest()?,
        score_report_id: score.score_report_id.clone(),
        score_report_hash,
        criterion_pass_rate_bps: score.criterion_pass_rate_bps,
        all_pass: score.all_pass,
        public_retained_claim,
    })
}

fn response_metadata_for_route(route: &ModelProviderRoute) -> Metadata {
    let mut metadata = route.metadata.clone();
    merge_route_metadata(route, &mut metadata);
    metadata
}

fn merge_route_metadata(route: &ModelProviderRoute, metadata: &mut Metadata) {
    metadata
        .entry(String::from("route_id"))
        .or_insert_with(|| Value::String(route.route_id.clone()));
    metadata
        .entry(String::from("route_model_id"))
        .or_insert_with(|| Value::String(route.model_id.clone()));
}

fn require_candidate_field(value: &str, field: &'static str) -> ModelAdapterResult<()> {
    if value.trim().is_empty() {
        return Err(candidate_error(format!(
            "candidate {field} must be present"
        )));
    }
    Ok(())
}

fn require_candidate_option(value: Option<&str>, field: &'static str) -> ModelAdapterResult<()> {
    match value {
        Some(value) => require_candidate_field(value, field),
        None => Err(candidate_error(format!(
            "candidate {field} must be present"
        ))),
    }
}

fn candidate_error(detail: impl Into<String>) -> ModelAdapterError {
    ModelAdapterError::new(ModelAdapterFailureKind::InvalidRequest, detail)
}

fn transport_error(detail: impl Into<String>) -> ModelAdapterError {
    ModelAdapterError::new(ModelAdapterFailureKind::TransportError, detail)
}

fn trim_url(url: String) -> String {
    url.trim_end_matches('/').to_owned()
}

fn now_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qwen_base_candidate() -> QwenLegalBasePlusAdapterCandidateIdentity {
        QwenLegalBasePlusAdapterCandidateIdentity::base(
            "candidate.qwen35.base",
            "qwen3.5-4b",
            "sha256:qwen35-4b-base",
            "sha256:qwen35-tokenizer",
            "sha256:qwen35-tokenizer-contract",
            "sha256:qwen35-template",
            "sha256:legal-dataset",
            "sha256:legal-eval-pack",
            "serving.qwen35.base.r1",
        )
        .expect("base candidate")
    }

    fn qwen_tuned_candidate() -> QwenLegalBasePlusAdapterCandidateIdentity {
        QwenLegalBasePlusAdapterCandidateIdentity::tuned_adapter(
            "candidate.qwen35.legal.r1",
            "qwen3.5-4b-legal-r1",
            "sha256:qwen35-4b-base",
            "sha256:qwen35-tokenizer",
            "sha256:qwen35-tokenizer-contract",
            "sha256:qwen35-template",
            "sha256:legal-dataset",
            "sha256:legal-eval-pack",
            "serving.qwen35.legal.r1",
            "qwen35-legal-adapter",
            "r1",
            "sha256:qwen35-legal-adapter-artifact",
            "sha256:qwen35-legal-adapter-identity",
            Some(String::from("r1")),
            Some(String::from("base")),
        )
        .expect("tuned candidate")
    }

    fn score_report(id: &str, rate: u32) -> ScoreReport {
        ScoreReport {
            schema_version: crate::LEGAL_BENCHMARK_SCHEMA_VERSION,
            score_report_id: id.to_string(),
            run_id: format!("run.{id}"),
            task_id: String::from("legal.task"),
            task_version: String::from("v1"),
            run_record_hash: format!("hash.run.{id}"),
            output_artifact_manifest_hash: format!("hash.output.{id}"),
            all_pass: rate == 10_000,
            criterion_pass_rate_bps: rate,
            criterion_results: Vec::new(),
            metrics: crate::RunMetrics {
                model_turns: 1,
                tool_call_count: 0,
                input_tokens: 1,
                output_tokens: 1,
                wall_time_ms: 1,
                estimated_cost_micro_usd: 0,
            },
            document_coverage_bps: rate,
            failure_diagnostics: Vec::new(),
            extraction_receipt_refs: Vec::new(),
            coverage_snapshot: None,
            failure_comparisons: Vec::new(),
            metadata: Metadata::new(),
        }
    }

    fn request() -> ModelRequest {
        ModelRequest::new(
            "req.legal.provider.1",
            vec![
                ModelMessage::new(ModelMessageRole::System, "Use only allowed tools."),
                ModelMessage::new(ModelMessageRole::User, "Draft a short memo."),
            ],
        )
    }

    #[test]
    fn reqwest_transport_refuses_unresolved_secret_headers() {
        let mut transport = ReqwestBlockingHttpTransport::default();
        let mut headers = BTreeMap::new();
        headers.insert(
            String::from("authorization"),
            String::from("Bearer <secret_ref:legal-benchmark-local>"),
        );
        let error = transport
            .send(ProviderHttpRequest {
                method: String::from("POST"),
                url: String::from("http://127.0.0.1:1/v1/chat/completions"),
                headers,
                body: json!({"model": "local"}),
                timeout_ms: 1,
            })
            .expect_err("unresolved secret should fail before network");

        assert_eq!(error.kind, ModelAdapterFailureKind::TransportError);
        assert!(
            error
                .detail
                .contains("refuses unresolved secret references")
        );
    }

    #[test]
    fn legal_tool_specs_cover_the_closed_tool_surface() {
        let specs = legal_benchmark_model_tool_specs();
        let names = specs
            .iter()
            .map(|spec| spec.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "shell",
                "read",
                "write",
                "edit",
                "glob",
                "grep",
                "inventory",
                "email_summary",
                "spreadsheet_summary",
                "pdf_search",
                "evidence_table",
                "validate_deliverables"
            ]
        );
        assert!(specs.iter().all(|spec| spec.input_schema.is_object()));
    }

    #[test]
    fn openai_adapter_normalizes_tool_calls_and_usage() {
        let route = ModelProviderRoute::openai_compatible(
            "openai.local",
            "http://127.0.0.1:8000/v1",
            "psionic-local",
            Some(String::from("secret.openai.local")),
        );
        let transport = MockHttpTransport::new(vec![Ok(ProviderHttpResponse {
            status: 200,
            headers: BTreeMap::new(),
            body: json!({
                "id": "chatcmpl_1",
                "choices": [{
                    "finish_reason": "tool_calls",
                    "message": {
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [{
                            "id": "call_read",
                            "type": "function",
                            "function": {
                                "name": "read",
                                "arguments": "{\"root\":\"documents\",\"relative_path\":\"case.txt\",\"prefer_extracted\":true}"
                            }
                        }]
                    }
                }],
                "usage": {
                    "prompt_tokens": 11,
                    "completion_tokens": 5,
                    "total_tokens": 16,
                    "prompt_tokens_details": {"cached_tokens": 3}
                }
            }),
            elapsed_ms: 42,
        })]);
        let mut adapter =
            OpenAiCompatibleAdapter::new(route, transport, ModelRetryPolicy::default());
        let response = adapter.complete(&request()).expect("openai response");
        assert_eq!(response.stop_reason, ModelStopReason::ToolCalls);
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].tool_name, "read");
        assert_eq!(response.usage.input_tokens, 11);
        assert_eq!(response.usage.cached_input_tokens, 3);

        let transport = adapter.into_transport();
        let sent = &transport.requests[0];
        assert_eq!(sent.url, "http://127.0.0.1:8000/v1/chat/completions");
        assert_eq!(
            sent.headers.get("authorization").map(String::as_str),
            Some("Bearer <secret_ref:secret.openai.local>")
        );
        assert!(!sent.body.to_string().contains("sk-"));
    }

    #[test]
    fn google_vertex_gemini_adapter_uses_generate_content_shape() {
        let mut route = ModelProviderRoute::google_vertex_gemini(
            "google.gemini3.flash",
            "openagentsgemini",
            "global",
            "gemini-3-flash-preview",
            Some(String::from("secret.google.vertex.adc")),
        );
        route.metadata.insert(
            String::from("thinking_level"),
            Value::String(String::from("HIGH")),
        );
        let transport = MockHttpTransport::new(vec![Ok(ProviderHttpResponse {
            status: 200,
            headers: BTreeMap::new(),
            body: json!({
                "responseId": "gemini-response-1",
                "candidates": [{
                    "finishReason": "STOP",
                    "content": {
                        "role": "model",
                        "parts": [{
                            "functionCall": {
                                "name": "read",
                                "args": {
                                    "root": "documents",
                                    "relative_path": "case.txt",
                                    "prefer_extracted": true
                                }
                            }
                        }]
                    }
                }],
                "usageMetadata": {
                    "promptTokenCount": 31,
                    "candidatesTokenCount": 9,
                    "totalTokenCount": 40,
                    "cachedContentTokenCount": 5
                }
            }),
            elapsed_ms: 64,
        })]);
        let mut adapter =
            GoogleVertexGeminiAdapter::new(route, transport, ModelRetryPolicy::default());
        let response = adapter.complete(&request()).expect("gemini response");
        assert_eq!(
            response.provider_family,
            ModelProviderFamily::GoogleVertexGemini
        );
        assert_eq!(response.stop_reason, ModelStopReason::ToolCalls);
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].tool_name, "read");
        assert_eq!(
            response.tool_calls[0].arguments["relative_path"],
            Value::String(String::from("case.txt"))
        );
        assert_eq!(response.usage.input_tokens, 31);
        assert_eq!(response.usage.cached_input_tokens, 5);

        let transport = adapter.into_transport();
        let sent = &transport.requests[0];
        assert_eq!(
            sent.url,
            "https://aiplatform.googleapis.com/v1/projects/openagentsgemini/locations/global/publishers/google/models/gemini-3-flash-preview:generateContent"
        );
        assert_eq!(
            sent.headers.get("authorization").map(String::as_str),
            Some("Bearer <secret_ref:secret.google.vertex.adc>")
        );
        assert_eq!(
            sent.headers.get("x-goog-user-project").map(String::as_str),
            Some("openagentsgemini")
        );
        assert_eq!(
            sent.body["systemInstruction"]["parts"][0]["text"],
            Value::String(String::from("Use only allowed tools."))
        );
        assert!(sent.body["tools"][0]["functionDeclarations"].is_array());
        assert_eq!(
            sent.body["toolConfig"]["functionCallingConfig"]["mode"],
            Value::String(String::from("AUTO"))
        );
        assert_eq!(
            sent.body["generationConfig"]["thinkingConfig"]["thinkingLevel"],
            Value::String(String::from("HIGH"))
        );
        assert!(!sent.body.to_string().contains("sk-"));
    }

    #[test]
    fn qwen_tuned_candidate_route_carries_adapter_receipt_metadata() {
        let candidate = qwen_tuned_candidate();
        let route = candidate
            .openai_compatible_route(
                "qwen.legal.tuned",
                "http://127.0.0.1:8000/v1",
                Some(String::from("secret.psionic.local")),
            )
            .expect("route");
        let transport = MockHttpTransport::new(vec![Ok(ProviderHttpResponse {
            status: 200,
            headers: BTreeMap::new(),
            body: json!({
                "id": "chatcmpl_qwen_tuned",
                "choices": [{
                    "finish_reason": "stop",
                    "message": {"role": "assistant", "content": "{\"action\":\"submit\",\"deliverables\":[\"memo.md\"]}"}
                }],
                "usage": {"prompt_tokens": 13, "completion_tokens": 8, "total_tokens": 21}
            }),
            elapsed_ms: 25,
        })]);
        let mut adapter =
            OpenAiCompatibleAdapter::new(route.clone(), transport, ModelRetryPolicy::default());
        let response = adapter.complete(&request()).expect("response");
        assert_eq!(response.model_id, "qwen3.5-4b-legal-r1");
        assert_eq!(
            response.metadata.get("adapter_artifact_digest"),
            Some(&Value::String(String::from(
                "sha256:qwen35-legal-adapter-artifact"
            )))
        );
        assert_eq!(
            response.metadata.get("base_served_artifact_digest"),
            Some(&Value::String(String::from("sha256:qwen35-4b-base")))
        );
        assert!(
            response
                .metadata
                .get("legal_candidate_identity_digest")
                .and_then(Value::as_str)
                .is_some()
        );
        let sent = &adapter.into_transport().requests[0];
        assert_eq!(
            sent.body["model"],
            Value::String(String::from("qwen3.5-4b-legal-r1"))
        );
        assert_eq!(
            sent.headers.get("x-psionic-adapter-artifact-digest"),
            Some(&String::from("sha256:qwen35-legal-adapter-artifact"))
        );
    }

    #[test]
    fn qwen_candidate_pair_refuses_template_drift() {
        let base = qwen_base_candidate();
        let mut tuned = qwen_tuned_candidate();
        tuned.prompt_template_digest = String::from("sha256:wrong-template");
        let error = ensure_qwen_candidate_pair_compatible(&base, &tuned)
            .expect_err("template drift must refuse");
        assert_eq!(error.kind, ModelAdapterFailureKind::InvalidRequest);
        assert!(error.detail.contains("prompt_template_digest"));
    }

    #[test]
    fn qwen_score_bundle_separates_mock_smoke_from_retained_claim() {
        let base = qwen_base_candidate();
        let mock =
            QwenLegalBasePlusAdapterCandidateIdentity::mock_local_smoke(qwen_tuned_candidate())
                .expect("mock");
        let bundle = qwen_legal_base_vs_adapter_score_bundle(
            "bundle.qwen.mock",
            &base,
            &score_report("score.base", 8_000),
            Some(&mock),
            Some(&score_report("score.mock", 8_500)),
        )
        .expect("bundle");
        assert!(bundle.mock_local_smoke_score.is_some());
        assert!(bundle.tuned_adapter_score.is_none());
        assert!(bundle.retained_score_claim.is_none());
        assert_eq!(
            bundle.bundle_digest,
            bundle.stable_digest().expect("digest")
        );

        let mut invalid_mock = mock;
        invalid_mock.score_claim_kind = LegalBenchmarkScoreClaimKind::RetainedScoreClaim;
        let error = invalid_mock
            .validate()
            .expect_err("mock retained claim must refuse");
        assert!(error.detail.contains("mock/local smoke"));
    }

    #[test]
    fn anthropic_adapter_normalizes_tool_calls_and_usage() {
        let route = ModelProviderRoute::anthropic(
            "anthropic.hosted",
            "https://api.anthropic.com/v1",
            "claude-sonnet-legal",
            Some(String::from("secret.anthropic.prod")),
        );
        let transport = MockHttpTransport::new(vec![Ok(ProviderHttpResponse {
            status: 200,
            headers: BTreeMap::new(),
            body: json!({
                "id": "msg_1",
                "type": "message",
                "role": "assistant",
                "stop_reason": "tool_use",
                "content": [{
                    "type": "tool_use",
                    "id": "toolu_1",
                    "name": "grep",
                    "input": {
                        "root": "documents",
                        "pattern": "indemnity",
                        "case_sensitive": false,
                        "max_results": 20,
                        "include_hidden": false
                    }
                }],
                "usage": {
                    "input_tokens": 21,
                    "output_tokens": 7,
                    "cache_read_input_tokens": 4
                }
            }),
            elapsed_ms: 77,
        })]);
        let mut adapter = AnthropicAdapter::new(route, transport, ModelRetryPolicy::default());
        let response = adapter.complete(&request()).expect("anthropic response");
        assert_eq!(response.stop_reason, ModelStopReason::ToolCalls);
        assert_eq!(response.tool_calls[0].tool_name, "grep");
        assert_eq!(
            response.tool_calls[0].arguments["pattern"],
            Value::String(String::from("indemnity"))
        );
        assert_eq!(response.usage.total_tokens, 28);

        let transport = adapter.into_transport();
        let sent = &transport.requests[0];
        assert_eq!(sent.url, "https://api.anthropic.com/v1/messages");
        assert_eq!(
            sent.headers.get("x-api-key").map(String::as_str),
            Some("<secret_ref:secret.anthropic.prod>")
        );
        assert!(sent.body["system"].as_str().is_some());
        assert!(!sent.body.to_string().contains("sk-"));
    }

    #[test]
    fn retryable_rate_limit_is_retried_and_counted() {
        let route = ModelProviderRoute::openai_compatible(
            "openai.retry",
            "http://127.0.0.1:8000/v1",
            "retry-model",
            None,
        );
        let transport = MockHttpTransport::new(vec![
            Ok(ProviderHttpResponse {
                status: 429,
                headers: BTreeMap::new(),
                body: json!({"error": {"message": "rate limited"}}),
                elapsed_ms: 5,
            }),
            Ok(ProviderHttpResponse {
                status: 200,
                headers: BTreeMap::new(),
                body: json!({
                    "id": "chatcmpl_2",
                    "choices": [{
                        "finish_reason": "stop",
                        "message": {"role": "assistant", "content": "Done."}
                    }],
                    "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
                }),
                elapsed_ms: 9,
            }),
        ]);
        let mut adapter =
            OpenAiCompatibleAdapter::new(route, transport, ModelRetryPolicy::default());
        let response = adapter.complete(&request()).expect("retried response");
        assert_eq!(response.retry_count, 1);
        assert_eq!(response.final_text.as_deref(), Some("Done."));
        assert_eq!(adapter.into_transport().requests.len(), 2);
    }

    #[test]
    fn context_overflow_is_structured_provider_error() {
        let route = ModelProviderRoute::openai_compatible(
            "openai.context",
            "http://127.0.0.1:8000/v1",
            "small-context",
            None,
        );
        let transport = MockHttpTransport::new(vec![Ok(ProviderHttpResponse {
            status: 400,
            headers: BTreeMap::new(),
            body: json!({"error": {"message": "context length exceeded"}}),
            elapsed_ms: 3,
        })]);
        let mut adapter =
            OpenAiCompatibleAdapter::new(route, transport, ModelRetryPolicy::default());
        let err = adapter.complete(&request()).expect_err("context overflow");
        assert_eq!(err.kind, ModelAdapterFailureKind::ContextOverflow);
        assert_eq!(err.status_code, Some(400));
        assert!(!err.retryable);
        assert!(err.raw_response_hash.is_some());
    }

    #[test]
    fn qwen_promoted_route_registers_stable_psionic_model_id() {
        let registration =
            register_qwen_legal_promoted_provider_route(qwen_legal_promoted_smoke_route_spec())
                .expect("registration");
        assert_eq!(registration.served_model_id, "qwen36-legal-grpo-001");
        assert_eq!(registration.route.model_id, "qwen36-legal-grpo-001");
        assert_eq!(registration.route.family, ModelProviderFamily::PsionicCompatible);
        assert_eq!(
            registration.route.endpoint_url(),
            "https://psionic.local/v1/chat/completions"
        );
        assert_eq!(
            registration
                .blueprint_route_metadata
                .get("promotion_id")
                .and_then(Value::as_str),
            Some("qwen.legal.full_artifact_promotion.001")
        );
    }

    #[test]
    fn qwen_promoted_route_receipt_matches_promoted_hashes() {
        let spec = qwen_legal_promoted_smoke_route_spec();
        let registration =
            register_qwen_legal_promoted_provider_route(spec.clone()).expect("registration");
        assert_eq!(
            registration.serving_receipt.base_model_hash,
            spec.artifacts.base_model_hash
        );
        assert_eq!(
            registration.serving_receipt.adapter_artifact_hash,
            spec.artifacts.adapter_artifact_hash
        );
        assert_eq!(
            registration.serving_receipt.checkpoint_artifact_hash,
            spec.artifacts.checkpoint_artifact_hash
        );
        assert_eq!(
            registration.serving_receipt.corpus_manifest_hash,
            spec.artifacts.corpus_manifest_hash
        );
        assert!(digest_is_complete(
            registration.serving_receipt.receipt_hash.as_str()
        ));
        assert_eq!(
            registration.route.redacted_headers.get("x-psionic-qwen-adapter-hash"),
            Some(&spec.artifacts.adapter_artifact_hash)
        );
    }

    #[test]
    fn qwen_promoted_route_documents_canary_and_rollback_commands() {
        let registration =
            register_qwen_legal_promoted_provider_route(qwen_legal_promoted_smoke_route_spec())
                .expect("registration");
        assert_eq!(
            registration.canary_receipt.command_kind,
            QwenLegalPromotedRouteTrafficMode::Canary
        );
        assert_eq!(registration.canary_receipt.traffic_bps, 500);
        assert!(
            registration
                .canary_receipt
                .command
                .contains(&String::from("qwen36-legal-grpo-001"))
        );
        assert_eq!(
            registration.rollback_receipt.command_kind,
            QwenLegalPromotedRouteTrafficMode::Rollback
        );
        assert_eq!(
            registration
                .rollback_receipt
                .rollback_served_model_id
                .as_deref(),
            Some("qwen36-legal-prior-champion-000")
        );
    }

    #[test]
    fn qwen_promoted_route_refuses_missing_adapter_before_generation() {
        let mut spec = qwen_legal_promoted_smoke_route_spec();
        spec.artifacts.adapter_artifact_present = false;
        let error = register_qwen_legal_promoted_provider_route(spec)
            .expect_err("missing adapter should refuse");
        assert_eq!(error.kind, ModelAdapterFailureKind::InvalidRequest);
        assert!(error.detail.contains("MissingAdapterArtifact"));
    }

    #[test]
    fn qwen_promoted_route_refuses_base_hash_mismatch_before_generation() {
        let mut spec = qwen_legal_promoted_smoke_route_spec();
        spec.artifacts.loaded_base_model_hash = digest("different-base");
        let error = register_qwen_legal_promoted_provider_route(spec)
            .expect_err("base hash mismatch should refuse");
        assert_eq!(error.kind, ModelAdapterFailureKind::InvalidRequest);
        assert!(error.detail.contains("BaseHashMismatch"));
    }

    #[test]
    fn qwen_promoted_route_keeps_retired_provider_as_explicit_fallback_only() {
        let registration =
            register_qwen_legal_promoted_provider_route(qwen_legal_promoted_smoke_route_spec())
                .expect("registration");
        assert_eq!(registration.fallback_routes.len(), 1);
        assert!(registration.fallback_routes[0].explicit_fallback_only);
        assert_eq!(
            registration.fallback_routes[0].route.family,
            ModelProviderFamily::OpenAiCompatible
        );

        let mut spec = qwen_legal_promoted_smoke_route_spec();
        spec.fallback_routes[0].explicit_fallback_only = false;
        let error = register_qwen_legal_promoted_provider_route(spec)
            .expect_err("fallback must be explicit");
        assert!(error.detail.contains("FallbackNotExplicit"));
    }

    #[test]
    fn mock_adapter_supports_ci_without_live_keys() {
        let route = ModelProviderRoute::mock("mock.ci", "deterministic-legal-model");
        let response = ModelResponse {
            schema_version: LEGAL_BENCHMARK_PROVIDER_SCHEMA_VERSION,
            response_id: String::from("mock.response.1"),
            request_id: String::from("req.legal.provider.1"),
            route_id: route.route_id.clone(),
            provider_family: route.family,
            model_id: route.model_id.clone(),
            model_config_hash: String::from("mock-config-hash"),
            secret_reference_id: None,
            final_text: Some(String::from("submit")),
            tool_calls: Vec::new(),
            stop_reason: ModelStopReason::Stop,
            usage: ModelUsage {
                input_tokens: 10,
                output_tokens: 1,
                total_tokens: 11,
                cached_input_tokens: 0,
                estimated_cost_micro_usd: 0,
            },
            elapsed_ms: 1,
            retry_count: 0,
            raw_response_hash: String::from("mock-raw-hash"),
            created_at_ms: 1,
            metadata: Metadata::new(),
        };
        let mut expected = response.clone();
        expected.metadata.insert(
            String::from("route_id"),
            Value::String(route.route_id.clone()),
        );
        expected.metadata.insert(
            String::from("route_model_id"),
            Value::String(route.model_id.clone()),
        );
        let mut adapter = MockModelAdapter::new(route, vec![Ok(response)]);
        assert_eq!(
            adapter.complete(&request()).expect("mock response"),
            expected
        );
    }
}
