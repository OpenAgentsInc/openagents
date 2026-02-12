import * as AiLanguageModel from "@effect/ai/LanguageModel";
import * as AiPrompt from "@effect/ai/Prompt";
import * as AiResponse from "@effect/ai/Response";
import * as AiToolkit from "@effect/ai/Toolkit";
import { Cause, Effect, Layer, Schema, Stream } from "effect";

import { makeWorkersAiLanguageModel } from "../../../autopilot-worker/src/effect/ai/languageModel";
import { makeOpenRouterLanguageModel } from "../../../autopilot-worker/src/effect/ai/openRouterLanguageModel";
import { makeFallbackLanguageModel } from "../../../autopilot-worker/src/effect/ai/fallbackLanguageModel";

import { BlobStore, Lm, Params, Predict, Receipt, type BlobRef } from "@openagentsinc/dse";
import { signatures as dseCatalogSignatures } from "../../../autopilot-worker/src/dseCatalog";
import { toolContracts } from "../../../autopilot-worker/src/tools";

import { api } from "../../convex/_generated/api";
import { E2E_COOKIE_NAME, mintE2eJwt } from "../auth/e2eAuth";
import { AuthService } from "../effect/auth";
import { ConvexService, type ConvexServiceApi } from "../effect/convex";
import { RequestContextService, makeServerRequestContext } from "../effect/requestContext";
import { TelemetryService } from "../effect/telemetry";

import { layerDsePredictEnvForAutopilotRun, makeDseLmClientWithOpenRouterPrimary } from "./dse";
import { getWorkerRuntime } from "./runtime";
import { OA_REQUEST_ID_HEADER, formatRequestIdLogToken } from "./requestId";
import { isEp212EndpointPreset, resolveEp212PresetUrl, sanitizeLightningHeadersForTask } from "./ep212Endpoints";
import { DEFAULT_EXECUTOR_PRESENCE_MAX_AGE_MS, isExecutorPresenceFresh } from "./lightningPresence";
import type { WorkerEnv } from "./env";
import type {
  CreateRunResult,
  EnsureOwnedThreadResult,
  GetBlueprintResult,
  GetThreadTraceBundleResult,
  GetThreadSnapshotResult,
  IsCancelRequestedResult,
  ThreadSnapshotMessage,
} from "./convexTypes";

/** Cloudflare Workers AI model (fallback when OpenRouter is used or only option when OPENROUTER_API_KEY is unset). */
const MODEL_ID_CF = "@cf/openai/gpt-oss-120b";
/** OpenRouter model used as primary when OPENROUTER_API_KEY is set. */
const PRIMARY_MODEL_OPENROUTER = "moonshotai/kimi-k2.5";
const MAX_CONTEXT_MESSAGES = 25;
const MAX_OUTPUT_TOKENS = 512;
export const AUTOPILOT_ADMIN_TEST_SUBJECT = "user_autopilot_admin_test";

const predictRlmSummarizeThread = Predict.make(dseCatalogSignatures.rlm_summarize_thread);
const predictDetectUpgradeRequest = Predict.make(dseCatalogSignatures.detect_upgrade_request);

type DseBudgetProfile = "small" | "medium" | "long";
type DseStrategyId = "direct.v1" | "rlm_lite.v1";

type UpgradeRequestDecision = {
  readonly isUpgradeRequest: boolean;
  readonly capabilityKey: string;
  readonly capabilityLabel: string;
  readonly summary: string;
  readonly notifyWhenAvailable: boolean;
  readonly confidence: number;
};

const budgetsForProfile = (profile: DseBudgetProfile): Params.DseExecutionBudgetsV1 => {
  switch (profile) {
    case "small":
      return {
        maxTimeMs: 10_000,
        maxLmCalls: 20,
        maxToolCalls: 0,
        maxOutputChars: 40_000,
        maxRlmIterations: 6,
        maxSubLmCalls: 10,
      };
    case "long":
      return {
        maxTimeMs: 40_000,
        maxLmCalls: 80,
        maxToolCalls: 0,
        maxOutputChars: 140_000,
        maxRlmIterations: 18,
        maxSubLmCalls: 40,
      };
    case "medium":
    default:
      return {
        maxTimeMs: 20_000,
        maxLmCalls: 40,
        maxToolCalls: 0,
        maxOutputChars: 80_000,
        maxRlmIterations: 10,
        maxSubLmCalls: 20,
      };
  }
};

const makeCanaryRecapSignature = (input: {
  readonly strategyId: DseStrategyId;
  readonly budgets: Params.DseExecutionBudgetsV1;
}) => {
  const base = dseCatalogSignatures.canary_recap_thread;
  return {
    ...base,
    defaults: {
      ...base.defaults,
      params: {
        ...base.defaults.params,
        strategy: { id: input.strategyId },
        budgets: input.budgets,
      } satisfies Params.DseParamsV1,
    },
  };
};

const json = (body: unknown, init?: ResponseInit): Response =>
  new Response(JSON.stringify(body), {
    ...init,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...(init?.headers ?? {}),
    },
  });

const readJson = async <T>(request: Request): Promise<T | null> => {
  try {
    return (await request.json()) as T;
  } catch {
    return null;
  }
};

type SendBody = {
  readonly threadId?: unknown;
  readonly text?: unknown;
};

type CancelBody = {
  readonly threadId?: unknown;
  readonly runId?: unknown;
};

type DseRecapBody = {
  readonly threadId?: unknown;
  readonly strategyId?: unknown;
  readonly budgetProfile?: unknown;
  readonly question?: unknown;
};

type AdminSendBody = {
  readonly threadId?: unknown;
  readonly text?: unknown;
  readonly resetThread?: unknown;
};

type AdminResetBody = {
  readonly threadId?: unknown;
};

type RunHandle = {
  readonly controller: AbortController;
  readonly startedAtMs: number;
};

// Best-effort per-isolate cancel map. We also persist cancelRequested in Convex so
// the streaming loop can observe cancellation even if the cancel request hits a
// different isolate.
const activeRuns = new Map<string, RunHandle>();

const emptyToolkit = AiToolkit.make();
const encodeStreamPart = Schema.encodeSync(AiResponse.StreamPart(emptyToolkit));

const shouldIgnoreWirePart = (part: AiResponse.StreamPartEncoded): boolean =>
  part.type === "reasoning-start" || part.type === "reasoning-delta" || part.type === "reasoning-end";

type DseSignatureModelPart = {
  readonly modelId?: string;
  readonly provider?: string;
  readonly route?: string;
  readonly fallbackModelId?: string;
};

const toRecord = (value: unknown): Record<string, unknown> | null =>
  value && typeof value === "object" ? (value as Record<string, unknown>) : null;

const readNonEmptyString = (value: unknown): string | null =>
  typeof value === "string" && value.trim().length > 0 ? value : null;

const readString = (value: unknown): string | null =>
  typeof value === "string" ? value : null;

const parseCreateRunResult = (value: unknown): CreateRunResult | null => {
  const rec = toRecord(value);
  const runId = readNonEmptyString(rec?.runId);
  const userMessageId = readNonEmptyString(rec?.userMessageId);
  const assistantMessageId = readNonEmptyString(rec?.assistantMessageId);
  if (!runId || !userMessageId || !assistantMessageId) return null;
  return { ok: true, runId, userMessageId, assistantMessageId };
};

const parseEnsureOwnedThreadResult = (value: unknown): EnsureOwnedThreadResult | null => {
  const rec = toRecord(value);
  const threadId = readNonEmptyString(rec?.threadId);
  if (!threadId) return null;
  return { ok: true, threadId };
};

const clampString = (value: string, max: number): string =>
  value.trim().slice(0, Math.max(0, max));

const LIGHTNING_L402_FETCH_TOOL_NAME = "lightning_l402_fetch" as const;
const LIGHTNING_L402_APPROVE_TOOL_NAME = "lightning_l402_approve" as const;
const LIGHTNING_TERMINAL_STATUSES = ["completed", "cached", "blocked", "failed"] as const;
type LightningTerminalStatus = (typeof LIGHTNING_TERMINAL_STATUSES)[number];
type LightningL402FetchToolStatus = "queued" | LightningTerminalStatus;

type L402CacheStatus = "miss" | "hit" | "stale" | "invalid";
type L402PaymentBackend = "spark" | "lnd_deterministic";

type LightningTaskStatus =
  | "queued"
  | "approved"
  | "running"
  | "paid"
  | "cached"
  | "blocked"
  | "failed"
  | "completed";

type LightningTaskDoc = {
  readonly taskId: string;
  readonly status: LightningTaskStatus;
  readonly request?: {
    readonly maxSpendMsats?: number;
  };
  readonly lastErrorCode?: string | undefined;
  readonly lastErrorMessage?: string | undefined;
};

type LightningTaskEventDoc = {
  readonly toStatus?: string | undefined;
  readonly reason?: string | undefined;
  readonly errorCode?: string | undefined;
  readonly errorMessage?: string | undefined;
  readonly metadata?: unknown;
};

type LightningToolTerminalResult = {
  readonly taskId: string | null;
  readonly status: LightningL402FetchToolStatus;
  readonly proofReference: string | null;
  readonly denyReason: string | null;
  readonly denyReasonCode: string | null;
  readonly host: string | null;
  readonly maxSpendMsats: number | null;
  readonly quotedAmountMsats: number | null;
  readonly paymentId: string | null;
  readonly amountMsats: number | null;
  readonly responseStatusCode: number | null;
  readonly responseContentType: string | null;
  readonly responseBytes: number | null;
  readonly responseBodyTextPreview: string | null;
  readonly responseBodySha256: string | null;
  readonly cacheHit: boolean;
  readonly paid: boolean;
  readonly cacheStatus: L402CacheStatus | null;
  readonly paymentBackend: L402PaymentBackend | null;
  readonly approvalRequired: boolean;
};

const decodeLightningL402FetchInput = Schema.decodeUnknown(toolContracts.lightning_l402_fetch.input);
const decodeLightningL402ApproveInput = Schema.decodeUnknown(toolContracts.lightning_l402_approve.input);

const isLightningTerminalStatus = (value: unknown): value is LightningTerminalStatus =>
  typeof value === "string" && (LIGHTNING_TERMINAL_STATUSES as ReadonlyArray<string>).includes(value);

const isLightningTaskStatus = (value: unknown): value is LightningTaskStatus =>
  typeof value === "string" &&
  ["queued", "approved", "running", "paid", "cached", "blocked", "failed", "completed"].includes(value);

const isL402CacheStatus = (value: unknown): value is L402CacheStatus =>
  typeof value === "string" && (["miss", "hit", "stale", "invalid"] as ReadonlyArray<string>).includes(value);

const isL402PaymentBackend = (value: unknown): value is L402PaymentBackend =>
  typeof value === "string" && (["spark", "lnd_deterministic"] as ReadonlyArray<string>).includes(value);

const parseLightningTaskDoc = (value: unknown): LightningTaskDoc | null => {
  const rec = toRecord(value);
  const taskId = readNonEmptyString(rec?.taskId);
  const statusRaw = readString(rec?.status);
  if (!taskId || !statusRaw || !isLightningTaskStatus(statusRaw)) return null;
  return {
    taskId,
    status: statusRaw,
    request: toRecord(rec?.request) as any,
    lastErrorCode: readString(rec?.lastErrorCode) ?? undefined,
    lastErrorMessage: readString(rec?.lastErrorMessage) ?? undefined,
  };
};

const parseLightningTaskEventDoc = (value: unknown): LightningTaskEventDoc | null => {
  const rec = toRecord(value);
  if (!rec) return null;
  return {
    toStatus: readString(rec.toStatus) ?? undefined,
    reason: readString(rec.reason) ?? undefined,
    errorCode: readString(rec.errorCode) ?? undefined,
    errorMessage: readString(rec.errorMessage) ?? undefined,
    metadata: rec.metadata,
  };
};

type LightningManualInvocation =
  | { readonly toolName: typeof LIGHTNING_L402_FETCH_TOOL_NAME; readonly rawParams: unknown; readonly source: "call" | "slash" }
  | { readonly toolName: typeof LIGHTNING_L402_APPROVE_TOOL_NAME; readonly rawParams: unknown; readonly source: "call" | "slash" };

const parseLightningToolInvocation = (text: string): LightningManualInvocation | null => {
  const trimmed = text.trim();
  if (!trimmed) return null;

  const callMatch = trimmed.match(/^lightning_l402_fetch\s*\(([\s\S]+)\)\s*$/i);
  if (callMatch) {
    const raw = callMatch[1]?.trim() ?? "";
    if (!raw) return { toolName: LIGHTNING_L402_FETCH_TOOL_NAME, rawParams: {}, source: "call" };
    try {
      return { toolName: LIGHTNING_L402_FETCH_TOOL_NAME, rawParams: JSON.parse(raw), source: "call" };
    } catch {
      return { toolName: LIGHTNING_L402_FETCH_TOOL_NAME, rawParams: { __invalidJson: raw }, source: "call" };
    }
  }

  const approveCallMatch = trimmed.match(/^lightning_l402_approve\s*\(([\s\S]+)\)\s*$/i);
  if (approveCallMatch) {
    const raw = approveCallMatch[1]?.trim() ?? "";
    if (!raw) return { toolName: LIGHTNING_L402_APPROVE_TOOL_NAME, rawParams: {}, source: "call" };
    try {
      return { toolName: LIGHTNING_L402_APPROVE_TOOL_NAME, rawParams: JSON.parse(raw), source: "call" };
    } catch {
      return { toolName: LIGHTNING_L402_APPROVE_TOOL_NAME, rawParams: { __invalidJson: raw }, source: "call" };
    }
  }

  const slashMatch = trimmed.match(/^\/l402\s+fetch\s+(\S+)(?:\s+max=(\d+))?(?:\s+scope=([^\s]+))?\s*$/i);
  if (slashMatch) {
    const url = slashMatch[1];
    const maxSpendMsats = slashMatch[2] ? Number(slashMatch[2]) : 50_000;
    const scope = slashMatch[3];
    return {
      toolName: LIGHTNING_L402_FETCH_TOOL_NAME,
      rawParams: {
        url,
        method: "GET",
        maxSpendMsats,
        ...(scope ? { scope } : {}),
      },
      source: "slash",
    };
  }

  const approveSlashMatch = trimmed.match(/^\/l402\s+approve\s+(\S+)\s*$/i);
  if (approveSlashMatch) {
    const taskId = approveSlashMatch[1];
    return {
      toolName: LIGHTNING_L402_APPROVE_TOOL_NAME,
      rawParams: { taskId },
      source: "slash",
    };
  }

  return null;
};

const terminalTextFromLightningToolResult = (result: LightningToolTerminalResult): string => {
  if (result.status === "queued") {
    const tid = result.taskId ? ` Task: ${result.taskId}.` : "";
    const approveHint = result.taskId
      ? `lightning_l402_approve({"taskId":"${result.taskId}"})`
      : `lightning_l402_approve({"taskId":"..."})`;
    return `L402 fetch queued.${tid} Approval required: approve with ${approveHint}.`;
  }

  if (result.status === "completed" || result.status === "cached") {
    const proof = result.proofReference ? ` Proof: ${result.proofReference}` : "";
    const code = typeof result.responseStatusCode === "number" ? ` HTTP ${result.responseStatusCode}.` : "";
    return `L402 fetch ${result.status}.${code}${proof}`.trim();
  }

  const fmtSats = (msats: number): string => {
    const sats = Math.round((msats / 1000) * 1000) / 1000;
    const text = Number.isInteger(sats)
      ? String(sats)
      : String(sats).includes(".")
        ? String(sats)
        : sats.toFixed(3).replace(/0+$/, "").replace(/\.$/, "");
    return `${text} sats`;
  };

  if (result.status === "blocked") {
    if (
      result.denyReasonCode === "amount_over_cap" &&
      typeof result.quotedAmountMsats === "number" &&
      typeof result.maxSpendMsats === "number"
    ) {
      return `Blocked: quoted ${fmtSats(result.quotedAmountMsats)} > cap ${fmtSats(result.maxSpendMsats)}`;
    }
    if (result.denyReasonCode === "host_not_allowlisted") {
      return result.host ? `Blocked: host not allowlisted (${result.host})` : "Blocked: host not allowlisted";
    }
    if (result.denyReasonCode === "host_blocked") {
      return result.host ? `Blocked: host blocked (${result.host})` : "Blocked: host blocked";
    }
  }

  const reason = result.denyReason ?? "unknown";
  return `L402 fetch ${result.status}. Reason: ${reason}`;
};

