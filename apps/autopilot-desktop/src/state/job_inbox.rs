use crate::app_state::PaneLoadState;
use crate::state::provider_runtime::ProviderMode;

pub const TARGETED_SAFE_AUTO_MIN_FRESHNESS_SECONDS: u64 = 30;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobDemandSource {
    OpenNetwork,
    StarterDemand,
}

impl JobDemandSource {
    pub const fn label(self) -> &'static str {
        match self {
            Self::OpenNetwork => "open-network",
            Self::StarterDemand => "starter-demand",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobDemandRiskClass {
    StarterDemand,
    TargetedOpenNetwork,
    SpeculativeOpenNetwork,
    TargetedMismatch,
}

impl JobDemandRiskClass {
    pub const fn label(self) -> &'static str {
        match self {
            Self::StarterDemand => "starter-demand",
            Self::TargetedOpenNetwork => "targeted-open-network",
            Self::SpeculativeOpenNetwork => "speculative-open-network",
            Self::TargetedMismatch => "targeted-mismatch",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobDemandRiskDisposition {
    AutoAcceptSafe,
    ManualReviewOnly,
    RejectByDefault,
}

impl JobDemandRiskDisposition {
    pub const fn label(self) -> &'static str {
        match self {
            Self::AutoAcceptSafe => "safe-auto",
            Self::ManualReviewOnly => "manual-only",
            Self::RejectByDefault => "reject-by-default",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobDemandRiskAssessment {
    pub class: JobDemandRiskClass,
    pub disposition: JobDemandRiskDisposition,
    pub note: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobInboxValidation {
    Valid,
    Pending,
    Invalid(String),
}

impl JobInboxValidation {
    pub fn label(&self) -> String {
        match self {
            Self::Valid => "valid".to_string(),
            Self::Pending => "pending".to_string(),
            Self::Invalid(reason) => format!("invalid ({reason})"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JobInboxDecision {
    Pending,
    Accepted { reason: String },
    Rejected { reason: String },
}

impl JobInboxDecision {
    pub fn label(&self) -> String {
        match self {
            Self::Pending => "pending".to_string(),
            Self::Accepted { reason } => format!("accepted ({reason})"),
            Self::Rejected { reason } => format!("rejected ({reason})"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobExecutionParam {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobInboxRequest {
    pub request_id: String,
    pub requester: String,
    pub demand_source: JobDemandSource,
    pub request_kind: u16,
    pub capability: String,
    pub execution_input: Option<String>,
    pub execution_prompt: Option<String>,
    pub execution_params: Vec<JobExecutionParam>,
    pub requested_model: Option<String>,
    pub requested_output_mime: Option<String>,
    pub target_provider_pubkeys: Vec<String>,
    pub skill_scope_id: Option<String>,
    pub skl_manifest_a: Option<String>,
    pub skl_manifest_event_id: Option<String>,
    pub sa_tick_request_event_id: Option<String>,
    pub sa_tick_result_event_id: Option<String>,
    pub ac_envelope_event_id: Option<String>,
    pub price_sats: u64,
    pub ttl_seconds: u64,
    pub created_at_epoch_seconds: Option<u64>,
    pub expires_at_epoch_seconds: Option<u64>,
    pub validation: JobInboxValidation,
    pub arrival_seq: u64,
    pub decision: JobInboxDecision,
}

impl JobInboxRequest {
    pub const fn preview_only(&self, provider_mode: ProviderMode) -> bool {
        matches!(provider_mode, ProviderMode::Offline)
    }

    pub fn is_targeted(&self) -> bool {
        !self.target_provider_pubkeys.is_empty()
    }

    pub fn demand_risk_assessment(&self) -> JobDemandRiskAssessment {
        match self.demand_source {
            JobDemandSource::StarterDemand => JobDemandRiskAssessment {
                class: JobDemandRiskClass::StarterDemand,
                disposition: JobDemandRiskDisposition::AutoAcceptSafe,
                note: "hosted starter demand is trusted bootstrap work".to_string(),
            },
            JobDemandSource::OpenNetwork if self.is_targeted() => JobDemandRiskAssessment {
                class: JobDemandRiskClass::TargetedOpenNetwork,
                disposition: JobDemandRiskDisposition::AutoAcceptSafe,
                note: "targeted open-network demand explicitly names this provider".to_string(),
            },
            JobDemandSource::OpenNetwork => JobDemandRiskAssessment {
                class: JobDemandRiskClass::SpeculativeOpenNetwork,
                disposition: JobDemandRiskDisposition::ManualReviewOnly,
                note: "untargeted open-network demand stays visible but requires manual review"
                    .to_string(),
            },
        }
    }

    pub fn demand_risk_assessment_at(&self, now_epoch_seconds: u64) -> JobDemandRiskAssessment {
        let baseline = self.demand_risk_assessment();
        if self.demand_source != JobDemandSource::OpenNetwork || !self.is_targeted() {
            return baseline;
        }

        let Some(created_at) = self.created_at_epoch_seconds else {
            return JobDemandRiskAssessment {
                class: JobDemandRiskClass::TargetedOpenNetwork,
                disposition: JobDemandRiskDisposition::ManualReviewOnly,
                note:
                    "targeted open-network demand names this provider but lacks freshness metadata"
                        .to_string(),
            };
        };
        let Some(expires_at) = self.expires_at_epoch_seconds else {
            return JobDemandRiskAssessment {
                class: JobDemandRiskClass::TargetedOpenNetwork,
                disposition: JobDemandRiskDisposition::ManualReviewOnly,
                note:
                    "targeted open-network demand names this provider but lacks freshness metadata"
                        .to_string(),
            };
        };
        if expires_at <= created_at {
            return JobDemandRiskAssessment {
                class: JobDemandRiskClass::TargetedOpenNetwork,
                disposition: JobDemandRiskDisposition::RejectByDefault,
                note: "targeted open-network demand has invalid freshness metadata".to_string(),
            };
        }
        if now_epoch_seconds >= expires_at {
            return JobDemandRiskAssessment {
                class: JobDemandRiskClass::TargetedOpenNetwork,
                disposition: JobDemandRiskDisposition::RejectByDefault,
                note: format!(
                    "targeted open-network demand expired {}s ago",
                    now_epoch_seconds.saturating_sub(expires_at)
                ),
            };
        }

        let freshness_remaining_seconds = expires_at.saturating_sub(now_epoch_seconds);
        if freshness_remaining_seconds < TARGETED_SAFE_AUTO_MIN_FRESHNESS_SECONDS {
            return JobDemandRiskAssessment {
                class: JobDemandRiskClass::TargetedOpenNetwork,
                disposition: JobDemandRiskDisposition::ManualReviewOnly,
                note: format!(
                    "targeted open-network demand names this provider but only has {}s remaining",
                    freshness_remaining_seconds
                ),
            };
        }

        JobDemandRiskAssessment {
            class: JobDemandRiskClass::TargetedOpenNetwork,
            disposition: JobDemandRiskDisposition::AutoAcceptSafe,
            note: format!(
                "targeted open-network demand explicitly names this provider and remains fresh for {}s",
                freshness_remaining_seconds
            ),
        }
    }

    pub const fn eligibility_label(&self, provider_mode: ProviderMode) -> &'static str {
        if self.preview_only(provider_mode) {
            "preview-only"
        } else {
            "claimable"
        }
    }

    pub fn is_expired_at(&self, now_epoch_seconds: u64) -> bool {
        self.expires_at_epoch_seconds
            .is_some_and(|expires_at| now_epoch_seconds >= expires_at)
    }

    pub fn expires_in_seconds(&self, now_epoch_seconds: u64) -> Option<u64> {
        self.expires_at_epoch_seconds
            .map(|expires_at| expires_at.saturating_sub(now_epoch_seconds))
    }

    pub fn expired_for_seconds(&self, now_epoch_seconds: u64) -> Option<u64> {
        self.expires_at_epoch_seconds
            .filter(|expires_at| now_epoch_seconds >= *expires_at)
            .map(|expires_at| now_epoch_seconds.saturating_sub(expires_at))
    }
}

pub(crate) fn normalize_provider_keys(values: &[String]) -> Vec<String> {
    let mut normalized = values
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

pub(crate) fn local_provider_keys(identity: &nostr::NostrIdentity) -> Vec<String> {
    normalize_provider_keys(&[identity.npub.clone(), identity.public_key_hex.clone()])
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobInboxNetworkRequest {
    pub request_id: String,
    pub requester: String,
    pub demand_source: JobDemandSource,
    pub request_kind: u16,
    pub capability: String,
    pub execution_input: Option<String>,
    pub execution_prompt: Option<String>,
    pub execution_params: Vec<JobExecutionParam>,
    pub requested_model: Option<String>,
    pub requested_output_mime: Option<String>,
    pub target_provider_pubkeys: Vec<String>,
    pub encrypted: bool,
    pub encrypted_payload: Option<String>,
    pub parsed_event_shape: Option<String>,
    pub raw_event_json: Option<String>,
    pub skill_scope_id: Option<String>,
    pub skl_manifest_a: Option<String>,
    pub skl_manifest_event_id: Option<String>,
    pub sa_tick_request_event_id: Option<String>,
    pub sa_tick_result_event_id: Option<String>,
    pub ac_envelope_event_id: Option<String>,
    pub price_sats: u64,
    pub ttl_seconds: u64,
    pub created_at_epoch_seconds: Option<u64>,
    pub expires_at_epoch_seconds: Option<u64>,
    pub validation: JobInboxValidation,
}

pub struct JobInboxState {
    pub load_state: PaneLoadState,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub requests: Vec<JobInboxRequest>,
    pub selected_request_id: Option<String>,
    next_arrival_seq: u64,
}

impl Default for JobInboxState {
    fn default() -> Self {
        Self {
            load_state: PaneLoadState::Loading,
            last_error: None,
            last_action: Some("Waiting for inbox lane snapshot".to_string()),
            requests: Vec::new(),
            selected_request_id: None,
            next_arrival_seq: 1,
        }
    }
}

impl JobInboxState {
    pub const fn preview_block_reason(&self, provider_mode: ProviderMode) -> Option<&'static str> {
        if matches!(provider_mode, ProviderMode::Offline) {
            Some("Preview only while offline. Click Go Online to claim jobs.")
        } else {
            None
        }
    }

    pub fn upsert_network_request(&mut self, request: JobInboxNetworkRequest) {
        if let Some(existing) = self
            .requests
            .iter_mut()
            .find(|existing| existing.request_id == request.request_id)
        {
            existing.requester = request.requester;
            existing.demand_source = request.demand_source;
            existing.request_kind = request.request_kind;
            existing.capability = request.capability;
            existing.execution_input = request.execution_input;
            existing.execution_prompt = request.execution_prompt;
            existing.execution_params = request.execution_params;
            existing.requested_model = request.requested_model;
            existing.requested_output_mime = request.requested_output_mime;
            existing.target_provider_pubkeys = request.target_provider_pubkeys;
            existing.skill_scope_id = request.skill_scope_id;
            existing.skl_manifest_a = request.skl_manifest_a;
            existing.skl_manifest_event_id = request.skl_manifest_event_id;
            existing.sa_tick_request_event_id = request.sa_tick_request_event_id;
            existing.sa_tick_result_event_id = request.sa_tick_result_event_id;
            existing.ac_envelope_event_id = request.ac_envelope_event_id;
            existing.price_sats = request.price_sats;
            existing.ttl_seconds = request.ttl_seconds;
            existing.created_at_epoch_seconds = request.created_at_epoch_seconds;
            existing.expires_at_epoch_seconds = request.expires_at_epoch_seconds;
            existing.validation = request.validation;
            return;
        }

        let arrival_seq = self.next_arrival_seq;
        self.next_arrival_seq = self.next_arrival_seq.saturating_add(1);
        self.requests.push(JobInboxRequest {
            request_id: request.request_id,
            requester: request.requester,
            demand_source: request.demand_source,
            request_kind: request.request_kind,
            capability: request.capability,
            execution_input: request.execution_input,
            execution_prompt: request.execution_prompt,
            execution_params: request.execution_params,
            requested_model: request.requested_model,
            requested_output_mime: request.requested_output_mime,
            target_provider_pubkeys: request.target_provider_pubkeys,
            skill_scope_id: request.skill_scope_id,
            skl_manifest_a: request.skl_manifest_a,
            skl_manifest_event_id: request.skl_manifest_event_id,
            sa_tick_request_event_id: request.sa_tick_request_event_id,
            sa_tick_result_event_id: request.sa_tick_result_event_id,
            ac_envelope_event_id: request.ac_envelope_event_id,
            price_sats: request.price_sats,
            ttl_seconds: request.ttl_seconds,
            created_at_epoch_seconds: request.created_at_epoch_seconds,
            expires_at_epoch_seconds: request.expires_at_epoch_seconds,
            validation: request.validation,
            arrival_seq,
            decision: JobInboxDecision::Pending,
        });
        self.requests.sort_by_key(|request| request.arrival_seq);
    }

    pub fn select_by_index(&mut self, index: usize) -> bool {
        let Some(request_id) = self
            .requests
            .get(index)
            .map(|request| request.request_id.clone())
        else {
            return false;
        };
        self.selected_request_id = Some(request_id);
        self.last_error = None;
        true
    }

    pub fn selected_request(&self) -> Option<&JobInboxRequest> {
        let selected_id = self.selected_request_id.as_deref()?;
        self.requests
            .iter()
            .find(|request| request.request_id == selected_id)
    }

    pub fn decide_request(
        &mut self,
        request_id: &str,
        accepted: bool,
        reason: &str,
    ) -> Result<String, String> {
        let Some(request) = self
            .requests
            .iter_mut()
            .find(|request| request.request_id == request_id)
        else {
            return Err("Selected request no longer exists".to_string());
        };

        let decision_reason = reason.trim().to_string();
        request.decision = if accepted {
            JobInboxDecision::Accepted {
                reason: decision_reason.clone(),
            }
        } else {
            JobInboxDecision::Rejected {
                reason: decision_reason.clone(),
            }
        };
        self.last_error = None;
        self.last_action = Some(if accepted {
            format!("Accepted {} ({decision_reason})", request.request_id)
        } else {
            format!("Rejected {} ({decision_reason})", request.request_id)
        });
        Ok(request.request_id.clone())
    }

    pub fn decide_selected(&mut self, accepted: bool, reason: &str) -> Result<String, String> {
        let selected_id = self
            .selected_request_id
            .as_deref()
            .ok_or_else(|| "Select a request first".to_string())?
            .to_string();
        self.decide_request(selected_id.as_str(), accepted, reason)
    }

    pub fn remove_requests_by_demand_source(
        &mut self,
        demand_source: JobDemandSource,
        keep_request_id: Option<&str>,
    ) -> usize {
        let original_len = self.requests.len();
        self.requests.retain(|request| {
            request.demand_source != demand_source
                || keep_request_id.is_some_and(|keep| request.request_id == keep)
        });
        if let Some(selected_id) = self.selected_request_id.as_deref()
            && !self
                .requests
                .iter()
                .any(|request| request.request_id == selected_id)
        {
            self.selected_request_id = self
                .requests
                .first()
                .map(|request| request.request_id.clone());
        }
        original_len.saturating_sub(self.requests.len())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        JobDemandSource, JobInboxDecision, JobInboxRequest, JobInboxState, JobInboxValidation,
    };
    use crate::app_state::PaneLoadState;
    use crate::state::provider_runtime::ProviderMode;

    fn fixture_request() -> JobInboxRequest {
        JobInboxRequest {
            request_id: "req-preview".to_string(),
            requester: "buyer".to_string(),
            demand_source: JobDemandSource::OpenNetwork,
            request_kind: 5050,
            capability: "summarize.text".to_string(),
            execution_input: Some("Summarize the attached text payload.".to_string()),
            execution_prompt: Some("Summarize the attached text payload.".to_string()),
            execution_params: Vec::new(),
            requested_model: Some("llama3.2:latest".to_string()),
            requested_output_mime: Some("text/plain".to_string()),
            target_provider_pubkeys: Vec::new(),
            skill_scope_id: None,
            skl_manifest_a: None,
            skl_manifest_event_id: None,
            sa_tick_request_event_id: None,
            sa_tick_result_event_id: None,
            ac_envelope_event_id: None,
            price_sats: 42,
            ttl_seconds: 60,
            created_at_epoch_seconds: Some(1_760_000_000),
            expires_at_epoch_seconds: Some(1_760_000_060),
            validation: JobInboxValidation::Valid,
            arrival_seq: 1,
            decision: JobInboxDecision::Pending,
        }
    }

    #[test]
    fn request_eligibility_is_preview_only_while_provider_offline() {
        let request = fixture_request();
        assert!(request.preview_only(ProviderMode::Offline));
        assert_eq!(
            request.eligibility_label(ProviderMode::Offline),
            "preview-only"
        );
        assert_eq!(request.eligibility_label(ProviderMode::Online), "claimable");
    }

    #[test]
    fn inbox_reports_preview_block_reason_only_while_offline() {
        let inbox = JobInboxState {
            load_state: PaneLoadState::Ready,
            last_error: None,
            last_action: None,
            requests: vec![fixture_request()],
            selected_request_id: Some("req-preview".to_string()),
            next_arrival_seq: 2,
        };

        assert_eq!(
            inbox.preview_block_reason(ProviderMode::Offline),
            Some("Preview only while offline. Click Go Online to claim jobs.")
        );
        assert_eq!(inbox.preview_block_reason(ProviderMode::Online), None);
    }
}
