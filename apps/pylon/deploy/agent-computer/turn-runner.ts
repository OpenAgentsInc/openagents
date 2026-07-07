#!/usr/bin/env bun
/**
 * Agent Computer turn-runner (issue #8503).
 *
 * A bounded, self-contained runtime that runs ONE coding turn inside a
 * Firecracker microVM using the real #8475 workspace-materializer to check out
 * a public repo at a pinned commit, then performs a real coding step and emits
 * Khala-shaped runtime events + a result bundle.
 *
 * Two phases:
 *   1. DETERMINISTIC (always): intent -> real repo checkout -> real coding step
 *      (file edit + staged git diff) -> events -> lifecycle result. Proven live
 *      inside the microVM (TURN-PROOF-RESULT: PASS 2026-07-07).
 *   2. MODEL TURN (only when the work-context carries an `inference` block):
 *      makes ONE hosted-inference call to the Khala gateway
 *      (`POST /v1/chat/completions`) authenticated by a programmatic AGENT
 *      token, then posts the EXACT model-token usage receipt to
 *      `POST /api/khala/cloud/runtime-turn-usage`. This mirrors, byte-for-byte,
 *      the two proven halves the org_cloud supervisor already runs in a plain
 *      process (`runWithHostedKhalaGateway` + `recordRuntimeTurnUsageReceipt`
 *      in `apps/pylon/src/orchestration/`), but here they run INSIDE the
 *      provisioned microVM under the agent's own bearer, which is the #8503
 *      DoD's "exact model-token receipt from inside the microVM" requirement.
 *
 * The agent token NEVER appears in emitted events, the result bundle, or logs.
 *
 * Input (argv[2] = path to a work-context JSON, or stdin):
 *   {
 *     "workContextRef": "work-context.<...>",
 *     "threadRef": "thread.<...>",           // Khala Sync thread id (scope.thread.<id>)
 *     "turnId": "turn-1",
 *     "repo": "owner/name",
 *     "commit": "<40-hex>",
 *     "branch": "main",
 *     "objective": "short public-safe objective",
 *     "inference": {                          // OPTIONAL — enables phase 2
 *       "baseUrl": "https://<khala gateway>", // monolith base (staging for the DoD)
 *       "agentToken": "<programmatic agent bearer>",
 *       "ownerUserId": "github:<id>",          // the mobile user's owner id
 *       "model": "gemini-2.5-flash",
 *       "lane": "hosted_khala",                // ingest-accepted lane
 *       "provider": "vertex-gemini",           // optional; matches HOSTED_KHALA_RUNTIME_PROVIDER
 *       "backendProfile": "omega-hosted-gemini",
 *       "pylonRef": "pylon.agent-computer.<...>"
 *     }
 *   }
 *
 * Output: newline-delimited runtime events on stdout; a result bundle written to
 * /qa/artifacts/result.json (copied out by the host provisioner).
 */