const normalizeCapabilityKey = (value: string): string => {
  const normalized = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
  return normalized.length > 0 ? normalized.slice(0, 120) : "unknown";
};

const normalizeUpgradeRequestDecision = (value: unknown): UpgradeRequestDecision => {
  const rec = toRecord(value);
  const isUpgradeRequest = rec?.isUpgradeRequest === true;
  const capabilityKeyRaw = readString(rec?.capabilityKey) ?? (isUpgradeRequest ? "unknown" : "none");
  const capabilityLabelRaw = readString(rec?.capabilityLabel) ?? (isUpgradeRequest ? capabilityKeyRaw : "none");
  const summaryRaw = readString(rec?.summary) ?? "";
  const confidenceRaw = typeof rec?.confidence === "number" && Number.isFinite(rec.confidence) ? rec.confidence : 0;

  return {
    isUpgradeRequest,
    capabilityKey: normalizeCapabilityKey(capabilityKeyRaw),
    capabilityLabel: clampString(capabilityLabelRaw, 160),
    summary: clampString(summaryRaw, 240),
    notifyWhenAvailable: rec?.notifyWhenAvailable === true,
    confidence: Math.max(0, Math.min(1, confidenceRaw)),
  };
};

const fallbackCapabilityFromMessage = (message: string): {
  readonly capabilityKey: string;
  readonly capabilityLabel: string;
  readonly summary: string;
  readonly confidence: number;
} => {
  const t = message.trim().toLowerCase();
  const hasGithub = /github|gitlab|bitbucket|repository|repo\b/.test(t);
  const hasCloudRun =
    /(remote|cloud|hosted)/.test(t) && /(run code|execute code|codex|agent run|sandbox|compute)/.test(t);
  const hasDeploy = /deploy|deployment|ci\/cd|pipeline/.test(t);
  const hasLiveExternal = /(browse|web|internet|external api|live access|real[- ]?time access)/.test(t);

  if (hasGithub && hasDeploy) {
    return {
      capabilityKey: "github_integration_and_auto_deploy",
      capabilityLabel: "GitHub integration and auto deploy",
      summary: "User requests GitHub integration with automated deployment support.",
      confidence: 0.72,
    };
  }

  if (hasGithub) {
    return {
      capabilityKey: "github_repo_access",
      capabilityLabel: "GitHub repository access",
      summary: "User asks for direct access to repository hosting platforms.",
      confidence: 0.7,
    };
  }

  if (hasCloudRun) {
    return {
      capabilityKey: "remote_cloud_execution",
      capabilityLabel: "Remote cloud execution",
      summary: "User asks for remote cloud execution against their codebase.",
      confidence: 0.68,
    };
  }

  if (hasLiveExternal) {
    return {
      capabilityKey: "live_external_system_access",
      capabilityLabel: "Live external system access",
      summary: "User asks for live browsing or external system access not currently available.",
      confidence: 0.64,
    };
  }

  if (hasDeploy) {
    return {
      capabilityKey: "automated_deployment_execution",
      capabilityLabel: "Automated deployment execution",
      summary: "User requests deployment automation capabilities.",
      confidence: 0.63,
    };
  }

  return {
    capabilityKey: "unknown_external_capability",
    capabilityLabel: "External capability request",
    summary: `User requested an unavailable capability: ${clampString(message, 140)}`,
    confidence: 0.58,
  };
};

const fallbackUpgradeRequestDecisionFromMessage = (message: string): UpgradeRequestDecision => {
  const cap = fallbackCapabilityFromMessage(message);
  const notifyWhenAvailable = /\b(notify|notification|email me|let me know|ping me|when available|when you can|update me)\b/i.test(
    message,
  );

  return {
    isUpgradeRequest: true,
    capabilityKey: normalizeCapabilityKey(cap.capabilityKey),
    capabilityLabel: clampString(cap.capabilityLabel, 160),
    summary: clampString(cap.summary, 240),
    notifyWhenAvailable,
    confidence: Math.max(0, Math.min(1, cap.confidence)),
  };
};

const looksLikeUpgradeRequestCandidate = (text: string): boolean => {
  const t = text.trim().toLowerCase();
  if (!t) return false;
  if (
    /(github|gitlab|bitbucket|repository|repo|repos|cloud|remote|deploy|codex|api|integration|plugin|webhook|browser access|live access)/i.test(
      t,
    )
  ) {
    return true;
  }
  return /\b(can you|could you|i want|i need|please)\b.*\b(connect|integrate|run|execute|deploy|access|learn|sync|upgrade)\b/i.test(
    t,
  );
};

const dseSignatureModelFromUnknown = (value: unknown): DseSignatureModelPart | undefined => {
  const model = toRecord(value);
  if (!model) return undefined;
  const modelId = readNonEmptyString(model.modelId);
  const provider = readNonEmptyString(model.provider);
  const route = readNonEmptyString(model.route);
  const fallbackModelId = readNonEmptyString(model.fallbackModelId);
  if (!modelId && !provider && !route && !fallbackModelId) return undefined;
  return {
    ...(modelId ? { modelId } : {}),
    ...(provider ? { provider } : {}),
    ...(route ? { route } : {}),
    ...(fallbackModelId ? { fallbackModelId } : {}),
  };
};

const errorMessageFromUnknown = (cause: unknown, fallback: string): string => {
  const message = readString(toRecord(cause)?.message);
  if (message && message.trim().length > 0) return message;
  const rendered = String(cause ?? fallback).trim();
  return rendered.length > 0 ? rendered : fallback;
};

const summaryFromOutput = (value: unknown): string => {
  const summary = readString(toRecord(value)?.summary);
  return summary ? summary.trim() : "";
};

const blobRefFromUnknown = (value: unknown): BlobRef | null => {
  const blob = toRecord(value);
  if (!blob) return null;
  const id = readNonEmptyString(blob.id);
  const hash = readNonEmptyString(blob.hash);
  const size = typeof blob.size === "number" && Number.isFinite(blob.size) ? blob.size : null;
  if (!id || !hash || size == null) return null;
  const mime = readNonEmptyString(blob.mime);
  return { id, hash, size, ...(mime ? { mime } : {}) };
};

const trimPromptRenderStats = (value: unknown): unknown => {
  const promptRenderStats = toRecord(value);
  if (!promptRenderStats) return value;
  const context = toRecord(promptRenderStats.context);
  if (!context) return value;
  const blobs = Array.isArray(context.blobs) ? context.blobs : null;
  if (!blobs || blobs.length <= 20) return value;
  return {
    ...promptRenderStats,
    context: {
      ...context,
      blobsDropped: Number(context.blobsDropped ?? 0) + (blobs.length - 20),
      blobs: blobs.slice(0, 20),
    },
  };
};

/** Blueprint shape (minimal) for bootstrap-aware prompt. */
type BlueprintHint = {
  readonly bootstrapState?: {
    readonly status?: string;
    readonly stage?: string;
  };
  readonly docs?: {
    readonly user?: { readonly addressAs?: string };
    readonly identity?: { readonly name?: string };
  };
} | null;

const BOOTSTRAP_ASK_USER_HANDLE_SYSTEM =
  "\n\nBootstrap: You are onboarding the user and collecting the user's preferred handle (what to call them). " +
  "If the user asks a question or says a greeting without giving a handle, answer briefly (1-2 sentences), then ask exactly: \"What shall I call you?\". " +
  "Once the user provides a handle, reply with exactly: \"Confirmed, <handle>. What should you call me?\" (Default: Autopilot). " +
  "Ask one question at a time and do not get stuck in the onboarding loop.";

const BOOTSTRAP_ASK_AGENT_NAME_SYSTEM =
  "\n\nBootstrap: You are collecting what the user should call you (your name). " +
  "If the user asks a question instead of giving a name, answer briefly (1-2 sentences), then ask exactly: \"What should you call me?\" (Default: Autopilot). " +
  "Once the user provides a name, reply with exactly: \"Confirmed. Pick one short operating vibe for me.\"";

const BOOTSTRAP_ASK_VIBE_SYSTEM =
  "\n\nBootstrap: You are collecting a short operating vibe for yourself (one short phrase). " +
  "If the user asks a question instead of giving a vibe, answer briefly (1-2 sentences), then ask exactly: \"Pick one short operating vibe for me.\". " +
  "Once the user provides a vibe, reply with exactly: \"Vibe confirmed: <vibe>. Any boundaries or preferences? Reply 'none' or list a few bullets.\"";

const BOOTSTRAP_ASK_BOUNDARIES_SYSTEM =
  "\n\nBootstrap: You are collecting optional boundaries/preferences. " +
  "If the user asks a question instead of giving boundaries, answer briefly (1-2 sentences), then ask exactly: \"Any boundaries or preferences? Reply 'none' or list a few bullets.\". " +
  "Once the user says none OR provides boundaries, confirm setup is complete and ask exactly: \"What would you like to do first?\"";

const concatTextFromPromptMessages = (
  messages: ReadonlyArray<{ readonly role: string; readonly text: string }>,
  blueprint: BlueprintHint = null,
  options?: { readonly extraSystem?: string | undefined },
): AiPrompt.RawInput => {
  const out: Array<{ role: string; content: string | Array<{ type: "text"; text: string }> }> = [];

  let systemContent =
    "You are Autopilot.\n" +
    "- Be concise, direct, and pragmatic.\n" +
    "- Do not claim web browsing capability.\n" +
    "- Do not reveal internal reasoning.\n" +
    "\n" +
    "When a user asks for something you cannot do yet: do not give a flat refusal. " +
    "Acknowledge you can't do that yet, then say the Autopilot network is evolving rapidly based on user requests and they should check back soon. " +
    "Offer that we can email them when we add that capability. " +
    "Close by asking if there is anything else you can help with in the meantime.";

  const status = blueprint?.bootstrapState?.status;
  const stage = blueprint?.bootstrapState?.stage;
  if (status !== "complete") {
    if (stage === "ask_user_handle") systemContent += BOOTSTRAP_ASK_USER_HANDLE_SYSTEM;
    if (stage === "ask_agent_name") systemContent += BOOTSTRAP_ASK_AGENT_NAME_SYSTEM;
    if (stage === "ask_vibe") systemContent += BOOTSTRAP_ASK_VIBE_SYSTEM;
    if (stage === "ask_boundaries") systemContent += BOOTSTRAP_ASK_BOUNDARIES_SYSTEM;
  }

  const extraSystem = options?.extraSystem;
  if (extraSystem && extraSystem.trim().length > 0) {
    systemContent += "\n\n" + extraSystem.trim();
  }

  out.push({ role: "system", content: systemContent });

  for (const m of messages) {
    const role = m.role === "assistant" ? "assistant" : "user";
    out.push({ role, content: [{ type: "text" as const, text: m.text }] });
  }

  return out as unknown as AiPrompt.RawInput;
};

const lastUserMessageFromSnapshot = (
  messages: ReadonlyArray<ThreadSnapshotMessage>,
): { readonly messageId: string; readonly text: string } => {
  for (let i = messages.length - 1; i >= 0; i--) {
    const m = messages[i];
    if (!m || typeof m !== "object") continue;
    if (String(m.role ?? "") !== "user") continue;
    const messageId = String(m.messageId ?? "").trim();
    const text = String(m.text ?? "").trim();
    if (!messageId || !text) continue;
    return { messageId, text };
  }
  return { messageId: "", text: "" };
};

const flushPartsToConvex = (input: {
  readonly convex: ConvexServiceApi;
  readonly threadId: string;
  readonly runId: string;
  readonly messageId: string;
  readonly parts: ReadonlyArray<{ readonly seq: number; readonly part: unknown }>;
}) =>
  input.convex.mutation(api.autopilot.messages.appendParts, {
    threadId: input.threadId,
    runId: input.runId,
    messageId: input.messageId,
    parts: input.parts.map((p) => ({ seq: p.seq, part: p.part })),
  });

const finalizeRunInConvex = (input: {
  readonly convex: ConvexServiceApi;
  readonly threadId: string;
  readonly runId: string;
  readonly messageId: string;
  readonly status: "final" | "error" | "canceled";
  readonly text?: string | undefined;
}) =>
  input.convex.mutation(api.autopilot.messages.finalizeRun, {
    threadId: input.threadId,
    runId: input.runId,
    messageId: input.messageId,
    status: input.status,
    ...(typeof input.text === "string" ? { text: input.text } : {}),
  });

const isCancelRequested = (input: {
  readonly convex: ConvexServiceApi;
  readonly threadId: string;
  readonly runId: string;
}) =>
  input.convex.query(api.autopilot.messages.isCancelRequested, {
    threadId: input.threadId,
    runId: input.runId,
  });

