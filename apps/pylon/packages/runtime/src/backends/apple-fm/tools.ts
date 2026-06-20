import { randomUUID } from "node:crypto";
import { Effect, Schema as S } from "effect";
import { APPLE_FM_BACKEND_KIND } from "./contract.js";
import { AppleFmBackendFailureReceipt, makeAppleFmFailureReceipt } from "./receipts.js";

export const AppleFmToolName = S.Literals([
  "read_file",
  "list_files",
  "code_search",
  "shell",
  "apply_patch",
  "consult_oracle",
  "analyze_repository",
  "propose_action_submission",
]);
export type AppleFmToolName = typeof AppleFmToolName.Type;

export const AppleFmToolPolicy = S.Literals(["allow", "approval_required", "deny"]);
export type AppleFmToolPolicy = typeof AppleFmToolPolicy.Type;

export const AppleFmToolCallbackStatus = S.Literals([
  "success",
  "approval_pending",
  "refused",
  "unauthorized",
  "unknown_tool",
  "round_trip_limit",
  "tool_failed",
]);
export type AppleFmToolCallbackStatus = typeof AppleFmToolCallbackStatus.Type;

export const AppleFmProjectedTool = S.Struct({
  name: AppleFmToolName,
  description: S.String,
  inputSchema: S.Record(S.String, S.Unknown),
});
export type AppleFmProjectedTool = typeof AppleFmProjectedTool.Type;

export const AppleFmToolTranscriptEntry = S.Struct({
  kind: S.Literal("probe_tool_callback"),
  backendKind: S.Literal(APPLE_FM_BACKEND_KIND),
  sessionId: S.String,
  toolCallId: S.String,
  toolName: AppleFmToolName,
  status: AppleFmToolCallbackStatus,
  input: S.Record(S.String, S.Unknown),
  output: S.optional(S.Unknown),
  message: S.optional(S.String),
  observedAt: S.String,
  callbackTokenRedacted: S.Literal(true),
  contentRedacted: S.Literal(true),
});
export type AppleFmToolTranscriptEntry = typeof AppleFmToolTranscriptEntry.Type;

export const AppleFmToolCallbackReceipt = S.Struct({
  kind: S.Literal("probe_backend_tool_callback"),
  backendKind: S.Literal(APPLE_FM_BACKEND_KIND),
  sessionId: S.String,
  toolCallId: S.String,
  toolName: AppleFmToolName,
  status: AppleFmToolCallbackStatus,
  callbackUrl: S.Literal("[redacted]"),
  callbackTokenRedacted: S.Literal(true),
  observedAt: S.String,
  contentRedacted: S.Literal(true),
});
export type AppleFmToolCallbackReceipt = typeof AppleFmToolCallbackReceipt.Type;

export interface AppleFmToolDefinition {
  readonly name: AppleFmToolName;
  readonly description: string;
  readonly inputSchema: Readonly<Record<string, unknown>>;
  readonly policy: AppleFmToolPolicy;
  readonly execute: (input: Readonly<Record<string, unknown>>) => Effect.Effect<unknown, never>;
}

export interface AppleFmToolCallbackRequest {
  readonly token: string;
  readonly toolCallId: string;
  readonly toolName: AppleFmToolName | string;
  readonly input: Readonly<Record<string, unknown>>;
}

export interface AppleFmToolCallbackResponse {
  readonly status: AppleFmToolCallbackStatus;
  readonly output?: unknown;
  readonly message?: string;
  readonly receipt: AppleFmToolCallbackReceipt;
  readonly transcriptEntry: AppleFmToolTranscriptEntry;
}

export interface AppleFmToolCallbackSession {
  readonly sessionId: string;
  readonly token: string;
  readonly callbackUrl: string;
  readonly redactedCallbackUrl: string;
  readonly projectedTools: ReadonlyArray<AppleFmProjectedTool>;
  readonly transcript: ReadonlyArray<AppleFmToolTranscriptEntry>;
  readonly receipts: ReadonlyArray<AppleFmToolCallbackReceipt>;
  readonly handleCallback: (
    request: AppleFmToolCallbackRequest,
  ) => Effect.Effect<AppleFmToolCallbackResponse, AppleFmToolCallbackError>;
  readonly publicDescriptor: () => AppleFmToolCallbackPublicDescriptor;
  readonly resumeFromTranscript: (
    transcript: ReadonlyArray<AppleFmToolTranscriptEntry>,
  ) => AppleFmToolCallbackSession;
}

export interface AppleFmToolCallbackPublicDescriptor {
  readonly sessionId: string;
  readonly callbackUrl: "[redacted]";
  readonly callbackTokenRedacted: true;
  readonly projectedTools: ReadonlyArray<AppleFmProjectedTool>;
  readonly transcript: ReadonlyArray<AppleFmToolTranscriptEntry>;
}

export class AppleFmToolCallbackError extends S.TaggedErrorClass<AppleFmToolCallbackError>()(
  "AppleFmToolCallbackError",
  {
    reason: S.String,
    receipt: S.optional(AppleFmBackendFailureReceipt),
  },
) {}