import { mkdir, writeFile, readdir } from 'node:fs/promises'
import { execFileSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { randomUUID } from 'node:crypto'
import {
  materializeGitCheckoutWorkspace,
  type GitCheckoutWorkspace,
} from '../../src/workspace-materializer.js'

const ARTIFACT_DIR = process.env.OA_ARTIFACT_DIR ?? '/qa/artifacts'
const CACHE_ROOT = process.env.OA_CACHE_ROOT ?? '/root/.agent-computer/turns'

// The exact ingest contract this runtime posts to. Canonical source of truth:
// `apps/openagents.com/workers/api/src/khala-cloud-runtime-usage-routes.ts`
// (route) and `apps/pylon/src/orchestration/runtime-turn-usage-receipts.ts`
// (client). Kept as literals here so the turn-runner bundles cleanly with no
// server/Effect deps inside the microVM.
export const KHALA_CLOUD_RUNTIME_USAGE_INGEST_PATH =
  '/api/khala/cloud/runtime-turn-usage'
export const KHALA_CLOUD_RUNTIME_USAGE_SCHEMA_VERSION =
  'openagents.khala_cloud_runtime_turn_usage.v1'
/** `runWithHostedKhalaGateway`'s HOSTED_KHALA_RUNTIME_PROVIDER. */
export const AGENT_COMPUTER_DEFAULT_PROVIDER = 'vertex-gemini'
/** The ingest route only accepts these lanes; the hosted-Khala model turn is `hosted_khala`. */
export const AGENT_COMPUTER_RECEIPT_LANE = 'hosted_khala'

/**
 * SINGLE-CHARGE HEADER (#8503 owner decision). The microVM's internal
 * `/v1/chat/completions` call is org capacity, NOT a customer-billable request:
 * the SINGLE authoritative customer debit + public served-token row for a
 * dispatched turn is the `/api/khala/cloud/runtime-turn-usage` receipt
 * (attributed to the mobile ownerUserId). Presenting this header with the value
 * that matches the gateway's `OA_CLOUD_RUNTIME_NO_METER_SECRET` suppresses the
 * gateway's OWN metering hook + served-token recorder for THAT request, so
 * exactly ONE `token_usage_events` row results per turn (the receipt's).
 * Fail-closed: with no secret configured on the gateway (prod default) the
 * header is ignored and the gateway meters normally.
 */
export const INFERENCE_ORG_CLOUD_RUNTIME_NO_METER_HEADER =
  'x-openagents-org-cloud-runtime-no-meter'

export type InferenceConfig = {
  baseUrl: string
  agentToken: string
  ownerUserId: string
  model: string
  lane?: string
  provider?: string
  backendProfile?: string
  pylonRef?: string
  /**
   * Shared secret that suppresses the gateway's own metering for THIS internal
   * org-capacity call (single-charge invariant). Sent as
   * `INFERENCE_ORG_CLOUD_RUNTIME_NO_METER_HEADER`; never serialized elsewhere.
   */
  noMeterSecret?: string
}

type WorkContext = {
  workContextRef: string
  threadRef?: string
  turnId?: string
  repo: string
  commit: string
  branch?: string
  objective?: string
  inference?: InferenceConfig
}

const nowIso = () => new Date().toISOString()
const integerOrZero = (value: unknown): number =>
  typeof value === 'number' && Number.isFinite(value)
    ? Math.max(0, Math.trunc(value))
    : 0

export type ChatCompletionUsage = {
  inputTokens: number
  outputTokens: number
  totalTokens: number
  reasoningTokens: number
  cacheReadTokens: number
}

/**
 * OpenAI-shaped usage extraction, mirroring the org-cloud supervisor's
 * `openAiChatCompletionUsage`, now EXACT for thinking + cached models (#8503
 * money gate). Extracts `completion_tokens_details.reasoning_tokens` and
 * `prompt_tokens_details.cached_tokens` instead of the prior hardcoded zeros
 * (which stamped `usage_truth=exact` while dropping reasoning attribution).
 *
 * Reasoning handling — the ingest route bills `outputTokens + reasoningTokens`
 * (`khalaCloudRuntimeUsageTokenCounts`). OpenAI's contract is that
 * `reasoning_tokens` is a SUBSET of `completion_tokens` (folded), so to keep the
 * BILLABLE total exact (= completion_tokens) we return the NON-reasoning
 * remainder as `outputTokens` and carry `reasoning_tokens` separately; the route
 * re-sums them to `completion_tokens`. If a provider ever reports reasoning
 * OUTSIDE `completion_tokens` (reasoning > completion), we carry it additively
 * instead of clamping, so no tokens are lost.
 *
 * Cache handling — `cached_tokens` is a SUBSET of `prompt_tokens` (input); the
 * ledger records it as a separate `cache_read_tokens` column and does NOT
 * subtract it from input, so we simply carry it (clamped to input).
 */
export const chatCompletionUsage = (body: unknown): ChatCompletionUsage => {
  const usage = (body as { usage?: Record<string, unknown> } | undefined)?.usage
  const inputTokens = integerOrZero(usage?.prompt_tokens ?? usage?.input_tokens)
  const completionTokens = integerOrZero(
    usage?.completion_tokens ?? usage?.output_tokens,
  )
  const completionDetails = usage?.completion_tokens_details as
    | Record<string, unknown>
    | undefined
  const promptDetails = usage?.prompt_tokens_details as
    | Record<string, unknown>
    | undefined
  const reasoningTokens = integerOrZero(completionDetails?.reasoning_tokens)
  const cacheReadTokens = Math.min(
    inputTokens,
    integerOrZero(promptDetails?.cached_tokens),
  )
  // Folded (OpenAI invariant): reasoning ⊆ completion. Split it out so the
  // route's `output + reasoning` re-sums to completion_tokens (billable
  // unchanged). Unfolded (spec-violating): carry reasoning additively.
  const folded = reasoningTokens <= completionTokens
  const outputTokens = folded
    ? completionTokens - reasoningTokens
    : completionTokens
  const billableOutput = outputTokens + reasoningTokens
  return {
    inputTokens,
    outputTokens,
    reasoningTokens,
    cacheReadTokens,
    totalTokens: Math.max(
      inputTokens + billableOutput,
      integerOrZero(usage?.total_tokens),
    ),
  }
}

/** First assistant message text, mirroring `openAiChatCompletionText`. */
export const chatCompletionText = (body: unknown): string => {
  const choices = (body as { choices?: unknown } | undefined)?.choices
  const first = Array.isArray(choices) ? choices[0] : undefined
  const content = (first as { message?: { content?: unknown } } | undefined)
    ?.message?.content
  return typeof content === 'string' ? content : ''
}

/**
 * The chat/completions request body + headers, byte-for-byte the shape
 * `runWithHostedKhalaGateway` sends (so the gateway routes this microVM turn
 * exactly like an org_cloud supervisor turn).
 */
export const chatCompletionsRequest = (
  inference: InferenceConfig,
  instructions: string,
): { url: string; headers: Record<string, string>; body: string } => ({
  url: new URL('/v1/chat/completions', inference.baseUrl).toString(),
  headers: {
    Authorization: `Bearer ${inference.agentToken}`,
    'content-type': 'application/json',
    'x-openagents-client': 'khala-code-mobile',
    'x-openagents-demand-kind': 'external',
    'x-openagents-demand-source': 'khala_mobile_org_cloud_runtime',
    // SINGLE-CHARGE (#8503): suppress the gateway's own metering for this
    // internal org-capacity call so only the receipt records the one billable
    // row. Omitted when unset => gateway meters normally (fail-closed).
    ...(inference.noMeterSecret === undefined
      ? {}
      : { [INFERENCE_ORG_CLOUD_RUNTIME_NO_METER_HEADER]: inference.noMeterSecret }),
  },
  body: JSON.stringify({
    messages: [{ content: instructions, role: 'user' }],
    model: inference.model,
    stream: false,
  }),
})

/**
 * The exact `/api/khala/cloud/runtime-turn-usage` ingest body. Matches
 * `KhalaCloudRuntimeUsageIngestBody` + `recordRuntimeTurnUsageReceipt`.
 */
export const usageIngestBody = (input: {
  inference: InferenceConfig
  threadId: string
  turnId: string
  observedAt: string
  usage: ChatCompletionUsage
  usageRef: string
  runtimeEventId?: string
}): Record<string, unknown> => ({
  schemaVersion: KHALA_CLOUD_RUNTIME_USAGE_SCHEMA_VERSION,
  ownerUserId: input.inference.ownerUserId,
  threadId: input.threadId,
  turnId: input.turnId,
  lane: input.inference.lane ?? AGENT_COMPUTER_RECEIPT_LANE,
  provider: input.inference.provider ?? AGENT_COMPUTER_DEFAULT_PROVIDER,
  model: input.inference.model,
  ...(input.inference.backendProfile === undefined
    ? {}
    : { backendProfile: input.inference.backendProfile }),
  observedAt: input.observedAt,
  ...(input.inference.pylonRef === undefined
    ? {}
    : { pylonRef: input.inference.pylonRef }),
  ...(input.runtimeEventId === undefined
    ? {}
    : { runtimeEventId: input.runtimeEventId }),
  usage: {
    usageRef: input.usageRef,
    inputTokens: integerOrZero(input.usage.inputTokens),
    // EXACT reasoning/cache (#8503): `outputTokens` is the non-reasoning
    // remainder and `reasoningTokens` is carried separately, because the ingest
    // route bills `output + reasoning`. See `chatCompletionUsage`.
    outputTokens: integerOrZero(input.usage.outputTokens),
    reasoningTokens: integerOrZero(input.usage.reasoningTokens),
    cacheReadInputTokens: integerOrZero(input.usage.cacheReadTokens),
    cacheWriteInputTokens: 0,
    totalTokens: integerOrZero(input.usage.totalTokens),
  },
})

export type ModelTurnReceipt =
  | {
      ok: true
      text: string
      usage: ChatCompletionUsage
      usageRef: string
      tokenUsageEventRef: string | null
      insertedTokenUsage: boolean
      tokensServedDelta: number
    }
  | {
      ok: false
      stage: 'inference' | 'no_exact_usage' | 'usage_receipt'
      error: string
      status: number | null
    }

/**
 * Phase-2 model turn: ONE hosted-inference call under the agent bearer, then
 * the EXACT usage receipt post. Fail-soft and token-safe — never throws (so the
 * deterministic checkout proof still completes), never returns/logs the token.
 */
export const runModelTurnReceipt = async (
  args: {
    inference: InferenceConfig
    threadId: string
    turnId: string
    instructions: string
    runtimeEventId?: string
    nowIsoImpl?: () => string
  },
  fetchImpl: typeof globalThis.fetch = globalThis.fetch,
): Promise<ModelTurnReceipt> => {
  const now = args.nowIsoImpl ?? nowIso
  const observedAt = now()

  // 1. Hosted inference.
  const req = chatCompletionsRequest(args.inference, args.instructions)
  let inferenceResponse: Response
  try {
    inferenceResponse = await fetchImpl(req.url, {
      method: 'POST',
      headers: req.headers,
      body: req.body,
    })
  } catch (error) {
    return {
      ok: false,
      stage: 'inference',
      error: error instanceof Error ? error.message : String(error),
      status: null,
    }
  }
  if (!inferenceResponse.ok) {
    return {
      ok: false,
      stage: 'inference',
      error: `hosted gateway returned HTTP ${inferenceResponse.status}`,
      status: inferenceResponse.status,
    }
  }
  const inferenceBody = (await inferenceResponse.json()) as unknown
  const usage = chatCompletionUsage(inferenceBody)
  const text = chatCompletionText(inferenceBody)
  if (usage.inputTokens + usage.outputTokens <= 0) {
    return {
      ok: false,
      stage: 'no_exact_usage',
      error: 'hosted gateway response did not include exact token usage',
      status: inferenceResponse.status,
    }
  }

  // 2. Exact usage receipt.
  const usageRef = `usage.hosted_khala.${randomUUID()}`
  const body = usageIngestBody({
    inference: args.inference,
    threadId: args.threadId,
    turnId: args.turnId,
    observedAt,
    usage,
    usageRef,
    ...(args.runtimeEventId === undefined
      ? {}
      : { runtimeEventId: args.runtimeEventId }),
  })
  let receiptResponse: Response
  try {
    receiptResponse = await fetchImpl(
      new URL(KHALA_CLOUD_RUNTIME_USAGE_INGEST_PATH, args.inference.baseUrl).toString(),
      {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${args.inference.agentToken}`,
          'content-type': 'application/json',
        },
        body: JSON.stringify(body),
      },
    )
  } catch (error) {
    return {
      ok: false,
      stage: 'usage_receipt',
      error: error instanceof Error ? error.message : String(error),
      status: null,
    }
  }
  const receiptJson = (await receiptResponse.json().catch(() => ({}))) as {
    tokenUsageEventRef?: unknown
    insertedTokenUsage?: unknown
    tokensServedDelta?: unknown
    reason?: unknown
  }
  if (!receiptResponse.ok) {
    return {
      ok: false,
      stage: 'usage_receipt',
      error:
        typeof receiptJson.reason === 'string'
          ? receiptJson.reason
          : `usage ingest returned HTTP ${receiptResponse.status}`,
      status: receiptResponse.status,
    }
  }
  return {
    ok: true,
    text,
    usage,
    usageRef,
    tokenUsageEventRef:
      typeof receiptJson.tokenUsageEventRef === 'string'
        ? receiptJson.tokenUsageEventRef
        : null,
    insertedTokenUsage: receiptJson.insertedTokenUsage === true,
    tokensServedDelta:
      typeof receiptJson.tokensServedDelta === 'number'
        ? receiptJson.tokensServedDelta
        : 0,
  }
}

const git = (cwd: string, args: string[]): string => {
  try {
    return execFileSync('git', args, { cwd, encoding: 'utf8', maxBuffer: 32 * 1024 * 1024 }).trim()
  } catch (error) {
    const e = error as { stdout?: string; stderr?: string }
    return `${e.stdout ?? ''}${e.stderr ?? ''}`.trim()
  }
}

async function main() {
  const events: unknown[] = []
  const emit = (event: Record<string, unknown>) => {
    const full = { schema: 'openagents.khala_runtime_event.v1', at: nowIso(), ...event }
    events.push(full)
    process.stdout.write(`${JSON.stringify(full)}\n`)
  }

  const argPath = process.argv[2]
  const raw = argPath ? readFileSync(argPath, 'utf8') : readFileSync(0, 'utf8')
  const wc = JSON.parse(raw) as WorkContext
  if (!wc.repo || !wc.commit || !wc.workContextRef) {
    throw new Error('work context requires repo, commit, workContextRef')
  }
  const branch = wc.branch ?? 'main'
  const turnId = wc.turnId ?? 'turn-1'
  const threadRef = wc.threadRef

  emit({
    kind: 'turn.started',
    turnId,
    workContextRef: wc.workContextRef,
    threadRef,
    objective: wc.objective ?? `checkout ${wc.repo}@${wc.commit.slice(0, 12)}`,
  })

  // 1. Real repo checkout via the #8475 materializer (unauthenticated depth-1
  //    clone + detached checkout of the pinned commit; public repo, no broker).
  emit({ kind: 'tool.call', turnId, tool: 'workspace.checkout', repo: wc.repo, commit: wc.commit })
  const checkout: GitCheckoutWorkspace = {
    kind: 'git_checkout',
    repository: {
      provider: 'github',
      fullName: wc.repo,
      commitSha: wc.commit,
      branch,
      visibility: 'public',
    },
    verificationCommand: { commandRef: 'verify.agent-computer.turn', args: ['git', 'status'] },
  }
  const ws = await materializeGitCheckoutWorkspace({
    cacheRoot: CACHE_ROOT,
    checkout,
    leaseRef: `lease.${wc.workContextRef}.${turnId}`,
    refPrefix: 'workspace.agent-computer',
  })
  const head = git(ws.workingDirectory, ['rev-parse', 'HEAD'])
  const subject = git(ws.workingDirectory, ['log', '-1', '--format=%s'])
  const entries = await readdir(ws.workingDirectory)
  emit({
    kind: 'tool.result',
    turnId,
    tool: 'workspace.checkout',
    headCommit: head,
    headSubject: subject,
    topLevelEntries: entries.length,
    workspaceRef: ws.workspaceRef,
  })

  // 2. Real coding step (deterministic, no model): add a proof note file and
  //    produce a genuine staged git diff — real repo mutation + real git.
  const proofPath = join(ws.workingDirectory, 'AGENT_COMPUTER_TURN.md')
  const proofBody =
    `# Agent Computer turn proof\n\n` +
    `- workContextRef: ${wc.workContextRef}\n` +
    `- turnId: ${turnId}\n` +
    `- repo: ${wc.repo}\n` +
    `- baseCommit: ${head}\n` +
    `- ranAt: ${nowIso()}\n` +
    `- host: firecracker microVM (OpenAgents Agent Computer)\n`
  await writeFile(proofPath, proofBody)
  git(ws.workingDirectory, ['add', 'AGENT_COMPUTER_TURN.md'])
  const diff = git(ws.workingDirectory, ['diff', '--cached', '--stat'])
  const diffFull = git(ws.workingDirectory, ['diff', '--cached'])
  emit({ kind: 'text.completed', turnId, text: `Checked out ${wc.repo}@${head.slice(0, 12)} and staged a 1-file change.\n${diff}` })

  // 3. Phase-2 model turn + exact usage receipt (only when configured).
  let modelTokenReceipt: ModelTurnReceipt | null = null
  if (wc.inference && threadRef) {
    const runtimeEventId = randomUUID()
    const instructions =
      `You are the OpenAgents Agent Computer coding runtime. In one sentence, ` +
      `describe the change staged in ${wc.repo}@${head.slice(0, 12)}: a new ` +
      `AGENT_COMPUTER_TURN.md proof note. Objective: ${wc.objective ?? 'agent-computer turn'}.`
    emit({ kind: 'tool.call', turnId, tool: 'inference.chat_completions', model: wc.inference.model })
    modelTokenReceipt = await runModelTurnReceipt({
      inference: wc.inference,
      threadId: threadRef,
      turnId,
      instructions,
      runtimeEventId,
    })
    if (modelTokenReceipt.ok) {
      emit({
        kind: 'usage.recorded',
        turnId,
        eventId: runtimeEventId,
        provider: wc.inference.provider ?? AGENT_COMPUTER_DEFAULT_PROVIDER,
        model: wc.inference.model,
        usage: modelTokenReceipt.usage,
        usageRef: modelTokenReceipt.usageRef,
        tokenUsageEventRef: modelTokenReceipt.tokenUsageEventRef,
        insertedTokenUsage: modelTokenReceipt.insertedTokenUsage,
      })
      emit({ kind: 'text.completed', turnId, text: modelTokenReceipt.text })
    } else {
      emit({
        kind: 'tool.result',
        turnId,
        tool: 'inference.chat_completions',
        status: 'failed',
        stage: modelTokenReceipt.stage,
        httpStatus: modelTokenReceipt.status,
      })
    }
  }

  // 4. Result bundle (copied out by the host provisioner). Token-safe: the
  //    agent bearer is never serialized here.
  await mkdir(ARTIFACT_DIR, { recursive: true })
  const result = {
    schemaVersion: 'openagents.agent_computer.turn_result.v1',
    workContextRef: wc.workContextRef,
    threadRef: threadRef ?? null,
    turnId,
    repo: wc.repo,
    baseCommit: head,
    headSubject: subject,
    stagedDiffStat: diff,
    stagedDiffBytes: diffFull.length,
    model: wc.inference?.model ?? null,
    modelTokenReceipt:
      modelTokenReceipt === null
        ? null
        : modelTokenReceipt.ok
          ? {
              ok: true,
              usage: modelTokenReceipt.usage,
              usageRef: modelTokenReceipt.usageRef,
              tokenUsageEventRef: modelTokenReceipt.tokenUsageEventRef,
              insertedTokenUsage: modelTokenReceipt.insertedTokenUsage,
              tokensServedDelta: modelTokenReceipt.tokensServedDelta,
            }
          : {
              ok: false,
              stage: modelTokenReceipt.stage,
              error: modelTokenReceipt.error,
              status: modelTokenReceipt.status,
            },
    modelTokenReceiptNote:
      wc.inference === undefined
        ? 'no hosted model invoked in this proof; supply a work-context `inference` block (agent token + gateway base url + model) to mint an exact model-token receipt from inside the microVM'
        : 'hosted model invoked under the agent bearer; see modelTokenReceipt for the exact token_usage_events outcome',
    ranAt: nowIso(),
    events,
  }
  await writeFile(join(ARTIFACT_DIR, 'result.json'), JSON.stringify(result, null, 2))
  await writeFile(join(ARTIFACT_DIR, 'staged.diff'), diffFull)
  emit({ kind: 'turn.finished', turnId, status: 'completed', artifactDir: ARTIFACT_DIR })
}

if (import.meta.main) {
  main().catch((error) => {
    const full = {
      schema: 'openagents.khala_runtime_event.v1',
      at: nowIso(),
      kind: 'turn.finished',
      turnId: 'turn-1',
      status: 'failed',
      error: String(error),
    }
    process.stdout.write(`${JSON.stringify(full)}\n`)
    process.exit(1)
  })
}