const runLightningL402FetchTool = (input: {
  readonly env: WorkerEnv;
  readonly convex: ConvexServiceApi;
  readonly requestId: string;
  readonly runId: string;
  readonly threadId: string;
  readonly controller: AbortController;
  readonly rawParams: unknown;
  readonly source: "call" | "slash";
  readonly queryTimeoutMs: number;
  readonly mutationTimeoutMs: number;
}) =>
  Effect.gen(function* () {
    const inputDecodeExit = yield* Effect.exit(decodeLightningL402FetchInput(input.rawParams));
    if (inputDecodeExit._tag === "Failure") {
      return {
        validatedInput: null,
        terminal: {
          taskId: null,
          status: "blocked" as const,
          proofReference: null,
          denyReason: "invalid_params",
          denyReasonCode: null,
          host: null,
          maxSpendMsats: null,
          quotedAmountMsats: null,
          paymentId: null,
          amountMsats: null,
          responseStatusCode: null,
          responseContentType: null,
          responseBytes: null,
          responseBodyTextPreview: null,
          responseBodySha256: null,
          cacheHit: false,
          paid: false,
          cacheStatus: null,
          paymentBackend: null,
          approvalRequired: false,
        } satisfies LightningToolTerminalResult,
      };
    }
    const decodedInput = inputDecodeExit.value;
    const requireApproval = decodedInput.requireApproval !== false;

    const resolvedUrl = (() => {
      if (isEp212EndpointPreset(decodedInput.endpointPreset)) {
        const resolved = resolveEp212PresetUrl(decodedInput.endpointPreset, input.env);
        if (!resolved.ok) {
          return { ok: false as const, errorCode: resolved.errorCode, message: resolved.message };
        }
        return { ok: true as const, url: resolved.url, mode: "preset" as const };
      }

      const rawUrl = typeof decodedInput.url === "string" ? decodedInput.url.trim() : "";
      if (!rawUrl) {
        return { ok: false as const, errorCode: "invalid_input" as const, message: "Missing url or endpointPreset" };
      }
      try {
        const parsed = new URL(rawUrl);
        if (parsed.username || parsed.password) {
          return { ok: false as const, errorCode: "invalid_input" as const, message: "URL must not include userinfo" };
        }
        return { ok: true as const, url: parsed.toString(), mode: "direct" as const };
      } catch {
        return { ok: false as const, errorCode: "invalid_input" as const, message: "Invalid url" };
      }
    })();

    if (!resolvedUrl.ok) {
      return {
        validatedInput: decodedInput,
        terminal: {
          taskId: null,
          status: "blocked" as const,
          proofReference: null,
          denyReason: resolvedUrl.message,
          denyReasonCode: resolvedUrl.errorCode,
          host: null,
          maxSpendMsats:
            typeof decodedInput.maxSpendMsats === "number" && Number.isFinite(decodedInput.maxSpendMsats)
              ? decodedInput.maxSpendMsats
              : null,
          quotedAmountMsats: null,
          paymentId: null,
          amountMsats: null,
          responseStatusCode: null,
          responseContentType: null,
          responseBytes: null,
          responseBodyTextPreview: null,
          responseBodySha256: null,
          cacheHit: false,
          paid: false,
          cacheStatus: null,
          paymentBackend: null,
          approvalRequired: false,
        } satisfies LightningToolTerminalResult,
      };
    }

    const requestUrl = resolvedUrl.url;
    const requestHeaders =
      resolvedUrl.mode === "preset" ? undefined : sanitizeLightningHeadersForTask(decodedInput.headers);
    const requestMethod = resolvedUrl.mode === "preset" ? "GET" : decodedInput.method;
    const requestBody = resolvedUrl.mode === "preset" ? undefined : decodedInput.body;

    const requestForTask: Record<string, unknown> = {
      url: requestUrl,
      ...(typeof requestMethod === "string" ? { method: requestMethod } : {}),
      ...(requestHeaders ? { headers: requestHeaders } : {}),
      ...(typeof requestBody === "string" ? { body: requestBody } : {}),
      maxSpendMsats: decodedInput.maxSpendMsats,
      ...(typeof decodedInput.challengeHeader === "string" ? { challengeHeader: decodedInput.challengeHeader } : {}),
      ...(typeof decodedInput.forceRefresh === "boolean" ? { forceRefresh: decodedInput.forceRefresh } : {}),
      ...(typeof decodedInput.scope === "string" ? { scope: decodedInput.scope } : {}),
      ...(typeof decodedInput.cacheTtlMs === "number" ? { cacheTtlMs: decodedInput.cacheTtlMs } : {}),
    };

    const createRaw = yield* input.convex
      .mutation(api.lightning.tasks.createTask, {
        request: requestForTask as any,
        idempotencyKey: `autopilot:${input.threadId}:${input.runId}`,
        source: `autopilot_${input.source}_tool`,
        requestId: input.requestId,
        metadata: {
          toolName: LIGHTNING_L402_FETCH_TOOL_NAME,
          runId: input.runId,
          threadId: input.threadId,
        },
      })
      .pipe(
        Effect.timeoutFail({
          duration: `${input.mutationTimeoutMs} millis`,
          onTimeout: () => new Error("lightning.createTask_timeout"),
        }),
      );
    const createdTask = parseLightningTaskDoc(toRecord(createRaw)?.task);
    if (!createdTask) {
      return {
        validatedInput: decodedInput,
        terminal: {
          taskId: null,
          status: "failed" as const,
          proofReference: null,
          denyReason: "invalid_task_shape",
          denyReasonCode: null,
          host: null,
          maxSpendMsats: null,
          quotedAmountMsats: null,
          paymentId: null,
          amountMsats: null,
          responseStatusCode: null,
          responseContentType: null,
          responseBytes: null,
          responseBodyTextPreview: null,
          responseBodySha256: null,
          cacheHit: false,
          paid: false,
          cacheStatus: null,
          paymentBackend: null,
          approvalRequired: false,
        } satisfies LightningToolTerminalResult,
      };
    }

    // EP212 requirement: explicit approval before any payment executes.
    if (requireApproval && !isLightningTerminalStatus(createdTask.status)) {
      return {
        validatedInput: decodedInput,
        terminal: {
          taskId: createdTask.taskId,
          status: "queued" as const,
          proofReference: null,
          denyReason: null,
          denyReasonCode: null,
          host: (() => {
            try {
              return new URL(String(requestUrl)).host;
            } catch {
              return null;
            }
          })(),
          maxSpendMsats:
            typeof decodedInput.maxSpendMsats === "number" && Number.isFinite(decodedInput.maxSpendMsats)
              ? decodedInput.maxSpendMsats
              : null,
          quotedAmountMsats: null,
          paymentId: null,
          amountMsats: null,
          responseStatusCode: null,
          responseContentType: null,
          responseBytes: null,
          responseBodyTextPreview: null,
          responseBodySha256: null,
          cacheHit: false,
          paid: false,
          cacheStatus: null,
          paymentBackend: null,
          approvalRequired: true,
        } satisfies LightningToolTerminalResult,
      };
    }

    const waitStartedAtMs = Date.now();
    const WAIT_TIMEOUT_MS = 4_500;
    const WAIT_INTERVAL_MS = 180;

    let task = createdTask;

    // If approval is explicitly disabled for this tool call, auto-approve the queued task.
    if (!requireApproval && task.status === "queued") {
      const presenceRaw = yield* input.convex
        .query(api.lightning.presence.getLatestExecutorPresence, {})
        .pipe(
          Effect.timeoutFail({
            duration: `${input.queryTimeoutMs} millis`,
            onTimeout: () => new Error("lightning.getLatestExecutorPresence_timeout"),
          }),
          Effect.catchAll(() => Effect.succeed({ ok: true, presence: null } as unknown)),
        );
      const presence = toRecord(toRecord(presenceRaw)?.presence);
      const lastSeenAtMs =
        typeof presence?.lastSeenAtMs === "number" && Number.isFinite(presence.lastSeenAtMs)
          ? Math.max(0, Math.floor(presence.lastSeenAtMs))
          : null;
      const executorOnline = isExecutorPresenceFresh({
        lastSeenAtMs,
        nowMs: Date.now(),
        maxAgeMs: DEFAULT_EXECUTOR_PRESENCE_MAX_AGE_MS,
      });

      if (!executorOnline) {
        const host = (() => {
          try {
            return new URL(String(requestUrl)).host;
          } catch {
            return null;
          }
        })();

        const transitionRaw = yield* input.convex
          .mutation(api.lightning.tasks.transitionTask, {
            taskId: task.taskId,
            toStatus: "blocked",
            actor: "system",
            reason: "desktop_executor_offline",
            requestId: input.requestId,
            errorCode: "desktop_executor_offline",
            errorMessage: "desktop executor heartbeat not seen recently",
            metadata: {
              denyReason: "desktop_executor_offline",
              denyReasonCode: "desktop_executor_offline",
              ...(host ? { host } : {}),
              toolName: LIGHTNING_L402_FETCH_TOOL_NAME,
              runId: input.runId,
            },
          })
          .pipe(
            Effect.timeoutFail({
              duration: `${input.mutationTimeoutMs} millis`,
              onTimeout: () => new Error("lightning.transitionTask_timeout"),
            }),
            Effect.catchAll(() => Effect.succeed({ ok: false } as unknown)),
          );
        task = parseLightningTaskDoc(toRecord(transitionRaw)?.task) ?? {
          ...task,
          status: "blocked",
        };
      } else {
      const approveRaw = yield* input.convex
        .mutation(api.lightning.tasks.transitionTask, {
          taskId: task.taskId,
          toStatus: "approved",
          actor: "web_worker",
          reason: "auto_approved",
          requestId: input.requestId,
          metadata: {
            toolName: LIGHTNING_L402_FETCH_TOOL_NAME,
            runId: input.runId,
            threadId: input.threadId,
          },
        })
        .pipe(
          Effect.timeoutFail({
            duration: `${input.mutationTimeoutMs} millis`,
            onTimeout: () => new Error("lightning.transitionTask_timeout"),
          }),
          Effect.catchAll(() => Effect.succeed({ ok: false } as unknown)),
        );
      task = parseLightningTaskDoc(toRecord(approveRaw)?.task) ?? task;
      }
    }

    while (
      !input.controller.signal.aborted &&
      !isLightningTerminalStatus(task.status) &&
      Date.now() - waitStartedAtMs <= WAIT_TIMEOUT_MS
    ) {
      yield* Effect.sleep(`${WAIT_INTERVAL_MS} millis`);
      const taskRaw = yield* input.convex
        .query(api.lightning.tasks.getTask, { taskId: task.taskId })
        .pipe(
          Effect.timeoutFail({
            duration: `${input.queryTimeoutMs} millis`,
            onTimeout: () => new Error("lightning.getTask_timeout"),
          }),
          Effect.catchAll(() => Effect.succeed({ ok: true, task } as unknown)),
        );
      task = parseLightningTaskDoc(toRecord(taskRaw)?.task) ?? task;
    }

    if (!isLightningTerminalStatus(task.status)) {
      const transitionRaw = yield* input.convex
        .mutation(api.lightning.tasks.transitionTask, {
          taskId: task.taskId,
          toStatus: "blocked",
          actor: "system",
          reason: "executor_timeout",
          requestId: input.requestId,
          errorCode: "desktop_executor_timeout",
          errorMessage: "desktop executor did not reach a terminal status before timeout",
          metadata: {
            denyReason: "desktop_executor_timeout",
            toolName: LIGHTNING_L402_FETCH_TOOL_NAME,
            runId: input.runId,
          },
        })
        .pipe(
          Effect.timeoutFail({
            duration: `${input.mutationTimeoutMs} millis`,
            onTimeout: () => new Error("lightning.transitionTask_timeout"),
          }),
          Effect.catchAll(() => Effect.succeed({ ok: false } as unknown)),
        );
      task = parseLightningTaskDoc(toRecord(transitionRaw)?.task) ?? {
        ...task,
        status: "blocked",
      };
    }

    const eventsRaw = yield* input.convex
      .query(api.lightning.tasks.listTaskEvents, { taskId: task.taskId, limit: 100 })
      .pipe(
        Effect.timeoutFail({
          duration: `${input.queryTimeoutMs} millis`,
          onTimeout: () => new Error("lightning.listTaskEvents_timeout"),
        }),
        Effect.catchAll(() => Effect.succeed({ ok: true, events: [] as unknown[] } as unknown)),
      );

    const events = Array.isArray(toRecord(eventsRaw)?.events)
      ? (toRecord(eventsRaw)?.events as ReadonlyArray<unknown>)
      : [];
    const parsedEvents = events.map(parseLightningTaskEventDoc).filter((row): row is LightningTaskEventDoc => row !== null);
    const latestEvent = [...parsedEvents].reverse().find((event) =>
      typeof event.toStatus === "string" ? event.toStatus === task.status : false,
    ) ?? parsedEvents.at(-1);

    const metadata = toRecord(latestEvent?.metadata);
    const proofReference =
      readNonEmptyString(metadata?.proofReference) ??
      readNonEmptyString(metadata?.l402ProofReference) ??
      null;
    const denyReason =
      readNonEmptyString(metadata?.denyReason) ??
      readNonEmptyString(latestEvent?.errorMessage) ??
      readNonEmptyString(latestEvent?.reason) ??
      readNonEmptyString(task.lastErrorMessage) ??
      (task.status === "blocked" || task.status === "failed" ? task.status : null);
    const denyReasonCode = readString(metadata?.denyReasonCode) ?? null;
    const host = readString(metadata?.host)?.trim() || null;
    const paymentId = readNonEmptyString(metadata?.paymentId) ?? null;
    const amountMsatsMeta =
      typeof metadata?.amountMsats === "number" && Number.isFinite(metadata.amountMsats) ? metadata.amountMsats : null;
    const amountMsatsRequest =
      typeof task.request?.maxSpendMsats === "number" && Number.isFinite(task.request.maxSpendMsats)
        ? task.request.maxSpendMsats
        : null;
    const maxSpendMsatsMeta =
      typeof metadata?.maxSpendMsats === "number" && Number.isFinite(metadata.maxSpendMsats) ? metadata.maxSpendMsats : null;
    const quotedAmountMsatsMeta =
      typeof metadata?.quotedAmountMsats === "number" && Number.isFinite(metadata.quotedAmountMsats)
        ? metadata.quotedAmountMsats
        : null;
    const responseStatusCode =
      typeof metadata?.responseStatusCode === "number" && Number.isFinite(metadata.responseStatusCode)
        ? metadata.responseStatusCode
        : null;

    const responseContentType = readString(metadata?.responseContentType)?.trim() || null;
    const responseBytes =
      typeof metadata?.responseBytes === "number" && Number.isFinite(metadata.responseBytes) ? metadata.responseBytes : null;
    const responseBodyTextPreview = readString(metadata?.responseBodyTextPreview) ?? null;
    const responseBodySha256 = readString(metadata?.responseBodySha256) ?? null;
    const cacheHit = metadata?.cacheHit === true;
    const paid = metadata?.paid === true;
    const cacheStatus = isL402CacheStatus(metadata?.cacheStatus) ? metadata.cacheStatus : null;
    const paymentBackend = isL402PaymentBackend(metadata?.paymentBackend) ? metadata.paymentBackend : null;

    return {
      validatedInput: decodedInput,
      terminal: {
        taskId: task.taskId,
        status: isLightningTerminalStatus(task.status) ? task.status : "failed",
        proofReference,
        denyReason,
        denyReasonCode,
        host,
        maxSpendMsats: maxSpendMsatsMeta ?? amountMsatsRequest,
        quotedAmountMsats: quotedAmountMsatsMeta,
        paymentId,
        amountMsats: amountMsatsMeta ?? amountMsatsRequest,
        responseStatusCode,
        responseContentType,
        responseBytes,
        responseBodyTextPreview,
        responseBodySha256,
        cacheHit,
        paid,
        cacheStatus,
        paymentBackend,
        approvalRequired: false,
      } satisfies LightningToolTerminalResult,
    };
  }).pipe(
    Effect.catchAll((error) =>
      Effect.succeed({
        validatedInput: null,
        terminal: {
          taskId: null,
          status: "failed" as const,
          proofReference: null,
          denyReason: errorMessageFromUnknown(error, "lightning_tool_failed"),
          denyReasonCode: null,
          host: null,
          maxSpendMsats: null,
          quotedAmountMsats: null,
          paymentId: null,
          amountMsats: null,
          responseStatusCode: null,
          responseContentType: null,
          responseBytes: null,
          responseBodyTextPreview: null,
          responseBodySha256: null,
          cacheHit: false,
          paid: false,
          cacheStatus: null,
          paymentBackend: null,
          approvalRequired: false,
        } satisfies LightningToolTerminalResult,
      }),
    ),
  );

type LightningToolApproveResult = {
  readonly taskId: string | null;
  readonly ok: boolean;
  readonly changed: boolean;
  readonly taskStatus: LightningTaskStatus | null;
  readonly denyReason: string | null;
};

