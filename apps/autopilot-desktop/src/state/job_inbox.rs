use crate::app_state::PaneLoadState;

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
pub struct JobInboxRequest {
    pub request_id: String,
    pub requester: String,
    pub capability: String,
    pub skill_scope_id: Option<String>,
    pub skl_manifest_a: Option<String>,
    pub skl_manifest_event_id: Option<String>,
    pub sa_tick_request_event_id: Option<String>,
    pub sa_tick_result_event_id: Option<String>,
    pub ac_envelope_event_id: Option<String>,
    pub price_sats: u64,
    pub ttl_seconds: u64,
    pub validation: JobInboxValidation,
    pub arrival_seq: u64,
    pub decision: JobInboxDecision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobInboxNetworkRequest {
    pub request_id: String,
    pub requester: String,
    pub capability: String,
    pub skill_scope_id: Option<String>,
    pub skl_manifest_a: Option<String>,
    pub skl_manifest_event_id: Option<String>,
    pub sa_tick_request_event_id: Option<String>,
    pub sa_tick_result_event_id: Option<String>,
    pub ac_envelope_event_id: Option<String>,
    pub price_sats: u64,
    pub ttl_seconds: u64,
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
    pub fn upsert_network_request(&mut self, request: JobInboxNetworkRequest) {
        if let Some(existing) = self
            .requests
            .iter_mut()
            .find(|existing| existing.request_id == request.request_id)
        {
            existing.requester = request.requester;
            existing.capability = request.capability;
            existing.skill_scope_id = request.skill_scope_id;
            existing.skl_manifest_a = request.skl_manifest_a;
            existing.skl_manifest_event_id = request.skl_manifest_event_id;
            existing.sa_tick_request_event_id = request.sa_tick_request_event_id;
            existing.sa_tick_result_event_id = request.sa_tick_result_event_id;
            existing.ac_envelope_event_id = request.ac_envelope_event_id;
            existing.price_sats = request.price_sats;
            existing.ttl_seconds = request.ttl_seconds;
            existing.validation = request.validation;
            return;
        }

        let arrival_seq = self.next_arrival_seq;
        self.next_arrival_seq = self.next_arrival_seq.saturating_add(1);
        self.requests.push(JobInboxRequest {
            request_id: request.request_id,
            requester: request.requester,
            capability: request.capability,
            skill_scope_id: request.skill_scope_id,
            skl_manifest_a: request.skl_manifest_a,
            skl_manifest_event_id: request.skl_manifest_event_id,
            sa_tick_request_event_id: request.sa_tick_request_event_id,
            sa_tick_result_event_id: request.sa_tick_result_event_id,
            ac_envelope_event_id: request.ac_envelope_event_id,
            price_sats: request.price_sats,
            ttl_seconds: request.ttl_seconds,
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

    pub fn decide_selected(&mut self, accepted: bool, reason: &str) -> Result<String, String> {
        let selected_id = self
            .selected_request_id
            .as_deref()
            .ok_or_else(|| "Select a request first".to_string())?
            .to_string();
        let Some(request) = self
            .requests
            .iter_mut()
            .find(|request| request.request_id == selected_id)
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
}
