/**
 * Codex local runtime (EP250 #8712 — owner mandate verbatim: "yeah i need
 * codex and claude both first class"): a top-level conversation turn on the
 * composer's Codex chip in local mode, mirroring fable-local-runtime's shape.
 *
 * Spawn recipe (receipted, codex-cli 0.144.1, 2026-07-11 — the child recipe
 * WITHOUT --ephemeral):
 *
 *   codex exec --json -m gpt-5.6-sol -c model_reasoning_effort=medium \
 *     -s danger-full-access --skip-git-repo-check -C <thread workspace> "<prompt>"
 *
 * DECISION (receipted): the chat lane does NOT pass `--ephemeral`, so codex
 * persists the session rollout JSONL under the isolated account home
 * (`<home>/sessions/YYYY/MM/DD/rollout-…-<thread_id>.jsonl` — live receipt on
 * codex-5) and the thread can be RESUMED. Delegate children keep --ephemeral
 * (bounded one-shot workers need no receipts/resume).
 *
 * MULTI-TURN (receipted): `codex exec resume <thread_id> --json …` resumes
 * the same thread_id with full context (live receipt: codeword ZEBRA-42
 * recalled on resume, same thread_id echoed in thread.started). `exec
 * resume` has NO `-s`/`-C` flags (its --help, receipted), so the sandbox is
 * pinned via `-c sandbox_mode="danger-full-access"` and the cwd comes from
 * the resumed session's recorded working root (the thread workspace from
 * turn 1) plus the spawn cwd. A thread's codex session is pinned to the
 * account that created it; when rotation lands on a DIFFERENT account, the
 * lane falls back to bounded-history prepend (same policy as fable).
 *
 * Rotation mirrors codex-child-runtime (same shared health memory, same
 * classifier): auth-class pre-content failures demote the account and rotate
 * VISIBLY (typed lane_notice event); other pre-content failures rotate
 * without demotion; post-content failures and timeouts are terminal. The
 * account order is health-ordered, which puts PROBE-VERIFIED accounts first
 * (the preflight records probe successes into the same health memory).
 *
 * Events reuse the FROZEN fable-local envelope (see codex-local-contract.ts)
 * so the existing renderer cards render codex turns identically:
 *   agent_message → text_delta; command_execution/mcp_tool_call/file_change/
 *   web_search → tool_use/tool_result cards; reasoning → reasoning lines;
 *   turn.completed usage → turn_completed (exact split, cached separate).
 *
 * This module never imports `electron` (unit-testable under `bun test`).
 */
import { mkdirSync, writeFileSync } from "node:fs"
import { join } from "node:path"

import {
  hashPylonAccountRef,
  pylonAccountEnvironment,
  type ResolvedPylonAccountSelection,
} from "@openagentsinc/pylon-core/custody/account-registry"

import {
  CODEX_CHILD_SANDBOX,
  CODEX_CHILD_SUMMARY_LIMIT,
  codexChildUsageFromTurnCompleted,
  isCodexQuotaExhaustionText,
  isCodexRateLimitText,
  isCodexReconnectRequiredText,
  type CodexChildUsage,
} from "./codex-child-contract.ts"
import {
  defaultSpawnCodex,
  discoverRegisteredCodexAccounts,
  makeCodexJsonLineConsumer,
  redactChildText,
  sharedCodexAccountHealth,
  type ChildLike,
  type CodexAccountHealth,
  type CodexChildAccount,
  type CodexChildSpawn,
} from "./codex-child-runtime.ts"
import {
  CODEX_LOCAL_MODEL,
  CODEX_LOCAL_REASONING_EFFORT,
  type CodexLocalAvailability,
} from "./codex-local-contract.ts"
import {
  FABLE_LOCAL_DELTA_LIMIT,
  FABLE_LOCAL_FINAL_TEXT_LIMIT,
  FABLE_LOCAL_FOLLOWUP_MESSAGE_LIMIT,
  FABLE_LOCAL_SUMMARY_LIMIT,
  type FableChildUsage,
  type FableLocalEvent,
  type FableLocalFailureReason,
  type FableLocalImageAttachment,
  type FableLocalQueueFollowupRequest,
} from "./fable-local-contract.ts"
import {
  FABLE_LOCAL_HISTORY_MESSAGES,
  FABLE_LOCAL_HISTORY_MESSAGE_LIMIT,
  fableThreadWorkspaceSlug,
  type FableLocalHistoryMessage,
} from "./fable-local-runtime.ts"
import type { CodexPreflight } from "./codex-preflight.ts"
import type {
  CodexModel,
  CodexReasoningEffort,
  FableLocalAnswerQuestionRequest,
  FableLocalQuestion,
} from "./fable-local-contract.ts"
import {
  runCodexAppServerTurn,
  type CodexAppServerTurnControl,
} from "./codex-app-server-turn.ts"
import type { CodexAppServerRequest, CodexAppServerSpawn } from "./codex-app-server-client.ts"
import type { FableLocalQueueFollowupOutcome } from "./fable-local-runtime.ts"

export type CodexLocalTurnInput = Readonly<{
  turnRef: string
  threadRef: string
  history: ReadonlyArray<FableLocalHistoryMessage>
  message: string
  accountRef?: string
  model?: CodexModel
  reasoningEffort?: CodexReasoningEffort
  /**
   * Optional image attachments (capability I1). `codex exec` accepts images
   * via `-i, --image <FILE>...` (local file paths), so the runtime writes each
   * attachment into a bounded per-turn subdir of the thread workspace and
   * passes its path. Absent/empty = the prior no-image invocation, unchanged.
   */
  images?: ReadonlyArray<FableLocalImageAttachment>
  /** Main-owned startup recovery. Exact account/thread only; never renderer supplied. */
  recovery?: Readonly<{ threadId: string; accountRef: string }>
  emit: (event: FableLocalEvent) => void
}>