const runLightningL402ApproveTool = (input: {
  readonly convex: ConvexServiceApi;
  readonly requestId: string;
  readonly runId: string;
  readonly threadId: string;
  readonly controller: AbortController;
  readonly rawParams: unknown;
  readonly source: "call" | "slash";
  readonly queryTimeoutMs: number;
  readonly mutationTimeoutMs: number;
}) =>
  Effect.gen(function* () {
    const inputDecodeExit = yield* Effect.exit(decodeLightningL402ApproveInput(input.rawParams));
    if (inputDecodeExit._tag === "Failure") {
      return {
        validatedInput: null,
        output: {
          taskId: null,
          ok: false,
          changed: false,
          taskStatus: null,
          denyReason: "invalid_params",
        } satisfies LightningToolApproveResult,
      };
    }

    const decodedInput = inputDecodeExit.value as { readonly taskId: string };
    const taskId = String(decodedInput.taskId ?? "").trim();
    if (!taskId) {
      return {
        validatedInput: decodedInput,
        output: {
          taskId: null,
          ok: false,
          changed: false,
          taskStatus: null,
          denyReason: "invalid_task_id",
        } satisfies LightningToolApproveResult,
      };
    }

    const taskRaw = yield* input.convex
      .query(api.lightning.tasks.getTask, { taskId })
      .pipe(
        Effect.timeoutFail({
          duration: `${input.queryTimeoutMs} millis`,
          onTimeout: () => new Error("lightning.getTask_timeout"),
        }),
        Effect.catchAll(() => Effect.succeed({ ok: true, task: null } as unknown)),
      );
    const task = parseLightningTaskDoc(toRecord(taskRaw)?.task);
    if (!task) {
      return {
        validatedInput: decodedInput,
        output: {
          taskId,
          ok: false,
          changed: false,
          taskStatus: null,
          denyReason: "task_not_found",
        } satisfies LightningToolApproveResult,
      };
    }

    if (task.status !== "queued") {
      return {
        validatedInput: decodedInput,
        output: {
          taskId: task.taskId,
          ok: true,
          changed: false,
          taskStatus: task.status,
          denyReason: null,
        } satisfies LightningToolApproveResult,
      };
    }

    const presenceRaw = yield* input.convex
      .query(api.lightning.presence.getLatestExecutorPresence, {})
      .pipe(
        Effect.timeoutFail({
          duration: `${input.queryTimeoutMs} millis`,
          onTimeout: () => new Error("lightning.getLatestExecutorPresence_timeout"),
        }),
        Effect.catchAll(() => Effect.succeed({ ok: true, presence: null } as unknown)),
      );
    const presence = toRecord(toRecord(presenceRaw)?.presence);
    const lastSeenAtMs =
      typeof presence?.lastSeenAtMs === "number" && Number.isFinite(presence.lastSeenAtMs)
        ? Math.max(0, Math.floor(presence.lastSeenAtMs))
        : null;
    const executorOnline = isExecutorPresenceFresh({
      lastSeenAtMs,
      nowMs: Date.now(),
      maxAgeMs: DEFAULT_EXECUTOR_PRESENCE_MAX_AGE_MS,
    });

    if (!executorOnline) {
      return {
        validatedInput: decodedInput,
        output: {
          taskId: task.taskId,
          ok: false,
          changed: false,
          taskStatus: task.status,
          denyReason: "desktop_executor_offline",
        } satisfies LightningToolApproveResult,
      };
    }

    const approveRaw = yield* input.convex
      .mutation(api.lightning.tasks.transitionTask, {
        taskId: task.taskId,
        toStatus: "approved",
        actor: "web_worker",
        reason: "user_approved",
        requestId: input.requestId,
        metadata: {
          toolName: LIGHTNING_L402_APPROVE_TOOL_NAME,
          runId: input.runId,
          threadId: input.threadId,
          source: input.source,
        },
      })
      .pipe(
        Effect.timeoutFail({
          duration: `${input.mutationTimeoutMs} millis`,
          onTimeout: () => new Error("lightning.transitionTask_timeout"),
        }),
      );

    const transitionedTask = parseLightningTaskDoc(toRecord(approveRaw)?.task);
    if (!transitionedTask) {
      return {
        validatedInput: decodedInput,
        output: {
          taskId: task.taskId,
          ok: false,
          changed: false,
          taskStatus: task.status,
          denyReason: "invalid_task_shape",
        } satisfies LightningToolApproveResult,
      };
    }

    return {
      validatedInput: decodedInput,
      output: {
        taskId: transitionedTask.taskId,
        ok: true,
        changed: toRecord(approveRaw)?.changed === true,
        taskStatus: transitionedTask.status,
        denyReason: null,
      } satisfies LightningToolApproveResult,
    };
  }).pipe(
    Effect.catchAll((error) =>
      Effect.succeed({
        validatedInput: null,
        output: {
          taskId: null,
          ok: false,
          changed: false,
          taskStatus: null,
          denyReason: errorMessageFromUnknown(error, "lightning_tool_failed"),
        } satisfies LightningToolApproveResult,
      }),
    ),
  );

const runDseCanaryRecap = (input: {
  readonly env: WorkerEnv & { readonly AI: Ai };
  readonly request: Request;
  readonly threadId: string;
  readonly runId: string;
  readonly userMessageId: string;
  readonly assistantMessageId: string;
  readonly controller: AbortController;
  readonly strategyId: DseStrategyId;
  readonly budgetProfile: DseBudgetProfile;
  readonly question: string;
  readonly e2eMode: "stub" | "off";
}) => {
  const { runtime } = getWorkerRuntime(input.env);
  const url = new URL(input.request.url);
  const requestId = input.request.headers.get(OA_REQUEST_ID_HEADER) ?? "missing";

  const telemetryBase = runtime.runSync(
    Effect.gen(function* () {
      return yield* TelemetryService;
    }),
  );
  const requestTelemetry = telemetryBase.withFields({
    requestId,
    method: input.request.method,
    pathname: url.pathname,
  });

  const CONVEX_QUERY_TIMEOUT_MS = 8_000;
  const CONVEX_MUTATION_TIMEOUT_MS = 8_000;
  const DSE_RECAP_TIMEOUT_MS = 90_000;
  const DSE_PREDICT_TIMEOUT_MS = 60_000;

  const effect = Effect.gen(function* () {
    const telemetry = yield* TelemetryService;
    const convex = yield* ConvexService;
    const t = telemetry.withNamespace("autopilot.dse_canary_recap");

    // Ensure auth is loaded (so ConvexService server client can setAuth token). Owner-only; no anon.
    const session = yield* Effect.flatMap(AuthService, (auth) => auth.getSession()).pipe(
      Effect.timeoutFail({
        duration: `${CONVEX_QUERY_TIMEOUT_MS} millis`,
        onTimeout: () => new Error("auth.getSession_timeout"),
      }),
      Effect.catchAll(() => Effect.succeed({ userId: null } as { readonly userId: string | null })),
    );

    yield* t.event("run.started", {
      threadId: input.threadId,
      runId: input.runId,
      strategyId: input.strategyId,
      budgetProfile: input.budgetProfile,
      e2eMode: input.e2eMode,
    }).pipe(Effect.catchAll(() => Effect.void));

    let status: "final" | "error" | "canceled" = "final";
    let outputText = "";
    let seq = 0;
    let emittedTextPart = false;
    const partId = `dsepart_sig_${input.runId}_canary_recap_thread`;
    const signatureId = dseCatalogSignatures.canary_recap_thread.id;
    const strategyReason = "forced_by_user";

    const flushParts = Effect.fn("autopilot.dse_canary_recap.flushParts")(function* (parts: ReadonlyArray<unknown>) {
      if (input.controller.signal.aborted) return;
      yield* flushPartsToConvex({
        convex,
        threadId: input.threadId,
        runId: input.runId,
        messageId: input.assistantMessageId,
        parts: parts.map((part) => ({ seq: seq++, part })),
      }).pipe(
        Effect.timeoutFail({
          duration: `${CONVEX_MUTATION_TIMEOUT_MS} millis`,
          onTimeout: () => new Error("convex.append_timeout"),
        }),
      );
    });

    const checkCanceled = Effect.fn("autopilot.dse_canary_recap.checkCanceled")(function* () {
      if (input.controller.signal.aborted) return true;
      const cancel = yield* isCancelRequested({ convex, threadId: input.threadId, runId: input.runId }).pipe(
        Effect.timeoutFail({
          duration: `${CONVEX_QUERY_TIMEOUT_MS} millis`,
          onTimeout: () => new Error("convex.isCancelRequested_timeout"),
        }),
        Effect.catchAll(() => Effect.succeed({ ok: true as const, cancelRequested: false } as IsCancelRequestedResult)),
      );
      return toRecord(cancel)?.cancelRequested === true;
    });

    const program = Effect.gen(function* () {
        // Check cancellation early.
        if (yield* checkCanceled()) return yield* Effect.fail(new Error("canceled"));

        const budgets = budgetsForProfile(input.budgetProfile);

        yield* flushParts([
          {
            type: "dse.signature",
            v: 1,
            id: partId,
            state: "start",
            tsMs: Date.now(),
            signatureId,
            strategyId: input.strategyId,
            strategyReason,
            budget: { limits: budgets },
          },
        ]);

        // E2E-only deterministic stub mode (no external model calls).
        const sessionUserId = readString(toRecord(session)?.userId);
        const isE2eUser = Boolean(sessionUserId?.startsWith("user_e2e_"));
        if (input.e2eMode === "stub" && isE2eUser) {
          const startedAtMs = Date.now();
          const receiptId = crypto.randomUUID();
          const summary = `E2E stub recap (${input.strategyId}, ${input.budgetProfile}).\n- This is deterministic and exists to assert UI wiring.\n- receiptId=${receiptId}`;

          const traceDoc = {
            format: "openagents.dse.rlm_trace",
            formatVersion: 1,
            signatureId,
            receiptId,
            strategyId: input.strategyId,
            events: [
              { _tag: "Iteration", i: 1, note: "stub" },
              { _tag: "Iteration", i: 2, note: "stub" },
              { _tag: "Final", output: { summary } },
            ],
          };

          const tracePut = yield* convex
            .mutation(api.dse.blobs.putText, {
              threadId: input.threadId,
              runId: input.runId,
              text: JSON.stringify(traceDoc, null, 2),
              mime: "application/json",
            })
            .pipe(
              Effect.timeoutFail({
                duration: `${CONVEX_MUTATION_TIMEOUT_MS} millis`,
                onTimeout: () => new Error("convex.dse_blobs.putText_timeout"),
              }),
            );

          const traceBlob = blobRefFromUnknown(toRecord(tracePut)?.blob);

          const receipt: Receipt.PredictReceiptV1 = {
            format: "openagents.dse.predict_receipt",
            formatVersion: 1,
            receiptId,
            runId: receiptId,
            createdAt: new Date().toISOString(),
            signatureId,
            compiled_id: "e2e_stub",
            strategyId: input.strategyId,
            hashes: {
              inputSchemaHash: "e2e_stub",
              outputSchemaHash: "e2e_stub",
              promptIrHash: "e2e_stub",
              paramsHash: "e2e_stub",
            },
            model: { modelId: "e2e_stub" },
            timing: {
              startedAtMs,
              endedAtMs: startedAtMs + 1,
              durationMs: 1,
            },
            budget: {
              limits: budgets,
              usage: {
                elapsedMs: 1,
                lmCalls: 0,
                toolCalls: 0,
                rlmIterations: 2,
                subLmCalls: 0,
                outputChars: summary.length,
              },
            },
            ...(traceBlob && typeof traceBlob === "object"
              ? { rlmTrace: { blob: traceBlob, eventCount: Array.isArray(traceDoc.events) ? traceDoc.events.length : 0 } }
              : {}),
            result: { _tag: "Ok" },
          };

          yield* convex
            .mutation(api.dse.receipts.recordPredictReceipt, {
              threadId: input.threadId,
              runId: input.runId,
              receipt,
            })
            .pipe(
              Effect.timeoutFail({
                duration: `${CONVEX_MUTATION_TIMEOUT_MS} millis`,
                onTimeout: () => new Error("convex.dse_receipts.recordPredictReceipt_timeout"),
              }),
            );

          // Emit assistant-visible text via stream wire parts so it renders alongside DSE cards.
          const textParts = [
            { type: "text-start", id: crypto.randomUUID(), metadata: {} },
            { type: "text-delta", id: crypto.randomUUID(), delta: summary, metadata: {} },
            { type: "text-end", id: crypto.randomUUID(), metadata: {} },
          ];

          emittedTextPart = true;
          yield* flushParts([
            {
              type: "dse.signature",
              v: 1,
              id: partId,
              state: "ok",
              tsMs: Date.now(),
              signatureId,
              compiled_id: receipt.compiled_id,
              receiptId: receipt.receiptId,
              timing: receipt.timing,
              budget: receipt.budget,
              strategyId: receipt.strategyId,
              strategyReason,
              contextPressure: receipt.contextPressure,
              promptRenderStats: receipt.promptRenderStats,
              rlmTrace: receipt.rlmTrace,
              outputPreview: { summary },
            },
            ...textParts,
          ]);

          return;
        }

        // Real path: DSE Predict (direct or rlm-lite strategy pinned in params).
        if (yield* checkCanceled()) return yield* Effect.fail(new Error("canceled"));

        const rawSnapshot = yield* convex
          .query(api.autopilot.messages.getThreadSnapshot, {
            threadId: input.threadId,
            maxMessages: 240,
            maxParts: 0,
          })
          .pipe(
            Effect.timeoutFail({
              duration: `${CONVEX_QUERY_TIMEOUT_MS} millis`,
              onTimeout: () => new Error("convex.getThreadSnapshot_timeout"),
            }),
          );
        const snapshot = rawSnapshot as GetThreadSnapshotResult;
        const messagesRaw: ReadonlyArray<ThreadSnapshotMessage> = Array.isArray(snapshot.messages)
          ? snapshot.messages
          : [];

        const history = messagesRaw
          .filter((m) => m && typeof m === "object")
          .filter((m) => String(m.role ?? "") === "user" || String(m.role ?? "") === "assistant")
          .filter(
            (m) =>
              String(m.status ?? "") !== "streaming" ||
              String(m.messageId ?? "") !== input.assistantMessageId,
          )
          .filter((m) => String(m.messageId ?? "") !== input.userMessageId)
          .map((m) => ({ role: String(m.role ?? "user"), text: String(m.text ?? "") }))
          .filter((m) => m.text.trim().length > 0);

        const historyText = history.map((m) => `${m.role}: ${m.text}`).join("\n\n");

        const dseLmClient = makeDseLmClientWithOpenRouterPrimary({
          env: input.env,
          defaultModelIdCf: MODEL_ID_CF,
          primaryModelOpenRouter: PRIMARY_MODEL_OPENROUTER,
        });

        type DseReceiptShape = {
          compiled_id?: string;
          strategyId?: string;
          timing?: { durationMs?: number };
          budget?: unknown;
          receiptId?: string;
          contextPressure?: unknown;
          promptRenderStats?: unknown;
          rlmTrace?: unknown;
          model?: {
            modelId?: string;
            provider?: string;
            route?: string;
            fallbackModelId?: string;
          };
        };
        let recordedReceipt: DseReceiptShape | null = null;
        const onReceipt = (r: unknown) => {
          recordedReceipt = r as DseReceiptShape;
        };

        const dseEnv = layerDsePredictEnvForAutopilotRun({
          threadId: input.threadId,
          runId: input.runId,
          onReceipt,
        });

        const sig = makeCanaryRecapSignature({
          strategyId: input.strategyId,
          budgets,
        });
        const predict = Predict.make(sig);

        const predictExit = yield* Effect.exit(
          Effect.gen(function* () {
            const blobStore = yield* BlobStore.BlobStoreService;

            // Bound history size so Convex blobs remain reasonable.
            const MAX_HISTORY_CHARS = 200_000;
            const boundedHistory =
              historyText.length > MAX_HISTORY_CHARS
                ? historyText.slice(historyText.length - MAX_HISTORY_CHARS)
                : historyText;

            // Pre-chunk history into stable BlobRefs so both direct and RLM strategies see the same input shape.
            const chunkChars = 20_000;
            const maxChunks = 12;
            const chunkTexts: Array<string> = [];
            for (let i = 0; i < boundedHistory.length && chunkTexts.length < maxChunks; i += chunkChars) {
              chunkTexts.push(boundedHistory.slice(i, Math.min(boundedHistory.length, i + chunkChars)));
            }

            const threadChunks = yield* Effect.forEach(
              chunkTexts,
              (text) => blobStore.putText({ text, mime: "text/plain" }),
              { concurrency: 3, discard: false },
            );

            return yield* predict({ question: input.question, threadChunks });
          }).pipe(
            Effect.timeoutFail({
              duration: `${DSE_PREDICT_TIMEOUT_MS} millis`,
              onTimeout: () => new Error("dse.predict_timeout"),
            }),
            Effect.provideService(Lm.LmClientService, dseLmClient),
            Effect.provide(dseEnv),
          ),
        );

        const receipt = recordedReceipt as DseReceiptShape | null;
        const trimmedPromptRenderStats = trimPromptRenderStats(receipt?.promptRenderStats);

        const state = predictExit._tag === "Success" ? "ok" : "error";
        const errorText =
          predictExit._tag === "Failure"
            ? errorMessageFromUnknown(predictExit.cause, "DSE predict failed")
            : null;

        const summary = predictExit._tag === "Success" ? summaryFromOutput(predictExit.value) : "";
        const boundedSummary = summary.length > 2000 ? summary.slice(0, 2000).trim() : summary;

        const textParts =
          state === "ok" && boundedSummary
            ? [
              { type: "text-start", id: crypto.randomUUID(), metadata: {} },
              { type: "text-delta", id: crypto.randomUUID(), delta: boundedSummary, metadata: {} },
              { type: "text-end", id: crypto.randomUUID(), metadata: {} },
            ]
            : [];
        if (textParts.length > 0) emittedTextPart = true;

        yield* flushParts([
          {
            type: "dse.signature",
            v: 1,
            id: partId,
            state,
            tsMs: Date.now(),
            signatureId,
            compiled_id: receipt?.compiled_id,
            receiptId: receipt?.receiptId,
            model: dseSignatureModelFromUnknown(receipt?.model),
            timing: receipt?.timing,
            budget: receipt?.budget,
            strategyId: receipt?.strategyId,
            strategyReason,
            contextPressure: receipt?.contextPressure,
            promptRenderStats: trimmedPromptRenderStats,
            rlmTrace: receipt?.rlmTrace,
            ...(state === "ok" ? { outputPreview: predictExit._tag === "Success" ? predictExit.value : undefined } : {}),
            ...(errorText ? { errorText } : {}),
          },
          ...textParts,
        ]);

        if (state !== "ok") return yield* Effect.fail(new Error(errorText ?? "DSE recap failed"));
      }).pipe(
        Effect.timeoutFail({
          duration: `${DSE_RECAP_TIMEOUT_MS} millis`,
          onTimeout: () => new Error("dse.recap_timeout"),
        }),
      );

    const exit = yield* Effect.exit(program);

    if (exit._tag === "Failure") {
      const msg = Cause.pretty(exit.cause).trim() || "dse_recap_failed";
      const canceled = msg.includes("canceled") || input.controller.signal.aborted;
      status = canceled ? "canceled" : "error";
      outputText = canceled ? "Canceled." : "DSE recap failed.";
      yield* t.log("error", "run.failed", { message: msg }).pipe(Effect.catchAll(() => Effect.void));
    } else if (input.controller.signal.aborted) {
      status = "canceled";
      if (outputText.trim().length === 0) outputText = "Canceled.";
    }

    if (status === "final") {
      // If we didn't emit any text parts, ensure the assistant message is still visible.
      if (!emittedTextPart && outputText.trim().length === 0) {
        outputText = "DSE recap complete.";
      }
    } else if (outputText.trim().length === 0) {
      outputText = status === "canceled" ? "Canceled." : "DSE recap failed.";
    }

    yield* finalizeRunInConvex({
      convex,
      threadId: input.threadId,
      runId: input.runId,
      messageId: input.assistantMessageId,
      status,
      text: outputText,
    })
      .pipe(
        Effect.timeoutFail({
          duration: `${CONVEX_MUTATION_TIMEOUT_MS} millis`,
          onTimeout: () => new Error("convex.finalize_timeout"),
        }),
      )
      .pipe(Effect.catchAll(() => Effect.void));

    yield* t.event("run.finished", { threadId: input.threadId, runId: input.runId, status }).pipe(
      Effect.catchAll(() => Effect.void),
    );
  }).pipe(
    Effect.provideService(RequestContextService, makeServerRequestContext(input.request)),
    Effect.provideService(TelemetryService, requestTelemetry),
    Effect.catchAll((err) => {
      console.error(`[autopilot.dse_canary_recap] ${formatRequestIdLogToken(requestId)}`, err);
      return Effect.void;
    }),
  );

  return runtime.runPromise(effect).finally(() => {
    activeRuns.delete(input.runId);
  });
};

