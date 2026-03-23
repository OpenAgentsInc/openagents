use super::{
    JobFeedback, JobInput, JobParam, JobRequest, JobResult, JobStatus, Nip90Error,
    create_job_feedback_event, create_job_request_event, create_job_result_event, get_request_kind,
};
use crate::nip01::{Event, EventTemplate};
use crate::nip_ds::{
    AddressableEventCoordinate, AddressableEventReference, KIND_DATASET_LISTING,
    KIND_DATASET_OFFER,
};
use crate::tag_parsing::find_tag_value;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub const OPENAGENTS_DATA_VENDING_PROFILE: &str = "openagents.ds-dvm.v1";
pub const OPENAGENTS_DATA_VENDING_LEGACY_PROFILE: &str = "openagents.data-vending.v1";

const PARAM_PROFILE: &str = "oa_profile";
const PARAM_ASSET_REF: &str = "oa_asset_ref";
const PARAM_LISTING_REF: &str = "oa_listing_ref";
const PARAM_OFFER_REF: &str = "oa_offer_ref";
const PARAM_ASSET_ID: &str = "oa_asset_id";
const PARAM_GRANT_ID: &str = "oa_grant_id";
const PARAM_SCOPE: &str = "oa_scope";
const PARAM_DELIVERY_MODE: &str = "oa_delivery_mode";
const PARAM_PREVIEW_POSTURE: &str = "oa_preview_posture";

const TAG_PROFILE: &str = "oa_profile";
const TAG_ASSET_REF: &str = "oa_asset_ref";
const TAG_ASSET_ID: &str = "oa_asset_id";
const TAG_GRANT_ID: &str = "oa_grant_id";
const TAG_DELIVERY_BUNDLE_ID: &str = "oa_delivery_bundle_id";
const TAG_DELIVERY_MODE: &str = "oa_delivery_mode";
const TAG_PREVIEW_POSTURE: &str = "oa_preview_posture";
const TAG_DELIVERY_REF: &str = "oa_delivery_ref";
const TAG_DELIVERY_DIGEST: &str = "x";
const TAG_REASON_CODE: &str = "oa_reason_code";
const TAG_REVOCATION_ID: &str = "oa_revocation_id";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataVendingDeliveryMode {
    InlinePreview,
    #[default]
    EncryptedPointer,
    DeliveryBundleRef,
}

impl DataVendingDeliveryMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InlinePreview => "inline_preview",
            Self::EncryptedPointer => "encrypted_pointer",
            Self::DeliveryBundleRef => "delivery_bundle_ref",
        }
    }
}