export type CodexLocalTurnResult =
  | Readonly<{
      ok: true
      text: string
      totalTokens: number | null
      accountRef: string
      usage?: FableChildUsage
      /** The codex thread id (session continuity receipt), when reported. */
      threadId: string | null
    }>
  | Readonly<{ ok: false; reason: FableLocalFailureReason; detail: string }>

export type CodexLocalRuntimeOptions = Readonly<{
  /** Resolved lazily (Electron's userData path is not final at module load). */
  scratchRoot: () => string
  /** Exact top-level coding cwd; replaceable by a future directory setting. */
  workspaceRoot?: () => string
  env?: Record<string, string | undefined>
  spawnImpl?: CodexChildSpawn
  discoverImpl?: () => Promise<ReadonlyArray<CodexChildAccount>>
  health?: CodexAccountHealth
  /**
   * The session preflight prober: availability = a PROBE-VERIFIED account
   * exists, and runTurn lazily probes before the FIRST dispatch this session
   * (the anti-speedbump gate). Absent only in unit tests that inject
   * discover/health directly.
   */
  preflight?: CodexPreflight
  /** Test-only host deadline. Production turns have no automatic cutoff. */
  timeoutMs?: number
  /**
   * Product runtime path. When present every real Codex turn uses the pinned
   * app-server protocol; the legacy exec parser remains only for deterministic
   * fixtures and compatibility tests that omit this option.
   */
  appServer?: Readonly<{
    binary: () => string | null
    installProductSpecSkill: (account: CodexChildAccount) => Readonly<{
      skillRoot: string
      skillPath: string
    }>
    spawnImpl?: CodexAppServerSpawn
    onServerRequest?: (request: CodexAppServerRequest) => Promise<unknown>
    productSpecDynamicTools?: ReadonlyArray<Readonly<Record<string, unknown>>>
    onProductSpecToolCall?: (request: CodexAppServerRequest) => Promise<unknown | null>
  }>
  /**
   * Typed per-account evidence feed (main wires this into the usage ledger
   * so the fleet readiness projection sees turn-observed reconnect evidence
   * and turn-verified recoveries without parsing display strings).
   */
  onAccountEvidence?: (input: Readonly<{
    accountRef: string
    evidence: "reconnect_required" | "verified"
  }>) => void
  /** Durable main-owned continuity restored before the first renderer exists. */
  initialSessions?: ReadonlyArray<Readonly<{
    threadRef: string
    threadId: string
    accountRef: string
  }>>
  /** Main-only dispatch receipt; never projected through the renderer bridge. */
  onDispatch?: (input: Readonly<{
    threadRef: string
    turnRef: string
    accountRef: string
  }>) => void
  /** Main-only provider identity receipt observed at `thread.started`. */
  onProviderSession?: (input: Readonly<{
    threadRef: string
    turnRef: string
    accountRef: string
    threadId: string
  }>) => void
}>

export type CodexLocalRuntime = Readonly<{
  availability: () => Promise<CodexLocalAvailability>
  runTurn: (input: CodexLocalTurnInput) => Promise<CodexLocalTurnResult>
  interrupt: (turnRef: string) => boolean
  steerCurrent: (request: FableLocalQueueFollowupRequest) => Promise<Readonly<{
    ok: boolean
    outcome: "delivered" | "unsupported" | "not_found"
  }>>
  queueFollowup: (request: FableLocalQueueFollowupRequest) => FableLocalQueueFollowupOutcome
  answerQuestion: (request: FableLocalAnswerQuestionRequest) => boolean
  dispose: () => void
}>

const bounded = (value: string, limit: number): string =>
  value.length > limit ? `${value.slice(0, limit - 1)}…` : value

const historyPrompt = (
  history: ReadonlyArray<FableLocalHistoryMessage>,
  message: string,
): string => {
  const window = history
    .filter(note => note.role !== "system")
    .slice(-FABLE_LOCAL_HISTORY_MESSAGES)
    .map(note =>
      `${note.role === "user" ? "User" : "Assistant"}: ${bounded(note.text, FABLE_LOCAL_HISTORY_MESSAGE_LIMIT)}`)
  if (window.length === 0) return message
  return [
    "Conversation so far (for context; reply only to the final user message):",
    ...window,
    `User: ${message}`,
  ].join("\n\n")
}

/** File extension for a supported image media type (`codex exec -i` path). */
const codexImageExtension = (mediaType: FableLocalImageAttachment["mediaType"]): string =>
  mediaType === "image/jpeg" ? "jpg" : mediaType.slice("image/".length)

/**
 * Write attached images (capability I1) into a bounded per-turn subdir of the
 * thread workspace and return their absolute paths for `codex exec -i <path>`.
 * The base64 is decoded in main (never a renderer filesystem read); the subdir
 * is turn-scoped so parallel turns never collide.
 */
export const writeCodexTurnImages = (
  workspace: string,
  turnRef: string,
  images: ReadonlyArray<FableLocalImageAttachment>,
): ReadonlyArray<string> => {
  if (images.length === 0) return []
  const slug = turnRef.replace(/[^A-Za-z0-9._-]/g, "_").slice(0, 120)
  const dir = join(workspace, ".oa-image-attachments", slug)
  mkdirSync(dir, { recursive: true })
  return images.map((image, index) => {
    const filePath = join(dir, `image-${index + 1}.${codexImageExtension(image.mediaType)}`)
    writeFileSync(filePath, Buffer.from(image.data, "base64"))
    return filePath
  })
}

type ParsedTurnAttempt = Readonly<{
  outcome: "success" | "reconnect_required" | "incompatible_workflow" | "failed" | "timeout" | "interrupted"
  text: string
  usage: CodexChildUsage | null
  threadId: string | null
  detail: string
  preContent: boolean
  quotaExhausted: boolean
  rateLimited: boolean
}>