const runAutopilotStream = (input: {
  readonly env: WorkerEnv & { readonly AI: Ai };
  readonly request: Request;
  readonly threadId: string;
  readonly runId: string;
  readonly assistantMessageId: string;
  readonly controller: AbortController;
}) => {
  const { runtime } = getWorkerRuntime(input.env);
  const url = new URL(input.request.url);
  const requestId = input.request.headers.get(OA_REQUEST_ID_HEADER) ?? "missing";
  const telemetryBase = runtime.runSync(
    Effect.gen(function* () {
      return yield* TelemetryService;
    }),
  );
  const requestTelemetry = telemetryBase.withFields({
    requestId,
    method: input.request.method,
    pathname: url.pathname,
  });

  // Bound worst-case wall time for worker-side Convex + model operations.
  // This must be conservative: `ctx.waitUntil` tasks can be evicted, and upstream streams can stall.
  const CONVEX_QUERY_TIMEOUT_MS = 8_000;
  const CONVEX_MUTATION_TIMEOUT_MS = 8_000;
  const MODEL_STREAM_TIMEOUT_MS = 60_000;

  const effect = Effect.gen(function* () {
    const telemetry = yield* TelemetryService;
    const convex = yield* ConvexService;

    const t = telemetry.withNamespace("autopilot.stream");
    // Chunking policy.
    const FLUSH_INTERVAL_MS = 350;
    const FLUSH_MAX_TEXT_CHARS = 1200;
    const FLUSH_MAX_PARTS = 32;

    let status: "final" | "error" | "canceled" = "final";
    let seq = 0;
    let bufferedDelta = "";
    let bufferedParts: Array<{ readonly seq: number; readonly part: unknown }> = [];
    let lastFlushAtMs = Date.now();
    let outputText = "";
    let cancelCheckAtMs = 0;
    let inferenceStartedAtMs: number | null = null;
    let firstTokenAtMs: number | null = null;
    let hasEmittedTextPart = false;

    const materializeDelta = () => {
      if (bufferedDelta.length === 0) return;
      const part: AiResponse.StreamPartEncoded = {
        type: "text-delta",
        id: crypto.randomUUID(),
        delta: bufferedDelta,
        metadata: {},
      };
      bufferedParts.push({ seq: seq++, part });
      bufferedDelta = "";
      hasEmittedTextPart = true;
    };

    const flush = Effect.fn("autopilot.flush")(function* (force: boolean) {
      if (input.controller.signal.aborted) return;

      const now = Date.now();
      const elapsed = now - lastFlushAtMs;

      const shouldFlushNow =
        force ||
        (!hasEmittedTextPart && bufferedDelta.length > 0) ||
        bufferedDelta.length >= FLUSH_MAX_TEXT_CHARS ||
        bufferedParts.length >= FLUSH_MAX_PARTS ||
        elapsed >= FLUSH_INTERVAL_MS;

      if (!shouldFlushNow) return;

      materializeDelta();

      if (bufferedParts.length === 0) return;

      // Check cancellation at most once per flush interval.
      if (now - cancelCheckAtMs >= FLUSH_INTERVAL_MS) {
        cancelCheckAtMs = now;
        const cancel = yield* isCancelRequested({
          convex,
          threadId: input.threadId,
          runId: input.runId,
        }).pipe(
          Effect.timeoutFail({
            duration: `${CONVEX_QUERY_TIMEOUT_MS} millis`,
            onTimeout: () => new Error("convex.isCancelRequested_timeout"),
          }),
          Effect.catchAll(() =>
            Effect.succeed({ ok: true as const, cancelRequested: false } satisfies IsCancelRequestedResult),
          ),
        );
        const cancelResult = cancel as IsCancelRequestedResult;
        if (cancelResult.cancelRequested) {
          input.controller.abort();
          return;
        }
      }

      const batch = bufferedParts;
      bufferedParts = [];
      lastFlushAtMs = now;

      yield* flushPartsToConvex({
        convex,
        threadId: input.threadId,
        runId: input.runId,
        messageId: input.assistantMessageId,
        parts: batch,
      })
        .pipe(
          Effect.timeoutFail({
            duration: `${CONVEX_MUTATION_TIMEOUT_MS} millis`,
            onTimeout: () => new Error("convex.append_timeout"),
          }),
        )
        .pipe(
          Effect.catchAll((err) =>
            t.log("error", "convex.append_failed", { message: err instanceof Error ? err.message : String(err) }),
          ),
      );
    });

    let extraSystemContext: string | null = null;

    const streamProgram = Effect.gen(function* () {
      yield* t.event("run.started", { threadId: input.threadId, runId: input.runId }).pipe(Effect.catchAll(() => Effect.void));

      // Load prompt context from Convex (messages only; omit parts). Owner-only.
      const rawSnapshot = yield* convex
        .query(api.autopilot.messages.getThreadSnapshot, {
          threadId: input.threadId,
          maxMessages: 120,
          maxParts: 0,
        })
        .pipe(
          Effect.timeoutFail({
            duration: `${CONVEX_QUERY_TIMEOUT_MS} millis`,
            onTimeout: () => new Error("convex.getThreadSnapshot_timeout"),
          }),
        );
      const snapshot: GetThreadSnapshotResult = rawSnapshot as GetThreadSnapshotResult;

      const bp = yield* convex
        .query(api.autopilot.blueprint.getBlueprint, {
          threadId: input.threadId,
        })
        .pipe(
          Effect.timeoutFail({
            duration: `${CONVEX_QUERY_TIMEOUT_MS} millis`,
            onTimeout: () => new Error("convex.getBlueprint_timeout"),
          }),
        )
        .pipe(
          Effect.catchAll(() =>
            Effect.succeed({ ok: true as const, blueprint: null, updatedAtMs: 0 } satisfies GetBlueprintResult),
          ),
        );
      const blueprint = (bp as GetBlueprintResult).blueprint as BlueprintHint | null;

      const messagesRaw: ReadonlyArray<ThreadSnapshotMessage> = Array.isArray(snapshot.messages) ? snapshot.messages : [];

      // Bootstrap is a deterministic state machine for MVP: avoid inference until the Blueprint is complete.
      const bootstrapStatus = String(blueprint?.bootstrapState?.status ?? "pending");
      const bootstrapStage = String(blueprint?.bootstrapState?.stage ?? "");
      const lastUserMessage = lastUserMessageFromSnapshot(messagesRaw);
      const lastUserMessageId = lastUserMessage.messageId;
      const lastUserText = lastUserMessage.text;
      const lightningInvocation = parseLightningToolInvocation(lastUserText);

      const promptMessages = messagesRaw
        .filter((m) => m && typeof m === "object")
        .filter((m) => String(m.role ?? "") === "user" || String(m.role ?? "") === "assistant")
        .filter((m) => String(m.status ?? "") !== "streaming" || String(m.messageId ?? "") !== input.assistantMessageId)
        .map((m) => ({ role: String(m.role ?? "user"), text: String(m.text ?? "") }))
        .filter((m) => m.text.trim().length > 0);

      const tail = promptMessages.slice(-MAX_CONTEXT_MESSAGES);

      if (lightningInvocation) {
        const toolName = lightningInvocation.toolName;
        const toolPartId = `dsepart_tool_${input.runId}_${toolName}`;
        const toolCallId = `toolcall_${input.runId}_${toolName}`;

        bufferedParts.push({
          seq: seq++,
          part: {
            type: "dse.tool",
            v: 1,
            id: toolPartId,
            state: "start",
            tsMs: Date.now(),
            toolName,
            toolCallId,
            input: lightningInvocation.rawParams,
          },
        });
        yield* flush(true);

        if (toolName === LIGHTNING_L402_FETCH_TOOL_NAME) {
          const toolExecution = yield* runLightningL402FetchTool({
            env: input.env,
            convex,
            requestId,
            runId: input.runId,
            threadId: input.threadId,
            controller: input.controller,
            rawParams: lightningInvocation.rawParams,
            source: lightningInvocation.source,
            queryTimeoutMs: CONVEX_QUERY_TIMEOUT_MS,
            mutationTimeoutMs: CONVEX_MUTATION_TIMEOUT_MS,
          });
          const terminal = toolExecution.terminal;
          const toolState =
            terminal.status === "queued"
              ? "approval-requested"
              : terminal.status === "completed" || terminal.status === "cached"
                ? "ok"
                : "error";

          bufferedParts.push({
            seq: seq++,
            part: {
              type: "dse.tool",
              v: 1,
              id: toolPartId,
              state: toolState,
              tsMs: Date.now(),
              toolName,
              toolCallId,
              input: toolExecution.validatedInput ?? lightningInvocation.rawParams,
              output: terminal,
              ...(toolState === "error" ? { errorText: terminal.denyReason ?? terminal.status } : {}),
            },
          });

          outputText = terminalTextFromLightningToolResult(terminal);
        } else {
          const toolExecution = yield* runLightningL402ApproveTool({
            convex,
            requestId,
            runId: input.runId,
            threadId: input.threadId,
            controller: input.controller,
            rawParams: lightningInvocation.rawParams,
            source: lightningInvocation.source,
            queryTimeoutMs: CONVEX_QUERY_TIMEOUT_MS,
            mutationTimeoutMs: CONVEX_MUTATION_TIMEOUT_MS,
          });
          const output = toolExecution.output;
          const toolState = output.ok ? "ok" : "error";

          bufferedParts.push({
            seq: seq++,
            part: {
              type: "dse.tool",
              v: 1,
              id: toolPartId,
              state: toolState,
              tsMs: Date.now(),
              toolName,
              toolCallId,
              input: toolExecution.validatedInput ?? lightningInvocation.rawParams,
              output,
              ...(toolState === "error" ? { errorText: output.denyReason ?? "approve_failed" } : {}),
            },
          });

          if (output.ok) {
            const status = output.taskStatus ?? "unknown";
            outputText = `L402 task ${output.taskId ?? "unknown"} approved (${status}).`;
          } else {
            const reason = output.denyReason ?? "unknown";
            outputText = `L402 approval failed. Reason: ${reason}`;
          }
        }

        if (outputText.trim().length > 0) {
          hasEmittedTextPart = true;
          bufferedParts.push({
            seq: seq++,
            part: {
              type: "text-delta",
              id: crypto.randomUUID(),
              delta: outputText,
              metadata: {},
            } as AiResponse.StreamPartEncoded,
          });
        }

        yield* flush(true);
        return;
      }

      const workersAiModel = makeWorkersAiLanguageModel({
        binding: input.env.AI,
        model: MODEL_ID_CF,
        maxOutputTokens: MAX_OUTPUT_TOKENS,
      });
      const openRouterApiKey = typeof input.env.OPENROUTER_API_KEY === "string" && input.env.OPENROUTER_API_KEY.length > 0
        ? input.env.OPENROUTER_API_KEY
        : null;
      const modelLayer = Layer.effect(
        AiLanguageModel.LanguageModel,
        openRouterApiKey
          ? Effect.gen(function* () {
            const fallback = yield* workersAiModel;
            const primary = yield* makeOpenRouterLanguageModel({
              apiKey: openRouterApiKey,
              model: PRIMARY_MODEL_OPENROUTER,
              maxOutputTokens: MAX_OUTPUT_TOKENS,
            });
            return yield* makeFallbackLanguageModel(primary, fallback);
          })
          : workersAiModel
      );
      const finishModelDefaults: {
        readonly modelId: string;
        readonly provider: string;
        readonly modelRoute: string;
        readonly modelFallbackId?: string;
      } = openRouterApiKey
        ? {
          modelId: PRIMARY_MODEL_OPENROUTER,
          provider: "openrouter",
          modelRoute: "openrouter_primary_cf_fallback",
          modelFallbackId: MODEL_ID_CF,
        }
        : {
          modelId: MODEL_ID_CF,
          provider: "cloudflare-workers-ai",
          modelRoute: "cloudflare-workers-ai",
        };

      // Phase D: optional RLM-lite pre-summary for long-context runs.
      yield* Effect.gen(function* () {
        if (input.controller.signal.aborted) return;
        if (bootstrapStatus !== "complete") return;
        if (!openRouterApiKey) return;

        const userText = lastUserText.trim();
        if (!userText) return;

        const olderMessages = promptMessages.slice(0, Math.max(0, promptMessages.length - tail.length));
        const olderText = olderMessages.map((m) => `${m.role}: ${m.text}`).join("\n\n");
        const olderChars = olderText.length;

        const explicit =
          userText.startsWith("/rlm") ||
          /\b(recaps?|summari[sz]e|remind me|what did we (decide|agree)|earlier you said)\b/i.test(userText);
        const highPressure = olderChars >= 20_000 || olderMessages.length >= 40;
        if (!explicit && !highPressure) return;

        const strategyReason = explicit
          ? "explicit_request"
          : `context_pressure olderChars=${olderChars} olderMessages=${olderMessages.length}`;

        const signatureId = dseCatalogSignatures.rlm_summarize_thread.id;
        const partId = `dsepart_sig_${input.runId}_rlm_summarize_thread`;

        bufferedParts.push({
          seq: seq++,
          part: {
            type: "dse.signature",
            v: 1,
            id: partId,
            state: "start",
            tsMs: Date.now(),
            signatureId,
            strategyReason,
          },
        });
        yield* flush(true);

        type DseReceiptShape = {
          compiled_id?: string;
          strategyId?: string;
          timing?: { durationMs?: number };
          budget?: unknown;
          receiptId?: string;
          contextPressure?: unknown;
          promptRenderStats?: unknown;
          rlmTrace?: unknown;
          model?: {
            modelId?: string;
            provider?: string;
            route?: string;
            fallbackModelId?: string;
          };
        };
        let recordedReceipt: DseReceiptShape | null = null;
        const setRecordedReceipt = (r: unknown) => {
          recordedReceipt = r as DseReceiptShape;
        };

        const dseLmClient = makeDseLmClientWithOpenRouterPrimary({
          env: input.env,
          defaultModelIdCf: MODEL_ID_CF,
          primaryModelOpenRouter: PRIMARY_MODEL_OPENROUTER,
        });
        const dseEnv = layerDsePredictEnvForAutopilotRun({
          threadId: input.threadId,
          runId: input.runId,
          onReceipt: setRecordedReceipt,
        });

        const exit = yield* Effect.exit(
          Effect.gen(function* () {
            const blobStore = yield* BlobStore.BlobStoreService;

            // Bound history size so Convex blobs remain reasonable.
            const MAX_HISTORY_CHARS = 200_000;
            const boundedHistory =
              olderText.length > MAX_HISTORY_CHARS ? olderText.slice(olderText.length - MAX_HISTORY_CHARS) : olderText;

            // Pre-chunk history into stable BlobRefs so the controller can use ExtractOverChunks.
            const chunkChars = 20_000;
            const maxChunks = 12;
            const chunkTexts: Array<string> = [];
            for (let i = 0; i < boundedHistory.length && chunkTexts.length < maxChunks; i += chunkChars) {
              chunkTexts.push(boundedHistory.slice(i, Math.min(boundedHistory.length, i + chunkChars)));
            }

            const threadChunks = yield* Effect.forEach(
              chunkTexts,
              (text) => blobStore.putText({ text, mime: "text/plain" }),
              { concurrency: 3, discard: false },
            );

            return yield* predictRlmSummarizeThread({ question: userText, threadChunks });
          }).pipe(Effect.provideService(Lm.LmClientService, dseLmClient), Effect.provide(dseEnv)),
        );

        const state = exit._tag === "Success" ? "ok" : "error";

        const errorText =
          exit._tag === "Failure"
            ? errorMessageFromUnknown(exit.cause, "DSE predict failed")
            : null;

        const receipt = recordedReceipt as DseReceiptShape | null;
        const trimmedPromptRenderStats = trimPromptRenderStats(receipt?.promptRenderStats);

        if (exit._tag === "Success") {
          const summary = summaryFromOutput(exit.value);
          if (summary) {
            const MAX_SUMMARY_CHARS = 1500;
            const bounded = summary.length > MAX_SUMMARY_CHARS ? summary.slice(0, MAX_SUMMARY_CHARS).trim() : summary;
            extraSystemContext = "Prior conversation summary (RLM-lite):\n" + bounded;
          }
        }

        bufferedParts.push({
          seq: seq++,
          part: {
            type: "dse.signature",
            v: 1,
            id: partId,
            state,
            tsMs: Date.now(),
            signatureId,
            compiled_id: receipt?.compiled_id,
            receiptId: receipt?.receiptId,
            model: dseSignatureModelFromUnknown(receipt?.model),
            timing: receipt?.timing,
            budget: receipt?.budget,
            strategyId: receipt?.strategyId,
            strategyReason,
            contextPressure: receipt?.contextPressure,
            promptRenderStats: trimmedPromptRenderStats,
            rlmTrace: receipt?.rlmTrace,
            ...(exit._tag === "Success" ? { outputPreview: exit.value } : {}),
            ...(errorText ? { errorText } : {}),
          },
        });
        yield* flush(true);
      }).pipe(
        Effect.timeoutFail({
          duration: "20 seconds",
          onTimeout: () => new Error("rlm_lite_timeout"),
        }),
        Effect.catchAll((err) =>
          t.log("warn", "rlm_lite_failed", { message: err instanceof Error ? err.message : String(err) }),
        ),
      );

      // Post-bootstrap capability classifier:
      // classify each user message for "missing capability / upgrade request" and persist to Convex when true.
      yield* Effect.gen(function* () {
        if (input.controller.signal.aborted) return;
        if (bootstrapStatus !== "complete") return;
        if (!lastUserText) return;
        if (!looksLikeUpgradeRequestCandidate(lastUserText)) return;

        const signatureId = dseCatalogSignatures.detect_upgrade_request.id;
        const partId = `dsepart_sig_${input.runId}_detect_upgrade_request`;
        const strategyReason = "post_bootstrap_capability_feedback";

        bufferedParts.push({
          seq: seq++,
          part: {
            type: "dse.signature",
            v: 1,
            id: partId,
            state: "start",
            tsMs: Date.now(),
            signatureId,
            strategyReason,
          },
        });
        yield* flush(true);

        type DseReceiptShape = {
          compiled_id?: string;
          strategyId?: string;
          timing?: { durationMs?: number };
          budget?: unknown;
          receiptId?: string;
          contextPressure?: unknown;
          promptRenderStats?: unknown;
          rlmTrace?: unknown;
          model?: {
            modelId?: string;
            provider?: string;
            route?: string;
            fallbackModelId?: string;
          };
        };
        let recordedReceipt: DseReceiptShape | null = null;
        const setRecordedReceipt = (receipt: unknown) => {
          recordedReceipt = receipt as DseReceiptShape;
        };

        const dseLmClient = makeDseLmClientWithOpenRouterPrimary({
          env: input.env,
          defaultModelIdCf: MODEL_ID_CF,
          primaryModelOpenRouter: PRIMARY_MODEL_OPENROUTER,
        });
        const dseEnv = layerDsePredictEnvForAutopilotRun({
          threadId: input.threadId,
          runId: input.runId,
          onReceipt: setRecordedReceipt,
        });

        const predictExit = yield* Effect.exit(
          predictDetectUpgradeRequest({ message: lastUserText }).pipe(
            Effect.timeoutFail({
              duration: "12 seconds",
              onTimeout: () => new Error("dse.detect_upgrade_request_timeout"),
            }),
            Effect.provideService(Lm.LmClientService, dseLmClient),
            Effect.provide(dseEnv),
          ),
        );

        let usedFallbackDecision = false;
        const errorText =
          predictExit._tag === "Failure"
            ? errorMessageFromUnknown(predictExit.cause, "DSE upgrade request detection failed")
            : null;
        let decision =
          predictExit._tag === "Success" ? normalizeUpgradeRequestDecision(predictExit.value) : null;

        if (!decision && predictExit._tag === "Failure" && looksLikeUpgradeRequestCandidate(lastUserText)) {
          decision = fallbackUpgradeRequestDecisionFromMessage(lastUserText);
          usedFallbackDecision = true;
        }
        const state: "ok" | "error" = predictExit._tag === "Success" || usedFallbackDecision ? "ok" : "error";

        const receipt = recordedReceipt as DseReceiptShape | null;
        const trimmedPromptRenderStats = trimPromptRenderStats(receipt?.promptRenderStats);

        let recordResult:
          | {
              readonly featureRequestId: string;
              readonly existed: boolean;
            }
          | null = null;
        let recordErrorText: string | null = null;

        if (decision?.isUpgradeRequest && lastUserMessageId) {
          const recordExit = yield* Effect.exit(
            convex
              .mutation(api.autopilot.featureRequests.recordFeatureRequest, {
                threadId: input.threadId,
                runId: input.runId,
                messageId: lastUserMessageId,
                userText: lastUserText,
                capabilityKey: decision.capabilityKey,
                capabilityLabel: decision.capabilityLabel || decision.capabilityKey,
                summary:
                  decision.summary ||
                  clampString(lastUserText, 200),
                confidence: decision.confidence,
                notifyWhenAvailable: decision.notifyWhenAvailable,
                source: {
                  signatureId,
                  ...(receipt?.compiled_id ? { compiled_id: receipt.compiled_id } : {}),
                  ...(receipt?.receiptId ? { receiptId: receipt.receiptId } : {}),
                  ...(receipt?.model?.modelId ? { modelId: receipt.model.modelId } : {}),
                  ...(receipt?.model?.provider ? { provider: receipt.model.provider } : {}),
                  ...(receipt?.model?.route ? { route: receipt.model.route } : {}),
                  ...(receipt?.model?.fallbackModelId ? { fallbackModelId: receipt.model.fallbackModelId } : {}),
                },
              })
              .pipe(
                Effect.timeoutFail({
                  duration: `${CONVEX_MUTATION_TIMEOUT_MS} millis`,
                  onTimeout: () => new Error("convex.recordFeatureRequest_timeout"),
                }),
              ),
          );

          if (recordExit._tag === "Success") {
            const rec = toRecord(recordExit.value);
            const featureRequestId = readString(rec?.featureRequestId);
            const existed = rec?.existed === true;
            if (featureRequestId) {
              recordResult = { featureRequestId, existed };
              const requestSummary = decision.summary || decision.capabilityLabel || decision.capabilityKey;
              const annotation = `Capability request tracked: ${requestSummary}`;
              extraSystemContext = extraSystemContext
                ? `${extraSystemContext}\n\n${annotation}`
                : annotation;
            } else {
              recordErrorText = "feature_request_id_missing";
            }
          } else {
            recordErrorText = errorMessageFromUnknown(recordExit.cause, "Failed to record feature request");
          }

          const toolPartId = `dsepart_tool_${input.runId}_record_feature_request`;
          bufferedParts.push({
            seq: seq++,
            part: {
              type: "dse.tool",
              v: 1,
              id: toolPartId,
              state: recordErrorText ? "error" : "ok",
              tsMs: Date.now(),
              toolName: "record_feature_request",
              toolCallId: `toolcall_${input.runId}_record_feature_request`,
              input: decision,
              ...(recordResult ? { output: recordResult } : {}),
              ...(recordErrorText ? { errorText: recordErrorText } : {}),
            },
          });
        }

        bufferedParts.push({
          seq: seq++,
          part: {
            type: "dse.signature",
            v: 1,
            id: partId,
            state,
            tsMs: Date.now(),
            signatureId,
            compiled_id: receipt?.compiled_id,
            receiptId: receipt?.receiptId,
            model: dseSignatureModelFromUnknown(receipt?.model),
            timing: receipt?.timing,
            budget: receipt?.budget,
            strategyId: receipt?.strategyId,
            strategyReason,
            contextPressure: receipt?.contextPressure,
            promptRenderStats: trimmedPromptRenderStats,
            rlmTrace: receipt?.rlmTrace,
            fallbackUsed: usedFallbackDecision,
            ...(decision ? { outputPreview: decision } : {}),
            ...(errorText ? { errorText } : {}),
          },
        });
        yield* flush(true);
      });

      const rawPrompt = concatTextFromPromptMessages(tail, blueprint, {
        ...(extraSystemContext ? { extraSystem: extraSystemContext } : {}),
      });
      const prompt = AiPrompt.make(rawPrompt);

      const stream = AiLanguageModel.streamText({
        prompt,
        toolChoice: "none",
        disableToolCallResolution: true,
      });
      inferenceStartedAtMs = Date.now();

      const runStream = Stream.runForEach(stream, (part) =>
        Effect.sync(() => {
          if (input.controller.signal.aborted) return;
          const encoded = encodeStreamPart(part) as AiResponse.StreamPartEncoded;
          if (shouldIgnoreWirePart(encoded)) return;

          if (encoded.type === "text-delta") {
            const delta = String(encoded.delta ?? "");
            if (delta.length > 0 && firstTokenAtMs == null) firstTokenAtMs = Date.now();
            bufferedDelta += delta;
            outputText += delta;
            return;
          }

          // For non-delta parts, materialize any pending delta first to preserve ordering.
          materializeDelta();

          if (encoded.type === "finish") {
            const finishPart = encoded as unknown as Record<string, unknown>;
            const finishedAtMs = Date.now();
            const timeToFirstTokenMs =
              firstTokenAtMs != null && inferenceStartedAtMs != null
                ? Math.max(0, firstTokenAtMs - inferenceStartedAtMs)
                : undefined;
            const timeToCompleteMs =
              inferenceStartedAtMs != null
                ? Math.max(0, finishedAtMs - inferenceStartedAtMs)
                : undefined;
            const normalizedFinish = {
              ...finishPart,
              ...(typeof finishPart.modelId === "string" && finishPart.modelId.trim().length > 0
                ? {}
                : { modelId: finishModelDefaults.modelId }),
              ...(typeof finishPart.provider === "string" && finishPart.provider.trim().length > 0
                ? {}
                : { provider: finishModelDefaults.provider }),
              ...(typeof finishPart.modelRoute === "string" && finishPart.modelRoute.trim().length > 0
                ? {}
                : { modelRoute: finishModelDefaults.modelRoute }),
              ...(typeof finishPart.modelFallbackId === "string" && finishPart.modelFallbackId.trim().length > 0
                ? {}
                : finishModelDefaults.modelFallbackId
                  ? { modelFallbackId: finishModelDefaults.modelFallbackId }
                  : {}),
              ...(typeof timeToFirstTokenMs === "number" ? { timeToFirstTokenMs } : {}),
              ...(typeof timeToCompleteMs === "number" ? { timeToCompleteMs } : {}),
            };
            bufferedParts.push({ seq: seq++, part: normalizedFinish });
            return;
          }

          // Encoded parts are small; we buffer them and flush on the same cadence as text.
          bufferedParts.push({ seq: seq++, part: encoded });
        }).pipe(Effect.zipRight(flush(false))),
      ).pipe(Effect.provide(modelLayer));

      // Best-effort bootstrap persistence: we still run inference so the assistant can answer questions,
      // but we opportunistically persist onboarding answers when they look like answers (not questions).
      if (bootstrapStatus !== "complete" && bootstrapStage) {
        const clamp = (s: string, max: number): string => {
          const t0 = s.trim();
          if (t0.length <= max) return t0;
          return t0.slice(0, max).trim();
        };

        const looksLikeGreeting = (s: string): boolean => {
          const t0 = s.trim().toLowerCase();
          return (
            t0 === "hi" ||
            t0 === "hello" ||
            t0 === "hey" ||
            t0 === "yo" ||
            t0 === "sup" ||
            t0 === "howdy" ||
            t0 === "hiya" ||
            t0 === "hi!" ||
            t0 === "hello!" ||
            t0 === "hey!"
          );
        };

        const looksLikeQuestion = (s: string): boolean => {
          const t0 = s.trim();
          if (!t0) return false;
          if (t0.includes("?")) return true;
          // Guard against "what is this" without punctuation.
          return /^(what|why|how|where|when|who|can|could|should|do|does|is|are)\b/i.test(t0);
        };

        const stripTrailingPunct = (s: string): string => s.replace(/[.!?,;:]+$/g, "").trim();

        const extractHandle = (raw: string): string | null => {
          const t0 = stripTrailingPunct(clamp(raw, 120));
          if (!t0) return null;
          if (looksLikeGreeting(t0)) return null;
          if (looksLikeQuestion(t0)) return null;
          if (t0.includes("\n")) return null;

          const m = t0.match(/\b(?:call me|my name is|i am|i'm|im)\s+(.+)$/i);
          const candidate = stripTrailingPunct(m ? m[1] : t0);
          const words = candidate.split(/\s+/g).filter(Boolean);
          if (words.length === 0) return null;
          if (words.length > 4) return null; // likely a sentence, not a handle
          return clamp(words.join(" "), 64);
        };

        const extractShortValue = (raw: string, max: number): string | null => {
          const t0 = stripTrailingPunct(clamp(raw, max * 2));
          if (!t0) return null;
          if (looksLikeGreeting(t0)) return null;
          if (looksLikeQuestion(t0)) return null;
          return clamp(t0, max);
        };

        type BootstrapApplyResult = { appliedVibe?: string };
        const applyBootstrap = Effect.gen(function* () {
          if (bootstrapStage === "ask_user_handle") {
            const handle = extractHandle(lastUserText);
            if (!handle) return {};
            yield* convex.mutation(api.autopilot.blueprint.applyBootstrapUserHandle, {
              threadId: input.threadId,
              handle,
            });
            return {};
          }

          if (bootstrapStage === "ask_agent_name") {
            const name = extractShortValue(lastUserText, 64);
            if (!name) return {};
            yield* convex.mutation(api.autopilot.blueprint.applyBootstrapAgentName, {
              threadId: input.threadId,
              name,
            });
            return {};
          }

          if (bootstrapStage === "ask_vibe") {
            const vibe = extractShortValue(lastUserText, 140);
            if (!vibe) return {};
            yield* convex.mutation(api.autopilot.blueprint.applyBootstrapAgentVibe, {
              threadId: input.threadId,
              vibe,
            });
            return { appliedVibe: vibe };
          }

          if (bootstrapStage === "ask_boundaries") {
            const t0 = stripTrailingPunct(clamp(lastUserText, 800));
            if (!t0) return {};
            if (looksLikeQuestion(t0)) return {};

            const lowered = t0.toLowerCase();
            const isNone =
              lowered === "none" ||
              lowered === "no" ||
              lowered === "nope" ||
              lowered === "nah" ||
              lowered === "nothing" ||
              lowered === "n/a" ||
              lowered === "na";

            const boundaries = isNone
              ? []
              : t0
                .split(/\n|,|;+/g)
                .map((b) => b.trim())
                .map((b) => (b.startsWith("- ") ? b.slice(2).trim() : b))
                .filter((b) => b.length > 0)
                .slice(0, 16);

            if (!isNone && boundaries.length === 0) return {};

            const completePayload: { threadId: string; boundaries?: string[] } =
              boundaries.length > 0
                ? { threadId: input.threadId, boundaries }
                : { threadId: input.threadId };
            yield* convex.mutation(api.autopilot.blueprint.applyBootstrapComplete, completePayload);
            return {};
          }
          return {};
        }).pipe(
          Effect.catchAllCause((cause) =>
            t.log("warn", "bootstrap.apply_failed", { message: String(cause) }).pipe(
              Effect.as({} as BootstrapApplyResult),
            ),
          ),
        );

        // Persist bootstrap answers before streaming so stage progression is reliable.
        const bootstrapResult = yield* applyBootstrap;

        // When we applied the vibe, inject the exact confirmation message so we don't rely on the model
        // (avoids model outputting "- -" or stopping before "Any boundaries or preferences?").
        if (bootstrapResult.appliedVibe) {
          const injectedText = `Vibe confirmed: ${bootstrapResult.appliedVibe}. Any boundaries or preferences? Reply 'none' or list a few bullets.`;
          if (inferenceStartedAtMs == null) inferenceStartedAtMs = Date.now();
          if (firstTokenAtMs == null) firstTokenAtMs = Date.now();
          outputText = injectedText;
          bufferedParts.push({
            seq: seq++,
            part: {
              type: "text-delta",
              id: crypto.randomUUID(),
              delta: injectedText,
              metadata: {},
            } as AiResponse.StreamPartEncoded,
          });
          yield* flush(true);
        } else {
          yield* runStream.pipe(
            Effect.timeoutFail({
              duration: `${MODEL_STREAM_TIMEOUT_MS} millis`,
              onTimeout: () => new Error("model.stream_timeout"),
            }),
          );
          yield* flush(true);
        }
      } else {
        yield* runStream.pipe(
          Effect.timeoutFail({
            duration: `${MODEL_STREAM_TIMEOUT_MS} millis`,
            onTimeout: () => new Error("model.stream_timeout"),
          }),
        );
        yield* flush(true);
      }
    });

    const exit = yield* Effect.exit(streamProgram);

    if (exit._tag === "Failure") {
      status = input.controller.signal.aborted ? "canceled" : "error";
      const msg = Cause.pretty(exit.cause).trim() || "stream_failed";
      yield* t.log("error", "run.failed", { message: msg }).pipe(Effect.catchAll(() => Effect.void));

      if (outputText.trim().length === 0) {
        outputText = status === "canceled" ? "Canceled." : "Error. Please try again.";
      }

      // Best-effort: append a terminal error part.
      const errorPart: AiResponse.StreamPartEncoded = { type: "error", error: msg, metadata: {} };
      yield* flushPartsToConvex({
        convex,
        threadId: input.threadId,
        runId: input.runId,
        messageId: input.assistantMessageId,
        parts: [{ seq: seq++, part: errorPart }],
      })
        .pipe(
          Effect.timeoutFail({
            duration: `${CONVEX_MUTATION_TIMEOUT_MS} millis`,
            onTimeout: () => new Error("convex.append_error_timeout"),
          }),
        )
        .pipe(Effect.catchAll(() => Effect.void));
    } else if (input.controller.signal.aborted) {
      status = "canceled";
      if (outputText.trim().length === 0) outputText = "Canceled.";
    }

    // Guardrail: never finalize a run with an invisible assistant message.
    // If the model produced no text/tool parts (e.g. only a finish part), fall back to a visible message.
    if (status === "final" && outputText.trim().length === 0) {
      outputText = "No response. Please try again.";
    }

    yield* finalizeRunInConvex({
      convex,
      threadId: input.threadId,
      runId: input.runId,
      messageId: input.assistantMessageId,
      status,
      text: outputText,
    })
      .pipe(
        Effect.timeoutFail({
          duration: `${CONVEX_MUTATION_TIMEOUT_MS} millis`,
          onTimeout: () => new Error("convex.finalize_timeout"),
        }),
      )
      .pipe(Effect.catchAll(() => Effect.void));

    yield* t.event("run.finished", { threadId: input.threadId, runId: input.runId, status }).pipe(
      Effect.catchAll(() => Effect.void),
    );
  }).pipe(
    Effect.provideService(RequestContextService, makeServerRequestContext(input.request)),
    Effect.provideService(TelemetryService, requestTelemetry),
    Effect.catchAll((err) => {
      console.error(`[autopilot.stream] ${formatRequestIdLogToken(requestId)}`, err);
      return Effect.void;
    }),
  );

  return runtime.runPromise(effect).finally(() => {
    activeRuns.delete(input.runId);
  });
};

const resolveAutopilotAdminSecret = (env: WorkerEnv): string | null => {
  const explicit = env.OA_AUTOPILOT_ADMIN_SECRET ?? process.env.OA_AUTOPILOT_ADMIN_SECRET;
  if (typeof explicit === "string" && explicit.length > 0) return explicit;
  const fallback = env.OA_DSE_ADMIN_SECRET ?? process.env.OA_DSE_ADMIN_SECRET;
  return typeof fallback === "string" && fallback.length > 0 ? fallback : null;
};

const isAutopilotAdminSecretAuthorized = (request: Request, env: WorkerEnv): boolean => {
  const secret = resolveAutopilotAdminSecret(env);
  if (!secret) return false;
  const authz = request.headers.get("authorization") ?? "";
  return authz.trim() === `Bearer ${secret}`;
};

const statusFromErrorMessage = (message: string): number => {
  const m = message.toLowerCase();
  if (m.includes("unauthorized") || m.includes("forbidden")) return 401;
  if (m.includes("invalid_input") || m.includes("thread_not_found")) return 400;
  return 500;
};

const parseBoundedInt = (raw: string | null, fallback: number, max: number): number => {
  if (typeof raw !== "string" || raw.trim().length === 0) return fallback;
  const n = Number(raw);
  if (!Number.isFinite(n)) return fallback;
  return Math.max(0, Math.min(max, Math.floor(n)));
};

const makeAutopilotAdminRequest = (request: Request, env: WorkerEnv) =>
  Effect.gen(function* () {
    const privateJwkJson = env.OA_E2E_JWT_PRIVATE_JWK ?? process.env.OA_E2E_JWT_PRIVATE_JWK;
    if (!privateJwkJson) return yield* Effect.fail(new Error("missing_OA_E2E_JWT_PRIVATE_JWK"));

    const token = yield* mintE2eJwt({
      privateJwkJson,
      user: { id: AUTOPILOT_ADMIN_TEST_SUBJECT },
      ttlSeconds: 60 * 15,
    });

    const headers = new Headers(request.headers);
    const existingCookie = headers.get("cookie");
    const adminCookie = `${E2E_COOKIE_NAME}=${encodeURIComponent(token)}`;
    headers.set("cookie", existingCookie && existingCookie.length > 0 ? `${existingCookie}; ${adminCookie}` : adminCookie);

    // Avoid cloning request bodies here; admin endpoints may have already consumed JSON.
    // For auth/session resolution we only need URL + method + headers.
    return new Request(request.url, { method: request.method, headers });
  });

export const handleAutopilotRequest = async (
  request: Request,
  env: WorkerEnv,
  ctx: ExecutionContext,
): Promise<Response | null> => {
  const url = new URL(request.url);
  if (!url.pathname.startsWith("/api/autopilot/")) return null;
  const requestId = request.headers.get(OA_REQUEST_ID_HEADER) ?? "missing";

  if (url.pathname === "/api/autopilot/admin/send") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });
    if (!isAutopilotAdminSecretAuthorized(request, env)) {
      return json({ ok: false, error: "unauthorized" }, { status: 401, headers: { "cache-control": "no-store" } });
    }

    const body = await readJson<AdminSendBody>(request);
    const providedThreadId = typeof body?.threadId === "string" ? body.threadId.trim() : "";
    const text = typeof body?.text === "string" ? body.text : "";
    const resetThread = body?.resetThread === true;

    if (!text.trim()) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    if (!env.AI) {
      return json({ ok: false, error: "ai_unbound" }, { status: 500, headers: { "cache-control": "no-store" } });
    }

    const { runtime } = getWorkerRuntime(env);
    const telemetryBase = runtime.runSync(
      Effect.gen(function* () {
        return yield* TelemetryService;
      }),
    );
    const requestTelemetry = telemetryBase.withFields({
      requestId,
      method: request.method,
      pathname: url.pathname,
    });

    const adminRequestExit = await runtime.runPromiseExit(makeAutopilotAdminRequest(request, env));
    if (adminRequestExit._tag === "Failure") {
      const message = String(adminRequestExit.cause);
      console.error(`[autopilot.admin.send] ${formatRequestIdLogToken(requestId)} admin_request_failed`, message);
      return json({ ok: false, error: message }, { status: statusFromErrorMessage(message), headers: { "cache-control": "no-store" } });
    }
    const adminRequest = adminRequestExit.value;

    const createRunExit = await runtime.runPromiseExit(
      Effect.gen(function* () {
        const convex = yield* ConvexService;
        yield* Effect.flatMap(AuthService, (auth) => auth.getSession()).pipe(Effect.catchAll(() => Effect.void));

        const ensuredRaw = yield* convex.mutation(api.autopilot.threads.ensureOwnedThread, {});
        const ensured = parseEnsureOwnedThreadResult(ensuredRaw);
        if (!ensured) return yield* Effect.fail(new Error("ensure_owned_thread_invalid_shape"));
        const threadId = providedThreadId.length > 0 ? providedThreadId : String(ensured.threadId ?? "");
        if (!threadId) return yield* Effect.fail(new Error("invalid_input"));

        if (resetThread) {
          yield* convex.mutation(api.autopilot.reset.resetThread, { threadId });
        }

        const createdRaw = yield* convex.mutation(api.autopilot.messages.createRun, {
          threadId,
          text,
        });
        const created = parseCreateRunResult(createdRaw);
        if (!created) return yield* Effect.fail(new Error("create_run_invalid_shape"));

        return { threadId, created };
      }).pipe(
        Effect.provideService(RequestContextService, makeServerRequestContext(adminRequest)),
        Effect.provideService(TelemetryService, requestTelemetry),
      ),
    );

    if (createRunExit._tag === "Failure") {
      console.error(`[autopilot.admin.send] ${formatRequestIdLogToken(requestId)} create_run_failed`, createRunExit.cause);
      return json({ ok: false, error: "create_run_failed" }, { status: 500, headers: { "cache-control": "no-store" } });
    }

    const threadId = String(createRunExit.value.threadId ?? "");
    const runId = String(createRunExit.value.created.runId ?? "");
    const userMessageId = String(createRunExit.value.created.userMessageId ?? "");
    const assistantMessageId = String(createRunExit.value.created.assistantMessageId ?? "");

    if (!threadId || !runId || !assistantMessageId) {
      return json({ ok: false, error: "create_run_invalid_shape" }, { status: 500, headers: { "cache-control": "no-store" } });
    }

    const controller = new AbortController();
    activeRuns.set(runId, { controller, startedAtMs: Date.now() });

    const envWithAi = env as WorkerEnv & { readonly AI: Ai };
    ctx.waitUntil(
      runAutopilotStream({
        env: envWithAi,
        request: adminRequest,
        threadId,
        runId,
        assistantMessageId,
        controller,
      }),
    );

    return json(
      {
        ok: true,
        testUserId: AUTOPILOT_ADMIN_TEST_SUBJECT,
        threadId,
        runId,
        userMessageId,
        assistantMessageId,
      },
      { status: 200, headers: { "cache-control": "no-store" } },
    );
  }

  if (url.pathname === "/api/autopilot/admin/reset") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });
    if (!isAutopilotAdminSecretAuthorized(request, env)) {
      return json({ ok: false, error: "unauthorized" }, { status: 401, headers: { "cache-control": "no-store" } });
    }

    const body = await readJson<AdminResetBody>(request);
    const providedThreadId = typeof body?.threadId === "string" ? body.threadId.trim() : "";

    const { runtime } = getWorkerRuntime(env);
    const telemetryBase = runtime.runSync(
      Effect.gen(function* () {
        return yield* TelemetryService;
      }),
    );
    const requestTelemetry = telemetryBase.withFields({
      requestId,
      method: request.method,
      pathname: url.pathname,
    });

    const adminRequestExit = await runtime.runPromiseExit(makeAutopilotAdminRequest(request, env));
    if (adminRequestExit._tag === "Failure") {
      const message = String(adminRequestExit.cause);
      console.error(`[autopilot.admin.reset] ${formatRequestIdLogToken(requestId)} admin_request_failed`, message);
      return json({ ok: false, error: message }, { status: statusFromErrorMessage(message), headers: { "cache-control": "no-store" } });
    }
    const adminRequest = adminRequestExit.value;

    const resetExit = await runtime.runPromiseExit(
      Effect.gen(function* () {
        const convex = yield* ConvexService;
        yield* Effect.flatMap(AuthService, (auth) => auth.getSession()).pipe(Effect.catchAll(() => Effect.void));

        const ensuredRaw = yield* convex.mutation(api.autopilot.threads.ensureOwnedThread, {});
        const ensured = parseEnsureOwnedThreadResult(ensuredRaw);
        if (!ensured) return yield* Effect.fail(new Error("ensure_owned_thread_invalid_shape"));
        const threadId = providedThreadId.length > 0 ? providedThreadId : String(ensured.threadId ?? "");
        if (!threadId) return yield* Effect.fail(new Error("invalid_input"));

        yield* convex.mutation(api.autopilot.reset.resetThread, { threadId });
        return { threadId };
      }).pipe(
        Effect.provideService(RequestContextService, makeServerRequestContext(adminRequest)),
        Effect.provideService(TelemetryService, requestTelemetry),
      ),
    );

    if (resetExit._tag === "Failure") {
      const message = String(resetExit.cause);
      console.error(`[autopilot.admin.reset] ${formatRequestIdLogToken(requestId)} reset_failed`, message);
      return json({ ok: false, error: message }, { status: statusFromErrorMessage(message), headers: { "cache-control": "no-store" } });
    }

    return json(
      {
        ok: true,
        testUserId: AUTOPILOT_ADMIN_TEST_SUBJECT,
        threadId: String(resetExit.value.threadId ?? ""),
      },
      { status: 200, headers: { "cache-control": "no-store" } },
    );
  }

  if (url.pathname === "/api/autopilot/admin/snapshot") {
    if (request.method !== "GET") return new Response("Method not allowed", { status: 405 });
    if (!isAutopilotAdminSecretAuthorized(request, env)) {
      return json({ ok: false, error: "unauthorized" }, { status: 401, headers: { "cache-control": "no-store" } });
    }

    const threadId = url.searchParams.get("threadId")?.trim() ?? "";
    if (!threadId) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    const maxMessages = parseBoundedInt(url.searchParams.get("maxMessages"), 400, 2_000);
    const maxParts = parseBoundedInt(url.searchParams.get("maxParts"), 8_000, 40_000);

    const { runtime } = getWorkerRuntime(env);
    const telemetryBase = runtime.runSync(
      Effect.gen(function* () {
        return yield* TelemetryService;
      }),
    );
    const requestTelemetry = telemetryBase.withFields({
      requestId,
      method: request.method,
      pathname: url.pathname,
    });

    const adminRequestExit = await runtime.runPromiseExit(makeAutopilotAdminRequest(request, env));
    if (adminRequestExit._tag === "Failure") {
      const message = String(adminRequestExit.cause);
      console.error(`[autopilot.admin.snapshot] ${formatRequestIdLogToken(requestId)} admin_request_failed`, message);
      return json({ ok: false, error: message }, { status: statusFromErrorMessage(message), headers: { "cache-control": "no-store" } });
    }
    const adminRequest = adminRequestExit.value;

    const snapshotExit = await runtime.runPromiseExit(
      Effect.gen(function* () {
        const convex = yield* ConvexService;
        yield* Effect.flatMap(AuthService, (auth) => auth.getSession()).pipe(Effect.catchAll(() => Effect.void));
        const snapshot = (yield* convex.query(api.autopilot.messages.getThreadSnapshot, {
          threadId,
          maxMessages,
          maxParts,
        })) as GetThreadSnapshotResult;
        return snapshot;
      }).pipe(
        Effect.provideService(RequestContextService, makeServerRequestContext(adminRequest)),
        Effect.provideService(TelemetryService, requestTelemetry),
      ),
    );

    if (snapshotExit._tag === "Failure") {
      const message = String(snapshotExit.cause);
      console.error(`[autopilot.admin.snapshot] ${formatRequestIdLogToken(requestId)} snapshot_failed`, message);
      return json({ ok: false, error: message }, { status: statusFromErrorMessage(message), headers: { "cache-control": "no-store" } });
    }

    return json(
      {
        ok: true,
        testUserId: AUTOPILOT_ADMIN_TEST_SUBJECT,
        snapshot: snapshotExit.value,
      },
      { status: 200, headers: { "cache-control": "no-store" } },
    );
  }

  if (url.pathname === "/api/autopilot/admin/trace") {
    if (request.method !== "GET") return new Response("Method not allowed", { status: 405 });
    if (!isAutopilotAdminSecretAuthorized(request, env)) {
      return json({ ok: false, error: "unauthorized" }, { status: 401, headers: { "cache-control": "no-store" } });
    }

    const threadId = url.searchParams.get("threadId")?.trim() ?? "";
    if (!threadId) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    const maxMessages = parseBoundedInt(url.searchParams.get("maxMessages"), 400, 2_000);
    const maxParts = parseBoundedInt(url.searchParams.get("maxParts"), 8_000, 40_000);
    const maxRuns = parseBoundedInt(url.searchParams.get("maxRuns"), 200, 2_000);
    const maxReceipts = parseBoundedInt(url.searchParams.get("maxReceipts"), 2_000, 20_000);
    const maxFeatureRequests = parseBoundedInt(url.searchParams.get("maxFeatureRequests"), 500, 5_000);
    const includeDseState = url.searchParams.get("includeDseState") === "1";
    const maxDseRowsPerRun = parseBoundedInt(url.searchParams.get("maxDseRowsPerRun"), 200, 2_000);

    const { runtime } = getWorkerRuntime(env);
    const telemetryBase = runtime.runSync(
      Effect.gen(function* () {
        return yield* TelemetryService;
      }),
    );
    const requestTelemetry = telemetryBase.withFields({
      requestId,
      method: request.method,
      pathname: url.pathname,
    });

    const adminRequestExit = await runtime.runPromiseExit(makeAutopilotAdminRequest(request, env));
    if (adminRequestExit._tag === "Failure") {
      const message = String(adminRequestExit.cause);
      console.error(`[autopilot.admin.trace] ${formatRequestIdLogToken(requestId)} admin_request_failed`, message);
      return json({ ok: false, error: message }, { status: statusFromErrorMessage(message), headers: { "cache-control": "no-store" } });
    }
    const adminRequest = adminRequestExit.value;

    const traceExit = await runtime.runPromiseExit(
      Effect.gen(function* () {
        const convex = yield* ConvexService;
        yield* Effect.flatMap(AuthService, (auth) => auth.getSession()).pipe(Effect.catchAll(() => Effect.void));

        const trace = (yield* convex.query(api.autopilot.traces.getThreadTraceBundle, {
          threadId,
          maxMessages,
          maxParts,
          maxRuns,
          maxReceipts,
          maxFeatureRequests,
          includeDseState,
          maxDseRowsPerRun,
        })) as GetThreadTraceBundleResult;

        return trace;
      }).pipe(
        Effect.provideService(RequestContextService, makeServerRequestContext(adminRequest)),
        Effect.provideService(TelemetryService, requestTelemetry),
      ),
    );

    if (traceExit._tag === "Failure") {
      const message = String(traceExit.cause);
      console.error(`[autopilot.admin.trace] ${formatRequestIdLogToken(requestId)} trace_failed`, message);
      return json({ ok: false, error: message }, { status: statusFromErrorMessage(message), headers: { "cache-control": "no-store" } });
    }

    return json(
      {
        ok: true,
        testUserId: AUTOPILOT_ADMIN_TEST_SUBJECT,
        trace: traceExit.value,
      },
      { status: 200, headers: { "cache-control": "no-store" } },
    );
  }

  if (url.pathname === "/api/autopilot/send") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });

    const body = await readJson<SendBody>(request);
    const threadId = typeof body?.threadId === "string" ? body.threadId : "";
    const text = typeof body?.text === "string" ? body.text : "";

    if (!threadId || !text.trim()) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    if (!env.AI) {
      return json({ ok: false, error: "ai_unbound" }, { status: 500, headers: { "cache-control": "no-store" } });
    }

    const { runtime } = getWorkerRuntime(env);
    const telemetryBase = runtime.runSync(
      Effect.gen(function* () {
        return yield* TelemetryService;
      }),
    );
    const requestTelemetry = telemetryBase.withFields({
      requestId,
      method: request.method,
      pathname: url.pathname,
    });
    const exit = await runtime.runPromiseExit(
      Effect.gen(function* () {
        const convex = yield* ConvexService;
        // Ensure auth is loaded (so ConvexService server client can setAuth token). Owner-only; no anon.
        yield* Effect.flatMap(AuthService, (auth) => auth.getSession()).pipe(Effect.catchAll(() => Effect.void));

        return yield* convex.mutation(api.autopilot.messages.createRun, {
          threadId,
          text,
        });
      }).pipe(
        Effect.provideService(RequestContextService, makeServerRequestContext(request)),
        Effect.provideService(TelemetryService, requestTelemetry),
      ),
    );

    if (exit._tag === "Failure") {
      console.error(`[autopilot.send] ${formatRequestIdLogToken(requestId)} create_run_failed`, exit.cause);
      return json({ ok: false, error: "create_run_failed" }, { status: 500, headers: { "cache-control": "no-store" } });
    }

    const value = parseCreateRunResult(exit.value);
    if (!value) {
      return json({ ok: false, error: "create_run_invalid_shape" }, { status: 500, headers: { "cache-control": "no-store" } });
    }
    const runId = String(value.runId ?? "");
    const assistantMessageId = String(value.assistantMessageId ?? "");

    const controller = new AbortController();
    activeRuns.set(runId, { controller, startedAtMs: Date.now() });

    const envWithAi = env as WorkerEnv & { readonly AI: Ai };
    ctx.waitUntil(
      runAutopilotStream({
        env: envWithAi,
        request,
        threadId,
        runId,
        assistantMessageId,
        controller,
      }),
    );

    return json(
      { ok: true, threadId, runId, assistantMessageId },
      { status: 200, headers: { "cache-control": "no-store" } },
    );
  }

  if (url.pathname === "/api/autopilot/dse/recap") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });

    const body = await readJson<DseRecapBody>(request);
    const threadId = typeof body?.threadId === "string" ? body.threadId : "";
    const strategyIdRaw = typeof body?.strategyId === "string" ? body.strategyId : "direct.v1";
    const budgetProfileRaw = typeof body?.budgetProfile === "string" ? body.budgetProfile : "medium";
    const question = typeof body?.question === "string" && body.question.trim().length > 0 ? body.question.trim() : "Recap this thread.";

    const strategyId: DseStrategyId =
      strategyIdRaw === "rlm_lite.v1" ? "rlm_lite.v1" : "direct.v1";
    const budgetProfile: DseBudgetProfile =
      budgetProfileRaw === "small" || budgetProfileRaw === "medium" || budgetProfileRaw === "long"
        ? (budgetProfileRaw as DseBudgetProfile)
        : "medium";

    if (!threadId) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    if (!env.AI) {
      return json({ ok: false, error: "ai_unbound" }, { status: 500, headers: { "cache-control": "no-store" } });
    }

    const e2eModeHeader = request.headers.get("x-oa-e2e-mode");
    const e2eMode: "stub" | "off" = e2eModeHeader === "stub" ? "stub" : "off";

    const { runtime } = getWorkerRuntime(env);
    const telemetryBase = runtime.runSync(
      Effect.gen(function* () {
        return yield* TelemetryService;
      }),
    );
    const requestTelemetry = telemetryBase.withFields({
      requestId,
      method: request.method,
      pathname: url.pathname,
    });

    const exit = await runtime.runPromiseExit(
      Effect.gen(function* () {
        const convex = yield* ConvexService;
        // Ensure auth is loaded (so ConvexService server client can setAuth token). Owner-only; no anon.
        yield* Effect.flatMap(AuthService, (auth) => auth.getSession()).pipe(Effect.catchAll(() => Effect.void));

        const text = `/dse recap strategy=${strategyId} budget=${budgetProfile}`;
        return yield* convex.mutation(api.autopilot.messages.createRun, { threadId, text });
      }).pipe(
        Effect.provideService(RequestContextService, makeServerRequestContext(request)),
        Effect.provideService(TelemetryService, requestTelemetry),
      ),
    );

    if (exit._tag === "Failure") {
      console.error(`[autopilot.dse_recap] ${formatRequestIdLogToken(requestId)} create_run_failed`, exit.cause);
      return json({ ok: false, error: "create_run_failed" }, { status: 500, headers: { "cache-control": "no-store" } });
    }

    const value = parseCreateRunResult(exit.value);
    if (!value) {
      return json({ ok: false, error: "create_run_invalid_shape" }, { status: 500, headers: { "cache-control": "no-store" } });
    }
    const runId = String(value.runId ?? "");
    const userMessageId = String(value.userMessageId ?? "");
    const assistantMessageId = String(value.assistantMessageId ?? "");

    const controller = new AbortController();
    activeRuns.set(runId, { controller, startedAtMs: Date.now() });

    const envWithAi = env as WorkerEnv & { readonly AI: Ai };
    ctx.waitUntil(
      runDseCanaryRecap({
        env: envWithAi,
        request,
        threadId,
        runId,
        userMessageId,
        assistantMessageId,
        controller,
        strategyId,
        budgetProfile,
        question,
        e2eMode,
      }),
    );

    return json(
      { ok: true, threadId, runId, assistantMessageId },
      { status: 200, headers: { "cache-control": "no-store" } },
    );
  }

  if (url.pathname === "/api/autopilot/cancel") {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405 });

    const body = await readJson<CancelBody>(request);
    const threadId = typeof body?.threadId === "string" ? body.threadId : "";
    const runId = typeof body?.runId === "string" ? body.runId : "";
    if (!threadId || !runId) {
      return json({ ok: false, error: "invalid_input" }, { status: 400, headers: { "cache-control": "no-store" } });
    }

    // Best-effort abort (in-isolate).
    activeRuns.get(runId)?.controller.abort();

    const { runtime } = getWorkerRuntime(env);
    const telemetryBase = runtime.runSync(
      Effect.gen(function* () {
        return yield* TelemetryService;
      }),
    );
    const requestTelemetry = telemetryBase.withFields({
      requestId,
      method: request.method,
      pathname: url.pathname,
    });
    const exit = await runtime.runPromiseExit(
      Effect.gen(function* () {
        const convex = yield* ConvexService;
        yield* Effect.flatMap(AuthService, (auth) => auth.getSession()).pipe(Effect.catchAll(() => Effect.void));
        return yield* convex.mutation(api.autopilot.messages.requestCancel, {
          threadId,
          runId,
        });
      }).pipe(
        Effect.provideService(RequestContextService, makeServerRequestContext(request)),
        Effect.provideService(TelemetryService, requestTelemetry),
      ),
    );

    if (exit._tag === "Failure") {
      console.error(`[autopilot.cancel] ${formatRequestIdLogToken(requestId)} cancel_failed`, exit.cause);
      return json({ ok: false, error: "cancel_failed" }, { status: 500, headers: { "cache-control": "no-store" } });
    }

    return json({ ok: true }, { status: 200, headers: { "cache-control": "no-store" } });
  }

  return new Response("Not found", { status: 404, headers: { "cache-control": "no-store" } });
};
