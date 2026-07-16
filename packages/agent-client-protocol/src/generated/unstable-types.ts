/*
Generated from Agent Client Protocol schema-v1.19.0 unstable schema.
Source: https://github.com/agentclientprotocol/agent-client-protocol/releases/download/schema-v1.19.0/schema.unstable.json
SHA-256: 8bdfd8347ce8bd2c8620b71bfd5460625f91c7db47a51268bb42b67014ea5b1f
Generate: pnpm --dir packages/agent-client-protocol generate
License: Apache-2.0; see THIRD_PARTY_NOTICES.md and upstream/schema-v1.19.0/LICENSE.
Do not edit.
The official SDK 1.2.1 is generated from this exact unstable artifact; these aliases never enter ./stable.
*/
export type AcceptNesNotification = import("@agentclientprotocol/sdk").AcceptNesNotification;
export type AuthCapabilities = import("@agentclientprotocol/sdk").AuthCapabilities;
export type AuthEnvVar = import("@agentclientprotocol/sdk").AuthEnvVar;
export type AuthMethodEnvVar = import("@agentclientprotocol/sdk").AuthMethodEnvVar;
export type AuthMethodTerminal = import("@agentclientprotocol/sdk").AuthMethodTerminal;
export type BooleanPropertySchema = import("@agentclientprotocol/sdk").BooleanPropertySchema;
export type ClientNesCapabilities = import("@agentclientprotocol/sdk").ClientNesCapabilities;
export type CloseNesRequest = import("@agentclientprotocol/sdk").CloseNesRequest;
export type CloseNesResponse = import("@agentclientprotocol/sdk").CloseNesResponse;
export type CompleteElicitationNotification =
  import("@agentclientprotocol/sdk").CompleteElicitationNotification;
export type ConnectMcpRequest = import("@agentclientprotocol/sdk").ConnectMcpRequest;
export type ConnectMcpResponse = import("@agentclientprotocol/sdk").ConnectMcpResponse;
export type CreateElicitationRequest = import("@agentclientprotocol/sdk").CreateElicitationRequest;
export type CreateElicitationResponse =
  import("@agentclientprotocol/sdk").CreateElicitationResponse;
export type DidChangeDocumentNotification =
  import("@agentclientprotocol/sdk").DidChangeDocumentNotification;
export type DidCloseDocumentNotification =
  import("@agentclientprotocol/sdk").DidCloseDocumentNotification;
export type DidFocusDocumentNotification =
  import("@agentclientprotocol/sdk").DidFocusDocumentNotification;
export type DidOpenDocumentNotification =
  import("@agentclientprotocol/sdk").DidOpenDocumentNotification;
export type DidSaveDocumentNotification =
  import("@agentclientprotocol/sdk").DidSaveDocumentNotification;
export type DisableProviderRequest = import("@agentclientprotocol/sdk").DisableProviderRequest;
export type DisableProviderResponse = import("@agentclientprotocol/sdk").DisableProviderResponse;
export type DisconnectMcpRequest = import("@agentclientprotocol/sdk").DisconnectMcpRequest;
export type DisconnectMcpResponse = import("@agentclientprotocol/sdk").DisconnectMcpResponse;
export type ElicitationAcceptAction = import("@agentclientprotocol/sdk").ElicitationAcceptAction;
export type ElicitationCapabilities = import("@agentclientprotocol/sdk").ElicitationCapabilities;
export type ElicitationContentValue = import("@agentclientprotocol/sdk").ElicitationContentValue;
export type ElicitationFormCapabilities =
  import("@agentclientprotocol/sdk").ElicitationFormCapabilities;
export type ElicitationFormMode = import("@agentclientprotocol/sdk").ElicitationFormMode;
export type ElicitationId = import("@agentclientprotocol/sdk").ElicitationId;
export type ElicitationPropertySchema =
  import("@agentclientprotocol/sdk").ElicitationPropertySchema;
export type ElicitationRequestScope = import("@agentclientprotocol/sdk").ElicitationRequestScope;
export type ElicitationSchema = import("@agentclientprotocol/sdk").ElicitationSchema;
export type ElicitationSchemaType = import("@agentclientprotocol/sdk").ElicitationSchemaType;
export type ElicitationSessionScope = import("@agentclientprotocol/sdk").ElicitationSessionScope;
export type ElicitationUrlCapabilities =
  import("@agentclientprotocol/sdk").ElicitationUrlCapabilities;
