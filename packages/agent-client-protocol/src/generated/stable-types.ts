/*
Generated from Agent Client Protocol schema-v1.19.0 stable schema.
Source: https://github.com/agentclientprotocol/agent-client-protocol/releases/download/schema-v1.19.0/schema.json
SHA-256: 92c1dfcda10dd47e99127500a3763da2b471f9ac61e12b9bf0430c32cf953796
Generate: pnpm --dir packages/agent-client-protocol generate
License: Apache-2.0; see THIRD_PARTY_NOTICES.md and upstream/schema-v1.19.0/LICENSE.
Do not edit.
These structural types are generated directly from the stable artifact; no unstable SDK types enter ./stable.
*/
export type AgentAuthCapabilities = {
  logout?: LogoutCapabilities | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type AgentCapabilities = {
  loadSession?: boolean;
  promptCapabilities?: PromptCapabilities;
  mcpCapabilities?: McpCapabilities;
  sessionCapabilities?: SessionCapabilities;
  auth?: AgentAuthCapabilities;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type AgentNotification = {
  method: string;
  params?: (SessionNotification | ExtNotification) | null;
  [key: string]: unknown;
};
export type AgentRequest = {
  id: RequestId;
  method: string;
  params?:
    | (
        | WriteTextFileRequest
        | ReadTextFileRequest
        | RequestPermissionRequest
        | CreateTerminalRequest
        | TerminalOutputRequest
        | ReleaseTerminalRequest
        | WaitForTerminalExitRequest
        | KillTerminalRequest
        | ExtRequest
      )
    | null;
  [key: string]: unknown;
};
export type AgentResponse =
  | {
      id: RequestId;
      result:
        | InitializeResponse
        | AuthenticateResponse
        | LogoutResponse
        | NewSessionResponse
        | LoadSessionResponse
        | ListSessionsResponse
        | DeleteSessionResponse
        | ResumeSessionResponse
        | CloseSessionResponse
        | SetSessionModeResponse
        | SetSessionConfigOptionResponse
        | PromptResponse
        | ExtResponse;
      [key: string]: unknown;
    }
  | { id: RequestId; error: Error; [key: string]: unknown };
export type Annotations = {
  audience?: Array<Role> | null;
  lastModified?: string | null;
  priority?: number | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type AudioContent = {
  annotations?: Annotations | null;
  data: string;
  mimeType: string;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type AuthMethod = AuthMethodAgent;
export type AuthMethodAgent = {
  id: AuthMethodId;
  name: string;
  description?: string | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type AuthMethodId = string;
export type AuthenticateRequest = {
  methodId: AuthMethodId;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type AuthenticateResponse = {
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type AvailableCommand = {
  name: string;
  description: string;
  input?: AvailableCommandInput | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type AvailableCommandInput = UnstructuredCommandInput;
export type AvailableCommandsUpdate = {
  availableCommands: Array<AvailableCommand>;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type BlobResourceContents = {
  blob: string;
  mimeType?: string | null;
  uri: string;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type BooleanConfigOptionCapabilities = {
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type CancelNotification = {
  sessionId: SessionId;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type CancelRequestNotification = {
  requestId: RequestId;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ClientCapabilities = {
  fs?: FileSystemCapabilities;
  terminal?: boolean;
  session?: ClientSessionCapabilities | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ClientNotification = {
  method: string;
  params?: (CancelNotification | ExtNotification) | null;
  [key: string]: unknown;
};
export type ClientRequest = {
  id: RequestId;
  method: string;
  params?:
    | (
        | InitializeRequest
        | AuthenticateRequest
        | LogoutRequest
        | NewSessionRequest
        | LoadSessionRequest
        | ListSessionsRequest
        | DeleteSessionRequest
        | ResumeSessionRequest
        | CloseSessionRequest
        | SetSessionModeRequest
        | SetSessionConfigOptionRequest
        | PromptRequest
        | ExtRequest
      )
    | null;
  [key: string]: unknown;
};
export type ClientResponse =
  | {
      id: RequestId;
      result:
        | WriteTextFileResponse
        | ReadTextFileResponse
        | RequestPermissionResponse
        | CreateTerminalResponse
        | TerminalOutputResponse
        | ReleaseTerminalResponse
        | WaitForTerminalExitResponse
        | KillTerminalResponse
        | ExtResponse;
      [key: string]: unknown;
    }
  | { id: RequestId; error: Error; [key: string]: unknown };
export type ClientSessionCapabilities = {
  configOptions?: SessionConfigOptionsCapabilities | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type CloseSessionRequest = {
  sessionId: SessionId;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type CloseSessionResponse = {
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ConfigOptionUpdate = {
  configOptions: Array<SessionConfigOption>;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type Content = {
  content: ContentBlock;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ContentBlock =
  | (TextContent & { type: "text" & string; [key: string]: unknown })
  | (ImageContent & { type: "image" & string; [key: string]: unknown })
  | (AudioContent & { type: "audio" & string; [key: string]: unknown })
  | (ResourceLink & { type: "resource_link" & string; [key: string]: unknown })
  | (EmbeddedResource & { type: "resource" & string; [key: string]: unknown });
export type ContentChunk = {
  content: ContentBlock;
  messageId?: MessageId | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type Cost = {
  amount: number;
  currency: string;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type CreateTerminalRequest = {
  sessionId: SessionId;
  command: string;
  args?: Array<string>;
  env?: Array<EnvVariable>;
  cwd?: string | null;
  outputByteLimit?: number | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type CreateTerminalResponse = {
  terminalId: TerminalId;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type CurrentModeUpdate = {
  currentModeId: SessionModeId;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type DeleteSessionRequest = {
  sessionId: SessionId;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type DeleteSessionResponse = {
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type Diff = {
  path: string;
  oldText?: string | null;
  newText: string;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type EmbeddedResource = {
  annotations?: Annotations | null;
  resource: EmbeddedResourceResource;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type EmbeddedResourceResource = TextResourceContents | BlobResourceContents;
export type EnvVariable = {
  name: string;
  value: string;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type Error = { code: ErrorCode; message: string; data?: unknown; [key: string]: unknown };
export type ErrorCode =
  | (-32700 & number)
  | (-32600 & number)
  | (-32601 & number)
  | (-32602 & number)
  | (-32603 & number)
  | (-32800 & number)
  | (-32000 & number)
  | (-32002 & number)
  | number;
export type ExtNotification = unknown;
export type ExtRequest = unknown;
export type ExtResponse = unknown;
export type FileSystemCapabilities = {
  readTextFile?: boolean;
  writeTextFile?: boolean;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type HttpHeader = {
  name: string;
  value: string;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ImageContent = {
  annotations?: Annotations | null;
  data: string;
  mimeType: string;
  uri?: string | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type Implementation = {
  name: string;
  title?: string | null;
  version: string;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type InitializeRequest = {
  protocolVersion: ProtocolVersion;
  clientCapabilities?: ClientCapabilities;
  clientInfo?: Implementation | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type InitializeResponse = {
  protocolVersion: ProtocolVersion;
  agentCapabilities?: AgentCapabilities;
  authMethods?: Array<AuthMethod>;
  agentInfo?: Implementation | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type KillTerminalRequest = {
  sessionId: SessionId;
  terminalId: TerminalId;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type KillTerminalResponse = {
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ListSessionsRequest = {
  cwd?: string | null;
  cursor?: string | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ListSessionsResponse = {
  sessions: Array<SessionInfo>;
  nextCursor?: string | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type LoadSessionRequest = {
  mcpServers: Array<McpServer>;
  cwd: string;
  additionalDirectories?: Array<string>;
  sessionId: SessionId;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type LoadSessionResponse = {
  modes?: SessionModeState | null;
  configOptions?: Array<SessionConfigOption> | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type LogoutCapabilities = {
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type LogoutRequest = { _meta?: { [key: string]: unknown } | null; [key: string]: unknown };
export type LogoutResponse = { _meta?: { [key: string]: unknown } | null; [key: string]: unknown };
export type McpCapabilities = {
  http?: boolean;
  sse?: boolean;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type McpServer =
  | (McpServerHttp & { type: "http" & string; [key: string]: unknown })
  | (McpServerSse & { type: "sse" & string; [key: string]: unknown })
  | McpServerStdio;
export type McpServerHttp = {
  name: string;
  url: string;
  headers: Array<HttpHeader>;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type McpServerSse = {
  name: string;
  url: string;
  headers: Array<HttpHeader>;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type McpServerStdio = {
  name: string;
  command: string;
  args: Array<string>;
  env: Array<EnvVariable>;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type MessageId = string;
export type NewSessionRequest = {
  cwd: string;
  additionalDirectories?: Array<string>;
  mcpServers: Array<McpServer>;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type NewSessionResponse = {
  sessionId: SessionId;
  modes?: SessionModeState | null;
  configOptions?: Array<SessionConfigOption> | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type PermissionOption = {
  optionId: PermissionOptionId;
  name: string;
  kind: PermissionOptionKind;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type PermissionOptionId = string;
export type PermissionOptionKind =
  | ("allow_once" & string)
  | ("allow_always" & string)
  | ("reject_once" & string)
  | ("reject_always" & string);
export type Plan = {
  entries: Array<PlanEntry>;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type PlanEntry = {
  content: string;
  priority: PlanEntryPriority;
  status: PlanEntryStatus;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type PlanEntryPriority = ("high" & string) | ("medium" & string) | ("low" & string);
export type PlanEntryStatus =
  | ("pending" & string)
  | ("in_progress" & string)
  | ("completed" & string);
export type PromptCapabilities = {
  image?: boolean;
  audio?: boolean;
  embeddedContext?: boolean;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type PromptRequest = {
  sessionId: SessionId;
  prompt: Array<ContentBlock>;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type PromptResponse = {
  stopReason: StopReason;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ProtocolVersion = number;
export type ReadTextFileRequest = {
  sessionId: SessionId;
  path: string;
  line?: number | null;
  limit?: number | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ReadTextFileResponse = {
  content: string;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ReleaseTerminalRequest = {
  sessionId: SessionId;
  terminalId: TerminalId;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ReleaseTerminalResponse = {
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type RequestId = null | number | string;
export type RequestPermissionOutcome =
  | { outcome: "cancelled" & string; [key: string]: unknown }
  | (SelectedPermissionOutcome & { outcome: "selected" & string; [key: string]: unknown });
export type RequestPermissionRequest = {
  sessionId: SessionId;
  toolCall: ToolCallUpdate;
  options: Array<PermissionOption>;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type RequestPermissionResponse = {
  outcome: RequestPermissionOutcome;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ResourceLink = {
  annotations?: Annotations | null;
  description?: string | null;
  mimeType?: string | null;
  name: string;
  size?: number | null;
  title?: string | null;
  uri: string;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ResumeSessionRequest = {
  sessionId: SessionId;
  cwd: string;
  additionalDirectories?: Array<string>;
  mcpServers?: Array<McpServer>;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ResumeSessionResponse = {
  modes?: SessionModeState | null;
  configOptions?: Array<SessionConfigOption> | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type Role = ("assistant" & string) | ("user" & string);
export type SelectedPermissionOutcome = {
  optionId: PermissionOptionId;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SessionAdditionalDirectoriesCapabilities = {
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SessionCapabilities = {
  list?: SessionListCapabilities | null;
  delete?: SessionDeleteCapabilities | null;
  additionalDirectories?: SessionAdditionalDirectoriesCapabilities | null;
  resume?: SessionResumeCapabilities | null;
  close?: SessionCloseCapabilities | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SessionCloseCapabilities = {
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SessionConfigBoolean = { currentValue: boolean; [key: string]: unknown };
export type SessionConfigGroupId = string;
export type SessionConfigId = string;
export type SessionConfigOption = (
  | (SessionConfigSelect & { type: "select" & string; [key: string]: unknown })
  | (SessionConfigBoolean & { type: "boolean" & string; [key: string]: unknown })
) & {
  id: SessionConfigId;
  name: string;
  description?: string | null;
  category?: SessionConfigOptionCategory | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SessionConfigOptionCategory =
  | ("mode" & string)
  | ("model" & string)
  | ("model_config" & string)
  | ("thought_level" & string)
  | string;
export type SessionConfigOptionsCapabilities = {
  boolean?: BooleanConfigOptionCapabilities | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SessionConfigSelect = {
  currentValue: SessionConfigValueId;
  options: SessionConfigSelectOptions;
  [key: string]: unknown;
};
export type SessionConfigSelectGroup = {
  group: SessionConfigGroupId;
  name: string;
  options: Array<SessionConfigSelectOption>;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SessionConfigSelectOption = {
  value: SessionConfigValueId;
  name: string;
  description?: string | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SessionConfigSelectOptions =
  | Array<SessionConfigSelectOption>
  | Array<SessionConfigSelectGroup>;
export type SessionConfigValueId = string;
export type SessionDeleteCapabilities = {
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SessionId = string;
export type SessionInfo = {
  sessionId: SessionId;
  cwd: string;
  additionalDirectories?: Array<string>;
  title?: string | null;
  updatedAt?: string | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SessionInfoUpdate = {
  title?: string | null;
  updatedAt?: string | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SessionListCapabilities = {
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SessionMode = {
  id: SessionModeId;
  name: string;
  description?: string | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SessionModeId = string;
export type SessionModeState = {
  currentModeId: SessionModeId;
  availableModes: Array<SessionMode>;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SessionNotification = {
  sessionId: SessionId;
  update: SessionUpdate;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SessionResumeCapabilities = {
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SessionUpdate =
  | (ContentChunk & { sessionUpdate: "user_message_chunk" & string; [key: string]: unknown })
  | (ContentChunk & { sessionUpdate: "agent_message_chunk" & string; [key: string]: unknown })
  | (ContentChunk & { sessionUpdate: "agent_thought_chunk" & string; [key: string]: unknown })
  | (ToolCall & { sessionUpdate: "tool_call" & string; [key: string]: unknown })
  | (ToolCallUpdate & { sessionUpdate: "tool_call_update" & string; [key: string]: unknown })
  | (Plan & { sessionUpdate: "plan" & string; [key: string]: unknown })
  | (AvailableCommandsUpdate & {
      sessionUpdate: "available_commands_update" & string;
      [key: string]: unknown;
    })
  | (CurrentModeUpdate & { sessionUpdate: "current_mode_update" & string; [key: string]: unknown })
  | (ConfigOptionUpdate & {
      sessionUpdate: "config_option_update" & string;
      [key: string]: unknown;
    })
  | (SessionInfoUpdate & { sessionUpdate: "session_info_update" & string; [key: string]: unknown })
  | (UsageUpdate & { sessionUpdate: "usage_update" & string; [key: string]: unknown });
export type SetSessionConfigOptionRequest = (
  | { value: boolean; type: "boolean" & string; [key: string]: unknown }
  | { value: SessionConfigValueId; [key: string]: unknown }
) & {
  sessionId: SessionId;
  configId: SessionConfigId;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SetSessionConfigOptionResponse = {
  configOptions: Array<SessionConfigOption>;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SetSessionModeRequest = {
  sessionId: SessionId;
  modeId: SessionModeId;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type SetSessionModeResponse = {
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type StopReason =
  | ("end_turn" & string)
  | ("max_tokens" & string)
  | ("max_turn_requests" & string)
  | ("refusal" & string)
  | ("cancelled" & string);
export type Terminal = {
  terminalId: TerminalId;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type TerminalExitStatus = {
  exitCode?: number | null;
  signal?: string | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type TerminalId = string;
export type TerminalOutputRequest = {
  sessionId: SessionId;
  terminalId: TerminalId;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type TerminalOutputResponse = {
  output: string;
  truncated: boolean;
  exitStatus?: TerminalExitStatus | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type TextContent = {
  annotations?: Annotations | null;
  text: string;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type TextResourceContents = {
  mimeType?: string | null;
  text: string;
  uri: string;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ToolCall = {
  toolCallId: ToolCallId;
  title: string;
  kind?: ToolKind;
  status?: ToolCallStatus;
  content?: Array<ToolCallContent>;
  locations?: Array<ToolCallLocation>;
  rawInput?: unknown;
  rawOutput?: unknown;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ToolCallContent =
  | (Content & { type: "content" & string; [key: string]: unknown })
  | (Diff & { type: "diff" & string; [key: string]: unknown })
  | (Terminal & { type: "terminal" & string; [key: string]: unknown });
export type ToolCallId = string;
export type ToolCallLocation = {
  path: string;
  line?: number | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ToolCallStatus =
  | ("pending" & string)
  | ("in_progress" & string)
  | ("completed" & string)
  | ("failed" & string);
export type ToolCallUpdate = {
  toolCallId: ToolCallId;
  kind?: ToolKind | null;
  status?: ToolCallStatus | null;
  title?: string | null;
  content?: Array<ToolCallContent> | null;
  locations?: Array<ToolCallLocation> | null;
  rawInput?: unknown;
  rawOutput?: unknown;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type ToolKind =
  | ("read" & string)
  | ("edit" & string)
  | ("delete" & string)
  | ("move" & string)
  | ("search" & string)
  | ("execute" & string)
  | ("think" & string)
  | ("fetch" & string)
  | ("switch_mode" & string)
  | ("other" & string);
export type UnstructuredCommandInput = {
  hint: string;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type UsageUpdate = {
  used: number;
  size: number;
  cost?: Cost | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type WaitForTerminalExitRequest = {
  sessionId: SessionId;
  terminalId: TerminalId;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type WaitForTerminalExitResponse = {
  exitCode?: number | null;
  signal?: string | null;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type WriteTextFileRequest = {
  sessionId: SessionId;
  path: string;
  content: string;
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
export type WriteTextFileResponse = {
  _meta?: { [key: string]: unknown } | null;
  [key: string]: unknown;
};