export interface MakeAppleFmToolCallbackSessionInput {
  readonly sessionId?: string;
  readonly token?: string;
  readonly callbackUrl?: string;
  readonly tools: ReadonlyArray<AppleFmToolDefinition>;
  readonly maxModelRoundTrips?: number;
  readonly now?: Date;
  readonly transcript?: ReadonlyArray<AppleFmToolTranscriptEntry>;
}

export function makeAppleFmToolCallbackSession(
  input: MakeAppleFmToolCallbackSessionInput,
): AppleFmToolCallbackSession {
  const sessionId = input.sessionId ?? `apple_fm_session_${randomUUID()}`;
  const token = input.token ?? randomUUID();
  const callbackUrl = input.callbackUrl ?? `http://127.0.0.1:0/apple-fm/tool-callback/${sessionId}`;
  const redactedCallbackUrl = redactCallbackUrl(callbackUrl);
  const projectedTools = input.tools.map(projectToolDefinition);
  const toolMap = new Map(input.tools.map((tool) => [tool.name, tool]));
  const maxModelRoundTrips = input.maxModelRoundTrips ?? Infinity;
  const transcript: AppleFmToolTranscriptEntry[] = [...(input.transcript ?? [])];
  const receipts: AppleFmToolCallbackReceipt[] = [];
  const now = () => (input.now ?? new Date()).toISOString();

  const session: AppleFmToolCallbackSession = {
    sessionId,
    token,
    callbackUrl,
    redactedCallbackUrl,
    projectedTools,
    transcript,
    receipts,
    handleCallback: (request) =>
      Effect.gen(function* () {
        const observedAt = now();

        if (request.token !== token) {
          return yield* Effect.fail(
            new AppleFmToolCallbackError({
              reason: "Apple FM tool callback token mismatch",
              receipt: makeAppleFmFailureReceipt({
                profileId: "apple-fm-local",
                model: "apple-foundation-model",
                baseUrl: callbackUrl,
                failureClass: "tool_callback_unauthorized",
                message: "Apple FM tool callback token mismatch",
                observedAt,
              }),
            }),
          );
        }

        if (transcript.length >= maxModelRoundTrips) {
          return recordToolCallback({
            sessionId,
            request,
            status: "round_trip_limit",
            message: "Apple FM tool callback round-trip limit reached",
            observedAt,
            transcript,
            receipts,
          });
        }

        const tool = toolMap.get(request.toolName as AppleFmToolName);

        if (tool === undefined) {
          return recordToolCallback({
            sessionId,
            request,
            status: "unknown_tool",
            message: `Unknown Apple FM tool callback: ${request.toolName}`,
            observedAt,
            transcript,
            receipts,
          });
        }

        if (tool.policy === "deny") {
          return recordToolCallback({
            sessionId,
            request,
            status: "refused",
            message: "Probe policy refused this Apple FM tool callback",
            observedAt,
            transcript,
            receipts,
          });
        }

        if (tool.policy === "approval_required") {
          return recordToolCallback({
            sessionId,
            request,
            status: "approval_pending",
            message: "Probe approval is required before this tool can run",
            observedAt,
            transcript,
            receipts,
          });
        }

        const output = yield* tool.execute(request.input);

        return recordToolCallback({
          sessionId,
          request,
          status: "success",
          output,
          observedAt,
          transcript,
          receipts,
        });
      }),
    publicDescriptor: () => ({
      sessionId,
      callbackUrl: "[redacted]",
      callbackTokenRedacted: true,
      projectedTools,
      transcript,
    }),
    resumeFromTranscript: (restoredTranscript) =>
      makeAppleFmToolCallbackSession({
        ...input,
        sessionId,
        token,
        callbackUrl,
        transcript: restoredTranscript,
      }),
  };

  return session;
}

export interface AppleFmToolCallbackServer {
  readonly callbackUrl: string;
  readonly redactedCallbackUrl: string;
  readonly stop: () => void;
}

