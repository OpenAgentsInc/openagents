//! Reusable Apple Foundation Models bridge contracts and client for Psionic.
//!
//! This crate owns transport-neutral request and response types for the current
//! Swift bridge plus the reusable HTTP client used by product code. App-level
//! supervision, pane orchestration, and process lifecycle stay out of this
//! crate on purpose.

pub mod client;
pub mod contract;

pub use client::{AppleFmBridgeClient, AppleFmBridgeClientError};
pub use contract::{
    APPLE_FM_BRIDGE_CHAT_COMPLETIONS_PATH, APPLE_FM_BRIDGE_HEALTH_PATH,
    APPLE_FM_BRIDGE_MODELS_PATH, APPLE_FM_BRIDGE_SESSIONS_PATH, AppleFmChatChoice,
    AppleFmChatCompletionRequest, AppleFmChatCompletionResponse, AppleFmChatMessage,
    AppleFmChatMessageRole, AppleFmChatResponseMessage, AppleFmChatUsage, AppleFmCompletionResult,
    AppleFmErrorDetail, AppleFmErrorResponse, AppleFmGenerationOptions,
    AppleFmGenerationOptionsValidationError, AppleFmHealthResponse, AppleFmModelInfo,
    AppleFmModelsResponse, AppleFmSamplingMode, AppleFmSamplingModeType, AppleFmSession,
    AppleFmSessionCreateRequest, AppleFmSessionCreateResponse, AppleFmSessionRespondRequest,
    AppleFmSessionRespondResponse, AppleFmSessionToolMetadata, AppleFmSystemLanguageModel,
    AppleFmSystemLanguageModelAvailability, AppleFmSystemLanguageModelGuardrails,
    AppleFmSystemLanguageModelUnavailableReason, AppleFmSystemLanguageModelUseCase,
    AppleFmTextGenerationRequest, AppleFmTextGenerationResponse, AppleFmUsageMeasurement,
    AppleFmUsageTruth, DEFAULT_APPLE_FM_MODEL_ID,
};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str =
    "apple foundation models bridge contracts, client, and conformance substrate";
