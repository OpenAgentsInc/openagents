//! Provider-neutral contract for durable logical computers and temporary runtimes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const CLOUD_COMPUTER_VERSION: &str = "openagents.cloud_computer.v1";
pub const CLOUD_COMPUTER_COMMAND_VERSION: &str = "openagents.cloud_computer_command.v1";
pub const CLOUD_COMPUTER_EVENT_VERSION: &str = "openagents.cloud_computer_event.v1";
pub const CLOUD_COMPUTER_CHECKPOINT_VERSION: &str = "openagents.cloud_computer_checkpoint.v1";
pub const CLOUD_COMPUTER_RECEIPT_VERSION: &str = "openagents.cloud_computer_receipt.v1";
pub const CLOUD_COMPUTER_LEASE_VERSION: &str = "openagents.cloud_computer_lease.v1";
pub const CLOUD_COMPUTER_EXECUTION_RECEIPT_VERSION: &str =
    "openagents.cloud_computer_execution_receipt.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeClass {
    Standard,
    Strong,
    Batch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputerKind {
    InteractiveRetained,
    OneShotBatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputerState {
    Cold,
    Queued,
    Starting,
    Active,
    Stopping,
    Failed,
    Destroyed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveProvider {
    Firecracker,
    GkeAgentSandbox,
    DedicatedGce,
    CloudRunBatch,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCapability {
    Interactive,
    Batch,
    Checkpoint,
    Restore,
    Fork,
    Attach,
    NetworkBroker,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComputerScope {
    pub owner_ref: String,
    pub tenant_ref: String,
    pub conversation_or_program_ref: String,
    pub work_unit_ref: String,
    pub runtime_profile_ref: String,
    pub authority_snapshot_digest: String,
    pub budget_snapshot_digest: String,
    pub capability_refs: Vec<String>,
}

impl ComputerScope {
    pub fn validate(&self) -> Result<(), CloudComputerError> {
        for value in [
            self.owner_ref.as_str(),
            self.tenant_ref.as_str(),
            self.conversation_or_program_ref.as_str(),
            self.work_unit_ref.as_str(),
            self.runtime_profile_ref.as_str(),
        ] {
            validate_public_ref(value)?;
        }
        validate_digest(&self.authority_snapshot_digest)?;
        validate_digest(&self.budget_snapshot_digest)?;
        if self.capability_refs.is_empty() {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::EmptyCapabilitySet,
            ));
        }
        let mut unique = self.capability_refs.clone();
        for capability_ref in &unique {
            validate_public_ref(capability_ref)?;
        }
        unique.sort();
        unique.dedup();
        if unique.len() != self.capability_refs.len() {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::DuplicateCapabilityRef,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloudComputer {
    pub schema: String,
    pub computer_ref: String,
    pub scope: ComputerScope,
    pub kind: ComputerKind,
    pub requested_runtime_class: RuntimeClass,
    pub generation: u64,
    pub version: u64,
    pub state: ComputerState,
    pub active_lease_ref: Option<String>,
    pub latest_checkpoint_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl CloudComputer {
    pub fn new_cold(
        computer_ref: impl Into<String>,
        scope: ComputerScope,
        kind: ComputerKind,
        requested_runtime_class: RuntimeClass,
        observed_at: impl Into<String>,
    ) -> Self {
        let observed_at = observed_at.into();
        Self {
            schema: CLOUD_COMPUTER_VERSION.to_owned(),
            computer_ref: computer_ref.into(),
            scope,
            kind,
            requested_runtime_class,
            generation: 1,
            version: 1,
            state: ComputerState::Cold,
            active_lease_ref: None,
            latest_checkpoint_ref: None,
            created_at: observed_at.clone(),
            updated_at: observed_at,
        }
    }

    pub fn validate(&self) -> Result<(), CloudComputerError> {
        if self.schema != CLOUD_COMPUTER_VERSION {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::UnsupportedSchema,
            ));
        }
        validate_public_ref(&self.computer_ref)?;
        self.scope.validate()?;
        validate_timestamp(&self.created_at)?;
        validate_timestamp(&self.updated_at)?;
        if self.generation == 0 || self.version == 0 {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::NonPositiveFence,
            ));
        }
        if !matches!(
            (self.kind, self.requested_runtime_class),
            (
                ComputerKind::InteractiveRetained,
                RuntimeClass::Standard | RuntimeClass::Strong
            ) | (ComputerKind::OneShotBatch, RuntimeClass::Batch)
        ) {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::KindRuntimeClassMismatch,
            ));
        }
        if matches!(self.state, ComputerState::Cold | ComputerState::Queued)
            && self.active_lease_ref.is_some()
        {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::InactiveStateHasLease,
            ));
        }
        if matches!(self.state, ComputerState::Active | ComputerState::Stopping)
            && self.active_lease_ref.is_none()
        {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::LiveStateMissingLease,
            ));
        }
        if self.state == ComputerState::Destroyed && self.active_lease_ref.is_some() {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::InactiveStateHasLease,
            ));
        }
        if let Some(active_lease_ref) = &self.active_lease_ref {
            validate_public_ref(active_lease_ref)?;
        }
        if let Some(checkpoint_ref) = &self.latest_checkpoint_ref {
            validate_public_ref(checkpoint_ref)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeLease {
    pub schema: String,
    pub lease_ref: String,
    pub computer_ref: String,
    pub scope: ComputerScope,
    pub generation: u64,
    pub effective_provider: EffectiveProvider,
    pub image_digest: String,
    pub policy_digest: String,
    pub issued_at: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloudComputerCheckpoint {
    pub schema: String,
    pub checkpoint_ref: String,
    pub computer_ref: String,
    pub scope: ComputerScope,
    pub generation: u64,
    pub parent_checkpoint_ref: Option<String>,
    pub content_digest: String,
    pub policy_digest: String,
    pub created_at: String,
}

impl CloudComputerCheckpoint {
    pub fn validate_for(&self, request: &ProviderBoundRequest) -> Result<(), CloudComputerError> {
        let matches_request = self.schema == CLOUD_COMPUTER_CHECKPOINT_VERSION
            && self.computer_ref == request.computer_ref
            && self.scope == request.scope
            && self.generation == request.generation;
        if !matches_request {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::ScopeMismatch,
            ));
        }
        validate_public_ref(&self.checkpoint_ref)?;
        if let Some(parent) = &self.parent_checkpoint_ref {
            validate_public_ref(parent)?;
        }
        validate_digest(&self.content_digest)?;
        validate_digest(&self.policy_digest)?;
        validate_timestamp(&self.created_at)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudComputerOperation {
    Create,
    List,
    Inspect,
    Start,
    Execute,
    Attach,
    Cancel,
    Stop,
    Checkpoint,
    Restore,
    Fork,
    Destroy,
}

impl CloudComputerOperation {
    pub fn mutates(&self) -> bool {
        !matches!(self, Self::List | Self::Inspect | Self::Attach)
    }

    pub fn allowed_for(&self, kind: ComputerKind) -> bool {
        match kind {
            ComputerKind::InteractiveRetained => true,
            ComputerKind::OneShotBatch => matches!(
                self,
                Self::Create
                    | Self::List
                    | Self::Inspect
                    | Self::Start
                    | Self::Execute
                    | Self::Cancel
                    | Self::Destroy
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloudComputerCommand {
    pub schema: String,
    pub command_ref: String,
    pub idempotency_key: String,
    pub request_digest: String,
    pub requested_by_ref: String,
    pub computer_ref: String,
    pub scope: ComputerScope,
    pub expected_generation: u64,
    pub operation: CloudComputerOperation,
    pub requested_at: String,
}

impl CloudComputerCommand {
    pub fn validate(&self) -> Result<(), CloudComputerError> {
        if self.schema != CLOUD_COMPUTER_COMMAND_VERSION {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::UnsupportedSchema,
            ));
        }
        for value in [
            self.command_ref.as_str(),
            self.idempotency_key.as_str(),
            self.requested_by_ref.as_str(),
            self.computer_ref.as_str(),
        ] {
            validate_public_ref(value)?;
        }
        validate_digest(&self.request_digest)?;
        self.scope.validate()?;
        validate_timestamp(&self.requested_at)?;
        if self.requested_by_ref != self.scope.owner_ref {
            return Err(CloudComputerError::permission(
                CloudComputerErrorDetail::ActorOwnerMismatch,
            ));
        }
        Ok(())
    }

    pub fn validate_authenticated(
        &self,
        authenticated_actor: &str,
    ) -> Result<(), CloudComputerError> {
        self.validate()?;
        validate_public_ref(authenticated_actor)?;
        if authenticated_actor != self.requested_by_ref {
            return Err(CloudComputerError::permission(
                CloudComputerErrorDetail::ActorOwnerMismatch,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderCapabilities {
    pub provider: EffectiveProvider,
    pub runtime_classes: Vec<RuntimeClass>,
    pub capabilities: Vec<ProviderCapability>,
}

impl ProviderCapabilities {
    pub fn supports(&self, runtime_class: RuntimeClass, required: &[ProviderCapability]) -> bool {
        self.runtime_classes.contains(&runtime_class)
            && required.iter().all(|item| self.capabilities.contains(item))
    }
}

pub fn admit_selected_provider(
    selected: &ProviderCapabilities,
    runtime_class: RuntimeClass,
    required: &[ProviderCapability],
) -> Result<EffectiveProvider, CloudComputerError> {
    if selected.supports(runtime_class, required) {
        Ok(selected.provider)
    } else {
        Err(CloudComputerError::unsupported_capability())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderStartRequest {
    computer_ref: String,
    scope: ComputerScope,
    generation: u64,
    command_ref: String,
    runtime_class: RuntimeClass,
    kind: ComputerKind,
    profile_ref: String,
    authority_snapshot_digest: String,
    budget_snapshot_digest: String,
    checkpoint_ref: Option<String>,
}

impl ProviderStartRequest {
    fn from_admitted_command(
        computer: &CloudComputer,
        command: &CloudComputerCommand,
    ) -> Result<Self, CloudComputerError> {
        if !matches!(
            command.operation,
            CloudComputerOperation::Start | CloudComputerOperation::Restore
        ) || command.computer_ref != computer.computer_ref
            || command.scope != computer.scope
        {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::ScopeMismatch,
            ));
        }
        if command.expected_generation != computer.generation {
            return Err(CloudComputerError::generation_mismatch(
                computer.generation,
                command.expected_generation,
            ));
        }
        if command.operation == CloudComputerOperation::Restore
            && computer.latest_checkpoint_ref.is_none()
        {
            return Err(invalid_transition());
        }
        computer.validate()?;
        validate_public_ref(&command.command_ref)?;
        Ok(Self {
            computer_ref: computer.computer_ref.clone(),
            scope: computer.scope.clone(),
            generation: computer.generation,
            command_ref: command.command_ref.clone(),
            runtime_class: computer.requested_runtime_class,
            kind: computer.kind,
            profile_ref: computer.scope.runtime_profile_ref.clone(),
            authority_snapshot_digest: computer.scope.authority_snapshot_digest.clone(),
            budget_snapshot_digest: computer.scope.budget_snapshot_digest.clone(),
            checkpoint_ref: computer.latest_checkpoint_ref.clone(),
        })
    }

    pub fn computer_ref(&self) -> &str {
        &self.computer_ref
    }

    pub fn scope(&self) -> &ComputerScope {
        &self.scope
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn command_ref(&self) -> &str {
        &self.command_ref
    }

    pub fn runtime_class(&self) -> RuntimeClass {
        self.runtime_class
    }

    pub fn kind(&self) -> ComputerKind {
        self.kind
    }

    pub fn profile_ref(&self) -> &str {
        &self.profile_ref
    }

    pub fn authority_snapshot_digest(&self) -> &str {
        &self.authority_snapshot_digest
    }

    pub fn budget_snapshot_digest(&self) -> &str {
        &self.budget_snapshot_digest
    }

    pub fn checkpoint_ref(&self) -> Option<&str> {
        self.checkpoint_ref.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderExecutionRequest {
    computer_ref: String,
    scope: ComputerScope,
    generation: u64,
    lease_ref: String,
    command_ref: String,
    input_digest: String,
}

impl ProviderExecutionRequest {
    pub fn new(
        computer: &CloudComputer,
        lease: &RuntimeLease,
        command_ref: String,
        input_digest: String,
    ) -> Result<Self, CloudComputerError> {
        validate_lease_for(computer, lease)?;
        validate_public_ref(&command_ref)?;
        validate_digest(&input_digest)?;
        Ok(Self {
            computer_ref: computer.computer_ref.clone(),
            scope: computer.scope.clone(),
            generation: computer.generation,
            lease_ref: lease.lease_ref.clone(),
            command_ref,
            input_digest,
        })
    }

    pub fn computer_ref(&self) -> &str {
        &self.computer_ref
    }

    pub fn scope(&self) -> &ComputerScope {
        &self.scope
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn lease_ref(&self) -> &str {
        &self.lease_ref
    }

    pub fn command_ref(&self) -> &str {
        &self.command_ref
    }

    pub fn input_digest(&self) -> &str {
        &self.input_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderCleanupReceipt {
    pub computer_ref: String,
    pub scope: ComputerScope,
    pub generation: u64,
    pub lease_ref: String,
    pub cleanup_digest: String,
    pub observed_at: String,
}

impl ProviderCleanupReceipt {
    pub fn validate_public(&self) -> Result<(), CloudComputerError> {
        for value in [self.computer_ref.as_str(), self.lease_ref.as_str()] {
            validate_public_ref(value)?;
        }
        self.scope.validate()?;
        if self.generation == 0 {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::NonPositiveFence,
            ));
        }
        validate_digest(&self.cleanup_digest)?;
        validate_timestamp(&self.observed_at)
    }

    pub fn validate_for(&self, request: &ProviderBoundRequest) -> Result<(), CloudComputerError> {
        if self.computer_ref != request.computer_ref
            || self.scope != request.scope
            || self.generation != request.generation
            || self.lease_ref != request.lease_ref
        {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::ScopeMismatch,
            ));
        }
        self.validate_public()
    }
}

impl Serialize for ProviderCleanupReceipt {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.validate_public()
            .map_err(|_| serde::ser::Error::custom("unsafe provider cleanup receipt"))?;
        #[derive(Serialize)]
        struct Wire<'a> {
            computer_ref: &'a str,
            scope: &'a ComputerScope,
            generation: u64,
            lease_ref: &'a str,
            cleanup_digest: &'a str,
            observed_at: &'a str,
        }
        Wire {
            computer_ref: &self.computer_ref,
            scope: &self.scope,
            generation: self.generation,
            lease_ref: &self.lease_ref,
            cleanup_digest: &self.cleanup_digest,
            observed_at: &self.observed_at,
        }
        .serialize(serializer)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderBoundRequest {
    computer_ref: String,
    scope: ComputerScope,
    generation: u64,
    lease_ref: String,
}

impl ProviderBoundRequest {
    pub fn new(computer: &CloudComputer, lease: &RuntimeLease) -> Result<Self, CloudComputerError> {
        validate_lease_for(computer, lease)?;
        Ok(Self {
            computer_ref: computer.computer_ref.clone(),
            scope: computer.scope.clone(),
            generation: computer.generation,
            lease_ref: lease.lease_ref.clone(),
        })
    }

    pub fn computer_ref(&self) -> &str {
        &self.computer_ref
    }

    pub fn scope(&self) -> &ComputerScope {
        &self.scope
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn lease_ref(&self) -> &str {
        &self.lease_ref
    }
}

pub trait CloudComputerProvider: Send + Sync {
    fn capabilities(&self) -> ProviderCapabilities;

    fn start(&self, request: &ProviderStartRequest) -> Result<RuntimeLease, CloudComputerError>;

    fn execute(
        &self,
        request: &ProviderExecutionRequest,
    ) -> Result<ProviderExecutionReceipt, CloudComputerError>;

    fn checkpoint(
        &self,
        request: &ProviderBoundRequest,
    ) -> Result<CloudComputerCheckpoint, CloudComputerError>;

    fn cleanup(
        &self,
        request: &ProviderBoundRequest,
    ) -> Result<ProviderCleanupReceipt, CloudComputerError>;
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderExecutionReceipt {
    pub schema: String,
    pub command_ref: String,
    pub computer_ref: String,
    pub scope: ComputerScope,
    pub generation: u64,
    pub lease_ref: String,
    pub execution_ref: String,
    pub output_digest: String,
    pub usage_digest: String,
    pub observed_at: String,
}

impl ProviderExecutionReceipt {
    pub fn validate_for(
        &self,
        request: &ProviderExecutionRequest,
    ) -> Result<(), CloudComputerError> {
        let matches_request = self.schema == CLOUD_COMPUTER_EXECUTION_RECEIPT_VERSION
            && self.command_ref == request.command_ref
            && self.computer_ref == request.computer_ref
            && self.scope == request.scope
            && self.generation == request.generation
            && self.lease_ref == request.lease_ref;
        if !matches_request {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::ScopeMismatch,
            ));
        }
        for value in [
            self.command_ref.as_str(),
            self.computer_ref.as_str(),
            self.lease_ref.as_str(),
            self.execution_ref.as_str(),
        ] {
            validate_public_ref(value)?;
        }
        validate_digest(&self.output_digest)?;
        validate_digest(&self.usage_digest)?;
        self.scope.validate()?;
        validate_timestamp(&self.observed_at)
    }
}

impl Serialize for ProviderExecutionReceipt {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.schema != CLOUD_COMPUTER_EXECUTION_RECEIPT_VERSION || self.generation == 0 {
            return Err(serde::ser::Error::custom(
                "unsafe provider execution receipt",
            ));
        }
        for value in [
            self.command_ref.as_str(),
            self.computer_ref.as_str(),
            self.lease_ref.as_str(),
            self.execution_ref.as_str(),
        ] {
            validate_public_ref(value)
                .map_err(|_| serde::ser::Error::custom("unsafe provider execution receipt"))?;
        }
        self.scope
            .validate()
            .map_err(|_| serde::ser::Error::custom("unsafe provider execution receipt"))?;
        validate_digest(&self.output_digest)
            .and_then(|_| validate_digest(&self.usage_digest))
            .and_then(|_| validate_timestamp(&self.observed_at))
            .map_err(|_| serde::ser::Error::custom("unsafe provider execution receipt"))?;
        #[derive(Serialize)]
        struct Wire<'a> {
            schema: &'a str,
            command_ref: &'a str,
            computer_ref: &'a str,
            scope: &'a ComputerScope,
            generation: u64,
            lease_ref: &'a str,
            execution_ref: &'a str,
            output_digest: &'a str,
            usage_digest: &'a str,
            observed_at: &'a str,
        }
        Wire {
            schema: &self.schema,
            command_ref: &self.command_ref,
            computer_ref: &self.computer_ref,
            scope: &self.scope,
            generation: self.generation,
            lease_ref: &self.lease_ref,
            execution_ref: &self.execution_ref,
            output_digest: &self.output_digest,
            usage_digest: &self.usage_digest,
            observed_at: &self.observed_at,
        }
        .serialize(serializer)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudComputerErrorCode {
    ValidationFailed,
    NotFound,
    PermissionDenied,
    IdempotencyConflict,
    GenerationMismatch,
    InvalidTransition,
    CapacityUnavailable,
    UnsupportedCapability,
    ProviderUnavailable,
    RecoveryRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudComputerErrorDetail {
    UnsupportedSchema,
    UnsafePublicRef,
    InvalidTimestamp,
    EmptyCapabilitySet,
    DuplicateCapabilityRef,
    NonPositiveFence,
    KindRuntimeClassMismatch,
    InactiveStateHasLease,
    LiveStateMissingLease,
    InvalidCreateCommand,
    IdempotencyKeyReused,
    MutationPending,
    MutationNotReserved,
    ComputerRefAlreadyExists,
    CloudComputerNotFound,
    ActorOwnerMismatch,
    ScopeMismatch,
    GenerationMismatch,
    InvalidTransition,
    LeaseScopeMismatch,
    LeaseGenerationMismatch,
    UnsupportedProviderCapability,
    ForbiddenPublicField,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloudComputerError {
    pub code: CloudComputerErrorCode,
    pub detail: CloudComputerErrorDetail,
    pub retryable: bool,
    pub receipt_ref: Option<String>,
    pub expected_generation: Option<u64>,
    pub received_generation: Option<u64>,
}

impl CloudComputerError {
    fn validation(detail: CloudComputerErrorDetail) -> Self {
        Self {
            code: CloudComputerErrorCode::ValidationFailed,
            detail,
            retryable: false,
            receipt_ref: None,
            expected_generation: None,
            received_generation: None,
        }
    }

    fn permission(detail: CloudComputerErrorDetail) -> Self {
        Self {
            code: CloudComputerErrorCode::PermissionDenied,
            detail,
            retryable: false,
            receipt_ref: None,
            expected_generation: None,
            received_generation: None,
        }
    }

    pub fn generation_mismatch(expected: u64, received: u64) -> Self {
        Self {
            code: CloudComputerErrorCode::GenerationMismatch,
            detail: CloudComputerErrorDetail::GenerationMismatch,
            retryable: false,
            receipt_ref: None,
            expected_generation: Some(expected),
            received_generation: Some(received),
        }
    }

    pub fn unsupported_capability() -> Self {
        Self {
            code: CloudComputerErrorCode::UnsupportedCapability,
            detail: CloudComputerErrorDetail::UnsupportedProviderCapability,
            retryable: false,
            receipt_ref: None,
            expected_generation: None,
            received_generation: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloudComputerEvent {
    pub schema: String,
    pub event_ref: String,
    pub computer_ref: String,
    pub scope: ComputerScope,
    pub generation: u64,
    pub sequence: u64,
    pub state: ComputerState,
    pub command_ref: String,
    pub observed_at: String,
}

impl CloudComputerEvent {
    pub fn validate_public(&self) -> Result<(), CloudComputerError> {
        if self.schema != CLOUD_COMPUTER_EVENT_VERSION || self.generation == 0 {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::UnsupportedSchema,
            ));
        }
        for value in [
            self.event_ref.as_str(),
            self.computer_ref.as_str(),
            self.command_ref.as_str(),
        ] {
            validate_public_ref(value)?;
        }
        self.scope.validate()?;
        validate_timestamp(&self.observed_at)
    }
}

impl Serialize for CloudComputerEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.validate_public()
            .map_err(|_| serde::ser::Error::custom("unsafe cloud computer event"))?;
        #[derive(Serialize)]
        struct Wire<'a> {
            schema: &'a str,
            event_ref: &'a str,
            computer_ref: &'a str,
            scope: &'a ComputerScope,
            generation: u64,
            sequence: u64,
            state: ComputerState,
            command_ref: &'a str,
            observed_at: &'a str,
        }
        Wire {
            schema: &self.schema,
            event_ref: &self.event_ref,
            computer_ref: &self.computer_ref,
            scope: &self.scope,
            generation: self.generation,
            sequence: self.sequence,
            state: self.state,
            command_ref: &self.command_ref,
            observed_at: &self.observed_at,
        }
        .serialize(serializer)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloudComputerReceipt {
    pub schema: String,
    pub receipt_ref: String,
    pub command_ref: String,
    pub computer_ref: String,
    pub scope: ComputerScope,
    pub generation: u64,
    pub operation: CloudComputerOperation,
    pub state: ComputerState,
    pub runtime_lease_ref: Option<String>,
    pub effective_provider: Option<EffectiveProvider>,
    pub image_digest: Option<String>,
    pub policy_digest: Option<String>,
    pub checkpoint_digest: Option<String>,
    pub usage_digest: Option<String>,
    pub cleanup_digest: Option<String>,
    pub error: Option<CloudComputerError>,
    pub observed_at: String,
}

impl CloudComputerReceipt {
    pub fn validate_public(&self) -> Result<(), CloudComputerError> {
        if self.schema != CLOUD_COMPUTER_RECEIPT_VERSION {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::UnsupportedSchema,
            ));
        }
        if self.generation == 0 {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::NonPositiveFence,
            ));
        }
        for value in [
            self.receipt_ref.as_str(),
            self.command_ref.as_str(),
            self.computer_ref.as_str(),
        ] {
            validate_public_ref(value)?;
        }
        self.scope.validate()?;
        validate_timestamp(&self.observed_at)?;
        if let Some(error) = &self.error {
            validate_public_error(error)?;
        }
        if let Some(runtime_lease_ref) = &self.runtime_lease_ref {
            validate_public_ref(runtime_lease_ref)?;
        }
        for value in [
            self.image_digest.as_deref(),
            self.policy_digest.as_deref(),
            self.checkpoint_digest.as_deref(),
            self.usage_digest.as_deref(),
            self.cleanup_digest.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            validate_digest(value)?;
        }
        Ok(())
    }
}

impl Serialize for CloudComputerReceipt {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.validate_public()
            .map_err(|_| serde::ser::Error::custom("unsafe cloud computer receipt"))?;
        #[derive(Serialize)]
        struct Wire<'a> {
            schema: &'a str,
            receipt_ref: &'a str,
            command_ref: &'a str,
            computer_ref: &'a str,
            scope: &'a ComputerScope,
            generation: u64,
            operation: &'a CloudComputerOperation,
            state: ComputerState,
            runtime_lease_ref: Option<&'a str>,
            effective_provider: Option<EffectiveProvider>,
            image_digest: Option<&'a str>,
            policy_digest: Option<&'a str>,
            checkpoint_digest: Option<&'a str>,
            usage_digest: Option<&'a str>,
            cleanup_digest: Option<&'a str>,
            error: Option<&'a CloudComputerError>,
            observed_at: &'a str,
        }
        Wire {
            schema: &self.schema,
            receipt_ref: &self.receipt_ref,
            command_ref: &self.command_ref,
            computer_ref: &self.computer_ref,
            scope: &self.scope,
            generation: self.generation,
            operation: &self.operation,
            state: self.state,
            runtime_lease_ref: self.runtime_lease_ref.as_deref(),
            effective_provider: self.effective_provider,
            image_digest: self.image_digest.as_deref(),
            policy_digest: self.policy_digest.as_deref(),
            checkpoint_digest: self.checkpoint_digest.as_deref(),
            usage_digest: self.usage_digest.as_deref(),
            cleanup_digest: self.cleanup_digest.as_deref(),
            error: self.error.as_ref(),
            observed_at: &self.observed_at,
        }
        .serialize(serializer)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoredCommand {
    canonical_request: Vec<u8>,
    receipt: Option<CloudComputerReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MutationAdmission {
    Proceed,
    Pending,
    Replay(Box<CloudComputerReceipt>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderStartAdmission {
    Proceed(Box<ProviderStartRequest>),
    Pending,
    Replay(Box<CloudComputerReceipt>),
}

/// A deterministic contract harness. Production storage can implement the same
/// command rules in a durable transaction without changing public bytes.
#[derive(Default)]
pub struct CloudComputerModel {
    computers: BTreeMap<String, CloudComputer>,
    commands: BTreeMap<String, StoredCommand>,
}

impl CloudComputerModel {
    pub fn create(
        &mut self,
        authenticated_actor: &str,
        command: &CloudComputerCommand,
        scope: ComputerScope,
        kind: ComputerKind,
        runtime_class: RuntimeClass,
    ) -> Result<CloudComputerReceipt, CloudComputerError> {
        command.validate_authenticated(authenticated_actor)?;
        if command.operation != CloudComputerOperation::Create
            || !command.operation.mutates()
            || command.scope != scope
        {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::InvalidCreateCommand,
            ));
        }
        let canonical_request =
            serde_json::to_vec(&(command, kind, runtime_class)).map_err(|_| {
                CloudComputerError::validation(CloudComputerErrorDetail::InvalidCreateCommand)
            })?;
        if let Some(stored) = self.commands.get(&command.idempotency_key) {
            return if stored.canonical_request == canonical_request {
                stored.receipt.clone().ok_or(CloudComputerError {
                    code: CloudComputerErrorCode::RecoveryRequired,
                    detail: CloudComputerErrorDetail::MutationPending,
                    retryable: true,
                    receipt_ref: None,
                    expected_generation: None,
                    received_generation: None,
                })
            } else {
                Err(CloudComputerError {
                    code: CloudComputerErrorCode::IdempotencyConflict,
                    detail: CloudComputerErrorDetail::IdempotencyKeyReused,
                    retryable: false,
                    receipt_ref: stored
                        .receipt
                        .as_ref()
                        .map(|receipt| receipt.receipt_ref.clone()),
                    expected_generation: None,
                    received_generation: None,
                })
            };
        }
        if self.computers.contains_key(&command.computer_ref) {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::ComputerRefAlreadyExists,
            ));
        }
        let computer = CloudComputer::new_cold(
            command.computer_ref.clone(),
            scope,
            kind,
            runtime_class,
            command.requested_at.clone(),
        );
        computer.validate()?;
        let receipt = CloudComputerReceipt {
            schema: CLOUD_COMPUTER_RECEIPT_VERSION.to_owned(),
            receipt_ref: format!("receipt.{}", command.command_ref),
            command_ref: command.command_ref.clone(),
            computer_ref: command.computer_ref.clone(),
            scope: command.scope.clone(),
            generation: computer.generation,
            operation: CloudComputerOperation::Create,
            state: ComputerState::Cold,
            runtime_lease_ref: None,
            effective_provider: None,
            image_digest: None,
            policy_digest: None,
            checkpoint_digest: None,
            usage_digest: None,
            cleanup_digest: None,
            error: None,
            observed_at: command.requested_at.clone(),
        };
        self.computers
            .insert(computer.computer_ref.clone(), computer);
        self.commands.insert(
            command.idempotency_key.clone(),
            StoredCommand {
                canonical_request,
                receipt: Some(receipt.clone()),
            },
        );
        Ok(receipt)
    }

    pub fn inspect(&self, computer_ref: &str) -> Option<&CloudComputer> {
        self.computers.get(computer_ref)
    }

    pub fn list(&self) -> impl Iterator<Item = &CloudComputer> {
        self.computers.values()
    }

    #[cfg(test)]
    fn transition(
        &mut self,
        computer_ref: &str,
        expected_generation: u64,
        next: ComputerState,
        observed_at: impl Into<String>,
    ) -> Result<CloudComputer, CloudComputerError> {
        let observed_at = observed_at.into();
        validate_timestamp(&observed_at)?;
        let computer = self
            .computers
            .get(computer_ref)
            .cloned()
            .ok_or_else(not_found)?;
        if computer.generation != expected_generation {
            return Err(CloudComputerError::generation_mismatch(
                computer.generation,
                expected_generation,
            ));
        }
        let allowed = matches!(
            (computer.state, next),
            (ComputerState::Cold, ComputerState::Queued)
                | (ComputerState::Queued, ComputerState::Starting)
                | (ComputerState::Queued, ComputerState::Cold)
                | (ComputerState::Starting, ComputerState::Failed)
                | (ComputerState::Active, ComputerState::Failed)
                | (ComputerState::Stopping, ComputerState::Failed)
                | (ComputerState::Cold, ComputerState::Destroyed)
                | (ComputerState::Failed, ComputerState::Destroyed)
        );
        if !allowed {
            return Err(CloudComputerError {
                code: CloudComputerErrorCode::InvalidTransition,
                detail: CloudComputerErrorDetail::InvalidTransition,
                retryable: false,
                receipt_ref: None,
                expected_generation: None,
                received_generation: None,
            });
        }
        let mut next_computer = computer;
        if matches!(next, ComputerState::Starting | ComputerState::Cold) {
            next_computer.generation += 1;
        }
        next_computer.state = next;
        next_computer.version += 1;
        next_computer.updated_at = observed_at;
        next_computer.validate()?;
        self.computers
            .insert(computer_ref.to_owned(), next_computer.clone());
        Ok(next_computer)
    }

    #[cfg(test)]
    fn activate(
        &mut self,
        computer_ref: &str,
        expected_generation: u64,
        lease: RuntimeLease,
        observed_at: impl Into<String>,
    ) -> Result<CloudComputer, CloudComputerError> {
        let observed_at = observed_at.into();
        validate_timestamp(&observed_at)?;
        let computer = self.computers.get_mut(computer_ref).ok_or_else(not_found)?;
        ensure_generation(computer, expected_generation)?;
        validate_lease_identity(computer, &lease)?;
        if computer.state != ComputerState::Starting {
            return Err(invalid_transition());
        }
        computer.active_lease_ref = Some(lease.lease_ref);
        computer.state = ComputerState::Active;
        computer.version += 1;
        computer.updated_at = observed_at;
        computer.validate()?;
        Ok(computer.clone())
    }

    #[cfg(test)]
    fn begin_stop(
        &mut self,
        computer_ref: &str,
        expected_generation: u64,
        observed_at: impl Into<String>,
    ) -> Result<CloudComputer, CloudComputerError> {
        let observed_at = observed_at.into();
        validate_timestamp(&observed_at)?;
        let computer = self.computers.get_mut(computer_ref).ok_or_else(not_found)?;
        ensure_generation(computer, expected_generation)?;
        if computer.state != ComputerState::Active || computer.active_lease_ref.is_none() {
            return Err(invalid_transition());
        }
        computer.state = ComputerState::Stopping;
        computer.version += 1;
        computer.updated_at = observed_at;
        computer.validate()?;
        Ok(computer.clone())
    }

    #[cfg(test)]
    fn settle_stopped(
        &mut self,
        computer_ref: &str,
        expected_generation: u64,
        checkpoint_ref: String,
        observed_at: impl Into<String>,
    ) -> Result<CloudComputer, CloudComputerError> {
        let observed_at = observed_at.into();
        validate_timestamp(&observed_at)?;
        let computer = self.computers.get_mut(computer_ref).ok_or_else(not_found)?;
        ensure_generation(computer, expected_generation)?;
        if computer.state != ComputerState::Stopping || computer.active_lease_ref.is_none() {
            return Err(invalid_transition());
        }
        validate_public_ref(&checkpoint_ref)?;
        computer.active_lease_ref = None;
        computer.latest_checkpoint_ref = Some(checkpoint_ref);
        computer.state = ComputerState::Cold;
        computer.generation += 1;
        computer.version += 1;
        computer.updated_at = observed_at;
        computer.validate()?;
        Ok(computer.clone())
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn settle_failed_cleanup(
        &mut self,
        computer_ref: &str,
        expected_generation: u64,
        observed_at: impl Into<String>,
    ) -> Result<CloudComputer, CloudComputerError> {
        let observed_at = observed_at.into();
        validate_timestamp(&observed_at)?;
        let computer = self.computers.get_mut(computer_ref).ok_or_else(not_found)?;
        ensure_generation(computer, expected_generation)?;
        if computer.state != ComputerState::Failed || computer.active_lease_ref.is_none() {
            return Err(invalid_transition());
        }
        computer.active_lease_ref = None;
        computer.version += 1;
        computer.updated_at = observed_at;
        computer.validate()?;
        Ok(computer.clone())
    }

    pub fn authorize_existing_command(
        &self,
        authenticated_actor: &str,
        command: &CloudComputerCommand,
    ) -> Result<&CloudComputer, CloudComputerError> {
        let computer = self.authenticate_existing_command(authenticated_actor, command)?;
        if computer.generation != command.expected_generation {
            return Err(CloudComputerError::generation_mismatch(
                computer.generation,
                command.expected_generation,
            ));
        }
        Ok(computer)
    }

    fn authenticate_existing_command(
        &self,
        authenticated_actor: &str,
        command: &CloudComputerCommand,
    ) -> Result<&CloudComputer, CloudComputerError> {
        command.validate_authenticated(authenticated_actor)?;
        let computer = self
            .computers
            .get(&command.computer_ref)
            .ok_or_else(not_found)?;
        if computer.scope != command.scope {
            return Err(CloudComputerError::permission(
                CloudComputerErrorDetail::ScopeMismatch,
            ));
        }
        if !command.operation.allowed_for(computer.kind) {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::KindRuntimeClassMismatch,
            ));
        }
        Ok(computer)
    }

    pub fn begin_mutation(
        &mut self,
        authenticated_actor: &str,
        command: &CloudComputerCommand,
    ) -> Result<MutationAdmission, CloudComputerError> {
        if !command.operation.mutates() || command.operation == CloudComputerOperation::Create {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::InvalidTransition,
            ));
        }
        self.authenticate_existing_command(authenticated_actor, command)?;
        let canonical_request = canonical_command(command)?;
        if let Some(stored) = self.commands.get(&command.idempotency_key) {
            if stored.canonical_request != canonical_request {
                return Err(CloudComputerError {
                    code: CloudComputerErrorCode::IdempotencyConflict,
                    detail: CloudComputerErrorDetail::IdempotencyKeyReused,
                    retryable: false,
                    receipt_ref: stored
                        .receipt
                        .as_ref()
                        .map(|receipt| receipt.receipt_ref.clone()),
                    expected_generation: None,
                    received_generation: None,
                });
            }
            return Ok(match &stored.receipt {
                Some(receipt) => MutationAdmission::Replay(Box::new(receipt.clone())),
                None => MutationAdmission::Pending,
            });
        }
        self.authorize_existing_command(authenticated_actor, command)?;
        self.commands.insert(
            command.idempotency_key.clone(),
            StoredCommand {
                canonical_request,
                receipt: None,
            },
        );
        Ok(MutationAdmission::Proceed)
    }

    pub fn admit_provider_start(
        &mut self,
        authenticated_actor: &str,
        command: &CloudComputerCommand,
    ) -> Result<ProviderStartAdmission, CloudComputerError> {
        if !matches!(
            command.operation,
            CloudComputerOperation::Start | CloudComputerOperation::Restore
        ) {
            return Err(invalid_transition());
        }
        if command.operation == CloudComputerOperation::Restore
            && self
                .computers
                .get(&command.computer_ref)
                .ok_or_else(not_found)?
                .latest_checkpoint_ref
                .is_none()
        {
            return Err(invalid_transition());
        }
        match self.begin_mutation(authenticated_actor, command)? {
            MutationAdmission::Pending => Ok(ProviderStartAdmission::Pending),
            MutationAdmission::Replay(receipt) => Ok(ProviderStartAdmission::Replay(receipt)),
            MutationAdmission::Proceed => {
                let request = ProviderStartRequest::from_admitted_command(
                    self.computers
                        .get(&command.computer_ref)
                        .ok_or_else(not_found)?,
                    command,
                )?;
                Ok(ProviderStartAdmission::Proceed(Box::new(request)))
            }
        }
    }

    pub fn settle_mutation(
        &mut self,
        command: &CloudComputerCommand,
        receipt: CloudComputerReceipt,
    ) -> Result<CloudComputerReceipt, CloudComputerError> {
        if !command.operation.mutates() || command.operation == CloudComputerOperation::Create {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::InvalidTransition,
            ));
        }
        let canonical_request = canonical_command(command)?;
        let stored = self.commands.get(&command.idempotency_key).ok_or_else(|| {
            CloudComputerError::validation(CloudComputerErrorDetail::MutationNotReserved)
        })?;
        if stored.canonical_request != canonical_request {
            return Err(CloudComputerError {
                code: CloudComputerErrorCode::IdempotencyConflict,
                detail: CloudComputerErrorDetail::IdempotencyKeyReused,
                retryable: false,
                receipt_ref: stored
                    .receipt
                    .as_ref()
                    .map(|receipt| receipt.receipt_ref.clone()),
                expected_generation: None,
                received_generation: None,
            });
        }
        if let Some(existing) = &stored.receipt {
            return Ok(existing.clone());
        }
        let current = self
            .computers
            .get(&command.computer_ref)
            .cloned()
            .ok_or_else(not_found)?;
        let matches_command = receipt.command_ref == command.command_ref
            && receipt.computer_ref == command.computer_ref
            && receipt.scope == command.scope
            && receipt.operation == command.operation;
        if !matches_command {
            return Err(CloudComputerError::validation(
                CloudComputerErrorDetail::ScopeMismatch,
            ));
        }
        receipt.validate_public()?;
        let next = apply_receipt_transition(&current, &receipt)?;
        self.computers.insert(command.computer_ref.clone(), next);
        self.commands
            .get_mut(&command.idempotency_key)
            .expect("reservation exists")
            .receipt = Some(receipt.clone());
        Ok(receipt)
    }
}

fn apply_receipt_transition(
    current: &CloudComputer,
    receipt: &CloudComputerReceipt,
) -> Result<CloudComputer, CloudComputerError> {
    let operation_transition_allowed = matches!(
        (receipt.operation.clone(), current.state, receipt.state),
        (
            CloudComputerOperation::Start | CloudComputerOperation::Restore,
            ComputerState::Cold,
            ComputerState::Queued
        ) | (
            CloudComputerOperation::Start | CloudComputerOperation::Restore,
            ComputerState::Queued,
            ComputerState::Starting
        ) | (
            CloudComputerOperation::Start | CloudComputerOperation::Restore,
            ComputerState::Starting,
            ComputerState::Active | ComputerState::Failed
        ) | (
            CloudComputerOperation::Execute,
            ComputerState::Active,
            ComputerState::Active | ComputerState::Failed
        ) | (
            CloudComputerOperation::Checkpoint,
            ComputerState::Active,
            ComputerState::Active | ComputerState::Failed
        ) | (
            CloudComputerOperation::Cancel,
            ComputerState::Queued,
            ComputerState::Cold
        ) | (
            CloudComputerOperation::Cancel,
            ComputerState::Starting,
            ComputerState::Failed
        ) | (
            CloudComputerOperation::Cancel | CloudComputerOperation::Stop,
            ComputerState::Active,
            ComputerState::Stopping
        ) | (
            CloudComputerOperation::Cancel | CloudComputerOperation::Stop,
            ComputerState::Stopping,
            ComputerState::Cold | ComputerState::Failed
        ) | (
            CloudComputerOperation::Fork,
            ComputerState::Cold,
            ComputerState::Cold
        ) | (
            CloudComputerOperation::Destroy,
            ComputerState::Cold | ComputerState::Failed,
            ComputerState::Destroyed
        )
    );
    if !operation_transition_allowed {
        return Err(invalid_transition());
    }

    let mut next = current.clone();
    match (current.state, receipt.state) {
        (state, next_state) if state == next_state => {}
        (ComputerState::Cold, ComputerState::Queued) => next.state = ComputerState::Queued,
        (ComputerState::Queued, ComputerState::Cold) => next.state = ComputerState::Cold,
        (ComputerState::Queued, ComputerState::Starting) => {
            next.state = ComputerState::Starting;
            next.generation = next
                .generation
                .checked_add(1)
                .ok_or_else(invalid_transition)?;
        }
        (ComputerState::Starting, ComputerState::Active) => {
            let lease_ref = receipt
                .runtime_lease_ref
                .clone()
                .ok_or_else(invalid_transition)?;
            next.active_lease_ref = Some(lease_ref);
            next.state = ComputerState::Active;
        }
        (ComputerState::Active, ComputerState::Stopping) => {
            next.state = ComputerState::Stopping;
        }
        (ComputerState::Stopping, ComputerState::Cold) => {
            if receipt.checkpoint_digest.is_none() || receipt.cleanup_digest.is_none() {
                return Err(invalid_transition());
            }
            next.active_lease_ref = None;
            next.latest_checkpoint_ref = Some(format!("checkpoint.{}", receipt.receipt_ref));
            next.state = ComputerState::Cold;
            next.generation = next
                .generation
                .checked_add(1)
                .ok_or_else(invalid_transition)?;
        }
        (ComputerState::Starting, ComputerState::Failed)
        | (ComputerState::Active, ComputerState::Failed)
        | (ComputerState::Stopping, ComputerState::Failed) => {
            next.state = ComputerState::Failed;
        }
        (ComputerState::Cold, ComputerState::Destroyed)
        | (ComputerState::Failed, ComputerState::Destroyed) => {
            if current.active_lease_ref.is_some() || receipt.cleanup_digest.is_none() {
                return Err(invalid_transition());
            }
            next.state = ComputerState::Destroyed;
        }
        _ => return Err(invalid_transition()),
    }
    next.version = next.version.checked_add(1).ok_or_else(invalid_transition)?;
    next.updated_at = receipt.observed_at.clone();
    if next.generation != receipt.generation {
        return Err(CloudComputerError::generation_mismatch(
            next.generation,
            receipt.generation,
        ));
    }
    next.validate()?;
    Ok(next)
}

pub fn validate_public_safe_json(value: &serde_json::Value) -> Result<(), CloudComputerError> {
    const FORBIDDEN_KEYS: &[&str] = &[
        "credential",
        "secret",
        "private_path",
        "guest_address",
        "raw_provider",
        "admin_handle",
        "access_token",
    ];
    match value {
        serde_json::Value::Object(entries) => {
            for (key, child) in entries {
                let normalized = key.to_ascii_lowercase();
                if FORBIDDEN_KEYS
                    .iter()
                    .any(|forbidden| normalized.contains(forbidden))
                {
                    return Err(CloudComputerError::validation(
                        CloudComputerErrorDetail::ForbiddenPublicField,
                    ));
                }
                validate_public_safe_json(child)?;
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                validate_public_safe_json(child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn validate_public_error(error: &CloudComputerError) -> Result<(), CloudComputerError> {
    if let Some(receipt_ref) = &error.receipt_ref {
        validate_public_ref(receipt_ref)?;
    }
    Ok(())
}

pub fn validate_lease_for(
    computer: &CloudComputer,
    lease: &RuntimeLease,
) -> Result<(), CloudComputerError> {
    validate_lease_identity(computer, lease)?;
    if !matches!(
        computer.state,
        ComputerState::Active | ComputerState::Stopping
    ) || computer.active_lease_ref.as_deref() != Some(lease.lease_ref.as_str())
    {
        return Err(CloudComputerError::validation(
            CloudComputerErrorDetail::LeaseScopeMismatch,
        ));
    }
    Ok(())
}

fn validate_lease_identity(
    computer: &CloudComputer,
    lease: &RuntimeLease,
) -> Result<(), CloudComputerError> {
    if lease.schema != CLOUD_COMPUTER_LEASE_VERSION {
        return Err(CloudComputerError::validation(
            CloudComputerErrorDetail::UnsupportedSchema,
        ));
    }
    if lease.computer_ref != computer.computer_ref || lease.scope != computer.scope {
        return Err(CloudComputerError::validation(
            CloudComputerErrorDetail::LeaseScopeMismatch,
        ));
    }
    if lease.generation != computer.generation {
        return Err(CloudComputerError::validation(
            CloudComputerErrorDetail::LeaseGenerationMismatch,
        ));
    }
    validate_public_ref(&lease.lease_ref)?;
    validate_digest(&lease.image_digest)?;
    validate_digest(&lease.policy_digest)?;
    validate_timestamp(&lease.issued_at)?;
    validate_timestamp(&lease.expires_at)?;
    Ok(())
}

fn validate_public_ref(value: &str) -> Result<(), CloudComputerError> {
    let lower = value.to_ascii_lowercase();
    const PREFIXES: &[&str] = &[
        "owner.",
        "tenant.",
        "conversation.",
        "program.",
        "work.",
        "profile.",
        "capability.",
        "computer.",
        "command.",
        "key.",
        "receipt.",
        "lease.",
        "checkpoint.",
        "event.",
        "execution.",
        "actor.",
        "adapter.",
    ];
    let allowed = value.len() <= 512
        && PREFIXES.iter().any(|prefix| value.starts_with(prefix))
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'.' | b'_' | b'-'))
        });
    let looks_like_ipv4 = value.split('.').count() == 4
        && value
            .split('.')
            .all(|segment| !segment.is_empty() && segment.parse::<u8>().is_ok());
    let unsafe_value = value.trim().is_empty()
        || !allowed
        || looks_like_ipv4
        || lower.contains("credential")
        || lower.contains("secret")
        || lower.contains("token")
        || lower.contains("private_path")
        || lower.contains("guest_address")
        || lower.starts_with("projects.")
        || lower.starts_with("zones.")
        || lower.starts_with("instances.");
    if unsafe_value {
        return Err(CloudComputerError::validation(
            CloudComputerErrorDetail::UnsafePublicRef,
        ));
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<(), CloudComputerError> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(CloudComputerError::validation(
            CloudComputerErrorDetail::UnsafePublicRef,
        ));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(CloudComputerError::validation(
            CloudComputerErrorDetail::UnsafePublicRef,
        ));
    }
    Ok(())
}

fn validate_timestamp(value: &str) -> Result<(), CloudComputerError> {
    let bytes = value.as_bytes();
    let shape_invalid =
        bytes.len() != 20 || bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-');
    if shape_invalid
        || bytes.get(10) != Some(&b'T')
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
        || bytes.get(19) != Some(&b'Z')
        || bytes.iter().enumerate().any(|(index, byte)| {
            !matches!(index, 4 | 7 | 10 | 13 | 16 | 19) && !byte.is_ascii_digit()
        })
    {
        return Err(CloudComputerError::validation(
            CloudComputerErrorDetail::InvalidTimestamp,
        ));
    }
    let parse = |start: usize, end: usize| {
        value
            .get(start..end)
            .and_then(|part| part.parse::<u32>().ok())
    };
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        parse(0, 4),
        parse(5, 7),
        parse(8, 10),
        parse(11, 13),
        parse(14, 16),
        parse(17, 19),
    ) else {
        return Err(CloudComputerError::validation(
            CloudComputerErrorDetail::InvalidTimestamp,
        ));
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    if day == 0 || day > days || hour > 23 || minute > 59 || second > 59 {
        return Err(CloudComputerError::validation(
            CloudComputerErrorDetail::InvalidTimestamp,
        ));
    }
    Ok(())
}

fn not_found() -> CloudComputerError {
    CloudComputerError {
        code: CloudComputerErrorCode::NotFound,
        detail: CloudComputerErrorDetail::CloudComputerNotFound,
        retryable: false,
        receipt_ref: None,
        expected_generation: None,
        received_generation: None,
    }
}

fn invalid_transition() -> CloudComputerError {
    CloudComputerError {
        code: CloudComputerErrorCode::InvalidTransition,
        detail: CloudComputerErrorDetail::InvalidTransition,
        retryable: false,
        receipt_ref: None,
        expected_generation: None,
        received_generation: None,
    }
}

#[cfg(test)]
fn ensure_generation(
    computer: &CloudComputer,
    expected_generation: u64,
) -> Result<(), CloudComputerError> {
    if computer.generation != expected_generation {
        return Err(CloudComputerError::generation_mismatch(
            computer.generation,
            expected_generation,
        ));
    }
    Ok(())
}

fn canonical_command(command: &CloudComputerCommand) -> Result<Vec<u8>, CloudComputerError> {
    serde_json::to_vec(command)
        .map_err(|_| CloudComputerError::validation(CloudComputerErrorDetail::InvalidCreateCommand))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn scope() -> ComputerScope {
        ComputerScope {
            owner_ref: "owner.1".to_owned(),
            tenant_ref: "tenant.1".to_owned(),
            conversation_or_program_ref: "conversation.1".to_owned(),
            work_unit_ref: "work.1".to_owned(),
            runtime_profile_ref: "profile.standard.v1".to_owned(),
            authority_snapshot_digest: digest('a'),
            budget_snapshot_digest: digest('b'),
            capability_refs: vec!["capability.command".to_owned()],
        }
    }

    fn create_command(index: usize, key: &str, digest: &str) -> CloudComputerCommand {
        CloudComputerCommand {
            schema: CLOUD_COMPUTER_COMMAND_VERSION.to_owned(),
            command_ref: format!("command.{index}"),
            idempotency_key: key.to_owned(),
            request_digest: digest.to_owned(),
            requested_by_ref: "owner.1".to_owned(),
            computer_ref: format!("computer.{index}"),
            scope: scope(),
            expected_generation: 0,
            operation: CloudComputerOperation::Create,
            requested_at: "2026-08-22T16:00:00Z".to_owned(),
        }
    }

    #[test]
    fn thirty_creates_stay_cold_without_runtime_leases() {
        let mut model = CloudComputerModel::default();
        for index in 0..30 {
            let command = create_command(index, &format!("key.{index}"), &digest('c'));
            let receipt = model
                .create(
                    "owner.1",
                    &command,
                    scope(),
                    ComputerKind::InteractiveRetained,
                    RuntimeClass::Standard,
                )
                .expect("create logical computer");
            assert_eq!(receipt.state, ComputerState::Cold);
            assert_eq!(receipt.effective_provider, None);
        }
        assert_eq!(model.list().count(), 30);
        assert!(model
            .list()
            .all(|computer| computer.active_lease_ref.is_none()));
    }

    #[test]
    fn exact_retry_returns_original_and_changed_bytes_conflict() {
        let mut model = CloudComputerModel::default();
        let command = create_command(1, "key.same", &digest('c'));
        assert_eq!(
            model
                .create(
                    "owner.2",
                    &command,
                    scope(),
                    ComputerKind::InteractiveRetained,
                    RuntimeClass::Strong,
                )
                .expect_err("authenticated actor mismatch")
                .code,
            CloudComputerErrorCode::PermissionDenied
        );
        let first = model
            .create(
                "owner.1",
                &command,
                scope(),
                ComputerKind::InteractiveRetained,
                RuntimeClass::Strong,
            )
            .expect("first create");
        let retry = model
            .create(
                "owner.1",
                &command,
                scope(),
                ComputerKind::InteractiveRetained,
                RuntimeClass::Strong,
            )
            .expect("exact retry");
        assert_eq!(retry, first);
        assert_eq!(model.list().count(), 1);
        assert_eq!(
            model
                .create(
                    "owner.1",
                    &command,
                    scope(),
                    ComputerKind::OneShotBatch,
                    RuntimeClass::Batch,
                )
                .expect_err("changed create payload")
                .code,
            CloudComputerErrorCode::IdempotencyConflict
        );

        let changed = CloudComputerCommand {
            request_digest: digest('d'),
            ..command
        };
        let error = model
            .create(
                "owner.1",
                &changed,
                scope(),
                ComputerKind::InteractiveRetained,
                RuntimeClass::Strong,
            )
            .expect_err("changed retry must fail");
        assert_eq!(error.code, CloudComputerErrorCode::IdempotencyConflict);
        assert_eq!(model.list().count(), 1);
    }

    #[test]
    fn stale_generation_cannot_mutate_current_computer() {
        let mut model = CloudComputerModel::default();
        let command = create_command(1, "key.1", &digest('c'));
        model
            .create(
                "owner.1",
                &command,
                scope(),
                ComputerKind::InteractiveRetained,
                RuntimeClass::Standard,
            )
            .expect("create");
        let queued = model
            .transition(
                "computer.1",
                1,
                ComputerState::Queued,
                "2026-08-22T16:01:00Z",
            )
            .expect("queue");
        let starting = model
            .transition(
                "computer.1",
                queued.generation,
                ComputerState::Starting,
                "2026-08-22T16:02:00Z",
            )
            .expect("start");
        assert_eq!(starting.generation, 2);
        let error = model
            .transition(
                "computer.1",
                1,
                ComputerState::Failed,
                "2026-08-22T16:03:00Z",
            )
            .expect_err("stale generation must fail");
        assert_eq!(error.code, CloudComputerErrorCode::GenerationMismatch);
    }

    #[test]
    fn capability_refusal_does_not_substitute_provider() {
        let gke = ProviderCapabilities {
            provider: EffectiveProvider::GkeAgentSandbox,
            runtime_classes: vec![RuntimeClass::Standard],
            capabilities: vec![
                ProviderCapability::Interactive,
                ProviderCapability::Checkpoint,
            ],
        };
        let error = admit_selected_provider(
            &gke,
            RuntimeClass::Strong,
            &[ProviderCapability::Interactive],
        )
        .expect_err("selected provider must refuse");
        assert_eq!(error.code, CloudComputerErrorCode::UnsupportedCapability);
        assert_eq!(
            error.detail,
            CloudComputerErrorDetail::UnsupportedProviderCapability
        );
        assert_eq!(
            admit_selected_provider(
                &gke,
                RuntimeClass::Standard,
                &[ProviderCapability::Checkpoint],
            )
            .expect("supported selection"),
            EffectiveProvider::GkeAgentSandbox
        );
    }

    #[test]
    fn public_receipts_reject_private_field_names() {
        let receipt = CloudComputerReceipt {
            schema: CLOUD_COMPUTER_RECEIPT_VERSION.to_owned(),
            receipt_ref: "receipt.1".to_owned(),
            command_ref: "command.1".to_owned(),
            computer_ref: "computer.1".to_owned(),
            scope: scope(),
            generation: 1,
            operation: CloudComputerOperation::Create,
            state: ComputerState::Cold,
            runtime_lease_ref: None,
            effective_provider: None,
            image_digest: None,
            policy_digest: None,
            checkpoint_digest: None,
            usage_digest: None,
            cleanup_digest: None,
            error: None,
            observed_at: "2026-08-22T16:00:00Z".to_owned(),
        };
        receipt.validate_public().expect("public receipt");
        let unsafe_value = serde_json::json!({"guest_address": "10.0.0.2"});
        assert!(validate_public_safe_json(&unsafe_value).is_err());
        for topology in [
            "10.0.0.2",
            "fd00::1",
            "us-central1",
            "instance-worker-1",
            "arn:aws:iam::123456789012:role:admin",
        ] {
            assert!(validate_public_ref(topology).is_err());
        }
        let unsafe_receipt = CloudComputerReceipt {
            receipt_ref: "secret://provider-token".to_owned(),
            ..receipt
        };
        assert!(unsafe_receipt.validate_public().is_err());
        assert!(serde_json::to_value(&unsafe_receipt).is_err());
        let unsafe_cleanup = ProviderCleanupReceipt {
            computer_ref: "10.0.0.2".to_owned(),
            scope: scope(),
            generation: 1,
            lease_ref: "lease.1".to_owned(),
            cleanup_digest: digest('a'),
            observed_at: "2026-08-22T16:00:00Z".to_owned(),
        };
        assert!(serde_json::to_value(&unsafe_cleanup).is_err());
        let unsafe_execution = ProviderExecutionReceipt {
            schema: CLOUD_COMPUTER_EXECUTION_RECEIPT_VERSION.to_owned(),
            command_ref: "command.1".to_owned(),
            computer_ref: "computer.1".to_owned(),
            scope: scope(),
            generation: 1,
            lease_ref: "lease.1".to_owned(),
            execution_ref: "execution.1".to_owned(),
            output_digest: "/private/workspace/output".to_owned(),
            usage_digest: digest('b'),
            observed_at: "2026-08-22T16:00:00Z".to_owned(),
        };
        assert!(serde_json::to_value(&unsafe_execution).is_err());
    }

    #[test]
    fn existing_mutations_require_exact_scope_and_generation() {
        let mut model = CloudComputerModel::default();
        let command = create_command(1, "key.1", &digest('c'));
        model
            .create(
                "owner.1",
                &command,
                scope(),
                ComputerKind::InteractiveRetained,
                RuntimeClass::Standard,
            )
            .expect("create");

        let inspect = CloudComputerCommand {
            operation: CloudComputerOperation::Stop,
            expected_generation: 1,
            ..command.clone()
        };
        model
            .authorize_existing_command("owner.1", &inspect)
            .expect("exact scope and generation");

        let mut foreign_scope = scope();
        foreign_scope.owner_ref = "owner.2".to_owned();
        let foreign = CloudComputerCommand {
            scope: foreign_scope,
            ..inspect.clone()
        };
        assert_eq!(
            model
                .authorize_existing_command("owner.1", &foreign)
                .expect_err("foreign owner")
                .code,
            CloudComputerErrorCode::PermissionDenied
        );

        for operation in [
            CloudComputerOperation::Execute,
            CloudComputerOperation::Checkpoint,
            CloudComputerOperation::Restore,
            CloudComputerOperation::Stop,
            CloudComputerOperation::Destroy,
        ] {
            let stale = CloudComputerCommand {
                operation,
                expected_generation: 0,
                ..inspect.clone()
            };
            assert_eq!(
                model
                    .authorize_existing_command("owner.1", &stale)
                    .expect_err("stale mutation")
                    .code,
                CloudComputerErrorCode::GenerationMismatch
            );
        }
    }

    #[test]
    fn activation_and_stop_require_the_current_lease() {
        let mut model = CloudComputerModel::default();
        let command = create_command(1, "key.1", &digest('c'));
        model
            .create(
                "owner.1",
                &command,
                scope(),
                ComputerKind::InteractiveRetained,
                RuntimeClass::Standard,
            )
            .expect("create");
        model
            .transition(
                "computer.1",
                1,
                ComputerState::Queued,
                "2026-08-22T16:01:00Z",
            )
            .expect("queue");
        let starting = model
            .transition(
                "computer.1",
                1,
                ComputerState::Starting,
                "2026-08-22T16:02:00Z",
            )
            .expect("starting");
        let lease = RuntimeLease {
            schema: CLOUD_COMPUTER_LEASE_VERSION.to_owned(),
            lease_ref: "lease.1".to_owned(),
            computer_ref: "computer.1".to_owned(),
            scope: scope(),
            generation: starting.generation,
            effective_provider: EffectiveProvider::GkeAgentSandbox,
            image_digest: digest('d'),
            policy_digest: digest('e'),
            issued_at: "2026-08-22T16:02:00Z".to_owned(),
            expires_at: "2026-08-22T17:02:00Z".to_owned(),
        };
        let active = model
            .activate(
                "computer.1",
                starting.generation,
                lease.clone(),
                "2026-08-22T16:03:00Z",
            )
            .expect("activate");
        assert_eq!(active.active_lease_ref.as_deref(), Some("lease.1"));
        let bound = ProviderBoundRequest::new(&active, &lease).expect("current provider request");
        let fabricated = RuntimeLease {
            lease_ref: "lease.2".to_owned(),
            ..lease.clone()
        };
        assert!(ProviderBoundRequest::new(&active, &fabricated).is_err());
        let checkpoint = CloudComputerCheckpoint {
            schema: CLOUD_COMPUTER_CHECKPOINT_VERSION.to_owned(),
            checkpoint_ref: "checkpoint.1".to_owned(),
            computer_ref: active.computer_ref.clone(),
            scope: active.scope.clone(),
            generation: active.generation,
            parent_checkpoint_ref: None,
            content_digest: digest('a'),
            policy_digest: digest('b'),
            created_at: "2026-08-22T16:03:00Z".to_owned(),
        };
        checkpoint.validate_for(&bound).expect("bound checkpoint");
        let cleanup = ProviderCleanupReceipt {
            computer_ref: active.computer_ref.clone(),
            scope: active.scope.clone(),
            generation: active.generation,
            lease_ref: lease.lease_ref.clone(),
            cleanup_digest: digest('c'),
            observed_at: "2026-08-22T16:03:00Z".to_owned(),
        };
        cleanup.validate_for(&bound).expect("bound cleanup");
        let execution_request = ProviderExecutionRequest::new(
            &active,
            &lease,
            "command.execute".to_owned(),
            digest('d'),
        )
        .expect("bound execution request");
        let execution_receipt = ProviderExecutionReceipt {
            schema: CLOUD_COMPUTER_EXECUTION_RECEIPT_VERSION.to_owned(),
            command_ref: "command.execute".to_owned(),
            computer_ref: active.computer_ref.clone(),
            scope: active.scope.clone(),
            generation: active.generation,
            lease_ref: lease.lease_ref.clone(),
            execution_ref: "execution.1".to_owned(),
            output_digest: digest('e'),
            usage_digest: digest('f'),
            observed_at: "2026-08-22T16:03:00Z".to_owned(),
        };
        execution_receipt
            .validate_for(&execution_request)
            .expect("bound execution receipt");
        let stopping = model
            .begin_stop("computer.1", active.generation, "2026-08-22T16:04:00Z")
            .expect("begin stop");
        let cold = model
            .settle_stopped(
                "computer.1",
                stopping.generation,
                "checkpoint.1".to_owned(),
                "2026-08-22T16:05:00Z",
            )
            .expect("settle stop");
        assert_eq!(cold.state, ComputerState::Cold);
        assert_eq!(cold.active_lease_ref, None);
        assert_eq!(cold.generation, stopping.generation + 1);
        assert!(ProviderBoundRequest::new(&cold, &lease).is_err());
    }

    #[test]
    fn every_existing_mutation_replays_by_exact_request_digest() {
        let mut model = CloudComputerModel::default();
        let create = create_command(1, "key.create", &digest('c'));
        model
            .create(
                "owner.1",
                &create,
                scope(),
                ComputerKind::InteractiveRetained,
                RuntimeClass::Standard,
            )
            .expect("create");
        let command = CloudComputerCommand {
            command_ref: "command.destroy".to_owned(),
            idempotency_key: "key.destroy".to_owned(),
            request_digest: digest('d'),
            operation: CloudComputerOperation::Destroy,
            expected_generation: 1,
            ..create
        };
        let receipt = CloudComputerReceipt {
            schema: CLOUD_COMPUTER_RECEIPT_VERSION.to_owned(),
            receipt_ref: "receipt.destroy".to_owned(),
            command_ref: command.command_ref.clone(),
            computer_ref: command.computer_ref.clone(),
            scope: command.scope.clone(),
            generation: 1,
            operation: CloudComputerOperation::Destroy,
            state: ComputerState::Destroyed,
            runtime_lease_ref: None,
            effective_provider: None,
            image_digest: None,
            policy_digest: None,
            checkpoint_digest: None,
            usage_digest: None,
            cleanup_digest: Some(digest('e')),
            error: None,
            observed_at: "2026-08-22T16:10:00Z".to_owned(),
        };
        assert_eq!(
            model
                .begin_mutation("owner.1", &command)
                .expect("reserve before effect"),
            MutationAdmission::Proceed
        );
        let invalid_receipt = CloudComputerReceipt {
            state: ComputerState::Cold,
            ..receipt.clone()
        };
        assert_eq!(
            model
                .settle_mutation(&command, invalid_receipt)
                .expect_err("destroy cannot settle without destroying")
                .detail,
            CloudComputerErrorDetail::InvalidTransition
        );
        let first = model
            .settle_mutation(&command, receipt)
            .expect("settle mutation");
        assert_eq!(
            model.inspect("computer.1").expect("computer").state,
            ComputerState::Destroyed
        );
        assert_eq!(
            model
                .begin_mutation("owner.2", &command)
                .expect_err("foreign replay")
                .code,
            CloudComputerErrorCode::PermissionDenied
        );
        assert_eq!(
            model
                .begin_mutation("owner.1", &command)
                .expect("exact replay"),
            MutationAdmission::Replay(Box::new(first.clone()))
        );
        let changed_bytes_same_digest = CloudComputerCommand {
            operation: CloudComputerOperation::Stop,
            ..command.clone()
        };
        assert_eq!(
            model
                .begin_mutation("owner.1", &changed_bytes_same_digest)
                .expect_err("changed canonical bytes")
                .code,
            CloudComputerErrorCode::IdempotencyConflict
        );
        let changed = CloudComputerCommand {
            request_digest: digest('f'),
            ..command
        };
        assert_eq!(
            model
                .begin_mutation("owner.1", &changed)
                .expect_err("changed retry")
                .code,
            CloudComputerErrorCode::IdempotencyConflict
        );
    }

    #[test]
    fn provider_start_requests_require_an_admitted_current_command() {
        let mut model = CloudComputerModel::default();
        let create = create_command(1, "key.create", &digest('c'));
        model
            .create(
                "owner.1",
                &create,
                scope(),
                ComputerKind::InteractiveRetained,
                RuntimeClass::Standard,
            )
            .expect("create");
        model
            .transition(
                "computer.1",
                1,
                ComputerState::Queued,
                "2026-08-22T16:01:00Z",
            )
            .expect("queue");
        let start = CloudComputerCommand {
            command_ref: "command.start".to_owned(),
            idempotency_key: "key.start".to_owned(),
            request_digest: digest('d'),
            operation: CloudComputerOperation::Start,
            expected_generation: 1,
            ..create.clone()
        };
        let request = match model
            .admit_provider_start("owner.1", &start)
            .expect("admit start")
        {
            ProviderStartAdmission::Proceed(request) => request,
            admission => panic!("unexpected admission: {admission:?}"),
        };
        assert_eq!(request.computer_ref(), "computer.1");
        assert_eq!(request.command_ref(), "command.start");
        assert_eq!(request.generation(), 1);
        assert_eq!(request.checkpoint_ref(), None);
        assert_eq!(
            model
                .admit_provider_start("owner.1", &start)
                .expect("exact pending retry"),
            ProviderStartAdmission::Pending
        );
        let receipt = CloudComputerReceipt {
            schema: CLOUD_COMPUTER_RECEIPT_VERSION.to_owned(),
            receipt_ref: "receipt.start".to_owned(),
            command_ref: start.command_ref.clone(),
            computer_ref: start.computer_ref.clone(),
            scope: start.scope.clone(),
            generation: 2,
            operation: CloudComputerOperation::Start,
            state: ComputerState::Starting,
            runtime_lease_ref: None,
            effective_provider: Some(EffectiveProvider::GkeAgentSandbox),
            image_digest: Some(digest('e')),
            policy_digest: Some(digest('f')),
            checkpoint_digest: None,
            usage_digest: None,
            cleanup_digest: None,
            error: None,
            observed_at: "2026-08-22T16:02:00Z".to_owned(),
        };
        let settled = model
            .settle_mutation(&start, receipt)
            .expect("settle generation-advancing start");
        assert_eq!(
            model
                .admit_provider_start("owner.1", &start)
                .expect("settled exact retry"),
            ProviderStartAdmission::Replay(Box::new(settled))
        );

        let stale = CloudComputerCommand {
            command_ref: "command.stale".to_owned(),
            idempotency_key: "key.stale".to_owned(),
            expected_generation: 0,
            ..start
        };
        assert_eq!(
            model
                .admit_provider_start("owner.1", &stale)
                .expect_err("stale start cannot reach provider")
                .code,
            CloudComputerErrorCode::GenerationMismatch
        );
    }

    #[test]
    fn contract_represents_interactive_and_batch_without_conflation() {
        let interactive = CloudComputer::new_cold(
            "computer.interactive",
            scope(),
            ComputerKind::InteractiveRetained,
            RuntimeClass::Standard,
            "2026-08-22T16:00:00Z",
        );
        let batch = CloudComputer::new_cold(
            "computer.batch",
            scope(),
            ComputerKind::OneShotBatch,
            RuntimeClass::Batch,
            "2026-08-22T16:00:00Z",
        );
        assert_ne!(interactive.kind, batch.kind);
        assert_eq!(batch.requested_runtime_class, RuntimeClass::Batch);
        assert!(interactive.validate().is_ok());
        assert!(batch.validate().is_ok());
        let invalid = CloudComputer::new_cold(
            "computer.invalid",
            scope(),
            ComputerKind::OneShotBatch,
            RuntimeClass::Standard,
            "2026-08-22T16:00:00Z",
        );
        assert_eq!(
            invalid.validate().expect_err("kind/class mismatch").detail,
            CloudComputerErrorDetail::KindRuntimeClassMismatch
        );
        assert!(!CloudComputerOperation::Attach.allowed_for(ComputerKind::OneShotBatch));
        assert!(!CloudComputerOperation::Stop.allowed_for(ComputerKind::OneShotBatch));
    }

    #[test]
    fn rust_validation_matches_closed_resource_schema_constraints() {
        let computer = CloudComputer::new_cold(
            "computer.1",
            scope(),
            ComputerKind::InteractiveRetained,
            RuntimeClass::Standard,
            "2026-08-22T16:00:00Z",
        );
        let mut wire = serde_json::to_value(&computer).expect("serialize computer");
        wire.as_object_mut()
            .expect("object")
            .insert("unknown".to_owned(), serde_json::json!(true));
        assert!(serde_json::from_value::<CloudComputer>(wire).is_err());

        let mut duplicate_scope = scope();
        duplicate_scope
            .capability_refs
            .push("capability.command".to_owned());
        let duplicate = CloudComputer::new_cold(
            "computer.duplicate",
            duplicate_scope,
            ComputerKind::InteractiveRetained,
            RuntimeClass::Standard,
            "2026-08-22T16:00:00Z",
        );
        assert_eq!(
            duplicate
                .validate()
                .expect_err("duplicate capability")
                .detail,
            CloudComputerErrorDetail::DuplicateCapabilityRef
        );

        let long_ref = CloudComputer::new_cold(
            "x".repeat(513),
            scope(),
            ComputerKind::InteractiveRetained,
            RuntimeClass::Standard,
            "2026-08-22T16:00:00Z",
        );
        assert_eq!(
            long_ref.validate().expect_err("long ref").detail,
            CloudComputerErrorDetail::UnsafePublicRef
        );

        let bad_time = CloudComputer::new_cold(
            "computer.bad-time",
            scope(),
            ComputerKind::InteractiveRetained,
            RuntimeClass::Standard,
            "not-a-time",
        );
        assert_eq!(
            bad_time.validate().expect_err("bad time").detail,
            CloudComputerErrorDetail::InvalidTimestamp
        );
        let impossible_date = CloudComputer::new_cold(
            "computer.bad-date",
            scope(),
            ComputerKind::InteractiveRetained,
            RuntimeClass::Standard,
            "2026-02-30T16:00:00Z",
        );
        assert_eq!(
            impossible_date
                .validate()
                .expect_err("impossible date")
                .detail,
            CloudComputerErrorDetail::InvalidTimestamp
        );
    }
}