impl FromStr for DataVendingDeliveryMode {
    type Err = Nip90Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "inline_preview" => Ok(Self::InlinePreview),
            "encrypted_pointer" => Ok(Self::EncryptedPointer),
            "delivery_bundle_ref" => Ok(Self::DeliveryBundleRef),
            _ => Err(Nip90Error::Serialization(format!(
                "invalid data vending delivery mode: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataVendingPreviewPosture {
    #[default]
    None,
    MetadataOnly,
    InlinePreview,
}

impl DataVendingPreviewPosture {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::MetadataOnly => "metadata_only",
            Self::InlinePreview => "inline_preview",
        }
    }
}

impl FromStr for DataVendingPreviewPosture {
    type Err = Nip90Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "none" | "no_preview" => Ok(Self::None),
            "metadata_only" => Ok(Self::MetadataOnly),
            "inline_preview" => Ok(Self::InlinePreview),
            _ => Err(Nip90Error::Serialization(format!(
                "invalid data vending preview posture: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataVendingRequest {
    pub profile_id: String,
    pub request_kind: u16,
    pub asset_ref: String,
    pub listing_ref: Option<AddressableEventReference>,
    pub offer_ref: Option<AddressableEventReference>,
    pub asset_id: Option<String>,
    pub grant_id: Option<String>,
    pub permission_scopes: Vec<String>,
    pub delivery_mode: DataVendingDeliveryMode,
    pub preview_posture: DataVendingPreviewPosture,
    pub inputs: Vec<JobInput>,
    pub output: Option<String>,
    pub bid: Option<u64>,
    pub relays: Vec<String>,
    pub service_providers: Vec<String>,
    pub encrypted: bool,
    pub content: String,
}

impl DataVendingRequest {
    pub fn new(
        request_kind: u16,
        asset_ref: impl Into<String>,
        permission_scope: impl Into<String>,
    ) -> Result<Self, Nip90Error> {
        let _ = JobRequest::new(request_kind)?;
        Ok(Self {
            profile_id: OPENAGENTS_DATA_VENDING_PROFILE.to_string(),
            request_kind,
            asset_ref: asset_ref.into(),
            listing_ref: None,
            offer_ref: None,
            asset_id: None,
            grant_id: None,
            permission_scopes: vec![permission_scope.into()],
            delivery_mode: DataVendingDeliveryMode::default(),
            preview_posture: DataVendingPreviewPosture::default(),
            inputs: Vec::new(),
            output: None,
            bid: None,
            relays: Vec::new(),
            service_providers: Vec::new(),
            encrypted: false,
            content: String::new(),
        })
    }

    pub fn with_profile_id(mut self, profile_id: impl Into<String>) -> Self {
        self.profile_id = profile_id.into();
        self
    }

    pub fn with_listing_ref(mut self, listing_ref: AddressableEventReference) -> Self {
        self.listing_ref = Some(listing_ref);
        self
    }

    pub fn with_listing_coordinate(
        mut self,
        listing_coordinate: impl AsRef<str>,
    ) -> Result<Self, Nip90Error> {
        self.listing_ref = Some(parse_ds_reference(
            listing_coordinate.as_ref(),
            KIND_DATASET_LISTING,
        )?);
        Ok(self)
    }

    pub fn with_offer_ref(mut self, offer_ref: AddressableEventReference) -> Self {
        self.offer_ref = Some(offer_ref);
        self
    }

    pub fn with_offer_coordinate(
        mut self,
        offer_coordinate: impl AsRef<str>,
    ) -> Result<Self, Nip90Error> {
        self.offer_ref = Some(parse_ds_reference(
            offer_coordinate.as_ref(),
            KIND_DATASET_OFFER,
        )?);
        Ok(self)
    }

    pub fn with_asset_id(mut self, asset_id: impl Into<String>) -> Self {
        self.asset_id = Some(asset_id.into());
        self
    }

    pub fn with_grant_id(mut self, grant_id: impl Into<String>) -> Self {
        self.grant_id = Some(grant_id.into());
        self
    }

    pub fn add_scope(mut self, scope: impl Into<String>) -> Self {
        self.permission_scopes.push(scope.into());
        self
    }

    pub fn with_delivery_mode(mut self, delivery_mode: DataVendingDeliveryMode) -> Self {
        self.delivery_mode = delivery_mode;
        self
    }

    pub fn with_preview_posture(mut self, preview_posture: DataVendingPreviewPosture) -> Self {
        self.preview_posture = preview_posture;
        self
    }

    pub fn add_input(mut self, input: JobInput) -> Self {
        self.inputs.push(input);
        self
    }

    pub fn with_output(mut self, output: impl Into<String>) -> Self {
        self.output = Some(output.into());
        self
    }

    pub fn with_bid(mut self, bid: u64) -> Self {
        self.bid = Some(bid);
        self
    }

    pub fn add_relay(mut self, relay: impl Into<String>) -> Self {
        self.relays.push(relay.into());
        self
    }

    pub fn add_service_provider(mut self, pubkey: impl Into<String>) -> Self {
        self.service_providers.push(pubkey.into());
        self
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    pub fn with_encrypted_content(mut self, content: impl Into<String>) -> Self {
        self.encrypted = true;
        self.content = content.into();
        self
    }

    pub fn to_job_request(&self) -> Result<JobRequest, Nip90Error> {
        ensure_non_empty(self.asset_ref.as_str(), PARAM_ASSET_REF)?;
        if self.permission_scopes.is_empty() {
            return Err(Nip90Error::MissingTag(PARAM_SCOPE.to_string()));
        }

        let mut request = JobRequest::new(self.request_kind)?;
        request.inputs = self.inputs.clone();
        request.output = self.output.clone();
        request.bid = self.bid;
        request.relays = self.relays.clone();
        request.service_providers = self.service_providers.clone();
        request.encrypted = self.encrypted;
        request.content = self.content.clone();
        request.params = vec![
            JobParam::new(PARAM_PROFILE, self.profile_id.clone()),
            JobParam::new(PARAM_ASSET_REF, self.asset_ref.clone()),
            JobParam::new(PARAM_DELIVERY_MODE, self.delivery_mode.as_str()),
            JobParam::new(PARAM_PREVIEW_POSTURE, self.preview_posture.as_str()),
        ];
        if let Some(listing_ref) = &self.listing_ref {
            request.params.push(JobParam::new(
                PARAM_LISTING_REF,
                listing_ref.coordinate.to_string(),
            ));
        }
        if let Some(offer_ref) = &self.offer_ref {
            request.params.push(JobParam::new(
                PARAM_OFFER_REF,
                offer_ref.coordinate.to_string(),
            ));
        }
        if let Some(asset_id) = self.asset_id.as_deref() {
            request.params.push(JobParam::new(PARAM_ASSET_ID, asset_id));
        }
        if let Some(grant_id) = self.grant_id.as_deref() {
            request.params.push(JobParam::new(PARAM_GRANT_ID, grant_id));
        }
        request.params.extend(
            self.permission_scopes
                .iter()
                .cloned()
                .map(|scope| JobParam::new(PARAM_SCOPE, scope)),
        );
        Ok(request)
    }

    pub fn to_event_template(&self) -> Result<EventTemplate, Nip90Error> {
        let mut template = create_job_request_event(&self.to_job_request()?);
        let listing_ref = self
            .listing_ref
            .as_ref()
            .ok_or_else(|| Nip90Error::MissingTag("a (dataset listing)".to_string()))?;
        template.tags.push(listing_ref.to_tag());
        if let Some(offer_ref) = &self.offer_ref {
            template.tags.push(offer_ref.to_tag());
        }
        Ok(template)
    }

    pub fn from_job_request(request: JobRequest) -> Result<Self, Nip90Error> {
        let profile_id = ensure_profile(request_param_value(&request.params, PARAM_PROFILE))?;
        let asset_ref = required_param(&request.params, PARAM_ASSET_REF)?;
        let permission_scopes = request_param_values(&request.params, PARAM_SCOPE);
        if permission_scopes.is_empty() {
            return Err(Nip90Error::MissingTag(PARAM_SCOPE.to_string()));
        }
        let delivery_mode = DataVendingDeliveryMode::from_str(required_param(
            &request.params,
            PARAM_DELIVERY_MODE,
        )?)?;
        let preview_posture = DataVendingPreviewPosture::from_str(required_param(
            &request.params,
            PARAM_PREVIEW_POSTURE,
        )?)?;

        Ok(Self {
            profile_id,
            request_kind: request.kind,
            asset_ref: asset_ref.to_string(),
            listing_ref: optional_param_reference(
                &request.params,
                PARAM_LISTING_REF,
                KIND_DATASET_LISTING,
            )?,
            offer_ref: optional_param_reference(
                &request.params,
                PARAM_OFFER_REF,
                KIND_DATASET_OFFER,
            )?,
            asset_id: request_param_value(&request.params, PARAM_ASSET_ID).map(str::to_string),
            grant_id: request_param_value(&request.params, PARAM_GRANT_ID).map(str::to_string),
            permission_scopes,
            delivery_mode,
            preview_posture,
            inputs: request.inputs,
            output: request.output,
            bid: request.bid,
            relays: request.relays,
            service_providers: request.service_providers,
            encrypted: request.encrypted,
            content: request.content,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, Nip90Error> {
        let mut request = Self::from_job_request(JobRequest::from_event(event)?)?;
        if let Some(listing_ref) = first_ds_reference(&event.tags, KIND_DATASET_LISTING)? {
            request.listing_ref = Some(listing_ref);
        }
        if let Some(offer_ref) = first_ds_reference(&event.tags, KIND_DATASET_OFFER)? {
            request.offer_ref = Some(offer_ref);
        }
        Ok(request)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataVendingResult {
    pub profile_id: String,
    pub request_kind: u16,
    pub request_id: String,
    pub request_relay: Option<String>,
    pub customer_pubkey: String,
    pub content: String,
    pub request: Option<String>,
    pub inputs: Vec<JobInput>,
    pub amount: Option<u64>,
    pub bolt11: Option<String>,
    pub encrypted: bool,
    pub asset_ref: String,
    pub listing_ref: Option<AddressableEventReference>,
    pub offer_ref: Option<AddressableEventReference>,
    pub asset_id: Option<String>,
    pub grant_id: Option<String>,
    pub delivery_bundle_id: String,
    pub delivery_mode: DataVendingDeliveryMode,
    pub preview_posture: DataVendingPreviewPosture,
    pub delivery_ref: Option<String>,
    pub delivery_digest: Option<String>,
}

impl DataVendingResult {
    pub fn new(
        request_kind: u16,
        request_id: impl Into<String>,
        customer_pubkey: impl Into<String>,
        asset_ref: impl Into<String>,
        delivery_bundle_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<Self, Nip90Error> {
        let _ = JobResult::new(request_kind, "", "", "")?;
        Ok(Self {
            profile_id: OPENAGENTS_DATA_VENDING_PROFILE.to_string(),
            request_kind,
            request_id: request_id.into(),
            request_relay: None,
            customer_pubkey: customer_pubkey.into(),
            content: content.into(),
            request: None,
            inputs: Vec::new(),
            amount: None,
            bolt11: None,
            encrypted: false,
            asset_ref: asset_ref.into(),
            listing_ref: None,
            offer_ref: None,
            asset_id: None,
            grant_id: None,
            delivery_bundle_id: delivery_bundle_id.into(),
            delivery_mode: DataVendingDeliveryMode::default(),
            preview_posture: DataVendingPreviewPosture::default(),
            delivery_ref: None,
            delivery_digest: None,
        })
    }

    pub fn with_profile_id(mut self, profile_id: impl Into<String>) -> Self {
        self.profile_id = profile_id.into();
        self
    }

    pub fn with_listing_ref(mut self, listing_ref: AddressableEventReference) -> Self {
        self.listing_ref = Some(listing_ref);
        self
    }

    pub fn with_listing_coordinate(
        mut self,
        listing_coordinate: impl AsRef<str>,
    ) -> Result<Self, Nip90Error> {
        self.listing_ref = Some(parse_ds_reference(
            listing_coordinate.as_ref(),
            KIND_DATASET_LISTING,
        )?);
        Ok(self)
    }

    pub fn with_offer_ref(mut self, offer_ref: AddressableEventReference) -> Self {
        self.offer_ref = Some(offer_ref);
        self
    }

    pub fn with_offer_coordinate(
        mut self,
        offer_coordinate: impl AsRef<str>,
    ) -> Result<Self, Nip90Error> {
        self.offer_ref = Some(parse_ds_reference(
            offer_coordinate.as_ref(),
            KIND_DATASET_OFFER,
        )?);
        Ok(self)
    }

    pub fn with_asset_id(mut self, asset_id: impl Into<String>) -> Self {
        self.asset_id = Some(asset_id.into());
        self
    }

    pub fn with_request(mut self, request_json: impl Into<String>) -> Self {
        self.request = Some(request_json.into());
        self
    }

    pub fn with_request_relay(mut self, relay: impl Into<String>) -> Self {
        self.request_relay = Some(relay.into());
        self
    }

    pub fn add_input(mut self, input: JobInput) -> Self {
        self.inputs.push(input);
        self
    }

    pub fn with_amount(mut self, amount: u64, bolt11: Option<String>) -> Self {
        self.amount = Some(amount);
        self.bolt11 = bolt11;
        self
    }

    pub fn with_encrypted_content(mut self) -> Self {
        self.encrypted = true;
        self
    }

    pub fn with_grant_id(mut self, grant_id: impl Into<String>) -> Self {
        self.grant_id = Some(grant_id.into());
        self
    }

    pub fn with_delivery_mode(mut self, delivery_mode: DataVendingDeliveryMode) -> Self {
        self.delivery_mode = delivery_mode;
        self
    }

    pub fn with_preview_posture(mut self, preview_posture: DataVendingPreviewPosture) -> Self {
        self.preview_posture = preview_posture;
        self
    }

    pub fn with_delivery_ref(mut self, delivery_ref: impl Into<String>) -> Self {
        self.delivery_ref = Some(delivery_ref.into());
        self
    }

    pub fn with_delivery_digest(mut self, delivery_digest: impl Into<String>) -> Self {
        self.delivery_digest = Some(delivery_digest.into());
        self
    }

    pub fn to_job_result(&self) -> Result<JobResult, Nip90Error> {
        ensure_non_empty(self.request_id.as_str(), "e (request event id)")?;
        ensure_non_empty(self.customer_pubkey.as_str(), "p (customer pubkey)")?;
        ensure_non_empty(self.asset_ref.as_str(), TAG_ASSET_REF)?;
        ensure_non_empty(self.delivery_bundle_id.as_str(), TAG_DELIVERY_BUNDLE_ID)?;

        let mut result = JobResult::new(
            self.request_kind,
            self.request_id.clone(),
            self.customer_pubkey.clone(),
            self.content.clone(),
        )?;
        result.request = self.request.clone();
        result.request_relay = self.request_relay.clone();
        result.inputs = self.inputs.clone();
        result.amount = self.amount;
        result.bolt11 = self.bolt11.clone();
        result.encrypted = self.encrypted;
        Ok(result)
    }

    pub fn to_event_template(&self) -> Result<EventTemplate, Nip90Error> {
        let mut template = create_job_result_event(&self.to_job_result()?);
        template.tags.push(vec![
            TAG_PROFILE.to_string(),
            self.profile_id.clone(),
        ]);
        let listing_ref = self
            .listing_ref
            .as_ref()
            .ok_or_else(|| Nip90Error::MissingTag("a (dataset listing)".to_string()))?;
        template.tags.push(listing_ref.to_tag());
        if let Some(offer_ref) = &self.offer_ref {
            template.tags.push(offer_ref.to_tag());
        }
        template
            .tags
            .push(vec![TAG_ASSET_REF.to_string(), self.asset_ref.clone()]);
        template.tags.push(vec![
            TAG_DELIVERY_BUNDLE_ID.to_string(),
            self.delivery_bundle_id.clone(),
        ]);
        template.tags.push(vec![
            TAG_DELIVERY_MODE.to_string(),
            self.delivery_mode.as_str().to_string(),
        ]);
        template.tags.push(vec![
            TAG_PREVIEW_POSTURE.to_string(),
            self.preview_posture.as_str().to_string(),
        ]);
        push_optional_tag(&mut template.tags, TAG_ASSET_ID, self.asset_id.as_deref());
        push_optional_tag(&mut template.tags, TAG_GRANT_ID, self.grant_id.as_deref());
        push_optional_tag(
            &mut template.tags,
            TAG_DELIVERY_REF,
            self.delivery_ref.as_deref(),
        );
        push_optional_tag(
            &mut template.tags,
            TAG_DELIVERY_DIGEST,
            self.delivery_digest.as_deref(),
        );
        Ok(template)
    }

    pub fn from_event(event: &Event) -> Result<Self, Nip90Error> {
        let result = JobResult::from_event(event)?;
        let profile_id = ensure_profile(find_tag_value(&event.tags, TAG_PROFILE))?;
        let request_kind = get_request_kind(event.kind)
            .ok_or_else(|| Nip90Error::InvalidKind(event.kind, "6000-6999".to_string()))?;

        Ok(Self {
            profile_id,
            request_kind,
            request_id: result.request_id,
            request_relay: result.request_relay,
            customer_pubkey: result.customer_pubkey,
            content: result.content,
            request: result.request,
            inputs: result.inputs,
            amount: result.amount,
            bolt11: result.bolt11,
            encrypted: result.encrypted,
            asset_ref: required_tag_value(&event.tags, TAG_ASSET_REF)?,
            listing_ref: first_ds_reference(&event.tags, KIND_DATASET_LISTING)?,
            offer_ref: first_ds_reference(&event.tags, KIND_DATASET_OFFER)?,
            asset_id: find_tag_value(&event.tags, TAG_ASSET_ID).map(str::to_owned),
            grant_id: find_tag_value(&event.tags, TAG_GRANT_ID).map(str::to_owned),
            delivery_bundle_id: required_tag_value(&event.tags, TAG_DELIVERY_BUNDLE_ID)?,
            delivery_mode: DataVendingDeliveryMode::from_str(&required_tag_value(
                &event.tags,
                TAG_DELIVERY_MODE,
            )?)?,
            preview_posture: DataVendingPreviewPosture::from_str(&required_tag_value(
                &event.tags,
                TAG_PREVIEW_POSTURE,
            )?)?,
            delivery_ref: find_tag_value(&event.tags, TAG_DELIVERY_REF).map(str::to_owned),
            delivery_digest: find_tag_value(&event.tags, TAG_DELIVERY_DIGEST).map(str::to_owned),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataVendingFeedback {
    pub profile_id: String,
    pub status: JobStatus,
    pub status_extra: Option<String>,
    pub request_id: String,
    pub request_relay: Option<String>,
    pub customer_pubkey: String,
    pub content: String,
    pub amount: Option<u64>,
    pub bolt11: Option<String>,
    pub asset_ref: String,
    pub listing_ref: Option<AddressableEventReference>,
    pub offer_ref: Option<AddressableEventReference>,
    pub asset_id: Option<String>,
    pub grant_id: Option<String>,
    pub delivery_bundle_id: Option<String>,
    pub reason_code: Option<String>,
    pub revocation_id: Option<String>,
}

impl DataVendingFeedback {
    pub fn new(
        status: JobStatus,
        request_id: impl Into<String>,
        customer_pubkey: impl Into<String>,
        asset_ref: impl Into<String>,
    ) -> Self {
        Self {
            profile_id: OPENAGENTS_DATA_VENDING_PROFILE.to_string(),
            status,
            status_extra: None,
            request_id: request_id.into(),
            request_relay: None,
            customer_pubkey: customer_pubkey.into(),
            content: String::new(),
            amount: None,
            bolt11: None,
            asset_ref: asset_ref.into(),
            listing_ref: None,
            offer_ref: None,
            asset_id: None,
            grant_id: None,
            delivery_bundle_id: None,
            reason_code: None,
            revocation_id: None,
        }
    }

    pub fn with_profile_id(mut self, profile_id: impl Into<String>) -> Self {
        self.profile_id = profile_id.into();
        self
    }

    pub fn with_listing_ref(mut self, listing_ref: AddressableEventReference) -> Self {
        self.listing_ref = Some(listing_ref);
        self
    }

    pub fn with_listing_coordinate(
        mut self,
        listing_coordinate: impl AsRef<str>,
    ) -> Result<Self, Nip90Error> {
        self.listing_ref = Some(parse_ds_reference(
            listing_coordinate.as_ref(),
            KIND_DATASET_LISTING,
        )?);
        Ok(self)
    }

    pub fn with_offer_ref(mut self, offer_ref: AddressableEventReference) -> Self {
        self.offer_ref = Some(offer_ref);
        self
    }

    pub fn with_offer_coordinate(
        mut self,
        offer_coordinate: impl AsRef<str>,
    ) -> Result<Self, Nip90Error> {
        self.offer_ref = Some(parse_ds_reference(
            offer_coordinate.as_ref(),
            KIND_DATASET_OFFER,
        )?);
        Ok(self)
    }

    pub fn with_asset_id(mut self, asset_id: impl Into<String>) -> Self {
        self.asset_id = Some(asset_id.into());
        self
    }

    pub fn with_status_extra(mut self, status_extra: impl Into<String>) -> Self {
        self.status_extra = Some(status_extra.into());
        self
    }

    pub fn with_request_relay(mut self, relay: impl Into<String>) -> Self {
        self.request_relay = Some(relay.into());
        self
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    pub fn with_amount(mut self, amount: u64, bolt11: Option<String>) -> Self {
        self.amount = Some(amount);
        self.bolt11 = bolt11;
        self
    }

    pub fn with_grant_id(mut self, grant_id: impl Into<String>) -> Self {
        self.grant_id = Some(grant_id.into());
        self
    }

    pub fn with_delivery_bundle_id(mut self, delivery_bundle_id: impl Into<String>) -> Self {
        self.delivery_bundle_id = Some(delivery_bundle_id.into());
        self
    }

    pub fn with_reason_code(mut self, reason_code: impl Into<String>) -> Self {
        self.reason_code = Some(reason_code.into());
        self
    }

    pub fn with_revocation_id(mut self, revocation_id: impl Into<String>) -> Self {
        self.revocation_id = Some(revocation_id.into());
        self
    }

    pub fn to_job_feedback(&self) -> Result<JobFeedback, Nip90Error> {
        ensure_non_empty(self.request_id.as_str(), "e (request event id)")?;
        ensure_non_empty(self.customer_pubkey.as_str(), "p (customer pubkey)")?;
        ensure_non_empty(self.asset_ref.as_str(), TAG_ASSET_REF)?;

        let mut feedback = JobFeedback::new(
            self.status.clone(),
            self.request_id.clone(),
            self.customer_pubkey.clone(),
        );
        feedback.status_extra = self.status_extra.clone();
        feedback.request_relay = self.request_relay.clone();
        feedback.content = self.content.clone();
        feedback.amount = self.amount;
        feedback.bolt11 = self.bolt11.clone();
        Ok(feedback)
    }

    pub fn to_event_template(&self) -> Result<EventTemplate, Nip90Error> {
        let mut template = create_job_feedback_event(&self.to_job_feedback()?);
        template.tags.push(vec![
            TAG_PROFILE.to_string(),
            self.profile_id.clone(),
        ]);
        let listing_ref = self
            .listing_ref
            .as_ref()
            .ok_or_else(|| Nip90Error::MissingTag("a (dataset listing)".to_string()))?;
        template.tags.push(listing_ref.to_tag());
        if let Some(offer_ref) = &self.offer_ref {
            template.tags.push(offer_ref.to_tag());
        }
        template
            .tags
            .push(vec![TAG_ASSET_REF.to_string(), self.asset_ref.clone()]);
        push_optional_tag(&mut template.tags, TAG_ASSET_ID, self.asset_id.as_deref());
        push_optional_tag(&mut template.tags, TAG_GRANT_ID, self.grant_id.as_deref());
        push_optional_tag(
            &mut template.tags,
            TAG_DELIVERY_BUNDLE_ID,
            self.delivery_bundle_id.as_deref(),
        );
        push_optional_tag(
            &mut template.tags,
            TAG_REASON_CODE,
            self.reason_code.as_deref(),
        );
        push_optional_tag(
            &mut template.tags,
            TAG_REVOCATION_ID,
            self.revocation_id.as_deref(),
        );
        Ok(template)
    }

    pub fn from_event(event: &Event) -> Result<Self, Nip90Error> {
        let feedback = JobFeedback::from_event(event)?;
        let profile_id = ensure_profile(find_tag_value(&event.tags, TAG_PROFILE))?;

        Ok(Self {
            profile_id,
            status: feedback.status,
            status_extra: feedback.status_extra,
            request_id: feedback.request_id,
            request_relay: feedback.request_relay,
            customer_pubkey: feedback.customer_pubkey,
            content: feedback.content,
            amount: feedback.amount,
            bolt11: feedback.bolt11,
            asset_ref: required_tag_value(&event.tags, TAG_ASSET_REF)?,
            listing_ref: first_ds_reference(&event.tags, KIND_DATASET_LISTING)?,
            offer_ref: first_ds_reference(&event.tags, KIND_DATASET_OFFER)?,
            asset_id: find_tag_value(&event.tags, TAG_ASSET_ID).map(str::to_owned),
            grant_id: find_tag_value(&event.tags, TAG_GRANT_ID).map(str::to_owned),
            delivery_bundle_id: find_tag_value(&event.tags, TAG_DELIVERY_BUNDLE_ID)
                .map(str::to_owned),
            reason_code: find_tag_value(&event.tags, TAG_REASON_CODE).map(str::to_owned),
            revocation_id: find_tag_value(&event.tags, TAG_REVOCATION_ID).map(str::to_owned),
        })
    }
}

pub fn create_data_vending_request_event(
    request: &DataVendingRequest,
) -> Result<EventTemplate, Nip90Error> {
    request.to_event_template()
}

pub fn create_data_vending_result_event(
    result: &DataVendingResult,
) -> Result<EventTemplate, Nip90Error> {
    result.to_event_template()
}

pub fn create_data_vending_feedback_event(
    feedback: &DataVendingFeedback,
) -> Result<EventTemplate, Nip90Error> {
    feedback.to_event_template()
}

fn ensure_non_empty(value: &str, field: &str) -> Result<(), Nip90Error> {
    if value.trim().is_empty() {
        Err(Nip90Error::MissingTag(field.to_string()))
    } else {
        Ok(())
    }
}

fn ensure_profile(value: Option<&str>) -> Result<String, Nip90Error> {
    match value {
        Some(value)
            if value == OPENAGENTS_DATA_VENDING_PROFILE
                || value == OPENAGENTS_DATA_VENDING_LEGACY_PROFILE =>
        {
            Ok(value.to_string())
        }
        Some(value) => Err(Nip90Error::Serialization(format!(
            "unexpected data vending profile: {value}"
        ))),
        None => Err(Nip90Error::MissingTag(PARAM_PROFILE.to_string())),
    }
}

fn parse_ds_reference(
    coordinate: &str,
    expected_kind: u16,
) -> Result<AddressableEventReference, Nip90Error> {
    let coordinate = AddressableEventCoordinate::parse(coordinate).map_err(|error| {
        Nip90Error::Serialization(format!("invalid dataset coordinate `{coordinate}`: {error}"))
    })?;
    if coordinate.kind != expected_kind {
        return Err(Nip90Error::Serialization(format!(
            "dataset coordinate kind {} does not match expected {expected_kind}",
            coordinate.kind
        )));
    }
    Ok(AddressableEventReference::new(coordinate))
}

fn optional_param_reference(
    params: &[JobParam],
    key: &str,
    expected_kind: u16,
) -> Result<Option<AddressableEventReference>, Nip90Error> {
    match request_param_value(params, key) {
        Some(value) => parse_ds_reference(value, expected_kind).map(Some),
        None => Ok(None),
    }
}

fn first_ds_reference(
    tags: &[Vec<String>],
    expected_kind: u16,
) -> Result<Option<AddressableEventReference>, Nip90Error> {
    for tag in tags {
        if tag.first().is_none_or(|value| value != "a") {
            continue;
        }
        let reference = AddressableEventReference::from_tag(tag).map_err(|error| {
            Nip90Error::Serialization(format!("invalid dataset address tag: {error}"))
        })?;
        if reference.coordinate.kind == expected_kind {
            return Ok(Some(reference));
        }
    }
    Ok(None)
}

fn request_param_value<'a>(params: &'a [JobParam], key: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|param| param.key == key && !param.value.trim().is_empty())
        .map(|param| param.value.as_str())
}

fn request_param_values(params: &[JobParam], key: &str) -> Vec<String> {
    params
        .iter()
        .filter(|param| param.key == key && !param.value.trim().is_empty())
        .map(|param| param.value.clone())
        .collect()
}

fn required_param<'a>(params: &'a [JobParam], key: &str) -> Result<&'a str, Nip90Error> {
    request_param_value(params, key).ok_or_else(|| Nip90Error::MissingTag(key.to_string()))
}

fn required_tag_value<'a>(tags: &'a [Vec<String>], key: &str) -> Result<String, Nip90Error> {
    find_tag_value(tags, key)
        .map(str::to_owned)
        .ok_or_else(|| Nip90Error::MissingTag(key.to_string()))
}

fn push_optional_tag(tags: &mut Vec<Vec<String>>, key: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        tags.push(vec![key.to_string(), value.to_string()]);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DataVendingDeliveryMode, DataVendingFeedback, DataVendingPreviewPosture,
        DataVendingRequest, DataVendingResult, OPENAGENTS_DATA_VENDING_PROFILE,
        create_data_vending_feedback_event, create_data_vending_request_event,
        create_data_vending_result_event,
    };
    use crate::nip01::Event;
    use crate::nip90::{JobInput, JobStatus};

    fn event_from_template(id: &str, pubkey: &str, template: crate::EventTemplate) -> Event {
        Event {
            id: id.to_string(),
            pubkey: pubkey.to_string(),
            created_at: template.created_at,
            kind: template.kind,
            tags: template.tags,
            content: template.content,
            sig: "00".repeat(64),
        }
    }

    #[test]
    fn data_vending_request_roundtrip_preserves_targeting_and_profile_fields() {
        let request = DataVendingRequest::new(5960, "asset://alpha", "read.context")
            .expect("request")
            .with_listing_coordinate(
                "30404:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:dataset.alpha",
            )
            .expect("listing coordinate")
            .with_offer_coordinate(
                "30406:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:offer.alpha",
            )
            .expect("offer coordinate")
            .with_asset_id("asset.alpha")
            .with_grant_id("grant.alpha")
            .add_scope("derive.summary")
            .with_delivery_mode(DataVendingDeliveryMode::EncryptedPointer)
            .with_preview_posture(DataVendingPreviewPosture::MetadataOnly)
            .add_input(JobInput::text("Need the latest repository context"))
            .add_service_provider("provider-pubkey")
            .add_relay("wss://relay.openagents.com");

        let template = create_data_vending_request_event(&request).expect("event template");
        let event = event_from_template("request-event", "buyer-pubkey", template);
        let parsed = DataVendingRequest::from_event(&event).expect("parsed request");

        assert_eq!(parsed.request_kind, 5960);
        assert_eq!(parsed.profile_id, OPENAGENTS_DATA_VENDING_PROFILE);
        assert_eq!(parsed.asset_ref, "asset://alpha");
        assert_eq!(parsed.asset_id.as_deref(), Some("asset.alpha"));
        assert_eq!(parsed.grant_id.as_deref(), Some("grant.alpha"));
        assert_eq!(
            parsed
                .listing_ref
                .as_ref()
                .map(|reference| reference.coordinate.to_string())
                .as_deref(),
            Some(
                "30404:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:dataset.alpha"
            )
        );
        assert_eq!(
            parsed
                .offer_ref
                .as_ref()
                .map(|reference| reference.coordinate.to_string())
                .as_deref(),
            Some(
                "30406:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:offer.alpha"
            )
        );
        assert_eq!(
            parsed.permission_scopes,
            vec!["read.context".to_string(), "derive.summary".to_string()]
        );
        assert_eq!(
            parsed.delivery_mode,
            DataVendingDeliveryMode::EncryptedPointer
        );
        assert_eq!(
            parsed.preview_posture,
            DataVendingPreviewPosture::MetadataOnly
        );
        assert_eq!(
            parsed.service_providers,
            vec!["provider-pubkey".to_string()]
        );
    }

    #[test]
    fn data_vending_result_and_feedback_roundtrip_preserve_bundle_linkage_and_reason() {
        let result = DataVendingResult::new(
            5960,
            "request-event",
            "buyer-pubkey",
            "asset://alpha",
            "bundle://delivery.alpha",
            "{\"delivery\":\"ready\"}",
        )
        .expect("result")
        .with_listing_coordinate(
            "30404:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:dataset.alpha",
        )
        .expect("listing coordinate")
        .with_offer_coordinate(
            "30406:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:offer.alpha",
        )
        .expect("offer coordinate")
        .with_asset_id("asset.alpha")
        .with_grant_id("grant://alpha")
        .with_delivery_mode(DataVendingDeliveryMode::DeliveryBundleRef)
        .with_preview_posture(DataVendingPreviewPosture::InlinePreview)
        .with_delivery_ref("oa://deliveries/alpha")
        .with_delivery_digest("sha256:delivery-alpha")
        .with_amount(25_000, Some("lnbc250u1...".to_string()));

        let result_event = event_from_template(
            "result-event",
            "provider-pubkey",
            create_data_vending_result_event(&result).expect("result template"),
        );
        let parsed_result = DataVendingResult::from_event(&result_event).expect("parsed result");
        assert_eq!(parsed_result.profile_id, OPENAGENTS_DATA_VENDING_PROFILE);
        assert_eq!(parsed_result.delivery_bundle_id, "bundle://delivery.alpha");
        assert_eq!(parsed_result.asset_id.as_deref(), Some("asset.alpha"));
        assert_eq!(
            parsed_result.delivery_mode,
            DataVendingDeliveryMode::DeliveryBundleRef
        );
        assert_eq!(
            parsed_result.preview_posture,
            DataVendingPreviewPosture::InlinePreview
        );
        assert_eq!(parsed_result.grant_id.as_deref(), Some("grant://alpha"));
        assert_eq!(
            parsed_result.delivery_digest.as_deref(),
            Some("sha256:delivery-alpha")
        );

        let feedback = DataVendingFeedback::new(
            JobStatus::Error,
            "request-event",
            "buyer-pubkey",
            "asset://alpha",
        )
        .with_listing_coordinate(
            "30404:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:dataset.alpha",
        )
        .expect("listing coordinate")
        .with_offer_coordinate(
            "30406:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:offer.alpha",
        )
        .expect("offer coordinate")
        .with_asset_id("asset.alpha")
        .with_reason_code("asset_policy_denied")
        .with_status_extra("provider refused export")
        .with_grant_id("grant://alpha")
        .with_revocation_id("revocation.alpha");

        let feedback_event = event_from_template(
            "feedback-event",
            "provider-pubkey",
            create_data_vending_feedback_event(&feedback).expect("feedback template"),
        );
        let parsed_feedback =
            DataVendingFeedback::from_event(&feedback_event).expect("parsed feedback");
        assert_eq!(parsed_feedback.profile_id, OPENAGENTS_DATA_VENDING_PROFILE);
        assert_eq!(parsed_feedback.status, JobStatus::Error);
        assert_eq!(
            parsed_feedback.reason_code.as_deref(),
            Some("asset_policy_denied")
        );
        assert_eq!(parsed_feedback.asset_id.as_deref(), Some("asset.alpha"));
        assert_eq!(
            parsed_feedback.revocation_id.as_deref(),
            Some("revocation.alpha")
        );
        assert_eq!(
            parsed_feedback.status_extra.as_deref(),
            Some("provider refused export")
        );
    }
}