export const makeCodexLocalRuntime = (options: CodexLocalRuntimeOptions): CodexLocalRuntime => {
  const env = options.env ?? (process.env as Record<string, string | undefined>)
  const spawnCodex = options.spawnImpl ?? defaultSpawnCodex
  const discover = options.discoverImpl ?? (() => discoverRegisteredCodexAccounts(env))
  const health = options.health ?? sharedCodexAccountHealth
  const timeoutMs = options.timeoutMs
  const activeTurns = new Map<string, {
    interrupted: boolean
    child: ChildLike | null
    interrupt: (() => void) | null
    steer: ((message: string) => Promise<boolean>) | null
  }>()
  const activeTurnByThread = new Map<string, Readonly<{
    turnRef: string
    emit: (event: FableLocalEvent) => void
    control: { steer: ((message: string) => Promise<boolean>) | null }
  }>>()
  const followupQueue = new Map<string, Array<{ queueRef: string; message: string }>>()
  let followupSequence = 0
  const pendingQuestions = new Map<string, Readonly<{
    turnRef: string
    accept: (request: FableLocalAnswerQuestionRequest) => boolean
    deny: () => void
  }>>()
  let questionSequence = 0
  /**
   * In-memory continuity: threadRef -> the codex thread session, pinned to
   * the account that created it (resume runs on the SAME isolated home only;
   * a rotated account gets bounded-history prepend instead).
   */
  const sessionByThread = new Map<string, { threadId: string; accountRef: string }>(
    (options.initialSessions ?? []).map(session => [
      session.threadRef,
      { threadId: session.threadId, accountRef: session.accountRef },
    ]),
  )

  const verifiedOrderedAccounts = async (): Promise<{
    accounts: ReadonlyArray<CodexChildAccount>
    verified: ReadonlyArray<CodexChildAccount>
  }> => {
    // Lazy first-dispatch probe (anti-speedbump): before the first dispatch
    // this session every registered account gets the real validity probe;
    // probe results land in the SAME health memory this ordering uses.
    if (options.preflight !== undefined) await options.preflight.ensureProbed()
    const discovered = health.order(await discover())
    // The Desktop MVP is a Codex wrapper: app-server turns use exactly the
    // user's ordinary logged-in Codex session. Named Pylon accounts remain a
    // fleet capability and are not eligible for this local workroom lane.
    const accounts = options.appServer === undefined
      ? discovered
      : discovered.filter(account => account.source === "current_session")
    const verifiedRefs = new Set(options.preflight?.verifiedRefs() ?? [])
    return {
      accounts,
      verified: accounts.filter(account =>
        account.source === "current_session" || verifiedRefs.has(account.ref)),
    }
  }

  const availability = async (): Promise<CodexLocalAvailability> => {
    const { accounts, verified } = await verifiedOrderedAccounts()
    if (accounts.length === 0) return { state: "unavailable", reason: "no_codex_account" }
    // CHIP EVIDENCE RULE: without a preflight (unit tests), health-ordering
    // alone decides nothing — the chip needs PROBE-VERIFIED evidence.
    if (options.preflight === undefined) {
      return { state: "unavailable", reason: "no_verified_account" }
    }
    const first = verified[0]
    if (first === undefined) {
      // Quota honesty (live receipt 2026-07-11): when the only obstacle is a
      // rate limit, "Reconnect in Settings" would be a lie — reconnecting
      // never restores quota. The reason names the rate limit instead.
      const quotaExhausted = options.preflight.results()
        .some(result => result.state === "quota_exhausted")
      const rateLimited = options.preflight.results()
        .some(result => result.state === "rate_limited")
      return {
        state: "unavailable",
        reason: quotaExhausted ? "quota_exhausted" : rateLimited ? "rate_limited" : "no_verified_account",
      }
    }
    return { state: "available", accountRef: first.ref, verifiedCount: verified.length }
  }

  const runAttempt = async (input: Readonly<{
    account: CodexChildAccount
    threadRef: string
    turnRef: string
    workspace: string
    prompt: string
    /**
     * Absolute paths to images written to the turn workspace (capability I1);
     * passed to `codex exec` as `-i <path>` flags. Empty = no image flags.
     */
    imagePaths: ReadonlyArray<string>
    resumeThreadId: string | null
    reasoningEffort?: CodexReasoningEffort
    model: CodexModel
    emit: (event: FableLocalEvent) => void
    control: {
      interrupted: boolean
      child: ChildLike | null
      interrupt: (() => void) | null
      steer: ((message: string) => Promise<boolean>) | null
    }
  }>): Promise<ParsedTurnAttempt> => {
    if (options.appServer !== undefined) {
      const binary = options.appServer.binary()
      if (binary === null) {
        return {
          outcome: "incompatible_workflow",
          text: "",
          usage: null,
          threadId: input.resumeThreadId,
          detail: "the package-owned Codex app-server executable is unavailable",
          preContent: true,
          quotaExhausted: false,
          rateLimited: false,
        }
      }
      let skill: Readonly<{ skillRoot: string; skillPath: string }>
      try {
        skill = options.appServer.installProductSpecSkill(input.account)
      } catch (error) {
        return {
          outcome: "incompatible_workflow",
          text: "",
          usage: null,
          threadId: input.resumeThreadId,
          detail: error instanceof Error ? error.message : "productspec-work skill installation failed",
          preContent: true,
          quotaExhausted: false,
          rateLimited: false,
        }
      }
      const selection: ResolvedPylonAccountSelection = {
        provider: "codex",
        selector: "registry_ref",
        accountRef: input.account.ref,
        accountRefHash: hashPylonAccountRef("codex", input.account.ref),
        home: input.account.home,
      }
      options.onDispatch?.({
        threadRef: input.threadRef,
        turnRef: input.turnRef,
        accountRef: input.account.ref,
      })
      const onServerRequest = async (request: CodexAppServerRequest): Promise<unknown> => {
        const isApproval = request.method === "item/commandExecution/requestApproval" ||
          request.method === "item/fileChange/requestApproval"
        if (isApproval) {
          const params = request.params !== null && typeof request.params === "object"
            ? request.params as Record<string, unknown>
            : {}
          const requestSummary = request.method === "item/commandExecution/requestApproval"
            ? (typeof params.command === "string" && params.command.trim() !== ""
                ? params.command
                : typeof params.reason === "string" ? params.reason : "Run the requested command")
            : (typeof params.reason === "string" && params.reason.trim() !== ""
                ? params.reason
                : "Apply the requested file changes")
          const questionRef = bounded(`approval.${input.turnRef}.${++questionSequence}`, 120)
          return new Promise(resolve => {
            let finished = false
            const finish = (decision: "accept" | "acceptForSession" | "decline", outcome: "answered" | "denied"): void => {
              if (finished) return
              finished = true
              pendingQuestions.delete(questionRef)
              input.emit({ kind: "question_resolved", questionRef, outcome })
              resolve({ decision })
            }
            const question = bounded(redactChildText(requestSummary, input.workspace), FABLE_LOCAL_SUMMARY_LIMIT)
            pendingQuestions.set(questionRef, {
              turnRef: input.turnRef,
              accept: answer => {
                const labels = answer.answers.flatMap(item => item.labels)
                if (labels.includes("Allow once")) {
                  finish("accept", "answered")
                  return true
                }
                if (labels.includes("Allow for session")) {
                  finish("acceptForSession", "answered")
                  return true
                }
                if (labels.includes("Decline")) {
                  finish("decline", "denied")
                  return true
                }
                return false
              },
              deny: () => finish("decline", "denied"),
            })
            input.emit({
              kind: "question_pending",
              questionRef,
              interactionKind: "tool_approval",
              decisionRef: bounded(String(request.id), 120),
              questions: [{
                question,
                header: request.method === "item/commandExecution/requestApproval" ? "Command approval" : "File approval",
                options: [
                  { label: "Allow once", description: "Approve only this request." },
                  { label: "Allow for session", description: "Approve matching requests for this Codex session." },
                  { label: "Decline", description: "Refuse this request." },
                ],
                multiSelect: false,
              }],
            })
          })
        }
        if (request.method !== "item/tool/requestUserInput") {
          if (options.appServer?.onServerRequest !== undefined) return options.appServer.onServerRequest(request)
          throw new Error(`unsupported Codex server request: ${request.method}`)
        }
        const params = request.params !== null && typeof request.params === "object"
          ? request.params as Record<string, unknown>
          : {}
        const rawQuestions = Array.isArray(params.questions) ? params.questions.slice(0, 4) : []
        const originals = rawQuestions.flatMap(raw => {
          if (raw === null || typeof raw !== "object") return []
          const value = raw as Record<string, unknown>
          if (typeof value.id !== "string" || typeof value.question !== "string") return []
          const options = Array.isArray(value.options) ? value.options.slice(0, 4).flatMap(option => {
            if (option === null || typeof option !== "object") return []
            const item = option as Record<string, unknown>
            return typeof item.label === "string"
              ? [{ label: bounded(item.label, 200), ...(typeof item.description === "string"
                  ? { description: bounded(item.description, FABLE_LOCAL_SUMMARY_LIMIT) }
                  : {}) }]
              : []
          }) : []
          const projected: FableLocalQuestion = {
            question: bounded(value.question, FABLE_LOCAL_SUMMARY_LIMIT),
            header: bounded(typeof value.header === "string" ? value.header : "Input", 120),
            options,
            multiSelect: false,
          }
          return [{ id: value.id, originalQuestion: value.question, projected }]
        })
        if (originals.length === 0) return { answers: {} }
        const questionRef = bounded(`q.${input.turnRef}.${++questionSequence}`, 120)
        return new Promise(resolve => {
          let finished = false
          const finish = (response: unknown, outcome: "answered" | "denied"): void => {
            if (finished) return
            finished = true
            pendingQuestions.delete(questionRef)
            input.emit({ kind: "question_resolved", questionRef, outcome })
            resolve(response)
          }
          pendingQuestions.set(questionRef, {
            turnRef: input.turnRef,
            accept: answer => {
              const answers: Record<string, { answers: string[] }> = {}
              for (const original of originals) {
                const selected = answer.answers.find(candidate => candidate.question === original.projected.question)
                if (selected !== undefined) answers[original.id] = { answers: [...selected.labels] }
              }
              if (Object.keys(answers).length === 0) return false
              finish({ answers }, "answered")
              return true
            },
            deny: () => finish({ answers: {} }, "denied"),
          })
          input.emit({ kind: "question_pending", questionRef, questions: originals.map(item => item.projected) })
        })
      }
      const appServerEnv = input.account.source === "current_session"
        ? (() => {
            const current = { ...env }
            delete current.CODEX_HOME
            return current
          })()
        : pylonAccountEnvironment(env, selection)
      return runCodexAppServerTurn({
        binary,
        env: appServerEnv,
        workspace: input.workspace,
        threadRef: input.threadRef,
        turnRef: input.turnRef,
        accountRef: input.account.ref,
        prompt: input.prompt,
        imagePaths: input.imagePaths,
        resumeThreadId: input.resumeThreadId,
        model: input.model,
        reasoningEffort: input.reasoningEffort ?? CODEX_LOCAL_REASONING_EFFORT,
        productSpecSkill: skill,
        ...(options.appServer.productSpecDynamicTools === undefined ? {} : { productSpecDynamicTools: options.appServer.productSpecDynamicTools }),
        ...(options.appServer.onProductSpecToolCall === undefined ? {} : { onProductSpecToolCall: options.appServer.onProductSpecToolCall }),
        control: input.control as CodexAppServerTurnControl,
        emit: input.emit,
        ...(options.appServer.spawnImpl === undefined ? {} : { spawnImpl: options.appServer.spawnImpl }),
        onServerRequest,
        ...(timeoutMs === undefined ? {} : { turnTimeoutMs: timeoutMs, requestTimeoutMs: timeoutMs }),
        onProviderSession: threadId => options.onProviderSession?.({
          threadRef: input.threadRef,
          turnRef: input.turnRef,
          accountRef: input.account.ref,
          threadId,
        }),
      })
    }
    return new Promise(resolve => {
      const redact = (value: string): string => redactChildText(value, input.workspace)
      const selection: ResolvedPylonAccountSelection = {
        provider: "codex",
        selector: "registry_ref",
        accountRef: input.account.ref,
        accountRefHash: hashPylonAccountRef("codex", input.account.ref),
        home: input.account.home,
      }
      // Fresh turns use the receipted exec recipe (NO --ephemeral: session
      // rollouts persist in the isolated home for resume + receipts).
      // Resumed turns use `exec resume <thread_id>`; resume has no -s/-C
      // flags, so the sandbox rides -c sandbox_mode (receipted).
      // Capability I1: `-i <path>` per image. Each flag is placed so a
      // non-variadic token (`-C` fresh, `--skip-git-repo-check` resume)
      // terminates the variadic `--image` list before the positional prompt —
      // otherwise the greedy `<FILE>...` arg would swallow the prompt.
      const imageFlags = input.imagePaths.flatMap((imagePath) => ["-i", imagePath])
      const args = input.resumeThreadId === null
        ? [
            "exec",
            "--json",
            "-m",
            input.model,
            "-c",
            `model_reasoning_effort=${input.reasoningEffort ?? CODEX_LOCAL_REASONING_EFFORT}`,
            "-s",
            CODEX_CHILD_SANDBOX,
            "--skip-git-repo-check",
            ...imageFlags,
            "-C",
            input.workspace,
            input.prompt,
          ]
        : [
            "exec",
            "resume",
            input.resumeThreadId,
            "--json",
            "-m",
            input.model,
            "-c",
            `model_reasoning_effort=${input.reasoningEffort ?? CODEX_LOCAL_REASONING_EFFORT}`,
            "-c",
            `sandbox_mode="${CODEX_CHILD_SANDBOX}"`,
            ...imageFlags,
            "--skip-git-repo-check",
            input.prompt,
          ]
      options.onDispatch?.({
        threadRef: input.threadRef,
        turnRef: input.turnRef,
        accountRef: input.account.ref,
      })
      const child = spawnCodex({
        args,
        env: input.account.source === "current_session"
          ? (() => {
              const current = { ...env }
              delete current.CODEX_HOME
              return current
            })()
          : pylonAccountEnvironment(env, selection),
        cwd: input.workspace,
      })
      if (child === null) {
        resolve({
          outcome: "failed",
          text: "",
          usage: null,
          threadId: null,
          detail: "codex executable unavailable",
          preContent: true,
          quotaExhausted: false,
          rateLimited: false,
        })
        return
      }
      input.control.child = child
      if (input.control.interrupted) child.kill("SIGTERM")

      let done = false
      let timedOut = false
      let stderrText = ""
      let agentText = ""
      let emittedText = ""
      let usage: CodexChildUsage | null = null
      let threadId: string | null = null
      let errorMessage: string | null = null
      const pendingToolNames = new Map<string, string>()

      // Long coding turns are normal and Codex can be quiet while reasoning
      // or running a tool. Production therefore has no host wall-clock kill;
      // the composer's Stop action is the explicit cancellation authority.
      // A bounded timer remains injectable for deterministic failure tests.
      const timer = timeoutMs === undefined
        ? undefined
        : setTimeout(() => {
            timedOut = true
            child.kill("SIGTERM")
          }, timeoutMs)

      const finish = (attempt: ParsedTurnAttempt): void => {
        if (done) return
        done = true
        if (timer !== undefined) clearTimeout(timer)
        input.control.child = null
        resolve(attempt)
      }

      /** Streams the not-yet-emitted suffix of the accumulated agent text. */
      const emitAgentDelta = (): void => {
        let pending = agentText.slice(emittedText.length)
        while (pending.length > 0) {
          const slice = pending.slice(0, FABLE_LOCAL_DELTA_LIMIT)
          input.emit({ kind: "text_delta", text: slice })
          emittedText += slice
          pending = pending.slice(slice.length)
        }
      }

      const toolFacts = (
        item: Record<string, unknown>,
      ): Readonly<{ toolName: string; summary: string; ok: boolean }> | null => {
        const type = typeof item.type === "string" ? item.type : ""
        if (type === "command_execution") {
          // Args summaries use the SAME JSON shape the fable lane emits
          // (JSON.stringify of the tool input) so the shared tool-card
          // humanizer extracts the command for the card's detail line.
          return {
            toolName: "Bash",
            summary: bounded(
              JSON.stringify({ command: redact(typeof item.command === "string" ? item.command : "") }),
              FABLE_LOCAL_SUMMARY_LIMIT,
            ),
            ok: (typeof item.exit_code === "number" ? item.exit_code === 0 : item.status !== "failed"),
          }
        }
        if (type === "mcp_tool_call") {
          const tool = typeof item.tool_name === "string"
            ? item.tool_name
            : typeof item.name === "string" ? item.name : "tool"
          return {
            toolName: bounded(redact(tool), 120),
            summary: "",
            ok: item.status !== "failed",
          }
        }
        if (type === "file_change") {
          const count = Array.isArray(item.changes) ? item.changes.length : 0
          return { toolName: "FileChange", summary: `${count} file change(s)`, ok: item.status !== "failed" }
        }
        if (type === "web_search") {
          return {
            toolName: "WebSearch",
            summary: bounded(
              JSON.stringify({ query: redact(typeof item.query === "string" ? item.query : "") }),
              FABLE_LOCAL_SUMMARY_LIMIT,
            ),
            ok: true,
          }
        }
        return null
      }

      const handleEvent = (event: Record<string, unknown>): void => {
        const type = typeof event.type === "string" ? event.type : ""
        if (type === "thread.started" && typeof event.thread_id === "string") {
          threadId = event.thread_id
          options.onProviderSession?.({
            threadRef: input.threadRef,
            turnRef: input.turnRef,
            accountRef: input.account.ref,
            threadId,
          })
          return
        }
        if (type === "item.started" || type === "item.updated" || type === "item.completed") {
          const item = event.item
          if (item === null || typeof item !== "object") return
          const record = item as Record<string, unknown>
          const itemType = typeof record.type === "string" ? record.type : ""
          const itemId = typeof record.id === "string" ? record.id : `${itemType}-${pendingToolNames.size}`
          if (itemType === "agent_message") {
            if (typeof record.text === "string" && record.text.length > 0) {
              const separator = agentText.length > 0 && type === "item.completed" &&
                !agentText.endsWith(record.text)
                ? ""
                : ""
              void separator
              // The exec stream carries the full text per event; accumulate
              // monotonically and emit only the unseen suffix.
              const candidate = agentText.length === 0
                ? record.text
                : record.text.startsWith(agentText)
                  ? record.text
                  : `${agentText}\n\n${record.text}`
              agentText = bounded(redact(candidate), FABLE_LOCAL_FINAL_TEXT_LIMIT)
              emitAgentDelta()
            }
            return
          }
          if (itemType === "reasoning") {
            if (type === "item.completed" && typeof record.text === "string" && record.text.trim() !== "") {
              input.emit({ kind: "reasoning", text: bounded(redact(record.text), FABLE_LOCAL_SUMMARY_LIMIT) })
            }
            return
          }
          const facts = toolFacts(record)
          if (facts === null) return
          if (type === "item.started" && !pendingToolNames.has(itemId)) {
            pendingToolNames.set(itemId, facts.toolName)
            input.emit({ kind: "tool_use", toolName: facts.toolName, summary: facts.summary })
            return
          }
          if (type === "item.completed") {
            if (!pendingToolNames.has(itemId)) {
              // No started event arrived (e.g. file_change): emit the pair
              // so the renderer's FIFO card pairing stays balanced.
              input.emit({ kind: "tool_use", toolName: facts.toolName, summary: facts.summary })
            }
            pendingToolNames.delete(itemId)
            const resultSummary = itemType === "command_execution" &&
              typeof record.aggregated_output === "string"
              ? bounded(redact(record.aggregated_output), FABLE_LOCAL_SUMMARY_LIMIT)
              : facts.summary
            input.emit({ kind: "tool_result", toolName: facts.toolName, ok: facts.ok, summary: resultSummary })
          }
          return
        }
        if (type === "turn.completed") {
          usage = codexChildUsageFromTurnCompleted(event)
          return
        }
        if (type === "error" && typeof event.message === "string") {
          errorMessage = event.message
          return
        }
        if (type === "turn.failed") {
          const inner = (event.error as { message?: unknown } | undefined)?.message
          if (typeof inner === "string") errorMessage = inner
        }
      }

      const jsonLines = makeCodexJsonLineConsumer(handleEvent)
      child.stdout?.on("data", (chunk: Buffer | string) => {
        jsonLines.push(typeof chunk === "string" ? chunk : chunk.toString("utf8"))
      })
      child.stderr?.on("data", (chunk: Buffer | string) => {
        stderrText += typeof chunk === "string" ? chunk : chunk.toString("utf8")
      })
      child.on("error", () => {
        finish({
          outcome: "failed",
          text: "",
          usage: null,
          threadId,
          detail: "codex process failed to start",
          preContent: true,
          quotaExhausted: false,
          rateLimited: false,
        })
      })
      child.on("close", (...args: unknown[]) => {
        jsonLines.flush()
        const exitCode = typeof args[0] === "number" ? args[0] : null
        const currentUsage: CodexChildUsage | null = usage
        const preContent = agentText.trim() === "" &&
          (currentUsage === null || currentUsage.totalTokens === 0)
        if (input.control.interrupted) {
          finish({
            outcome: "interrupted",
            text: "",
            usage,
            threadId,
            detail: "turn interrupted",
            preContent,
            quotaExhausted: false,
            rateLimited: false,
          })
          return
        }
        if (timedOut) {
          finish({
            outcome: "timeout",
            text: "",
            usage,
            threadId,
            detail: `test deadline reached (${Math.round((timeoutMs ?? 0) / 1000)}s)`,
            preContent,
            quotaExhausted: false,
            rateLimited: false,
          })
          return
        }
        const failureText = `${errorMessage ?? ""}\n${stderrText}`
        if ((exitCode !== 0 || errorMessage !== null) && isCodexReconnectRequiredText(failureText)) {
          finish({
            outcome: "reconnect_required",
            text: "",
            usage,
            threadId,
            detail: "credentials rejected (auth-class failure) — reconnect this Codex account",
            preContent,
            quotaExhausted: false,
            rateLimited: false,
          })
          return
        }
        if (exitCode !== 0 || errorMessage !== null) {
          finish({
            outcome: "failed",
            text: "",
            usage,
            threadId,
            detail: bounded(
              redact(errorMessage ?? `codex exec exited ${exitCode ?? "abnormally"}`),
              CODEX_CHILD_SUMMARY_LIMIT,
            ),
            preContent,
            quotaExhausted: isCodexQuotaExhaustionText(failureText),
            rateLimited: isCodexRateLimitText(failureText),
          })
          return
        }
        if (agentText.trim() === "") {
          finish({
            outcome: "failed",
            text: "",
            usage,
            threadId,
            detail: "the turn produced no agent_message text",
            preContent,
            quotaExhausted: false,
            rateLimited: false,
          })
          return
        }
        finish({
          outcome: "success",
          text: agentText,
          usage,
          threadId,
          detail: "",
          preContent: false,
          quotaExhausted: false,
          rateLimited: false,
        })
      })
    })
  }

  const runTurn = async (input: CodexLocalTurnInput): Promise<CodexLocalTurnResult> => {
    const failure = (
      reason: FableLocalFailureReason,
      detail: string,
    ): Extract<CodexLocalTurnResult, { ok: false }> =>
      ({ ok: false, reason, detail: bounded(detail, FABLE_LOCAL_SUMMARY_LIMIT) })
    const emitFailure = (
      result: Extract<CodexLocalTurnResult, { ok: false }>,
    ): CodexLocalTurnResult => {
      input.emit({ kind: "turn_failed", reason: result.reason, detail: result.detail })
      return result
    }

    const discovered = await verifiedOrderedAccounts()
    const selectedAccountRef = input.recovery?.accountRef ?? input.accountRef
    const accounts = selectedAccountRef === undefined
      ? discovered.accounts
      : discovered.accounts.filter(account => account.ref === selectedAccountRef)
    if (accounts.length === 0) {
      return emitFailure(failure(
        "no_codex_account",
        "no Codex account is registered in the pylon account registry",
      ))
    }

    // Desktop uses its launch directory for top-level coding turns. The
    // isolated per-thread fallback remains available to tests/other hosts.
    const workspace = options.workspaceRoot?.() ??
      join(options.scratchRoot(), "threads", fableThreadWorkspaceSlug(input.threadRef))
    try {
      mkdirSync(workspace, { recursive: true })
    } catch {
      return emitFailure(failure("session_failed", "thread workspace unavailable"))
    }

    // Capability I1: write each attachment into a bounded per-turn subdir and
    // collect its absolute path for `codex exec -i <path>`. A write failure is
    // a turn failure (honest — the model would otherwise silently miss the
    // image the user attached).
    let imagePaths: ReadonlyArray<string>
    try {
      imagePaths = writeCodexTurnImages(workspace, input.turnRef, input.images ?? [])
    } catch {
      return emitFailure(failure("session_failed", "could not stage attached images"))
    }

    const control = {
      interrupted: false,
      child: null as ChildLike | null,
      interrupt: null as (() => void) | null,
      steer: null as ((message: string) => Promise<boolean>) | null,
    }
    activeTurns.set(input.turnRef, control)
    activeTurnByThread.set(input.threadRef, { turnRef: input.turnRef, emit: input.emit, control })
    input.emit({ kind: "turn_started" })
    // Spawn-config truth caption (the exec stream echoes no model back).
    const requestedModel = input.model ?? CODEX_LOCAL_MODEL
    input.emit({ kind: "model_effective", model: `${requestedModel} (requested)` })

    try {
      let reconnectCount = 0
      let otherFailures = 0
      let lastDetail = ""
      for (const account of accounts) {
        if (control.interrupted) return emitFailure(failure("interrupted", "turn interrupted"))
        const continuity = sessionByThread.get(input.threadRef)
        const resumeThreadId = input.recovery !== undefined
          ? input.recovery.threadId
          : continuity !== undefined && continuity.accountRef === account.ref
            ? continuity.threadId
            : null
        const prompt = resumeThreadId === null
          ? historyPrompt(input.history, input.message)
          : input.message
        const attempt = await runAttempt({
          account,
          threadRef: input.threadRef,
          turnRef: input.turnRef,
          workspace,
          prompt,
          imagePaths,
          resumeThreadId,
          emit: input.emit,
          control,
          reasoningEffort: input.reasoningEffort,
          model: requestedModel,
        })
        if (attempt.outcome === "success") {
          health.recordSuccess(account.ref)
          options.onAccountEvidence?.({ accountRef: account.ref, evidence: "verified" })
          if (attempt.threadId !== null) {
            sessionByThread.set(input.threadRef, {
              threadId: attempt.threadId,
              accountRef: account.ref,
            })
          }
          const usage = attempt.usage
          const split: FableChildUsage | null = usage === null ? null : {
            inputTokens: usage.inputTokens,
            cachedInputTokens: usage.cachedInputTokens,
            outputTokens: usage.outputTokens,
            reasoningTokens: usage.reasoningOutputTokens,
            totalTokens: usage.totalTokens,
          }
          input.emit({
            kind: "turn_completed",
            totalTokens: split?.totalTokens ?? null,
            accountRef: account.ref,
            ...(split === null ? {} : { usage: split }),
          })
          return {
            ok: true,
            text: attempt.text,
            totalTokens: split?.totalTokens ?? null,
            accountRef: account.ref,
            ...(split === null ? {} : { usage: split }),
            threadId: attempt.threadId,
          }
        }
        if (attempt.outcome === "interrupted") {
          return emitFailure(failure("interrupted", "turn interrupted"))
        }
        if (attempt.outcome === "timeout") {
          return emitFailure(failure("timeout", attempt.detail))
        }
        if (attempt.outcome === "reconnect_required") {
          // Typed VISIBLE rotation — never silent: the transcript carries a
          // lane notice before the next candidate account gets the turn.
          health.recordAuthFailure(account.ref)
          options.onAccountEvidence?.({ accountRef: account.ref, evidence: "reconnect_required" })
          reconnectCount += 1
          lastDetail = attempt.detail
          input.emit({
            kind: "lane_notice",
            text: bounded(
              `Codex account ${account.ref} needs reconnect — rotating to the next candidate account`,
              FABLE_LOCAL_SUMMARY_LIMIT,
            ),
          })
          continue
        }
        if (attempt.outcome === "incompatible_workflow") {
          return emitFailure(failure("incompatible_workflow", attempt.detail))
        }
        if (attempt.preContent) {
          // Non-auth pre-content failure: rotation-eligible, no demotion.
          otherFailures += 1
          lastDetail = attempt.detail
          input.emit({
            kind: "lane_notice",
            text: bounded(
              attempt.quotaExhausted
                ? `Codex account ${account.ref} exhausted its usage quota (${attempt.detail}) — rotating to the next candidate account`
                : attempt.rateLimited
                  ? `Codex account ${account.ref} is rate-limited (${attempt.detail}) — rotating to the next candidate account`
                  : `Codex account ${account.ref} failed before producing content (${attempt.detail}) — rotating to the next candidate account`,
              FABLE_LOCAL_SUMMARY_LIMIT,
            ),
          })
          continue
        }
        // Post-content failure: terminal — a partial reply never double-runs.
        return emitFailure(failure("session_failed", attempt.detail))
      }
      if (otherFailures === 0) {
        return emitFailure(failure(
          "account_reconnect_required",
          `all ${reconnectCount} available Codex session(s) need reconnect (credentials rejected)`,
        ))
      }
      return emitFailure(failure(
        "session_failed",
        `all ${accounts.length} available Codex session(s) failed before producing content` +
          ` (${reconnectCount} need reconnect, ${otherFailures} other failure(s)); last: ${lastDetail}`,
      ))
    } finally {
      activeTurns.delete(input.turnRef)
      const active = activeTurnByThread.get(input.threadRef)
      if (active?.turnRef === input.turnRef) activeTurnByThread.delete(input.threadRef)
      const queue = followupQueue.get(input.threadRef)
      if (queue !== undefined && queue.length > 0) {
        const next = queue.shift()!
        if (queue.length === 0) followupQueue.delete(input.threadRef)
        input.emit({ kind: "followup_promoted", queueRef: next.queueRef, message: next.message })
      }
    }
  }

  return {
    availability,
    runTurn,
    interrupt: turnRef => {
      const active = activeTurns.get(turnRef)
      if (active === undefined) return false
      for (const pending of pendingQuestions.values()) {
        if (pending.turnRef === turnRef) pending.deny()
      }
      active.interrupted = true
      active.interrupt?.()
      active.child?.kill("SIGTERM")
      return true
    },
    steerCurrent: async request => {
      const active = activeTurnByThread.get(request.threadRef)
      if (active === undefined) return { ok: false, outcome: "not_found" }
      if (active.control.steer === null) return { ok: false, outcome: "unsupported" }
      const delivered = await active.control.steer(bounded(request.message, FABLE_LOCAL_FOLLOWUP_MESSAGE_LIMIT))
      active.emit({
        kind: "lane_notice",
        text: delivered
          ? "Current Codex turn steered with the submitted message"
          : "Current Codex turn rejected the steer request",
      })
      return delivered ? { ok: true, outcome: "delivered" } : { ok: false, outcome: "unsupported" }
    },
    queueFollowup: request => {
      const active = activeTurnByThread.get(request.threadRef)
      if (active === undefined) return { ok: false, queued: false, reason: "no_active_turn" }
      followupSequence += 1
      const queueRef = bounded(
        `followup.${fableThreadWorkspaceSlug(request.threadRef)}.${followupSequence}`,
        120,
      )
      const queue = followupQueue.get(request.threadRef) ?? []
      queue.push({ queueRef, message: bounded(request.message, FABLE_LOCAL_FOLLOWUP_MESSAGE_LIMIT) })
      followupQueue.set(request.threadRef, queue)
      active.emit({ kind: "followup_queued", queueRef, position: queue.length })
      return { ok: true, queued: true, queueRef, position: queue.length }
    },
    answerQuestion: request => {
      const pending = pendingQuestions.get(request.questionRef)
      return pending !== undefined && pending.turnRef === request.turnRef
        ? pending.accept(request)
        : false
    },
    dispose: () => {
      for (const pending of pendingQuestions.values()) pending.deny()
      pendingQuestions.clear()
      for (const active of activeTurns.values()) {
        active.interrupted = true
        active.interrupt?.()
        active.child?.kill("SIGTERM")
      }
      activeTurns.clear()
      activeTurnByThread.clear()
      followupQueue.clear()
      sessionByThread.clear()
    },
  }
}