export function startAppleFmToolCallbackServer(
  session: AppleFmToolCallbackSession,
  options: { readonly port?: number } = {},
): AppleFmToolCallbackServer {
  const server = Bun.serve({
    port: options.port ?? 0,
    fetch: async (request) => {
      const authorization = request.headers.get("Authorization") ?? "";
      const body = await request.json() as {
        readonly session_token?: string;
        readonly tool_name?: string;
        readonly arguments?: {
          readonly generation_id?: string;
          readonly content?: unknown;
          readonly is_complete?: boolean;
        };
        readonly tool_call_id?: string;
        readonly toolCallId?: string;
        readonly toolName?: string;
        readonly input?: Readonly<Record<string, unknown>>;
      };
      const swiftBridgePayload = body.session_token !== undefined || body.tool_name !== undefined || body.arguments !== undefined;
      const token = authorization.startsWith("Bearer ") ? authorization.slice("Bearer ".length) : body.session_token ?? "";
      const toolCallId = body.toolCallId ?? body.tool_call_id ?? body.arguments?.generation_id ?? `tool_call_${randomUUID()}`;
      const toolName = body.toolName ?? body.tool_name ?? "read_file";
      const input = body.input ?? generatedContentToInput(body.arguments?.content);
      const callbackResponse = await Effect.runPromise(
        session.handleCallback({
          token,
          toolCallId,
          toolName,
          input,
        }).pipe(
          Effect.catch((error: AppleFmToolCallbackError) =>
            Effect.succeed({
              status: "unauthorized" as const,
              message: error.reason,
              receipt: {
                kind: "probe_backend_tool_callback" as const,
                backendKind: APPLE_FM_BACKEND_KIND,
                sessionId: session.sessionId,
                toolCallId,
                toolName: "read_file" as const,
                status: "unauthorized" as const,
                callbackUrl: "[redacted]" as const,
                callbackTokenRedacted: true as const,
                observedAt: new Date().toISOString(),
                contentRedacted: true as const,
              },
              transcriptEntry: {
                kind: "probe_tool_callback" as const,
                backendKind: APPLE_FM_BACKEND_KIND,
                sessionId: session.sessionId,
                toolCallId,
                toolName: "read_file" as const,
                status: "unauthorized" as const,
                input: {},
                message: error.reason,
                observedAt: new Date().toISOString(),
                callbackTokenRedacted: true as const,
                contentRedacted: true as const,
              },
            }),
          ),
        ),
      );

      if (swiftBridgePayload) {
        if (callbackResponse.status === "success") {
          return Response.json({
            output: stringifyToolOutput(callbackResponse.output),
          });
        }

        return Response.json(
          {
            tool_name: toolName,
            underlying_error: callbackResponse.message ?? callbackResponse.status,
          },
          { status: callbackResponse.status === "unauthorized" ? 401 : 409 },
        );
      }

      return Response.json(callbackResponse, { status: callbackResponse.status === "unauthorized" ? 401 : 200 });
    },
  });
  const callbackUrl = `http://127.0.0.1:${server.port}/apple-fm/tool-callback`;

  return {
    callbackUrl,
    redactedCallbackUrl: redactCallbackUrl(callbackUrl),
    stop: () => server.stop(true),
  };
}

export function projectToolDefinition(tool: AppleFmToolDefinition): AppleFmProjectedTool {
  return {
    name: tool.name,
    description: tool.description,
    inputSchema: normalizeRootToolSchema(tool.inputSchema),
  };
}

export function normalizeRootToolSchema(schema: Readonly<Record<string, unknown>>): Record<string, unknown> {
  return {
    type: "object",
    properties: {},
    additionalProperties: false,
    ...schema,
  };
}

function recordToolCallback(input: {
  readonly sessionId: string;
  readonly request: AppleFmToolCallbackRequest;
  readonly status: AppleFmToolCallbackStatus;
  readonly output?: unknown;
  readonly message?: string;
  readonly observedAt: string;
  readonly transcript: AppleFmToolTranscriptEntry[];
  readonly receipts: AppleFmToolCallbackReceipt[];
}): AppleFmToolCallbackResponse {
  const toolName = normalizeToolName(input.request.toolName);
  const receipt: AppleFmToolCallbackReceipt = {
    kind: "probe_backend_tool_callback",
    backendKind: APPLE_FM_BACKEND_KIND,
    sessionId: input.sessionId,
    toolCallId: input.request.toolCallId,
    toolName,
    status: input.status,
    callbackUrl: "[redacted]",
    callbackTokenRedacted: true,
    observedAt: input.observedAt,
    contentRedacted: true,
  };
  const transcriptEntry: AppleFmToolTranscriptEntry = {
    kind: "probe_tool_callback",
    backendKind: APPLE_FM_BACKEND_KIND,
    sessionId: input.sessionId,
    toolCallId: input.request.toolCallId,
    toolName,
    status: input.status,
    input: input.request.input,
    output: input.output,
    message: input.message,
    observedAt: input.observedAt,
    callbackTokenRedacted: true,
    contentRedacted: true,
  };

  input.receipts.push(receipt);
  input.transcript.push(transcriptEntry);

  return {
    status: input.status,
    output: input.output,
    message: input.message,
    receipt,
    transcriptEntry,
  };
}

function normalizeToolName(value: string): AppleFmToolName {
  return value === "read_file" ||
    value === "list_files" ||
    value === "code_search" ||
    value === "shell" ||
    value === "apply_patch" ||
    value === "consult_oracle" ||
    value === "analyze_repository" ||
    value === "propose_action_submission"
    ? value
    : "read_file";
}

function redactCallbackUrl(value: string): string {
  try {
    const url = new URL(value);
    return `${url.origin}${url.pathname.replace(/\/[^/]+$/, "/[redacted]")}`;
  } catch {
    return "[redacted]";
  }
}

function generatedContentToInput(value: unknown): Readonly<Record<string, unknown>> {
  if (typeof value === "object" && value !== null && !Array.isArray(value)) {
    return value as Readonly<Record<string, unknown>>;
  }

  return {
    value,
  };
}

function stringifyToolOutput(value: unknown): string {
  return typeof value === "string" ? value : JSON.stringify(value);
}
