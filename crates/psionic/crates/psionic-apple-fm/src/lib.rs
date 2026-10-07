//! Reusable Apple Foundation Models bridge contracts and client for Psionic.
//!
//! This crate owns transport-neutral request and response types for the current
//! Swift bridge plus the reusable HTTP client used by product code. App-level
//! supervision, pane orchestration, and process lifecycle stay out of this
//! crate on purpose.

pub mod client;
pub mod contract;
pub mod error;
pub mod structured;
mod tassadar_post_article_starter_plugin_tools;
pub mod tool;
pub mod transcript;

pub use client::{
    AppleFmAsyncBridgeClient, AppleFmBridgeClient, AppleFmBridgeClientError,
    AppleFmBridgeStreamError, AppleFmTextResponseStream,
    TASSADAR_POST_ARTICLE_APPLE_FM_PLUGIN_SESSION_PILOT_BUNDLE_REF,
    TASSADAR_POST_ARTICLE_APPLE_FM_PLUGIN_SESSION_PILOT_RUN_ROOT_REF,
    TassadarPostArticleAppleFmPluginSessionPilotBundle,
    TassadarPostArticleAppleFmPluginSessionPilotError, TassadarPostArticleAppleFmSessionCaseRow,
    TassadarPostArticleAppleFmSessionStepRow, TassadarPostArticleAppleFmToolDefinitionRow,
    build_tassadar_post_article_apple_fm_plugin_session_pilot_bundle,
    tassadar_post_article_apple_fm_plugin_session_pilot_bundle_path,
    write_tassadar_post_article_apple_fm_plugin_session_pilot_bundle,
};
pub use contract::{
    APPLE_FM_BRIDGE_ADAPTER_SUFFIX, APPLE_FM_BRIDGE_ADAPTERS_PATH,
    APPLE_FM_BRIDGE_CHAT_COMPLETIONS_PATH, APPLE_FM_BRIDGE_HEALTH_PATH,
    APPLE_FM_BRIDGE_MODELS_PATH, APPLE_FM_BRIDGE_SESSIONS_PATH, APPLE_FM_BRIDGE_STREAM_SUFFIX,
    APPLE_FM_BRIDGE_STRUCTURED_SUFFIX, APPLE_FM_BRIDGE_TRANSCRIPT_SUFFIX,
    AppleFmAdapterAttachRequest, AppleFmAdapterCompatibility, AppleFmAdapterInventoryEntry,
    AppleFmAdapterLoadRequest, AppleFmAdapterLoadResponse, AppleFmAdapterSelection,
    AppleFmAdaptersResponse, AppleFmChatChoice, AppleFmChatCompletionRequest,
    AppleFmChatCompletionResponse, AppleFmChatMessage, AppleFmChatMessageRole,
    AppleFmChatResponseMessage, AppleFmChatUsage, AppleFmCompletionResult, AppleFmErrorCode,
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
pub use error::AppleFmFoundationModelsError;
pub use structured::{
    AppleFmGeneratedContent, AppleFmGenerationId, AppleFmGenerationSchema, AppleFmStructuredType,
    AppleFmStructuredValueError,
};
pub use tassadar_post_article_starter_plugin_tools::{
    TassadarStarterPluginAppleFmTool, TassadarStarterPluginAppleFmToolError,
    tassadar_starter_plugin_apple_fm_tool_definitions, tassadar_starter_plugin_apple_fm_tools,
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