export type ElicitationUrlMode = import("@agentclientprotocol/sdk").ElicitationUrlMode;
export type EnumOption = import("@agentclientprotocol/sdk").EnumOption;
export type ForkSessionRequest = import("@agentclientprotocol/sdk").ForkSessionRequest;
export type ForkSessionResponse = import("@agentclientprotocol/sdk").ForkSessionResponse;
export type IntegerPropertySchema = import("@agentclientprotocol/sdk").IntegerPropertySchema;
export type ListProvidersRequest = import("@agentclientprotocol/sdk").ListProvidersRequest;
export type ListProvidersResponse = import("@agentclientprotocol/sdk").ListProvidersResponse;
export type LlmProtocol = import("@agentclientprotocol/sdk").LlmProtocol;
export type McpConnectionId = import("@agentclientprotocol/sdk").McpConnectionId;
export type McpServerAcp = import("@agentclientprotocol/sdk").McpServerAcp;
export type McpServerAcpId = import("@agentclientprotocol/sdk").McpServerAcpId;
export type MessageMcpNotification = import("@agentclientprotocol/sdk").MessageMcpNotification;
export type MessageMcpRequest = import("@agentclientprotocol/sdk").MessageMcpRequest;
export type MessageMcpResponse = import("@agentclientprotocol/sdk").MessageMcpResponse;
export type MultiSelectItems = import("@agentclientprotocol/sdk").MultiSelectItems;
export type MultiSelectPropertySchema =
  import("@agentclientprotocol/sdk").MultiSelectPropertySchema;
export type NesCapabilities = import("@agentclientprotocol/sdk").NesCapabilities;
export type NesContextCapabilities = import("@agentclientprotocol/sdk").NesContextCapabilities;
export type NesDiagnostic = import("@agentclientprotocol/sdk").NesDiagnostic;
export type NesDiagnosticSeverity = import("@agentclientprotocol/sdk").NesDiagnosticSeverity;
export type NesDiagnosticsCapabilities =
  import("@agentclientprotocol/sdk").NesDiagnosticsCapabilities;
export type NesDocumentDidChangeCapabilities =
  import("@agentclientprotocol/sdk").NesDocumentDidChangeCapabilities;
export type NesDocumentDidCloseCapabilities =
  import("@agentclientprotocol/sdk").NesDocumentDidCloseCapabilities;
export type NesDocumentDidFocusCapabilities =
  import("@agentclientprotocol/sdk").NesDocumentDidFocusCapabilities;
export type NesDocumentDidOpenCapabilities =
  import("@agentclientprotocol/sdk").NesDocumentDidOpenCapabilities;
export type NesDocumentDidSaveCapabilities =
  import("@agentclientprotocol/sdk").NesDocumentDidSaveCapabilities;
export type NesDocumentEventCapabilities =
  import("@agentclientprotocol/sdk").NesDocumentEventCapabilities;
export type NesEditHistoryCapabilities =
  import("@agentclientprotocol/sdk").NesEditHistoryCapabilities;
export type NesEditHistoryEntry = import("@agentclientprotocol/sdk").NesEditHistoryEntry;
export type NesEditSuggestion = import("@agentclientprotocol/sdk").NesEditSuggestion;
export type NesEventCapabilities = import("@agentclientprotocol/sdk").NesEventCapabilities;
export type NesExcerpt = import("@agentclientprotocol/sdk").NesExcerpt;
export type NesJumpCapabilities = import("@agentclientprotocol/sdk").NesJumpCapabilities;
export type NesJumpSuggestion = import("@agentclientprotocol/sdk").NesJumpSuggestion;
export type NesOpenFile = import("@agentclientprotocol/sdk").NesOpenFile;
export type NesOpenFilesCapabilities = import("@agentclientprotocol/sdk").NesOpenFilesCapabilities;
export type NesRecentFile = import("@agentclientprotocol/sdk").NesRecentFile;
export type NesRecentFilesCapabilities =
  import("@agentclientprotocol/sdk").NesRecentFilesCapabilities;
export type NesRejectReason = import("@agentclientprotocol/sdk").NesRejectReason;
export type NesRelatedSnippet = import("@agentclientprotocol/sdk").NesRelatedSnippet;
export type NesRelatedSnippetsCapabilities =
  import("@agentclientprotocol/sdk").NesRelatedSnippetsCapabilities;