// ---------------------------------------------------------------------------
// Scripted fixtures (smoke + tests): a full codex-local turn stream driven
// through the REAL JSONL parser above. Never used in normal runs; main.ts
// logs when they are active.
// ---------------------------------------------------------------------------

export const FIXTURE_CODEX_LOCAL_TEXT = "Codex local **fixture** proof."
export const FIXTURE_CODEX_LOCAL_ACCOUNT: CodexChildAccount = {
  // Shares the fleet's first-READY codex account identity: the provider-accounts
  // fixture advertises `codex-3` (this account) as ready and lists it before
  // `codex`, whose fixture story is probe-revoked/reconnect-required. The
  // exact-provider-target feature (#8701 CUT-21) binds the fleet-selected
  // accountRef and the runtime filters discovered homes by it, so the smoke's
  // fleet identity and the codex-local discover identity MUST be the same
  // account — exactly the production invariant (the account you pick in the
  // fleet is the account the turn runs on). It must NOT reuse ref `codex`,
  // because this lane's probe/turn successes would erase that account's
  // deliberate reconnect-required narrative in the fleet assertions.
  ref: "codex-3",
  home: "/nonexistent/codex-local-fixture-home",
}

export const fixtureCodexLocalTurnStdout = (threadId = "thread-codex-local-fixture"): string =>
  [
    JSON.stringify({ type: "thread.started", thread_id: threadId }),
    JSON.stringify({ type: "turn.started" }),
    JSON.stringify({
      type: "item.completed",
      item: { id: "item_0", type: "reasoning", text: "planned the fixture reply" },
    }),
    JSON.stringify({
      type: "item.started",
      item: { id: "item_1", type: "command_execution", command: "echo fixture", status: "in_progress" },
    }),
    JSON.stringify({
      type: "item.completed",
      item: {
        id: "item_1",
        type: "command_execution",
        command: "echo fixture",
        aggregated_output: "fixture",
        exit_code: 0,
        status: "completed",
      },
    }),
    JSON.stringify({
      type: "item.completed",
      item: { id: "item_2", type: "agent_message", text: FIXTURE_CODEX_LOCAL_TEXT },
    }),
    JSON.stringify({
      type: "turn.completed",
      usage: { input_tokens: 900, cached_input_tokens: 600, output_tokens: 40, reasoning_output_tokens: 12 },
    }),
  ].join("\n")
