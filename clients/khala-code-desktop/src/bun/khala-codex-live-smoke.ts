import {
  spawnCodexInstances,
  spawnVerifiedTokenTotal,
  type KhalaCodexFleetProgressPayload,
  type KhalaCodexFleetToolOptions,
} from "./khala-codex-fleet-tools.js"
import { khalaCodeConfigFromRuntimeEnv } from "./khala-code-config.js"

export const TWO_CODEX_READONLY_SMOKE_COUNT = 2
export const TWO_CODEX_READONLY_SMOKE_DEFAULT_TIMEOUT_MS = 600_000
export const TWO_CODEX_READONLY_SMOKE_CLAIM_REF = "claim.public.khala_code.readonly_live_smoke"
export const TWO_CODEX_READONLY_SMOKE_READONLY_VERIFY = "bun scripts/khala-code-readonly-verify.ts"

type SpawnResult = Awaited<ReturnType<typeof spawnCodexInstances>>

export type TwoCodexReadOnlySmokeWork =
  | { readonly kind: "fixture" }
  | {
      readonly branch?: string | undefined
      readonly claimRef?: string | undefined
      readonly commit: string
      readonly kind: "repository"
      readonly repo: string
      readonly verify?: string | undefined
    }

export type TwoCodexReadOnlySmokeOptions = Pick<
  KhalaCodexFleetToolOptions,
  "delegationParameters" | "env" | "runner" | "sleep"
> & {
  readonly onProgress?: KhalaCodexFleetToolOptions["onProgress"] | undefined
  readonly prompt?: string | undefined
  readonly timeoutMs?: number | undefined
  readonly work?: TwoCodexReadOnlySmokeWork | undefined
}

export type TwoCodexReadOnlySmokeSummary = {
  readonly acceptedCount: number
  readonly assignmentRefs: readonly string[]
  readonly blockerRefs: readonly string[]
  readonly failures: readonly string[]
  readonly finishedAt: string
  readonly mode: TwoCodexReadOnlySmokeWork["kind"]
  readonly ok: boolean
  readonly perSlotTokens: readonly number[]
  readonly progressEventCount: number
  readonly progressPayloadCount: number
  readonly pylonRef: string | null
  readonly readOnlyVerify: string | null
  readonly requestedCount: number
  readonly slotSummaries: readonly string[]
  readonly startedAt: string
  readonly streamedAssignmentRefs: readonly string[]
  readonly tokensVerified: number
}

export function twoCodexReadOnlySmokePrompt(): string {
  return [
    "Khala Code Desktop live two-Codex read-only smoke.",
    "Run only read-only inspection commands. Do not edit, create, delete, commit, push, open pull requests, or write files.",
    "Inspect repository metadata and return a concise public-safe closeout confirming the assignment ran, streamed lifecycle evidence was visible, and token accounting completed.",
    "Useful commands are pwd, git status --short, git rev-parse --short HEAD, and rg --files | head.",
  ].join("\n")
}

export async function runTwoCodexReadOnlySmoke(
  options: TwoCodexReadOnlySmokeOptions = {},
): Promise<TwoCodexReadOnlySmokeSummary> {
  const progressPayloads: KhalaCodexFleetProgressPayload[] = []
  const work = options.work ?? { kind: "fixture" as const }
  const prompt = options.prompt ?? twoCodexReadOnlySmokePrompt()
  const timeoutMs = options.timeoutMs ?? TWO_CODEX_READONLY_SMOKE_DEFAULT_TIMEOUT_MS
  const startedAt = new Date().toISOString()
  const baseEnv = options.env ?? khalaCodeConfigFromRuntimeEnv().env
  const env = {
    ...baseEnv,
    OPENAGENTS_PYLON_DISABLE_ASSIGNMENT_PR: "1",
    OPENAGENTS_PYLON_DISABLE_PR_TITLE_MODEL: "1",
  }
  const readOnlyVerify = work.kind === "repository"
    ? work.verify ?? TWO_CODEX_READONLY_SMOKE_READONLY_VERIFY
    : null

  const spawnInput = work.kind === "repository"
    ? {
        branch: work.branch ?? "main",
        claimRef: work.claimRef ?? TWO_CODEX_READONLY_SMOKE_CLAIM_REF,
        commit: work.commit,
        count: TWO_CODEX_READONLY_SMOKE_COUNT,
        fixture: false,
        prompt,
        repo: work.repo,
        timeoutMs,
        verify: readOnlyVerify ?? TWO_CODEX_READONLY_SMOKE_READONLY_VERIFY,
      }
    : {
        count: TWO_CODEX_READONLY_SMOKE_COUNT,
        fixture: true,
        prompt,
        timeoutMs,
      }

  const result = await spawnCodexInstances(spawnInput, {
    delegationParameters: options.delegationParameters,
    env,
    onProgress: async payload => {
      progressPayloads.push(payload)
      await options.onProgress?.(payload)
    },
    runner: options.runner,
    sleep: options.sleep,
  })

  return summarizeTwoCodexReadOnlySmoke({
    finishedAt: new Date().toISOString(),
    mode: work.kind,
    progressPayloads,
    readOnlyVerify,
    result,
    startedAt,
  })
}