export type NesRenameCapabilities = import("@agentclientprotocol/sdk").NesRenameCapabilities;
export type NesRenameSuggestion = import("@agentclientprotocol/sdk").NesRenameSuggestion;
export type NesRepository = import("@agentclientprotocol/sdk").NesRepository;
export type NesSearchAndReplaceCapabilities =
  import("@agentclientprotocol/sdk").NesSearchAndReplaceCapabilities;
export type NesSearchAndReplaceSuggestion =
  import("@agentclientprotocol/sdk").NesSearchAndReplaceSuggestion;
export type NesSuggestContext = import("@agentclientprotocol/sdk").NesSuggestContext;
export type NesSuggestion = import("@agentclientprotocol/sdk").NesSuggestion;
export type NesSuggestionId = import("@agentclientprotocol/sdk").NesSuggestionId;
export type NesTextEdit = import("@agentclientprotocol/sdk").NesTextEdit;
export type NesTriggerKind = import("@agentclientprotocol/sdk").NesTriggerKind;
export type NesUserAction = import("@agentclientprotocol/sdk").NesUserAction;
export type NesUserActionsCapabilities =
  import("@agentclientprotocol/sdk").NesUserActionsCapabilities;
export type NumberPropertySchema = import("@agentclientprotocol/sdk").NumberPropertySchema;
export type PlanCapabilities = import("@agentclientprotocol/sdk").PlanCapabilities;
export type PlanFile = import("@agentclientprotocol/sdk").PlanFile;
export type PlanId = import("@agentclientprotocol/sdk").PlanId;
export type PlanItems = import("@agentclientprotocol/sdk").PlanItems;
export type PlanMarkdown = import("@agentclientprotocol/sdk").PlanMarkdown;
export type PlanRemoved = import("@agentclientprotocol/sdk").PlanRemoved;
export type PlanUpdate = import("@agentclientprotocol/sdk").PlanUpdate;
export type PlanUpdateContent = import("@agentclientprotocol/sdk").PlanUpdateContent;
export type Position = import("@agentclientprotocol/sdk").Position;
export type PositionEncodingKind = import("@agentclientprotocol/sdk").PositionEncodingKind;
export type ProviderCurrentConfig = import("@agentclientprotocol/sdk").ProviderCurrentConfig;
export type ProviderId = import("@agentclientprotocol/sdk").ProviderId;
export type ProviderInfo = import("@agentclientprotocol/sdk").ProviderInfo;
export type ProvidersCapabilities = import("@agentclientprotocol/sdk").ProvidersCapabilities;
export type Range = import("@agentclientprotocol/sdk").Range;
export type RejectNesNotification = import("@agentclientprotocol/sdk").RejectNesNotification;
export type SessionForkCapabilities = import("@agentclientprotocol/sdk").SessionForkCapabilities;
export type SetProviderRequest = import("@agentclientprotocol/sdk").SetProviderRequest;
export type SetProviderResponse = import("@agentclientprotocol/sdk").SetProviderResponse;
export type StartNesRequest = import("@agentclientprotocol/sdk").StartNesRequest;
export type StartNesResponse = import("@agentclientprotocol/sdk").StartNesResponse;
export type StringFormat = import("@agentclientprotocol/sdk").StringFormat;
export type StringMultiSelectItems = import("@agentclientprotocol/sdk").StringMultiSelectItems;
export type StringPropertySchema = import("@agentclientprotocol/sdk").StringPropertySchema;
export type SuggestNesRequest = import("@agentclientprotocol/sdk").SuggestNesRequest;
export type SuggestNesResponse = import("@agentclientprotocol/sdk").SuggestNesResponse;
export type TextDocumentContentChangeEvent =
  import("@agentclientprotocol/sdk").TextDocumentContentChangeEvent;
export type TextDocumentSyncKind = import("@agentclientprotocol/sdk").TextDocumentSyncKind;
export type TitledMultiSelectItems = import("@agentclientprotocol/sdk").TitledMultiSelectItems;
export type Usage = import("@agentclientprotocol/sdk").Usage;
export type WorkspaceFolder = import("@agentclientprotocol/sdk").WorkspaceFolder;
