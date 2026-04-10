//! TRN: Training Relay Notes typed event helpers.
//!
//! This module implements the typed MVP event kinds defined in
//! `crates/nostr/nips/TRN.md`.

use crate::nip01::{Event, EventTemplate, is_addressable_kind};
use crate::tag_parsing::{collect_tag_values, find_tag_value, tag_field, tag_name};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const KIND_TRAINING_NETWORK_CONTRACT: u16 = 39_500;
pub const KIND_TRAINING_NODE_RECORD: u16 = 39_501;
pub const KIND_TRAINING_WINDOW: u16 = 39_510;
pub const KIND_TRAINING_RECEIPT: u16 = 39_511;
pub const KIND_TRAINING_VALIDATOR_VERDICT: u16 = 39_512;
pub const KIND_TRAINING_ARTIFACT_LOCATOR: u16 = 39_520;
pub const KIND_TRAINING_CLOSEOUT: u16 = 39_530;

#[derive(Debug, Error)]
pub enum NipTrnError {
    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("unsupported TRN event kind: {0}")]
    UnsupportedKind(u16),

    #[error("missing required tag `{0}`")]
    MissingTag(&'static str),

    #[error("missing required field `{0}`")]
    MissingField(&'static str),

    #[error("invalid coordinate kind: {0}")]
    InvalidCoordinateKind(u16),

    #[error("invalid lowercase hex field `{field}`: {value}")]
    InvalidHexField { field: &'static str, value: String },

    #[error("invalid JSON content: {0}")]
    InvalidJsonContent(String),
}

pub fn is_trn_kind(kind: u16) -> bool {
    matches!(
        kind,
        KIND_TRAINING_NETWORK_CONTRACT
            | KIND_TRAINING_NODE_RECORD
            | KIND_TRAINING_WINDOW
            | KIND_TRAINING_RECEIPT
            | KIND_TRAINING_VALIDATOR_VERDICT
            | KIND_TRAINING_ARTIFACT_LOCATOR
            | KIND_TRAINING_CLOSEOUT
    )
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TrnEvent {
    NetworkContract(TrainingNetworkContractEvent),
    NodeRecord(TrainingNodeRecordEvent),
    Window(TrainingWindowEvent),
    Receipt(TrainingReceiptEvent),
    ValidatorVerdict(TrainingValidatorVerdictEvent),
    ArtifactLocator(TrainingArtifactLocatorEvent),
    Closeout(TrainingCloseoutEvent),
}

impl TrnEvent {
    pub fn kind(&self) -> u16 {
        match self {
            Self::NetworkContract(_) => KIND_TRAINING_NETWORK_CONTRACT,
            Self::NodeRecord(_) => KIND_TRAINING_NODE_RECORD,
            Self::Window(_) => KIND_TRAINING_WINDOW,
            Self::Receipt(_) => KIND_TRAINING_RECEIPT,
            Self::ValidatorVerdict(_) => KIND_TRAINING_VALIDATOR_VERDICT,
            Self::ArtifactLocator(_) => KIND_TRAINING_ARTIFACT_LOCATOR,
            Self::Closeout(_) => KIND_TRAINING_CLOSEOUT,
        }
    }

    pub fn validate(&self) -> Result<(), NipTrnError> {
        match self {
            Self::NetworkContract(event) => event.validate(),
            Self::NodeRecord(event) => event.validate(),
            Self::Window(event) => event.validate(),
            Self::Receipt(event) => event.validate(),
            Self::ValidatorVerdict(event) => event.validate(),
            Self::ArtifactLocator(event) => event.validate(),
            Self::Closeout(event) => event.validate(),
        }
    }

    pub fn normalize(&self) -> Self {
        match self {
            Self::NetworkContract(event) => Self::NetworkContract(event.normalize()),
            Self::NodeRecord(event) => Self::NodeRecord(event.normalize()),
            Self::Window(event) => Self::Window(event.normalize()),
            Self::Receipt(event) => Self::Receipt(event.normalize()),
            Self::ValidatorVerdict(event) => Self::ValidatorVerdict(event.normalize()),
            Self::ArtifactLocator(event) => Self::ArtifactLocator(event.normalize()),
            Self::Closeout(event) => Self::Closeout(event.normalize()),
        }
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, NipTrnError> {
        match self {
            Self::NetworkContract(event) => event.to_event_template(created_at),
            Self::NodeRecord(event) => event.to_event_template(created_at),
            Self::Window(event) => event.to_event_template(created_at),
            Self::Receipt(event) => event.to_event_template(created_at),
            Self::ValidatorVerdict(event) => event.to_event_template(created_at),
            Self::ArtifactLocator(event) => event.to_event_template(created_at),
            Self::Closeout(event) => event.to_event_template(created_at),
        }
    }

    pub fn from_event(event: &Event) -> Result<Self, NipTrnError> {
        match event.kind {
            KIND_TRAINING_NETWORK_CONTRACT => Ok(Self::NetworkContract(
                TrainingNetworkContractEvent::from_event(event)?,
            )),
            KIND_TRAINING_NODE_RECORD => Ok(Self::NodeRecord(TrainingNodeRecordEvent::from_event(
                event,
            )?)),
            KIND_TRAINING_WINDOW => Ok(Self::Window(TrainingWindowEvent::from_event(event)?)),
            KIND_TRAINING_RECEIPT => Ok(Self::Receipt(TrainingReceiptEvent::from_event(event)?)),
            KIND_TRAINING_VALIDATOR_VERDICT => Ok(Self::ValidatorVerdict(
                TrainingValidatorVerdictEvent::from_event(event)?,
            )),
            KIND_TRAINING_ARTIFACT_LOCATOR => Ok(Self::ArtifactLocator(
                TrainingArtifactLocatorEvent::from_event(event)?,
            )),
            KIND_TRAINING_CLOSEOUT => Ok(Self::Closeout(TrainingCloseoutEvent::from_event(event)?)),
            other => Err(NipTrnError::UnsupportedKind(other)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TrnPubkeyReference {
    pub pubkey: String,
    pub relay_url: Option<String>,
    pub marker: Option<String>,
}

impl TrnPubkeyReference {
    pub fn new(
        pubkey: impl Into<String>,
        relay_url: Option<impl Into<String>>,
        marker: Option<impl Into<String>>,
    ) -> Self {
        Self {
            pubkey: pubkey.into(),
            relay_url: relay_url.map(Into::into),
            marker: marker.map(Into::into),
        }
    }

    pub fn subject(pubkey: impl Into<String>) -> Self {
        Self::new(pubkey, None::<String>, Some("subject"))
    }

    pub fn coordinator(pubkey: impl Into<String>) -> Self {
        Self::new(pubkey, None::<String>, Some("coordinator"))
    }

    pub fn validator(pubkey: impl Into<String>) -> Self {
        Self::new(pubkey, None::<String>, Some("validator"))
    }

    pub fn to_tag(&self) -> Vec<String> {
        let mut tag = vec!["p".to_string(), self.pubkey.clone()];
        if let Some(relay_url) = &self.relay_url {
            tag.push(relay_url.clone());
        }
        if let Some(marker) = &self.marker {
            if self.relay_url.is_none() {
                tag.push(String::new());
            }
            tag.push(marker.clone());
        }
        tag
    }

    pub fn from_tag(tag: &[String]) -> Option<Self> {
        if tag_name(tag) != Some("p") {
            return None;
        }
        let pubkey = trim_optional(tag_field(tag, 1))?;
        Some(Self {
            pubkey,
            relay_url: trim_optional(tag_field(tag, 2)),
            marker: trim_optional(tag_field(tag, 3)),
        })
    }

    fn validate(&self) -> Result<(), NipTrnError> {
        validate_lower_hex("p", self.pubkey.as_str())
    }

    fn normalize(&self) -> Self {
        Self {
            pubkey: self.pubkey.trim().to_string(),
            relay_url: self
                .relay_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            marker: self
                .marker
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TrnEventReference {
    pub event_id: String,
    pub relay_url: Option<String>,
    pub marker: Option<String>,
}

impl TrnEventReference {
    pub fn new(
        event_id: impl Into<String>,
        relay_url: Option<impl Into<String>>,
        marker: Option<impl Into<String>>,
    ) -> Self {
        Self {
            event_id: event_id.into(),
            relay_url: relay_url.map(Into::into),
            marker: marker.map(Into::into),
        }
    }

    pub fn to_tag(&self) -> Vec<String> {
        let mut tag = vec!["e".to_string(), self.event_id.clone()];
        if let Some(relay_url) = &self.relay_url {
            tag.push(relay_url.clone());
        }
        if let Some(marker) = &self.marker {
            if self.relay_url.is_none() {
                tag.push(String::new());
            }
            tag.push(marker.clone());
        }
        tag
    }

    pub fn from_tag(tag: &[String]) -> Option<Self> {
        if tag_name(tag) != Some("e") {
            return None;
        }
        let event_id = trim_optional(tag_field(tag, 1))?;
        Some(Self {
            event_id,
            relay_url: trim_optional(tag_field(tag, 2)),
            marker: trim_optional(tag_field(tag, 3)),
        })
    }

    fn validate(&self) -> Result<(), NipTrnError> {
        validate_lower_hex("e", self.event_id.as_str())
    }

    fn normalize(&self) -> Self {
        Self {
            event_id: self.event_id.trim().to_string(),
            relay_url: self
                .relay_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            marker: self
                .marker
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TrnAddressReference {
    pub coordinate: String,
    pub relay_url: Option<String>,
    pub marker: Option<String>,
}

impl TrnAddressReference {
    pub fn new(
        coordinate: impl Into<String>,
        relay_url: Option<impl Into<String>>,
        marker: Option<impl Into<String>>,
    ) -> Self {
        Self {
            coordinate: coordinate.into(),
            relay_url: relay_url.map(Into::into),
            marker: marker.map(Into::into),
        }
    }

    pub fn to_tag(&self) -> Vec<String> {
        let mut tag = vec!["a".to_string(), self.coordinate.clone()];
        if let Some(relay_url) = &self.relay_url {
            tag.push(relay_url.clone());
        }
        if let Some(marker) = &self.marker {
            if self.relay_url.is_none() {
                tag.push(String::new());
            }
            tag.push(marker.clone());
        }
        tag
    }

    pub fn from_tag(tag: &[String]) -> Option<Self> {
        if tag_name(tag) != Some("a") {
            return None;
        }
        let coordinate = trim_optional(tag_field(tag, 1))?;
        Some(Self {
            coordinate,
            relay_url: trim_optional(tag_field(tag, 2)),
            marker: trim_optional(tag_field(tag, 3)),
        })
    }

    fn validate(&self) -> Result<(), NipTrnError> {
        validate_coordinate_string(self.coordinate.as_str())
    }

    fn normalize(&self) -> Self {
        Self {
            coordinate: self.coordinate.trim().to_string(),
            relay_url: self
                .relay_url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            marker: self
                .marker
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TrnCapability {
    pub name: String,
    pub value: String,
}

impl TrnCapability {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    pub fn to_tag(&self) -> Vec<String> {
        vec!["cap".to_string(), self.name.clone(), self.value.clone()]
    }

    pub fn from_tag(tag: &[String]) -> Option<Self> {
        if tag_name(tag) != Some("cap") {
            return None;
        }
        Some(Self {
            name: trim_optional(tag_field(tag, 1))?,
            value: trim_optional(tag_field(tag, 2))?,
        })
    }

    fn normalize(&self) -> Self {
        Self {
            name: self.name.trim().to_string(),
            value: self.value.trim().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainingNetworkContractEvent {
    pub identifier: String,
    pub network_id: String,
    pub status: String,
    pub content: Value,
    pub model_family: Option<String>,
    pub window_cadence: Option<String>,
    pub roles: Vec<String>,
    pub profiles: Vec<String>,
    pub address_refs: Vec<TrnAddressReference>,
    pub extra_tags: Vec<Vec<String>>,
}

impl TrainingNetworkContractEvent {
    pub fn validate(&self) -> Result<(), NipTrnError> {
        require_nonempty("identifier", self.identifier.as_str())?;
        require_nonempty("network_id", self.network_id.as_str())?;
        require_nonempty("status", self.status.as_str())?;
        validate_json_object("content", &self.content)?;
        for address_ref in &self.address_refs {
            address_ref.validate()?;
        }
        Ok(())
    }

    pub fn normalize(&self) -> Self {
        let mut normalized = Self {
            identifier: self.identifier.trim().to_string(),
            network_id: self.network_id.trim().to_string(),
            status: self.status.trim().to_string(),
            content: self.content.clone(),
            model_family: normalize_option(self.model_family.as_deref()),
            window_cadence: normalize_option(self.window_cadence.as_deref()),
            roles: normalize_vec(self.roles.clone()),
            profiles: normalize_vec(self.profiles.clone()),
            address_refs: self
                .address_refs
                .iter()
                .map(TrnAddressReference::normalize)
                .collect(),
            extra_tags: normalize_extra_tags(self.extra_tags.clone()),
        };
        normalized.address_refs.sort();
        normalized.address_refs.dedup();
        normalized
    }

    pub fn coordinate(&self, publisher_pubkey: &str) -> Result<String, NipTrnError> {
        coordinate_for_kind(
            KIND_TRAINING_NETWORK_CONTRACT,
            publisher_pubkey,
            self.identifier.as_str(),
        )
    }

    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let normalized = self.normalize();
        let mut tags = vec![
            vec!["d".to_string(), normalized.identifier.clone()],
            vec!["network".to_string(), normalized.network_id.clone()],
            vec!["status".to_string(), normalized.status.clone()],
        ];
        if let Some(model_family) = normalized.model_family {
            tags.push(vec!["model_family".to_string(), model_family]);
        }
        if let Some(window_cadence) = normalized.window_cadence {
            tags.push(vec!["window_cadence".to_string(), window_cadence]);
        }
        for role in normalized.roles {
            tags.push(vec!["role".to_string(), role]);
        }
        for profile in normalized.profiles {
            tags.push(vec!["profile".to_string(), profile]);
        }
        for address_ref in normalized.address_refs {
            tags.push(address_ref.to_tag());
        }
        tags.extend(normalized.extra_tags);
        tags
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, NipTrnError> {
        let normalized = self.normalize();
        normalized.validate()?;
        Ok(EventTemplate {
            created_at,
            kind: KIND_TRAINING_NETWORK_CONTRACT,
            tags: normalized.to_tags(),
            content: serialize_json_content(&normalized.content)?,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, NipTrnError> {
        if event.kind != KIND_TRAINING_NETWORK_CONTRACT {
            return Err(NipTrnError::InvalidKind {
                expected: KIND_TRAINING_NETWORK_CONTRACT,
                actual: event.kind,
            });
        }
        let identifier = required_tag_value(&event.tags, "d")?;
        let network_id = find_tag_value(&event.tags, "network")
            .map(str::to_string)
            .unwrap_or_else(|| identifier.clone());
        let status = required_tag_value(&event.tags, "status")?;
        let content = parse_json_content(event.content.as_str())?;
        let mut extra_tags = Vec::new();
        for tag in &event.tags {
            match tag_name(tag) {
                Some("d")
                | Some("network")
                | Some("status")
                | Some("model_family")
                | Some("window_cadence")
                | Some("role")
                | Some("profile")
                | Some("a") => {}
                _ => extra_tags.push(tag.clone()),
            }
        }
        let parsed = Self {
            identifier,
            network_id,
            status,
            content,
            model_family: trim_optional(find_tag_value(&event.tags, "model_family")),
            window_cadence: trim_optional(find_tag_value(&event.tags, "window_cadence")),
            roles: collect_tag_values(&event.tags, "role"),
            profiles: collect_tag_values(&event.tags, "profile"),
            address_refs: collect_address_refs(&event.tags),
            extra_tags,
        };
        parsed.validate()?;
        Ok(parsed.normalize())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainingNodeRecordEvent {
    pub identifier: String,
    pub network_id: String,
    pub status: String,
    pub content: Value,
    pub roles: Vec<String>,
    pub classes: Vec<String>,
    pub build_digest: Option<String>,
    pub capabilities: Vec<TrnCapability>,
    pub relay_urls: Vec<String>,
    pub extra_tags: Vec<Vec<String>>,
}

impl TrainingNodeRecordEvent {
    pub fn validate(&self) -> Result<(), NipTrnError> {
        require_nonempty("identifier", self.identifier.as_str())?;
        require_nonempty("network_id", self.network_id.as_str())?;
        require_nonempty("status", self.status.as_str())?;
        validate_json_object("content", &self.content)?;
        Ok(())
    }

    pub fn normalize(&self) -> Self {
        let mut normalized = Self {
            identifier: self.identifier.trim().to_string(),
            network_id: self.network_id.trim().to_string(),
            status: self.status.trim().to_string(),
            content: self.content.clone(),
            roles: normalize_vec(self.roles.clone()),
            classes: normalize_vec(self.classes.clone()),
            build_digest: normalize_option(self.build_digest.as_deref()),
            capabilities: self
                .capabilities
                .iter()
                .map(TrnCapability::normalize)
                .collect(),
            relay_urls: normalize_vec(self.relay_urls.clone()),
            extra_tags: normalize_extra_tags(self.extra_tags.clone()),
        };
        normalized.capabilities.sort();
        normalized.capabilities.dedup();
        normalized
    }

    pub fn coordinate(&self, node_pubkey: &str) -> Result<String, NipTrnError> {
        coordinate_for_kind(
            KIND_TRAINING_NODE_RECORD,
            node_pubkey,
            self.identifier.as_str(),
        )
    }

    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let normalized = self.normalize();
        let mut tags = vec![
            vec!["d".to_string(), normalized.identifier.clone()],
            vec!["network".to_string(), normalized.network_id.clone()],
            vec!["status".to_string(), normalized.status.clone()],
        ];
        for role in normalized.roles {
            tags.push(vec!["role".to_string(), role]);
        }
        for class in normalized.classes {
            tags.push(vec!["class".to_string(), class]);
        }
        if let Some(build_digest) = normalized.build_digest {
            tags.push(vec!["build".to_string(), build_digest]);
        }
        for capability in normalized.capabilities {
            tags.push(capability.to_tag());
        }
        for relay_url in normalized.relay_urls {
            tags.push(vec!["relay".to_string(), relay_url]);
        }
        tags.extend(normalized.extra_tags);
        tags
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, NipTrnError> {
        let normalized = self.normalize();
        normalized.validate()?;
        Ok(EventTemplate {
            created_at,
            kind: KIND_TRAINING_NODE_RECORD,
            tags: normalized.to_tags(),
            content: serialize_json_content(&normalized.content)?,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, NipTrnError> {
        if event.kind != KIND_TRAINING_NODE_RECORD {
            return Err(NipTrnError::InvalidKind {
                expected: KIND_TRAINING_NODE_RECORD,
                actual: event.kind,
            });
        }
        let identifier = required_tag_value(&event.tags, "d")?;
        let network_id = find_tag_value(&event.tags, "network")
            .map(str::to_string)
            .unwrap_or_else(|| identifier.clone());
        let status = required_tag_value(&event.tags, "status")?;
        let content = parse_json_content(event.content.as_str())?;
        let mut extra_tags = Vec::new();
        let mut capabilities = Vec::new();
        for tag in &event.tags {
            match tag_name(tag) {
                Some("d") | Some("network") | Some("status") | Some("role") | Some("class")
                | Some("build") | Some("relay") => {}
                Some("cap") => {
                    if let Some(capability) = TrnCapability::from_tag(tag) {
                        capabilities.push(capability);
                    }
                }
                _ => extra_tags.push(tag.clone()),
            }
        }
        let parsed = Self {
            identifier,
            network_id,
            status,
            content,
            roles: collect_tag_values(&event.tags, "role"),
            classes: collect_tag_values(&event.tags, "class"),
            build_digest: trim_optional(find_tag_value(&event.tags, "build")),
            capabilities,
            relay_urls: collect_tag_values(&event.tags, "relay"),
            extra_tags,
        };
        parsed.validate()?;
        Ok(parsed.normalize())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainingWindowEvent {
    pub identifier: String,
    pub network_id: String,
    pub status: String,
    pub content: Value,
    pub policy_revision: Option<String>,
    pub assignment_seed: Option<String>,
    pub workload_family: Option<String>,
    pub address_refs: Vec<TrnAddressReference>,
    pub extra_tags: Vec<Vec<String>>,
}

impl TrainingWindowEvent {
    pub fn validate(&self) -> Result<(), NipTrnError> {
        require_nonempty("identifier", self.identifier.as_str())?;
        require_nonempty("network_id", self.network_id.as_str())?;
        require_nonempty("status", self.status.as_str())?;
        validate_json_object("content", &self.content)?;
        for address_ref in &self.address_refs {
            address_ref.validate()?;
        }
        Ok(())
    }

    pub fn normalize(&self) -> Self {
        let mut normalized = Self {
            identifier: self.identifier.trim().to_string(),
            network_id: self.network_id.trim().to_string(),
            status: self.status.trim().to_string(),
            content: self.content.clone(),
            policy_revision: normalize_option(self.policy_revision.as_deref()),
            assignment_seed: normalize_option(self.assignment_seed.as_deref()),
            workload_family: normalize_option(self.workload_family.as_deref()),
            address_refs: self
                .address_refs
                .iter()
                .map(TrnAddressReference::normalize)
                .collect(),
            extra_tags: normalize_extra_tags(self.extra_tags.clone()),
        };
        normalized.address_refs.sort();
        normalized.address_refs.dedup();
        normalized
    }

    pub fn coordinate(&self, publisher_pubkey: &str) -> Result<String, NipTrnError> {
        coordinate_for_kind(
            KIND_TRAINING_WINDOW,
            publisher_pubkey,
            self.identifier.as_str(),
        )
    }

    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let normalized = self.normalize();
        let mut tags = vec![
            vec!["d".to_string(), normalized.identifier.clone()],
            vec!["network".to_string(), normalized.network_id.clone()],
            vec!["status".to_string(), normalized.status.clone()],
        ];
        if let Some(policy_revision) = normalized.policy_revision {
            tags.push(vec!["policy".to_string(), policy_revision]);
        }
        if let Some(assignment_seed) = normalized.assignment_seed {
            tags.push(vec!["assignment_seed".to_string(), assignment_seed]);
        }
        if let Some(workload_family) = normalized.workload_family {
            tags.push(vec!["workload".to_string(), workload_family]);
        }
        for address_ref in normalized.address_refs {
            tags.push(address_ref.to_tag());
        }
        tags.extend(normalized.extra_tags);
        tags
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, NipTrnError> {
        let normalized = self.normalize();
        normalized.validate()?;
        Ok(EventTemplate {
            created_at,
            kind: KIND_TRAINING_WINDOW,
            tags: normalized.to_tags(),
            content: serialize_json_content(&normalized.content)?,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, NipTrnError> {
        if event.kind != KIND_TRAINING_WINDOW {
            return Err(NipTrnError::InvalidKind {
                expected: KIND_TRAINING_WINDOW,
                actual: event.kind,
            });
        }
        let identifier = required_tag_value(&event.tags, "d")?;
        let network_id = required_tag_value(&event.tags, "network")?;
        let status = required_tag_value(&event.tags, "status")?;
        let content = parse_json_content(event.content.as_str())?;
        let mut extra_tags = Vec::new();
        for tag in &event.tags {
            match tag_name(tag) {
                Some("d")
                | Some("network")
                | Some("status")
                | Some("policy")
                | Some("assignment_seed")
                | Some("workload")
                | Some("a") => {}
                _ => extra_tags.push(tag.clone()),
            }
        }
        let parsed = Self {
            identifier,
            network_id,
            status,
            content,
            policy_revision: trim_optional(find_tag_value(&event.tags, "policy")),
            assignment_seed: trim_optional(find_tag_value(&event.tags, "assignment_seed")),
            workload_family: trim_optional(find_tag_value(&event.tags, "workload")),
            address_refs: collect_address_refs(&event.tags),
            extra_tags,
        };
        parsed.validate()?;
        Ok(parsed.normalize())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainingReceiptEvent {
    pub network_id: String,
    pub window_id: String,
    pub status: String,
    pub content: Value,
    pub assignment_id: Option<String>,
    pub policy_revision: Option<String>,
    pub role: Option<String>,
    pub artifact_id: Option<String>,
    pub checkpoint_id: Option<String>,
    pub actors: Vec<TrnPubkeyReference>,
    pub reason_codes: Vec<String>,
    pub classes: Vec<String>,
    pub address_refs: Vec<TrnAddressReference>,
    pub event_refs: Vec<TrnEventReference>,
    pub extra_tags: Vec<Vec<String>>,
}

impl TrainingReceiptEvent {
    pub fn validate(&self) -> Result<(), NipTrnError> {
        require_nonempty("network_id", self.network_id.as_str())?;
        require_nonempty("window_id", self.window_id.as_str())?;
        require_nonempty("status", self.status.as_str())?;
        validate_json_object("content", &self.content)?;
        for actor in &self.actors {
            actor.validate()?;
        }
        for address_ref in &self.address_refs {
            address_ref.validate()?;
        }
        for event_ref in &self.event_refs {
            event_ref.validate()?;
        }
        Ok(())
    }

    pub fn normalize(&self) -> Self {
        let mut normalized = Self {
            network_id: self.network_id.trim().to_string(),
            window_id: self.window_id.trim().to_string(),
            status: self.status.trim().to_string(),
            content: self.content.clone(),
            assignment_id: normalize_option(self.assignment_id.as_deref()),
            policy_revision: normalize_option(self.policy_revision.as_deref()),
            role: normalize_option(self.role.as_deref()),
            artifact_id: normalize_option(self.artifact_id.as_deref()),
            checkpoint_id: normalize_option(self.checkpoint_id.as_deref()),
            actors: self
                .actors
                .iter()
                .map(TrnPubkeyReference::normalize)
                .collect(),
            reason_codes: normalize_vec(self.reason_codes.clone()),
            classes: normalize_vec(self.classes.clone()),
            address_refs: self
                .address_refs
                .iter()
                .map(TrnAddressReference::normalize)
                .collect(),
            event_refs: self
                .event_refs
                .iter()
                .map(TrnEventReference::normalize)
                .collect(),
            extra_tags: normalize_extra_tags(self.extra_tags.clone()),
        };
        normalized.actors.sort();
        normalized.actors.dedup();
        normalized.address_refs.sort();
        normalized.address_refs.dedup();
        normalized.event_refs.sort();
        normalized.event_refs.dedup();
        normalized
    }

    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let normalized = self.normalize();
        let mut tags = vec![
            vec!["network".to_string(), normalized.network_id.clone()],
            vec!["window".to_string(), normalized.window_id.clone()],
            vec!["status".to_string(), normalized.status.clone()],
        ];
        if let Some(assignment_id) = normalized.assignment_id {
            tags.push(vec!["assignment".to_string(), assignment_id]);
        }
        if let Some(policy_revision) = normalized.policy_revision {
            tags.push(vec!["policy".to_string(), policy_revision]);
        }
        if let Some(role) = normalized.role {
            tags.push(vec!["role".to_string(), role]);
        }
        if let Some(artifact_id) = normalized.artifact_id {
            tags.push(vec!["artifact".to_string(), artifact_id]);
        }
        if let Some(checkpoint_id) = normalized.checkpoint_id {
            tags.push(vec!["checkpoint".to_string(), checkpoint_id]);
        }
        for actor in normalized.actors {
            tags.push(actor.to_tag());
        }
        for reason_code in normalized.reason_codes {
            tags.push(vec!["reason".to_string(), reason_code]);
        }
        for class in normalized.classes {
            tags.push(vec!["class".to_string(), class]);
        }
        for address_ref in normalized.address_refs {
            tags.push(address_ref.to_tag());
        }
        for event_ref in normalized.event_refs {
            tags.push(event_ref.to_tag());
        }
        tags.extend(normalized.extra_tags);
        tags
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, NipTrnError> {
        let normalized = self.normalize();
        normalized.validate()?;
        Ok(EventTemplate {
            created_at,
            kind: KIND_TRAINING_RECEIPT,
            tags: normalized.to_tags(),
            content: serialize_json_content(&normalized.content)?,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, NipTrnError> {
        if event.kind != KIND_TRAINING_RECEIPT {
            return Err(NipTrnError::InvalidKind {
                expected: KIND_TRAINING_RECEIPT,
                actual: event.kind,
            });
        }
        parse_training_receipt_like_event(event).map(|parsed| parsed.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainingValidatorVerdictEvent {
    pub network_id: String,
    pub window_id: String,
    pub status: String,
    pub content: Value,
    pub assignment_id: Option<String>,
    pub artifact_id: Option<String>,
    pub policy_revision: Option<String>,
    pub validator_policy: Option<String>,
    pub digest: Option<String>,
    pub actors: Vec<TrnPubkeyReference>,
    pub reason_codes: Vec<String>,
    pub event_refs: Vec<TrnEventReference>,
    pub extra_tags: Vec<Vec<String>>,
}

impl TrainingValidatorVerdictEvent {
    pub fn validate(&self) -> Result<(), NipTrnError> {
        require_nonempty("network_id", self.network_id.as_str())?;
        require_nonempty("window_id", self.window_id.as_str())?;
        require_nonempty("status", self.status.as_str())?;
        validate_json_object("content", &self.content)?;
        for actor in &self.actors {
            actor.validate()?;
        }
        for event_ref in &self.event_refs {
            event_ref.validate()?;
        }
        Ok(())
    }

    pub fn normalize(&self) -> Self {
        let mut normalized = Self {
            network_id: self.network_id.trim().to_string(),
            window_id: self.window_id.trim().to_string(),
            status: self.status.trim().to_string(),
            content: self.content.clone(),
            assignment_id: normalize_option(self.assignment_id.as_deref()),
            artifact_id: normalize_option(self.artifact_id.as_deref()),
            policy_revision: normalize_option(self.policy_revision.as_deref()),
            validator_policy: normalize_option(self.validator_policy.as_deref()),
            digest: normalize_option(self.digest.as_deref()),
            actors: self
                .actors
                .iter()
                .map(TrnPubkeyReference::normalize)
                .collect(),
            reason_codes: normalize_vec(self.reason_codes.clone()),
            event_refs: self
                .event_refs
                .iter()
                .map(TrnEventReference::normalize)
                .collect(),
            extra_tags: normalize_extra_tags(self.extra_tags.clone()),
        };
        normalized.actors.sort();
        normalized.actors.dedup();
        normalized.event_refs.sort();
        normalized.event_refs.dedup();
        normalized
    }

    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let normalized = self.normalize();
        let mut tags = vec![
            vec!["network".to_string(), normalized.network_id.clone()],
            vec!["window".to_string(), normalized.window_id.clone()],
            vec!["status".to_string(), normalized.status.clone()],
        ];
        if let Some(assignment_id) = normalized.assignment_id {
            tags.push(vec!["assignment".to_string(), assignment_id]);
        }
        if let Some(artifact_id) = normalized.artifact_id {
            tags.push(vec!["artifact".to_string(), artifact_id]);
        }
        if let Some(policy_revision) = normalized.policy_revision {
            tags.push(vec!["policy".to_string(), policy_revision]);
        }
        if let Some(validator_policy) = normalized.validator_policy {
            tags.push(vec!["validator_policy".to_string(), validator_policy]);
        }
        if let Some(digest) = normalized.digest {
            tags.push(vec!["x".to_string(), digest]);
        }
        for actor in normalized.actors {
            tags.push(actor.to_tag());
        }
        for reason_code in normalized.reason_codes {
            tags.push(vec!["reason".to_string(), reason_code]);
        }
        for event_ref in normalized.event_refs {
            tags.push(event_ref.to_tag());
        }
        tags.extend(normalized.extra_tags);
        tags
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, NipTrnError> {
        let normalized = self.normalize();
        normalized.validate()?;
        Ok(EventTemplate {
            created_at,
            kind: KIND_TRAINING_VALIDATOR_VERDICT,
            tags: normalized.to_tags(),
            content: serialize_json_content(&normalized.content)?,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, NipTrnError> {
        if event.kind != KIND_TRAINING_VALIDATOR_VERDICT {
            return Err(NipTrnError::InvalidKind {
                expected: KIND_TRAINING_VALIDATOR_VERDICT,
                actual: event.kind,
            });
        }
        let network_id = required_tag_value(&event.tags, "network")?;
        let window_id = required_tag_value(&event.tags, "window")?;
        let status = required_tag_value(&event.tags, "status")?;
        let content = parse_json_content(event.content.as_str())?;
        let mut extra_tags = Vec::new();
        for tag in &event.tags {
            match tag_name(tag) {
                Some("network")
                | Some("window")
                | Some("status")
                | Some("assignment")
                | Some("artifact")
                | Some("policy")
                | Some("validator_policy")
                | Some("x")
                | Some("p")
                | Some("reason")
                | Some("e") => {}
                _ => extra_tags.push(tag.clone()),
            }
        }
        let parsed = Self {
            network_id,
            window_id,
            status,
            content,
            assignment_id: trim_optional(find_tag_value(&event.tags, "assignment")),
            artifact_id: trim_optional(find_tag_value(&event.tags, "artifact")),
            policy_revision: trim_optional(find_tag_value(&event.tags, "policy")),
            validator_policy: trim_optional(find_tag_value(&event.tags, "validator_policy")),
            digest: trim_optional(find_tag_value(&event.tags, "x")),
            actors: collect_pubkey_refs(&event.tags),
            reason_codes: collect_tag_values(&event.tags, "reason"),
            event_refs: collect_event_refs(&event.tags),
            extra_tags,
        };
        parsed.validate()?;
        Ok(parsed.normalize())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainingArtifactLocatorEvent {
    pub identifier: String,
    pub network_id: String,
    pub status: String,
    pub content: Value,
    pub artifact_id: Option<String>,
    pub checkpoint_id: Option<String>,
    pub manifest_digest: Option<String>,
    pub file_digest: Option<String>,
    pub url_hint: Option<String>,
    pub artifact_class: Option<String>,
    pub window_id: Option<String>,
    pub policy_revision: Option<String>,
    pub reason_codes: Vec<String>,
    pub address_refs: Vec<TrnAddressReference>,
    pub extra_tags: Vec<Vec<String>>,
}

impl TrainingArtifactLocatorEvent {
    pub fn validate(&self) -> Result<(), NipTrnError> {
        require_nonempty("identifier", self.identifier.as_str())?;
        require_nonempty("network_id", self.network_id.as_str())?;
        require_nonempty("status", self.status.as_str())?;
        validate_json_object("content", &self.content)?;
        for address_ref in &self.address_refs {
            address_ref.validate()?;
        }
        Ok(())
    }

    pub fn normalize(&self) -> Self {
        let mut normalized = Self {
            identifier: self.identifier.trim().to_string(),
            network_id: self.network_id.trim().to_string(),
            status: self.status.trim().to_string(),
            content: self.content.clone(),
            artifact_id: normalize_option(self.artifact_id.as_deref()),
            checkpoint_id: normalize_option(self.checkpoint_id.as_deref()),
            manifest_digest: normalize_option(self.manifest_digest.as_deref()),
            file_digest: normalize_option(self.file_digest.as_deref()),
            url_hint: normalize_option(self.url_hint.as_deref()),
            artifact_class: normalize_option(self.artifact_class.as_deref()),
            window_id: normalize_option(self.window_id.as_deref()),
            policy_revision: normalize_option(self.policy_revision.as_deref()),
            reason_codes: normalize_vec(self.reason_codes.clone()),
            address_refs: self
                .address_refs
                .iter()
                .map(TrnAddressReference::normalize)
                .collect(),
            extra_tags: normalize_extra_tags(self.extra_tags.clone()),
        };
        normalized.address_refs.sort();
        normalized.address_refs.dedup();
        normalized
    }

    pub fn coordinate(&self, publisher_pubkey: &str) -> Result<String, NipTrnError> {
        coordinate_for_kind(
            KIND_TRAINING_ARTIFACT_LOCATOR,
            publisher_pubkey,
            self.identifier.as_str(),
        )
    }

    pub fn score_snapshot(
        identifier: impl Into<String>,
        network_id: impl Into<String>,
        status: impl Into<String>,
        file_digest: impl Into<String>,
        url_hint: impl Into<String>,
        content: Value,
    ) -> Self {
        Self {
            identifier: identifier.into(),
            network_id: network_id.into(),
            status: status.into(),
            content,
            artifact_id: None,
            checkpoint_id: None,
            manifest_digest: None,
            file_digest: Some(file_digest.into()),
            url_hint: Some(url_hint.into()),
            artifact_class: Some("score".to_string()),
            window_id: None,
            policy_revision: None,
            reason_codes: Vec::new(),
            address_refs: Vec::new(),
            extra_tags: Vec::new(),
        }
    }

    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let normalized = self.normalize();
        let mut tags = vec![
            vec!["d".to_string(), normalized.identifier.clone()],
            vec!["network".to_string(), normalized.network_id.clone()],
            vec!["status".to_string(), normalized.status.clone()],
        ];
        if let Some(artifact_id) = normalized.artifact_id {
            tags.push(vec!["artifact".to_string(), artifact_id]);
        }
        if let Some(checkpoint_id) = normalized.checkpoint_id {
            tags.push(vec!["checkpoint".to_string(), checkpoint_id]);
        }
        if let Some(manifest_digest) = normalized.manifest_digest {
            tags.push(vec!["manifest".to_string(), manifest_digest]);
        }
        if let Some(file_digest) = normalized.file_digest {
            tags.push(vec!["x".to_string(), file_digest]);
        }
        if let Some(url_hint) = normalized.url_hint {
            tags.push(vec!["url".to_string(), url_hint]);
        }
        if let Some(artifact_class) = normalized.artifact_class {
            tags.push(vec!["class".to_string(), artifact_class]);
        }
        if let Some(window_id) = normalized.window_id {
            tags.push(vec!["window".to_string(), window_id]);
        }
        if let Some(policy_revision) = normalized.policy_revision {
            tags.push(vec!["policy".to_string(), policy_revision]);
        }
        for reason_code in normalized.reason_codes {
            tags.push(vec!["reason".to_string(), reason_code]);
        }
        for address_ref in normalized.address_refs {
            tags.push(address_ref.to_tag());
        }
        tags.extend(normalized.extra_tags);
        tags
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, NipTrnError> {
        let normalized = self.normalize();
        normalized.validate()?;
        Ok(EventTemplate {
            created_at,
            kind: KIND_TRAINING_ARTIFACT_LOCATOR,
            tags: normalized.to_tags(),
            content: serialize_json_content(&normalized.content)?,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, NipTrnError> {
        if event.kind != KIND_TRAINING_ARTIFACT_LOCATOR {
            return Err(NipTrnError::InvalidKind {
                expected: KIND_TRAINING_ARTIFACT_LOCATOR,
                actual: event.kind,
            });
        }
        let identifier = required_tag_value(&event.tags, "d")?;
        let network_id = required_tag_value(&event.tags, "network")?;
        let status = required_tag_value(&event.tags, "status")?;
        let content = parse_json_content(event.content.as_str())?;
        let mut extra_tags = Vec::new();
        for tag in &event.tags {
            match tag_name(tag) {
                Some("d") | Some("network") | Some("status") | Some("artifact")
                | Some("checkpoint") | Some("manifest") | Some("x") | Some("url")
                | Some("class") | Some("window") | Some("policy") | Some("reason") | Some("a") => {}
                _ => extra_tags.push(tag.clone()),
            }
        }
        let parsed = Self {
            identifier,
            network_id,
            status,
            content,
            artifact_id: trim_optional(find_tag_value(&event.tags, "artifact")),
            checkpoint_id: trim_optional(find_tag_value(&event.tags, "checkpoint")),
            manifest_digest: trim_optional(find_tag_value(&event.tags, "manifest")),
            file_digest: trim_optional(find_tag_value(&event.tags, "x")),
            url_hint: trim_optional(find_tag_value(&event.tags, "url")),
            artifact_class: trim_optional(find_tag_value(&event.tags, "class")),
            window_id: trim_optional(find_tag_value(&event.tags, "window")),
            policy_revision: trim_optional(find_tag_value(&event.tags, "policy")),
            reason_codes: collect_tag_values(&event.tags, "reason"),
            address_refs: collect_address_refs(&event.tags),
            extra_tags,
        };
        parsed.validate()?;
        Ok(parsed.normalize())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainingCloseoutEvent {
    pub network_id: String,
    pub window_id: String,
    pub status: String,
    pub content: Value,
    pub assignment_id: Option<String>,
    pub artifact_id: Option<String>,
    pub policy_revision: Option<String>,
    pub amount_msats: Option<String>,
    pub actors: Vec<TrnPubkeyReference>,
    pub reason_codes: Vec<String>,
    pub event_refs: Vec<TrnEventReference>,
    pub extra_tags: Vec<Vec<String>>,
}

impl TrainingCloseoutEvent {
    pub fn validate(&self) -> Result<(), NipTrnError> {
        require_nonempty("network_id", self.network_id.as_str())?;
        require_nonempty("window_id", self.window_id.as_str())?;
        require_nonempty("status", self.status.as_str())?;
        validate_json_object("content", &self.content)?;
        for actor in &self.actors {
            actor.validate()?;
        }
        for event_ref in &self.event_refs {
            event_ref.validate()?;
        }
        Ok(())
    }

    pub fn normalize(&self) -> Self {
        let mut normalized = Self {
            network_id: self.network_id.trim().to_string(),
            window_id: self.window_id.trim().to_string(),
            status: self.status.trim().to_string(),
            content: self.content.clone(),
            assignment_id: normalize_option(self.assignment_id.as_deref()),
            artifact_id: normalize_option(self.artifact_id.as_deref()),
            policy_revision: normalize_option(self.policy_revision.as_deref()),
            amount_msats: normalize_option(self.amount_msats.as_deref()),
            actors: self
                .actors
                .iter()
                .map(TrnPubkeyReference::normalize)
                .collect(),
            reason_codes: normalize_vec(self.reason_codes.clone()),
            event_refs: self
                .event_refs
                .iter()
                .map(TrnEventReference::normalize)
                .collect(),
            extra_tags: normalize_extra_tags(self.extra_tags.clone()),
        };
        normalized.actors.sort();
        normalized.actors.dedup();
        normalized.event_refs.sort();
        normalized.event_refs.dedup();
        normalized
    }

    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let normalized = self.normalize();
        let mut tags = vec![
            vec!["network".to_string(), normalized.network_id.clone()],
            vec!["window".to_string(), normalized.window_id.clone()],
            vec!["status".to_string(), normalized.status.clone()],
        ];
        if let Some(assignment_id) = normalized.assignment_id {
            tags.push(vec!["assignment".to_string(), assignment_id]);
        }
        if let Some(artifact_id) = normalized.artifact_id {
            tags.push(vec!["artifact".to_string(), artifact_id]);
        }
        if let Some(policy_revision) = normalized.policy_revision {
            tags.push(vec!["policy".to_string(), policy_revision]);
        }
        if let Some(amount_msats) = normalized.amount_msats {
            tags.push(vec!["amount".to_string(), amount_msats]);
        }
        for actor in normalized.actors {
            tags.push(actor.to_tag());
        }
        for reason_code in normalized.reason_codes {
            tags.push(vec!["reason".to_string(), reason_code]);
        }
        for event_ref in normalized.event_refs {
            tags.push(event_ref.to_tag());
        }
        tags.extend(normalized.extra_tags);
        tags
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, NipTrnError> {
        let normalized = self.normalize();
        normalized.validate()?;
        Ok(EventTemplate {
            created_at,
            kind: KIND_TRAINING_CLOSEOUT,
            tags: normalized.to_tags(),
            content: serialize_json_content(&normalized.content)?,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, NipTrnError> {
        if event.kind != KIND_TRAINING_CLOSEOUT {
            return Err(NipTrnError::InvalidKind {
                expected: KIND_TRAINING_CLOSEOUT,
                actual: event.kind,
            });
        }
        let network_id = required_tag_value(&event.tags, "network")?;
        let window_id = required_tag_value(&event.tags, "window")?;
        let status = required_tag_value(&event.tags, "status")?;
        let content = parse_json_content(event.content.as_str())?;
        let mut extra_tags = Vec::new();
        for tag in &event.tags {
            match tag_name(tag) {
                Some("network") | Some("window") | Some("status") | Some("assignment")
                | Some("artifact") | Some("policy") | Some("amount") | Some("p")
                | Some("reason") | Some("e") => {}
                _ => extra_tags.push(tag.clone()),
            }
        }
        let parsed = Self {
            network_id,
            window_id,
            status,
            content,
            assignment_id: trim_optional(find_tag_value(&event.tags, "assignment")),
            artifact_id: trim_optional(find_tag_value(&event.tags, "artifact")),
            policy_revision: trim_optional(find_tag_value(&event.tags, "policy")),
            amount_msats: trim_optional(find_tag_value(&event.tags, "amount")),
            actors: collect_pubkey_refs(&event.tags),
            reason_codes: collect_tag_values(&event.tags, "reason"),
            event_refs: collect_event_refs(&event.tags),
            extra_tags,
        };
        parsed.validate()?;
        Ok(parsed.normalize())
    }
}

fn parse_training_receipt_like_event(
    event: &Event,
) -> Result<(TrainingReceiptEvent, Value), NipTrnError> {
    let network_id = required_tag_value(&event.tags, "network")?;
    let window_id = required_tag_value(&event.tags, "window")?;
    let status = required_tag_value(&event.tags, "status")?;
    let content = parse_json_content(event.content.as_str())?;
    let mut extra_tags = Vec::new();
    for tag in &event.tags {
        match tag_name(tag) {
            Some("network") | Some("window") | Some("status") | Some("assignment")
            | Some("policy") | Some("role") | Some("artifact") | Some("checkpoint") | Some("p")
            | Some("reason") | Some("class") | Some("a") | Some("e") => {}
            _ => extra_tags.push(tag.clone()),
        }
    }
    let parsed = TrainingReceiptEvent {
        network_id,
        window_id,
        status,
        content: content.clone(),
        assignment_id: trim_optional(find_tag_value(&event.tags, "assignment")),
        policy_revision: trim_optional(find_tag_value(&event.tags, "policy")),
        role: trim_optional(find_tag_value(&event.tags, "role")),
        artifact_id: trim_optional(find_tag_value(&event.tags, "artifact")),
        checkpoint_id: trim_optional(find_tag_value(&event.tags, "checkpoint")),
        actors: collect_pubkey_refs(&event.tags),
        reason_codes: collect_tag_values(&event.tags, "reason"),
        classes: collect_tag_values(&event.tags, "class"),
        address_refs: collect_address_refs(&event.tags),
        event_refs: collect_event_refs(&event.tags),
        extra_tags,
    };
    parsed.validate()?;
    Ok((parsed.normalize(), content))
}

fn collect_pubkey_refs(tags: &[Vec<String>]) -> Vec<TrnPubkeyReference> {
    tags.iter()
        .filter_map(|tag| TrnPubkeyReference::from_tag(tag))
        .collect()
}

fn collect_event_refs(tags: &[Vec<String>]) -> Vec<TrnEventReference> {
    tags.iter()
        .filter_map(|tag| TrnEventReference::from_tag(tag))
        .collect()
}

fn collect_address_refs(tags: &[Vec<String>]) -> Vec<TrnAddressReference> {
    tags.iter()
        .filter_map(|tag| TrnAddressReference::from_tag(tag))
        .collect()
}

fn required_tag_value(tags: &[Vec<String>], name: &'static str) -> Result<String, NipTrnError> {
    trim_optional(find_tag_value(tags, name)).ok_or(NipTrnError::MissingTag(name))
}

fn parse_json_content(content: &str) -> Result<Value, NipTrnError> {
    let parsed: Value = serde_json::from_str(content)
        .map_err(|error| NipTrnError::InvalidJsonContent(error.to_string()))?;
    validate_json_object("content", &parsed)?;
    Ok(parsed)
}

fn serialize_json_content(content: &Value) -> Result<String, NipTrnError> {
    validate_json_object("content", content)?;
    serde_json::to_string(content)
        .map_err(|error| NipTrnError::InvalidJsonContent(error.to_string()))
}

fn validate_json_object(field: &'static str, content: &Value) -> Result<(), NipTrnError> {
    if !content.is_object() {
        return Err(NipTrnError::InvalidJsonContent(format!(
            "{field} must be a JSON object"
        )));
    }
    Ok(())
}

fn require_nonempty(field: &'static str, value: &str) -> Result<(), NipTrnError> {
    if value.trim().is_empty() {
        return Err(NipTrnError::MissingField(field));
    }
    Ok(())
}

fn normalize_option(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn trim_optional(value: Option<&str>) -> Option<String> {
    normalize_option(value)
}

fn normalize_vec(mut values: Vec<String>) -> Vec<String> {
    values = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
    values.sort();
    values.dedup();
    values
}

fn normalize_extra_tags(mut tags: Vec<Vec<String>>) -> Vec<Vec<String>> {
    for tag in &mut tags {
        for field in tag.iter_mut() {
            *field = field.trim().to_string();
        }
        while tag.last().is_some_and(|field| field.is_empty()) {
            tag.pop();
        }
    }
    tags.retain(|tag| !tag.is_empty() && !tag[0].is_empty());
    tags.sort();
    tags.dedup();
    tags
}

fn coordinate_for_kind(kind: u16, pubkey: &str, identifier: &str) -> Result<String, NipTrnError> {
    if !is_addressable_kind(kind) {
        return Err(NipTrnError::InvalidCoordinateKind(kind));
    }
    validate_lower_hex("pubkey", pubkey)?;
    require_nonempty("identifier", identifier)?;
    Ok(format!("{kind}:{pubkey}:{identifier}"))
}

fn validate_coordinate_string(coordinate: &str) -> Result<(), NipTrnError> {
    let mut parts = coordinate.splitn(3, ':');
    let kind = parts
        .next()
        .ok_or(NipTrnError::MissingField("coordinate_kind"))?
        .parse::<u16>()
        .map_err(|_| NipTrnError::InvalidCoordinateKind(0))?;
    let pubkey = parts
        .next()
        .ok_or(NipTrnError::MissingField("coordinate_pubkey"))?;
    let identifier = parts
        .next()
        .ok_or(NipTrnError::MissingField("coordinate_identifier"))?;
    if !is_addressable_kind(kind) {
        return Err(NipTrnError::InvalidCoordinateKind(kind));
    }
    validate_lower_hex("coordinate_pubkey", pubkey)?;
    require_nonempty("coordinate_identifier", identifier)?;
    Ok(())
}

fn validate_lower_hex(field: &'static str, value: &str) -> Result<(), NipTrnError> {
    let trimmed = value.trim();
    if trimmed.len() != 64
        || !trimmed
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(NipTrnError::InvalidHexField {
            field,
            value: value.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn fake_event(template: EventTemplate) -> Event {
        Event {
            id: "11".repeat(32),
            pubkey: "22".repeat(32),
            created_at: template.created_at,
            kind: template.kind,
            tags: template.tags,
            content: template.content,
            sig: "33".repeat(64),
        }
    }

    #[test]
    fn trn_network_contract_round_trips() {
        let event = TrainingNetworkContractEvent {
            identifier: "trainnet.alpha".to_string(),
            network_id: "trainnet.alpha".to_string(),
            status: "active".to_string(),
            content: json!({"schema": 1, "policy_family": "psion.train.v1"}),
            model_family: Some("psion".to_string()),
            window_cadence: Some("3600".to_string()),
            roles: vec!["validator".to_string(), "miner".to_string()],
            profiles: vec!["trn-discovery".to_string(), "trn-challenge".to_string()],
            address_refs: vec![TrnAddressReference::new(
                format!("39520:{}:bootstrap-artifact", "aa".repeat(32)),
                Some("wss://relay.example.com"),
                Some("bootstrap"),
            )],
            extra_tags: vec![vec!["run".to_string(), "run.alpha".to_string()]],
        };
        let template = event.to_event_template(1_774_160_001).expect("template");
        let parsed =
            TrainingNetworkContractEvent::from_event(&fake_event(template)).expect("parsed");
        assert_eq!(parsed, event.normalize());
        assert_eq!(
            parsed.coordinate(&"44".repeat(32)).expect("coordinate"),
            format!("39500:{}:trainnet.alpha", "44".repeat(32))
        );
    }

    #[test]
    fn trn_node_record_round_trips() {
        let event = TrainingNodeRecordEvent {
            identifier: "trainnet.alpha".to_string(),
            network_id: "trainnet.alpha".to_string(),
            status: "online".to_string(),
            content: json!({"label": "validator-alpha"}),
            roles: vec!["validator".to_string()],
            classes: vec!["audited".to_string()],
            build_digest: Some("sha256:build-alpha".to_string()),
            capabilities: vec![TrnCapability::new("backend", "cuda")],
            relay_urls: vec!["wss://relay.example.com".to_string()],
            extra_tags: Vec::new(),
        };
        let template = event.to_event_template(1_774_160_002).expect("template");
        let parsed = TrainingNodeRecordEvent::from_event(&fake_event(template)).expect("parsed");
        assert_eq!(parsed, event.normalize());
        assert_eq!(
            parsed.coordinate(&"55".repeat(32)).expect("coordinate"),
            format!("39501:{}:trainnet.alpha", "55".repeat(32))
        );
    }

    #[test]
    fn trn_window_round_trips() {
        let event = TrainingWindowEvent {
            identifier: "window.0001".to_string(),
            network_id: "trainnet.alpha".to_string(),
            status: "sealed".to_string(),
            content: json!({"window_id": "window.0001", "assignment_plan_count": 1}),
            policy_revision: Some("policy://validator/mvp/v1".to_string()),
            assignment_seed: Some("sha256:seed-alpha".to_string()),
            workload_family: Some("adapter-delta".to_string()),
            address_refs: vec![TrnAddressReference::new(
                format!("39520:{}:bootstrap-artifact", "66".repeat(32)),
                None::<String>,
                Some("bootstrap"),
            )],
            extra_tags: Vec::new(),
        };
        let template = event.to_event_template(1_774_160_003).expect("template");
        let parsed = TrainingWindowEvent::from_event(&fake_event(template)).expect("parsed");
        assert_eq!(parsed, event.normalize());
    }

    #[test]
    fn trn_receipt_round_trips() {
        let event = TrainingReceiptEvent {
            network_id: "trainnet.alpha".to_string(),
            window_id: "window.0001".to_string(),
            status: "assignment_published".to_string(),
            content: json!({"subject_pubkey": "77", "expected_artifact_class": "delta"}),
            assignment_id: Some("assign.alpha".to_string()),
            policy_revision: Some("policy://validator/mvp/v1".to_string()),
            role: Some("worker".to_string()),
            artifact_id: Some("artifact.delta.alpha".to_string()),
            checkpoint_id: Some("checkpoint.alpha".to_string()),
            actors: vec![
                TrnPubkeyReference::subject("77".repeat(32)),
                TrnPubkeyReference::coordinator("88".repeat(32)),
            ],
            reason_codes: vec!["ready".to_string()],
            classes: vec!["delta".to_string()],
            address_refs: vec![TrnAddressReference::new(
                format!("39510:{}:window.0001", "99".repeat(32)),
                None::<String>,
                Some("window"),
            )],
            event_refs: vec![TrnEventReference::new(
                "aa".repeat(32),
                None::<String>,
                Some("window"),
            )],
            extra_tags: Vec::new(),
        };
        let template = event.to_event_template(1_774_160_004).expect("template");
        let parsed = TrainingReceiptEvent::from_event(&fake_event(template)).expect("parsed");
        assert_eq!(parsed, event.normalize());
    }

    #[test]
    fn trn_validator_verdict_round_trips() {
        let event = TrainingValidatorVerdictEvent {
            network_id: "trainnet.alpha".to_string(),
            window_id: "window.0001".to_string(),
            status: "accepted".to_string(),
            content: json!({"verdict": "accepted"}),
            assignment_id: Some("assign.alpha".to_string()),
            artifact_id: Some("artifact.delta.alpha".to_string()),
            policy_revision: Some("policy://validator/mvp/v1".to_string()),
            validator_policy: Some("policy://validator/mvp/v1".to_string()),
            digest: Some("sha256:artifact-alpha".to_string()),
            actors: vec![
                TrnPubkeyReference::subject("bb".repeat(32)),
                TrnPubkeyReference::validator("cc".repeat(32)),
            ],
            reason_codes: vec!["passed".to_string()],
            event_refs: vec![TrnEventReference::new(
                "dd".repeat(32),
                None::<String>,
                Some("challenge"),
            )],
            extra_tags: Vec::new(),
        };
        let template = event.to_event_template(1_774_160_005).expect("template");
        let parsed =
            TrainingValidatorVerdictEvent::from_event(&fake_event(template)).expect("parsed");
        assert_eq!(parsed, event.normalize());
    }

    #[test]
    fn trn_artifact_locator_round_trips_and_score_snapshot_helper() {
        let event = TrainingArtifactLocatorEvent {
            identifier: "artifact.alpha".to_string(),
            network_id: "trainnet.alpha".to_string(),
            status: "accepted".to_string(),
            content: json!({"artifact_id": "artifact.alpha"}),
            artifact_id: Some("artifact.alpha".to_string()),
            checkpoint_id: Some("checkpoint.alpha".to_string()),
            manifest_digest: Some("sha256:manifest-alpha".to_string()),
            file_digest: Some("sha256:file-alpha".to_string()),
            url_hint: Some("gs://bucket/object".to_string()),
            artifact_class: Some("proof".to_string()),
            window_id: Some("window.0001".to_string()),
            policy_revision: Some("policy://validator/mvp/v1".to_string()),
            reason_codes: vec!["stored".to_string()],
            address_refs: vec![TrnAddressReference::new(
                format!("39520:{}:artifact.alpha.parent", "ee".repeat(32)),
                None::<String>,
                Some("source"),
            )],
            extra_tags: Vec::new(),
        };
        let template = event.to_event_template(1_774_160_006).expect("template");
        let parsed =
            TrainingArtifactLocatorEvent::from_event(&fake_event(template)).expect("parsed");
        assert_eq!(parsed, event.normalize());

        let score = TrainingArtifactLocatorEvent::score_snapshot(
            "score.window.0001",
            "trainnet.alpha",
            "accepted",
            "sha256:score-alpha",
            "gs://bucket/score.json",
            json!({"scope": "window", "window_id": "window.0001"}),
        );
        assert_eq!(score.artifact_class.as_deref(), Some("score"));
        assert_eq!(score.file_digest.as_deref(), Some("sha256:score-alpha"));
    }

    #[test]
    fn trn_closeout_round_trips() {
        let event = TrainingCloseoutEvent {
            network_id: "trainnet.alpha".to_string(),
            window_id: "window.0001".to_string(),
            status: "rewarded".to_string(),
            content: json!({"closeout_status": "rewarded"}),
            assignment_id: Some("assign.alpha".to_string()),
            artifact_id: Some("artifact.delta.alpha".to_string()),
            policy_revision: Some("policy://validator/mvp/v1".to_string()),
            amount_msats: Some("1000".to_string()),
            actors: vec![
                TrnPubkeyReference::subject("ff".repeat(32)),
                TrnPubkeyReference::coordinator("00".repeat(32)),
            ],
            reason_codes: vec!["payout_eligible".to_string()],
            event_refs: vec![TrnEventReference::new(
                "11".repeat(32),
                None::<String>,
                Some("verdict"),
            )],
            extra_tags: Vec::new(),
        };
        let template = event.to_event_template(1_774_160_007).expect("template");
        let parsed = TrainingCloseoutEvent::from_event(&fake_event(template)).expect("parsed");
        assert_eq!(parsed, event.normalize());
    }

    #[test]
    fn trn_generic_parser_routes_kinds() {
        let template = TrainingReceiptEvent {
            network_id: "trainnet.alpha".to_string(),
            window_id: "window.0001".to_string(),
            status: "artifact_uploaded".to_string(),
            content: json!({"ok": true}),
            assignment_id: None,
            policy_revision: None,
            role: None,
            artifact_id: None,
            checkpoint_id: None,
            actors: Vec::new(),
            reason_codes: Vec::new(),
            classes: Vec::new(),
            address_refs: Vec::new(),
            event_refs: Vec::new(),
            extra_tags: Vec::new(),
        }
        .to_event_template(1_774_160_008)
        .expect("template");
        let parsed = TrnEvent::from_event(&fake_event(template)).expect("parsed");
        assert!(matches!(parsed, TrnEvent::Receipt(_)));
        parsed.validate().expect("validated");
    }

    #[test]
    fn trn_required_tags_are_validated() {
        let missing_network = Event {
            id: "11".repeat(32),
            pubkey: "22".repeat(32),
            created_at: 1_774_160_009,
            kind: KIND_TRAINING_RECEIPT,
            tags: vec![
                vec!["window".to_string(), "window.0001".to_string()],
                vec!["status".to_string(), "window_sealed".to_string()],
            ],
            content: json!({"ok": true}).to_string(),
            sig: "33".repeat(64),
        };
        let error =
            TrainingReceiptEvent::from_event(&missing_network).expect_err("missing network");
        assert!(matches!(error, NipTrnError::MissingTag("network")));
    }

    #[test]
    fn trn_actor_tag_helpers_round_trip() {
        let subject = TrnPubkeyReference::subject("aa".repeat(32));
        assert_eq!(
            TrnPubkeyReference::from_tag(&subject.to_tag()).expect("subject tag"),
            subject
        );

        let coordinator = TrnPubkeyReference::coordinator("bb".repeat(32));
        assert_eq!(coordinator.marker.as_deref(), Some("coordinator"));

        let validator = TrnPubkeyReference::validator("cc".repeat(32));
        assert_eq!(validator.marker.as_deref(), Some("validator"));
    }

    #[test]
    fn trn_kind_helpers_cover_all_training_kinds() {
        for kind in [
            KIND_TRAINING_NETWORK_CONTRACT,
            KIND_TRAINING_NODE_RECORD,
            KIND_TRAINING_WINDOW,
            KIND_TRAINING_RECEIPT,
            KIND_TRAINING_VALIDATOR_VERDICT,
            KIND_TRAINING_ARTIFACT_LOCATOR,
            KIND_TRAINING_CLOSEOUT,
        ] {
            assert!(is_trn_kind(kind));
        }
        assert!(!is_trn_kind(1));
    }
}
