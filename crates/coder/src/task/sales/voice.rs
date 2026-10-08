//! Consented booked voice participation under human control (REV-75).
//! Disabled by default: a voice session exists only over an accepted
//! meeting the recipient requested, under a separately reviewed voice
//! authority, with a named human supervisor who starts, mutes, takes
//! over, and ends it. No outbound call can be constructed from here; the
//! session only speaks into a meeting the human already joined. Generated
//! speech and transcripts are untrusted data.
use super::*;
use std::collections::BTreeSet;

pub const AUTHORITY_SCHEMA: &str = "openagents.sales-voice-authority.v1";
const MAX_SESSIONS: usize = 64;
const MAX_TURNS: u32 = 200;
const MAX_AUTHORITY_SECS: u64 = 180 * 86_400;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Medium {
    WebMeeting,
    Telephone,
}

/// The owner's separately reviewed voice scope; recording stays off unless
/// `recording` names its own consent, recipients, and retention.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Authority {
    pub schema: String,
    pub version: u64,
    pub medium: Medium,
    pub written_workflow_reference: String,
    pub legal_review_reference: String,
    pub reviewer: String,
    pub ai_identity: String,
    pub permitted_assistance: Vec<String>,
    pub reviewed_at: u64,
    pub expires_at: u64,
    pub max_cost_usd_millionths: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording: Option<Recording>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Recording {
    pub consent_reference: String,
    pub recipients: Vec<String>,
    pub retain_secs: u64,
}
impl Authority {
    fn check(&self, now: u64) -> Result<()> {
        for s in [
            &self.written_workflow_reference,
            &self.legal_review_reference,
            &self.reviewer,
            &self.ai_identity,
        ] {
            text(s, 256)?;
            if s.trim().is_empty() {
                return Err("voice authority needs its references".into());
            }
        }
        for s in &self.permitted_assistance {
            text(s, 256)?;
        }
        if self.schema != AUTHORITY_SCHEMA
            || self.version == 0
            || self.medium != Medium::WebMeeting
            || self.permitted_assistance.is_empty()
            || self.permitted_assistance.len() > 8
            || self.reviewed_at > now
            || self.expires_at <= now
            || self.expires_at > self.reviewed_at.saturating_add(MAX_AUTHORITY_SECS)
            || self.max_cost_usd_millionths == 0
        {
            return Err(
                "voice authority needs a reviewed web-meeting scope, assistance list, expiry, and cost bound"
                    .into(),
            );
        }
        if let Some(r) = &self.recording {
            text(&r.consent_reference, 256)?;
            if r.consent_reference.trim().is_empty()
                || r.recipients.is_empty()
                || r.recipients.len() > 8
                || !(1..=RETENTION_MAX).contains(&r.retain_secs)
            {
                return Err("recording needs explicit consent, recipients, and retention".into());
            }
        }
        Ok(())
    }
    pub fn sha256(&self) -> Result<String> {
        Ok(digest(
            &serde_json::to_vec(self).map_err(|_| "voice authority serialization failed")?,
        ))
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Granted,
    Live,
    Muted,
    HumanOnly,
    Ended,
    Revoked,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Completed,
    Cancelled,
    Revoked,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub id: String,
    pub meeting: String,
    pub meeting_revision: u64,
    /// Participant identities the recipient named; digested, never free text.
    pub participants: Vec<String>,
    pub recipient_request_reference: String,
    pub expires_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub id: String,
    pub revision: u64,
    pub meeting: String,
    pub meeting_revision: u64,
    pub lead: String,
    pub supervisor: String,
    pub authority_sha256: String,
    pub authority_version: u64,
    pub ai_identity: String,
    pub participants_sha256: Vec<String>,
    pub recipient_request_sha256: String,
    pub expires_at: u64,
    pub phase: Phase,
    pub disclosed_at: Option<u64>,
    pub started_at: Option<u64>,
    pub ended_at: Option<u64>,
    pub outcome: Option<Outcome>,
    pub turns: u32,
    pub refused_turns: u32,
    pub handoffs: u32,
    pub cost_usd_millionths: u64,
    pub transcript_retained: bool,
    pub retain_until: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Book {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    authority: Option<Authority>,
    #[serde(default)]
    authority_sha256: Option<String>,
    #[serde(default)]
    revoked_at: Option<u64>,
    #[serde(default)]
    sessions: BTreeMap<String, Session>,
    /// Transcript lines live only under an admitted recording grant.
    #[serde(default)]
    transcripts: BTreeMap<String, Vec<String>>,
}
impl Book {
    pub(super) fn check(&self) -> Result<()> {
        if self.sessions.len() > MAX_SESSIONS {
            return Err("voice sessions exceed their bound".into());
        }
        Ok(())
    }
    pub(super) fn is_empty(&self) -> bool {
        self.authority.is_none() && self.sessions.is_empty()
    }
    fn current(&self, now: u64) -> Option<&Authority> {
        self.authority
            .as_ref()
            .filter(|a| self.revoked_at.is_none() && a.expires_at > now)
    }
}

/// What a supervisor or owner may do to a live session.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    Start,
    Mute,
    Unmute,
    Takeover,
    End,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub enum Turn {
    /// Speech the session may synthesize for the named AI identity.
    Speak { text: String },
    /// The question needs the human; nothing is spoken.
    Handoff { reason: String },
}

fn participant_digests(participants: &[String]) -> Result<Vec<String>> {
    let mut out = BTreeSet::new();
    for p in participants {
        text(p, 256)?;
        let normalized = p.trim().to_ascii_lowercase();
        if normalized.is_empty() || !out.insert(digest(normalized.as_bytes())) {
            return Err("voice participants must be distinct named identities".into());
        }
    }
    if out.is_empty() || out.len() > 8 {
        return Err("voice participants must name one to eight identities".into());
    }
    Ok(out.into_iter().collect())
}

fn commercial(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    [
        "price", "pricing", "cost", "discount", "quote", "contract", "terms", "sign", "pay",
        "invoice", "commit",
    ]
    .iter()
    .any(|k| t.contains(k))
}

impl Store {
    /// Records or replaces the voice authority; owner only, next version only.
    pub fn publish_voice_authority(
        &mut self,
        owner: &Access,
        authority: Authority,
    ) -> Result<String> {
        self.refresh()?;
        self.admin(owner)?;
        let now = (self.clock)();
        authority.check(now)?;
        let expected = self
            .state
            .voice
            .authority
            .as_ref()
            .map_or(1, |a| a.version.saturating_add(1));
        if authority.version != expected {
            return Err("voice authority is not the next version".into());
        }
        let sha256 = authority.sha256()?;
        let mut next = self.state.clone();
        next.voice.authority = Some(authority);
        next.voice.authority_sha256 = Some(sha256.clone());
        next.voice.revoked_at = None;
        self.persist(next)?;
        Ok(sha256)
    }
    pub fn revoke_voice_authority(&mut self, owner: &Access, reference: &str) -> Result<()> {
        self.refresh()?;
        self.admin(owner)?;
        text(reference, 256)?;
        let now = (self.clock)();
        let mut next = self.state.clone();
        next.voice.revoked_at = Some(now);
        for s in next.voice.sessions.values_mut() {
            if !matches!(s.phase, Phase::Ended | Phase::Revoked) {
                s.phase = Phase::Revoked;
                s.ended_at = Some(now);
                s.outcome = Some(Outcome::Revoked);
                s.revision += 1;
            }
        }
        self.persist(next)?;
        Ok(())
    }
    /// Grants one session over an accepted, requested meeting. The supervisor
    /// is the meeting's accepting human; the session cannot start on its own.
    pub fn grant_voice_session(&mut self, owner: &Access, grant: Grant) -> Result<Session> {
        self.refresh()?;
        self.admin(owner)?;
        id(&grant.id)?;
        text(&grant.recipient_request_reference, 256)?;
        privacy::check_credentials(&self.state, &grant.recipient_request_reference)?;
        let now = (self.clock)();
        let authority = self
            .state
            .voice
            .current(now)
            .ok_or("voice participation is disabled: no current voice authority")?;
        let m = self
            .state
            .meetings
            .meetings
            .get(&grant.meeting)
            .ok_or("voice session needs a booked meeting")?;
        if m.revision != grant.meeting_revision
            || m.phase != super::meetings::Phase::Accepted
            || m.accepted_by.is_none()
            || m.customer_request_reference.is_none()
            || grant.recipient_request_reference.trim().is_empty()
        {
            return Err(
                "voice session needs the exact accepted meeting the recipient requested".into(),
            );
        }
        if grant.expires_at <= now || grant.expires_at > m.slot.end_at.saturating_add(900) {
            return Err("voice grant must expire with the meeting".into());
        }
        let lead = self
            .state
            .leads
            .get(&m.lead)
            .ok_or("voice session lead is unavailable")?;
        if lead.revision != m.lead_revision
            || lead.details.permission.state != PermissionState::Granted
            || lead.details.permission.expires_at <= now
        {
            return Err("voice session needs current lead permission".into());
        }
        self.contact_admitted(lead, "email")
            .map_err(|e| format!("voice session needs an admitted business contact: {e}"))?;
        if self.state.voice.sessions.contains_key(&grant.id)
            || self.state.voice.sessions.values().any(|s| {
                s.meeting == grant.meeting && !matches!(s.phase, Phase::Ended | Phase::Revoked)
            })
        {
            return Err("voice session already exists for this meeting".into());
        }
        let session = Session {
            id: grant.id.clone(),
            revision: 1,
            meeting: m.id.clone(),
            meeting_revision: m.revision,
            lead: m.lead.clone(),
            supervisor: m.accepted_by.clone().unwrap_or_default(),
            authority_sha256: self
                .state
                .voice
                .authority_sha256
                .clone()
                .unwrap_or_default(),
            authority_version: authority.version,
            ai_identity: authority.ai_identity.clone(),
            participants_sha256: participant_digests(&grant.participants)?,
            recipient_request_sha256: digest(grant.recipient_request_reference.as_bytes()),
            expires_at: grant.expires_at,
            phase: Phase::Granted,
            disclosed_at: None,
            started_at: None,
            ended_at: None,
            outcome: None,
            turns: 0,
            refused_turns: 0,
            handoffs: 0,
            cost_usd_millionths: 0,
            transcript_retained: authority.recording.is_some(),
            retain_until: m.retain_until,
        };
        let mut next = self.state.clone();
        next.voice
            .sessions
            .insert(session.id.clone(), session.clone());
        next.voice.check()?;
        self.persist(next)?;
        Ok(session)
    }
    /// Supervisor controls. `Start` requires the human to be present with the
    /// named participants and discloses the AI identity first; `End` and
    /// `Takeover` stop audio dispatch at once, and a later `Start` on an ended
    /// session is refused rather than resumed.
    pub fn control_voice_session(
        &mut self,
        human: &Access,
        id: &str,
        revision: u64,
        control: Control,
        participants: &[String],
    ) -> Result<Session> {
        self.refresh()?;
        self.check(human)?;
        let now = (self.clock)();
        let mut s = self
            .state
            .voice
            .sessions
            .get(id)
            .cloned()
            .ok_or("voice session is unavailable")?;
        let owner = self.admin(human).is_ok();
        if (human.principal != s.supervisor && !(owner && control == Control::End))
            || s.revision != revision
        {
            return Err(
                "only the named supervisor controls the exact current voice session".into(),
            );
        }
        if matches!(s.phase, Phase::Ended | Phase::Revoked) {
            return Err("voice session has ended and cannot resume".into());
        }
        s.phase = match control {
            Control::Start => {
                if s.phase != Phase::Granted {
                    return Err("voice session is already running".into());
                }
                if self.state.voice.current(now).map(|a| a.version) != Some(s.authority_version)
                    || s.expires_at <= now
                {
                    return Err("voice authority or grant is no longer current".into());
                }
                if participant_digests(participants)? != s.participants_sha256 {
                    return Err("voice participants changed; the human continues alone".into());
                }
                s.disclosed_at = Some(now);
                s.started_at = Some(now);
                Phase::Live
            }
            Control::Mute => Phase::Muted,
            Control::Unmute => {
                if s.phase != Phase::Muted {
                    return Err("only a muted voice session resumes speaking".into());
                }
                if participant_digests(participants)? != s.participants_sha256 {
                    return Err("voice participants changed; the human continues alone".into());
                }
                Phase::Live
            }
            Control::Takeover => Phase::HumanOnly,
            Control::End => {
                s.ended_at = Some(now);
                s.outcome = Some(if s.turns == 0 {
                    Outcome::Cancelled
                } else {
                    Outcome::Completed
                });
                Phase::Ended
            }
        };
        s.revision += 1;
        let mut next = self.state.clone();
        if s.phase == Phase::Ended && !s.transcript_retained {
            next.voice.transcripts.remove(id);
        }
        next.voice.sessions.insert(id.into(), s.clone());
        self.persist(next)?;
        Ok(s)
    }
    /// One assisted turn: `heard` is untrusted recipient speech, `proposed`
    /// the assistant's candidate reply. Speech is dispatched only while the
    /// session is live, current, in budget, and the turn is not commercial.
    pub fn voice_turn(
        &mut self,
        human: &Access,
        id: &str,
        heard: &str,
        proposed: &str,
        cost_usd_millionths: u64,
    ) -> Result<Turn> {
        self.refresh()?;
        self.check(human)?;
        text(heard, 4096)?;
        text(proposed, 2048)?;
        let now = (self.clock)();
        let mut s = self
            .state
            .voice
            .sessions
            .get(id)
            .cloned()
            .ok_or("voice session is unavailable")?;
        if human.principal != s.supervisor {
            return Err("only the supervisor relays voice turns".into());
        }
        let authority = self
            .state
            .voice
            .current(now)
            .filter(|a| a.version == s.authority_version)
            .ok_or("voice authority is no longer current; the human continues alone")?;
        if s.phase != Phase::Live || s.expires_at <= now {
            return Err("voice session is not live; nothing is spoken".into());
        }
        let turn = match super::replies::safety(heard, false) {
            super::replies::Safety::OptOut | super::replies::Safety::AmbiguousOptOut => {
                s.phase = Phase::Ended;
                s.ended_at = Some(now);
                s.outcome = Some(Outcome::Cancelled);
                Turn::Handoff {
                    reason: "recipient asked to stop; the session ended".into(),
                }
            }
            super::replies::Safety::Injection | super::replies::Safety::CredentialMaterial => {
                Turn::Handoff {
                    reason: "unsafe recipient speech is not followed".into(),
                }
            }
            _ if commercial(heard) || commercial(proposed) => Turn::Handoff {
                reason: "pricing or terms belong to the human".into(),
            },
            _ if s.turns >= MAX_TURNS
                || s.cost_usd_millionths.saturating_add(cost_usd_millionths)
                    > authority.max_cost_usd_millionths =>
            {
                s.phase = Phase::HumanOnly;
                Turn::Handoff {
                    reason: "voice budget reached; the human continues alone".into(),
                }
            }
            _ => Turn::Speak {
                text: proposed.trim().to_string(),
            },
        };
        s.cost_usd_millionths = s.cost_usd_millionths.saturating_add(cost_usd_millionths);
        match &turn {
            Turn::Speak { .. } => s.turns += 1,
            Turn::Handoff { .. } => {
                s.refused_turns += 1;
                s.handoffs += 1;
            }
        }
        s.revision += 1;
        let mut next = self.state.clone();
        if authority.recording.is_some() {
            let line = match &turn {
                Turn::Speak { text } => format!("{}: {text}", s.ai_identity),
                Turn::Handoff { reason } => format!("handoff: {reason}"),
            };
            next.voice
                .transcripts
                .entry(id.into())
                .or_default()
                .push(line);
        }
        next.voice.sessions.insert(id.into(), s);
        self.persist(next)?;
        Ok(turn)
    }
    pub fn voice_session(&mut self, access: &Access, id: &str) -> Result<Session> {
        self.refresh()?;
        self.check(access)?;
        let s = self
            .state
            .voice
            .sessions
            .get(id)
            .cloned()
            .ok_or("voice session is unavailable")?;
        if access.principal != s.supervisor && self.admin(access).is_err() {
            return Err("voice session is private to its supervisor and the owner".into());
        }
        Ok(s)
    }
    pub fn voice_transcript(&mut self, owner: &Access, id: &str) -> Result<Vec<String>> {
        self.refresh()?;
        self.admin(owner)?;
        Ok(self
            .state
            .voice
            .transcripts
            .get(id)
            .cloned()
            .unwrap_or_default())
    }
    pub fn sales_voice_view(&mut self, owner: &Access) -> Result<serde_json::Value> {
        self.refresh()?;
        self.admin(owner)?;
        let now = (self.clock)();
        let v = &self.state.voice;
        Ok(serde_json::json!({
            "enabled": v.current(now).is_some(),
            "authority_version": v.authority.as_ref().map(|a| a.version),
            "authority_sha256": v.authority_sha256,
            "revoked_at": v.revoked_at,
            "medium": v.authority.as_ref().map(|a| a.medium),
            "recording": v.authority.as_ref().is_some_and(|a| a.recording.is_some()),
            "sessions": v.sessions.values().map(|s| serde_json::json!({
                "id": s.id, "meeting": s.meeting, "phase": s.phase, "outcome": s.outcome,
                "turns": s.turns, "handoffs": s.handoffs, "cost_usd_millionths": s.cost_usd_millionths,
                "transcript_retained": s.transcript_retained,
            })).collect::<Vec<_>>(),
            "cold_calls_available": false,
        }))
    }
}
