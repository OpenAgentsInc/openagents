//! Reusable Apple Foundation Models bridge contracts and client for Psionic.
//!
//! This crate owns transport-neutral request and response types for the current
//! Swift bridge plus the reusable HTTP client used by product code. App-level
//! supervision, pane orchestration, and process lifecycle stay out of this
//! crate on purpose.

pub mod client;
pub mod contract;
pub mod structured;
pub mod tool;
pub mod transcript;

pub use client::{
    AppleFmAsyncBridgeClient, AppleFmBridgeClient, AppleFmBridgeClientError,
    AppleFmBridgeStreamError, AppleFmTextResponseStream,
};
pub use contract::{
    APPLE_FM_BRIDGE_CHAT_COMPLETIONS_PATH, APPLE_FM_BRIDGE_HEALTH_PATH,
    APPLE_FM_BRIDGE_MODELS_PATH, APPLE_FM_BRIDGE_SESSIONS_PATH, APPLE_FM_BRIDGE_STREAM_SUFFIX,
    APPLE_FM_BRIDGE_STRUCTURED_SUFFIX, APPLE_FM_BRIDGE_TRANSCRIPT_SUFFIX, AppleFmChatChoice,
    AppleFmChatCompletionRequest, AppleFmChatCompletionResponse, AppleFmChatMessage,
    AppleFmChatMessageRole, AppleFmChatResponseMessage, AppleFmChatUsage, AppleFmCompletionResult,
    AppleFmErrorDetail, AppleFmErrorResponse, AppleFmGenerationOptions,
    AppleFmGenerationOptionsValidationError, AppleFmHealthResponse, AppleFmModelInfo,
    AppleFmModelsResponse, AppleFmSamplingMode, AppleFmSamplingModeType, AppleFmSession,
    AppleFmSessionCreateRequest, AppleFmSessionCreateResponse, AppleFmSessionRespondRequest,
    AppleFmSessionRespondResponse, AppleFmSessionStructuredGenerationRequest,
    AppleFmSessionStructuredGenerationResponse, AppleFmSessionToolMetadata,
    AppleFmStructuredGenerationRequest, AppleFmStructuredGenerationResponse,
    AppleFmSystemLanguageModel, AppleFmSystemLanguageModelAvailability,
    AppleFmSystemLanguageModelGuardrails, AppleFmSystemLanguageModelUnavailableReason,
    AppleFmSystemLanguageModelUseCase, AppleFmTextGenerationRequest, AppleFmTextGenerationResponse,
    AppleFmTextStreamEvent, AppleFmTextStreamEventKind, AppleFmToolCallError,
    AppleFmToolCallRequest, AppleFmToolCallResponse, AppleFmToolCallbackConfiguration,
    AppleFmToolDefinition, AppleFmUsageMeasurement, AppleFmUsageTruth, DEFAULT_APPLE_FM_MODEL_ID,
};
pub use structured::{
    AppleFmGeneratedContent, AppleFmGenerationId, AppleFmGenerationSchema, AppleFmStructuredType,
    AppleFmStructuredValueError,
};
pub use tool::AppleFmTool;
pub use transcript::{
    APPLE_FM_TRANSCRIPT_TYPE, APPLE_FM_TRANSCRIPT_VERSION, AppleFmTranscript,
    AppleFmTranscriptContent, AppleFmTranscriptEntry, AppleFmTranscriptError,
    AppleFmTranscriptPayload,
};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str =
    "apple foundation models bridge contracts, client, and conformance substrate";