export function summarizeTwoCodexReadOnlySmoke(input: {
  readonly finishedAt: string
  readonly mode: TwoCodexReadOnlySmokeWork["kind"]
  readonly progressPayloads: readonly KhalaCodexFleetProgressPayload[]
  readonly readOnlyVerify: string | null
  readonly result: SpawnResult
  readonly startedAt: string
}): TwoCodexReadOnlySmokeSummary {
  const tokensVerified = spawnVerifiedTokenTotal(input.result)
  const assignmentRefs = unique(input.result.results.flatMap(slot =>
    slot.assignmentRef === null ? [] : [slot.assignmentRef]
  ))
  const streamedAssignmentRefs = unique(input.progressPayloads.flatMap(payload =>
    payload.events.flatMap(event =>
      event.assignmentRef === undefined ? [] : [event.assignmentRef]
    )
  ))
  const perSlotTokens = input.result.results.map(slot => slot.tokensVerified ?? 0)
  const progressEventCount = input.progressPayloads.reduce(
    (sum, payload) => sum + payload.events.length,
    0,
  )
  const blockerRefs = unique(input.result.results.flatMap(slot => blockerRefsFromSummary(slot.summary)))
  const slotSummaries = input.result.results.map(slot =>
    `slot ${slot.slot}: ${slot.status}${slot.assignmentRef === null ? "" : ` ${slot.assignmentRef}`}\n${slot.summary}`
  )
  const failures = twoCodexReadOnlySmokeFailures({
    assignmentRefs,
    perSlotTokens,
    progressEventCount,
    progressPayloadCount: input.progressPayloads.length,
    result: input.result,
    streamedAssignmentRefs,
    tokensVerified,
  })

  return {
    acceptedCount: input.result.acceptedCount,
    assignmentRefs,
    blockerRefs,
    failures,
    finishedAt: input.finishedAt,
    mode: input.mode,
    ok: failures.length === 0,
    perSlotTokens,
    progressEventCount,
    progressPayloadCount: input.progressPayloads.length,
    pylonRef: input.result.pylonRef,
    readOnlyVerify: input.readOnlyVerify,
    requestedCount: input.result.requestedCount,
    slotSummaries,
    startedAt: input.startedAt,
    streamedAssignmentRefs,
    tokensVerified,
  }
}

function twoCodexReadOnlySmokeFailures(input: {
  readonly assignmentRefs: readonly string[]
  readonly perSlotTokens: readonly number[]
  readonly progressEventCount: number
  readonly progressPayloadCount: number
  readonly result: SpawnResult
  readonly streamedAssignmentRefs: readonly string[]
  readonly tokensVerified: number
}): readonly string[] {
  const failures: string[] = []
  if (input.result.requestedCount !== TWO_CODEX_READONLY_SMOKE_COUNT) {
    failures.push(`expected ${TWO_CODEX_READONLY_SMOKE_COUNT} requested Codex assignments, got ${input.result.requestedCount}`)
  }
  if (input.result.acceptedCount !== TWO_CODEX_READONLY_SMOKE_COUNT) {
    failures.push(`expected ${TWO_CODEX_READONLY_SMOKE_COUNT} accepted Codex assignments, got ${input.result.acceptedCount}`)
  }
  if (input.assignmentRefs.length < TWO_CODEX_READONLY_SMOKE_COUNT) {
    failures.push(`expected ${TWO_CODEX_READONLY_SMOKE_COUNT} assignment refs, got ${input.assignmentRefs.length}`)
  }
  if (input.tokensVerified <= 0) {
    failures.push("expected verified token accounting to be greater than zero")
  }
  if (input.perSlotTokens.some(tokens => tokens <= 0)) {
    failures.push("expected each Codex slot to have a positive verified token count")
  }
  if (input.progressPayloadCount <= 0 || input.progressEventCount <= 0) {
    failures.push("expected live lifecycle progress payloads from stderr streaming")
  }
  if (input.streamedAssignmentRefs.length < TWO_CODEX_READONLY_SMOKE_COUNT) {
    failures.push(`expected streamed lifecycle refs for ${TWO_CODEX_READONLY_SMOKE_COUNT} Codex assignments, got ${input.streamedAssignmentRefs.length}`)
  }
  return failures
}

function blockerRefsFromSummary(summary: string): readonly string[] {
  return summary
    .split(/\r?\n/u)
    .flatMap(line => {
      const match = /^blocker refs:\s*(?<refs>.+)$/iu.exec(line.trim())
      const refs = match?.groups?.refs
      if (refs === undefined || refs === "none") return []
      return refs.split(",").map(ref => ref.trim()).filter(ref => ref.length > 0)
    })
}

function unique(values: readonly string[]): readonly string[] {
  return [...new Set(values)]
}
