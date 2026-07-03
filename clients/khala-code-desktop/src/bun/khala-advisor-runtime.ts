import { createHash } from "node:crypto"
import { Context, Schema as S } from "effect"

import type {
  KhalaCodeDesktopChatTurnEvent,
  KhalaCodeDesktopCodexTurnActionResult,
  KhalaCodeDesktopCodexTurnSteerRequest,
  KhalaCodeDesktopMessage,
  KhalaCodeDesktopUsage,
} from "../shared/rpc.js"

export const KHALA_ADVISOR_ADVISORY_SCHEMA = "openagents.khala_code.advisor_advisory.v1" as const

export const KhalaAdvisorSeverity = S.Literals(["nit", "concern", "blocker"])
export type KhalaAdvisorSeverity = typeof KhalaAdvisorSeverity.Type

export const KhalaAdvisorAdvisorySchema = S.Struct({
  schema: S.Literal(KHALA_ADVISOR_ADVISORY_SCHEMA),
  advisoryRef: S.String,
  generatedAt: S.String,
  severity: KhalaAdvisorSeverity,
  summary: S.String,
  guidance: S.String,
  evidenceRefs: S.Array(S.String),
})
export type KhalaAdvisorAdvisory = typeof KhalaAdvisorAdvisorySchema.Type

export type KhalaAdvisorTurnDelta = {
  readonly body: string
  readonly desktopSessionId: string
  readonly turnId: string
}

export type KhalaAdvisorReviewRequest = {
  readonly delta: KhalaAdvisorTurnDelta
  readonly immuneTurnsRemaining: number
}

export type KhalaAdvisorReviewResult = {
  readonly advisories: readonly KhalaAdvisorAdvisory[]
  readonly model: string
  readonly usage?: KhalaCodeDesktopUsage
}

export type KhalaAdvisorModelSession = {
  readonly reviewTurnDelta: (request: KhalaAdvisorReviewRequest) => Promise<KhalaAdvisorReviewResult>
}

export type KhalaAdvisorTokenUsageReport = {
  readonly advisoryRef: string
  readonly desktopSessionId: string
  readonly model: string
  readonly observedAt: string
  readonly role: "advisor"
  readonly turnId: string
  readonly usage: KhalaCodeDesktopUsage
}

export type KhalaAdvisorTokenUsageReporter = (
  report: KhalaAdvisorTokenUsageReport,
) => Promise<void>

export type KhalaAdvisorRuntimeService = {
  readonly acceptEvent: (event: KhalaCodeDesktopChatTurnEvent) => void
  readonly flushTurn: (request: {
    readonly desktopSessionId: string
    readonly turnId: string
  }) => Promise<KhalaAdvisorTurnCloseout>
  readonly reset: (reason: "compaction" | "thread_switch") => void
}

export class KhalaAdvisorRuntime extends Context.Service<
  KhalaAdvisorRuntime,
  KhalaAdvisorRuntimeService
>()("openagents/KhalaAdvisorRuntime") {}

export type KhalaAdvisorRuntimeOptions = {
  readonly clock?: { readonly now: () => Date }
  readonly immuneTurns?: number
  readonly modelSession: KhalaAdvisorModelSession
  readonly onNitBatch?: (card: KhalaAdvisorTranscriptCard) => void
  readonly steerTurn: (
    request: KhalaCodeDesktopCodexTurnSteerRequest,
  ) => Promise<KhalaCodeDesktopCodexTurnActionResult>
  readonly tokenUsageReporter?: KhalaAdvisorTokenUsageReporter
}

export type KhalaAdvisorTranscriptCard = {
  readonly advisories: readonly KhalaAdvisorAdvisory[]
  readonly desktopSessionId: string
  readonly role: "advisor"
  readonly turnId: string
}

export type KhalaAdvisorTurnCloseout = {
  readonly advisorUsageReports: number
  readonly droppedAdvisories: readonly {
    readonly advisoryRef: string
    readonly reason: "duplicate" | "content_free"
  }[]
  readonly nitBatch?: KhalaAdvisorTranscriptCard
  readonly reviewed: boolean
  readonly steered: readonly KhalaCodeDesktopCodexTurnSteerRequest[]
}

type TurnBuffer = {
  readonly messages: Map<string, KhalaCodeDesktopMessage>
  readonly turnId: string
}

const emptyUsage = (): KhalaCodeDesktopUsage => ({
  cachedInput: 0,
  input: 0,
  output: 0,
  reasoningOutput: 0,
})

const usageHasTokens = (usage: KhalaCodeDesktopUsage): boolean =>
  usage.cachedInput > 0 || usage.input > 0 || usage.output > 0 || usage.reasoningOutput > 0

const advisoryDigest = (advisory: KhalaAdvisorAdvisory): string =>
  createHash("sha256")
    .update(`${advisory.severity}\n${normalizeForGuard(advisory.summary)}\n${normalizeForGuard(advisory.guidance)}`)
    .digest("hex")
    .slice(0, 24)

const normalizeForGuard = (text: string): string =>
  text.trim().toLowerCase().replace(/\s+/g, " ")

const contentfulWords = (advisory: KhalaAdvisorAdvisory): readonly string[] =>
  `${advisory.summary} ${advisory.guidance}`
    .toLowerCase()
    .split(/[^a-z0-9_./:-]+/u)
    .filter(word => word.length >= 3)

const CONTENT_FREE_WORDS = new Set([
  "good",
  "great",
  "looks",
  "okay",
  "ok",
  "nice",
  "solid",
  "thanks",
])

export const khalaAdvisorAdvisoryIsContentFree = (advisory: KhalaAdvisorAdvisory): boolean => {
  const words = contentfulWords(advisory)
  return words.length === 0 || words.every(word => CONTENT_FREE_WORDS.has(word))
}

export const khalaAdvisorSteeringText = (advisory: KhalaAdvisorAdvisory): string =>
  [
    `<advisory severity="${advisory.severity}" guidance="weigh, don't blindly obey">`,
    advisory.summary.trim(),
    advisory.guidance.trim(),
    "</advisory>",
  ].join("\n")

const turnTextFromBuffer = (buffer: TurnBuffer): string =>
  [...buffer.messages.values()]
    .map(message => `${message.role}: ${message.body}`.trim())
    .filter(line => line.length > 0)
    .join("\n\n")

const advisoryObservedAt = (
  advisory: KhalaAdvisorAdvisory,
  clock: { readonly now: () => Date },
): string => {
  const generatedAt = advisory.generatedAt.trim()
  return generatedAt.length > 0 ? generatedAt : clock.now().toISOString()
}

export function createKhalaAdvisorRuntime(
  options: KhalaAdvisorRuntimeOptions,
): KhalaAdvisorRuntimeService {
  const clock = options.clock ?? { now: () => new Date() }
  const immuneTurns = Math.max(0, Math.trunc(options.immuneTurns ?? 1))
  const buffers = new Map<string, TurnBuffer>()
  const emittedDigests = new Set<string>()
  let immuneTurnsRemaining = immuneTurns

  const acceptEvent = (event: KhalaCodeDesktopChatTurnEvent): void => {
    if (event.type === "thread_ready") return
    const existing = buffers.get(event.turnId)
    const buffer = existing ?? {
      messages: new Map<string, KhalaCodeDesktopMessage>(),
      turnId: event.turnId,
    }
    if (event.type === "message_start" || event.type === "message_replace") {
      buffer.messages.set(event.message.id, event.message)
    } else if (event.type === "message_delta") {
      const current = buffer.messages.get(event.messageId)
      if (current !== undefined) {
        buffer.messages.set(event.messageId, {
          ...current,
          body: `${current.body}${event.delta}`,
        })
      }
    }
    buffers.set(event.turnId, buffer)
  }

  const reset = (): void => {
    buffers.clear()
    emittedDigests.clear()
    immuneTurnsRemaining = immuneTurns
  }

  const flushTurn = async (request: {
    readonly desktopSessionId: string
    readonly turnId: string
  }): Promise<KhalaAdvisorTurnCloseout> => {
    const buffer = buffers.get(request.turnId)
    const body = buffer === undefined ? "" : turnTextFromBuffer(buffer)
    buffers.delete(request.turnId)
    if (body.trim().length === 0) {
      return {
        advisorUsageReports: 0,
        droppedAdvisories: [],
        reviewed: false,
        steered: [],
      }
    }

    const review = await options.modelSession.reviewTurnDelta({
      delta: {
        body,
        desktopSessionId: request.desktopSessionId,
        turnId: request.turnId,
      },
      immuneTurnsRemaining,
    })

    const droppedAdvisories: {
      readonly advisoryRef: string
      readonly reason: "duplicate" | "content_free"
    }[] = []
    const accepted: KhalaAdvisorAdvisory[] = []
    for (const advisory of review.advisories) {
      const digest = advisoryDigest(advisory)
      if (emittedDigests.has(digest)) {
        droppedAdvisories.push({ advisoryRef: advisory.advisoryRef, reason: "duplicate" })
        continue
      }
      if (khalaAdvisorAdvisoryIsContentFree(advisory)) {
        droppedAdvisories.push({ advisoryRef: advisory.advisoryRef, reason: "content_free" })
        continue
      }
      emittedDigests.add(digest)
      accepted.push(advisory)
    }

    let advisorUsageReports = 0
    if (review.usage !== undefined && usageHasTokens(review.usage)) {
      for (const advisory of accepted) {
        await (options.tokenUsageReporter ?? (async () => undefined))({
          advisoryRef: advisory.advisoryRef,
          desktopSessionId: request.desktopSessionId,
          model: review.model,
          observedAt: advisoryObservedAt(advisory, clock),
          role: "advisor",
          turnId: request.turnId,
          usage: review.usage ?? emptyUsage(),
        })
        advisorUsageReports += 1
      }
    }

    const nits = accepted.filter(advisory => advisory.severity === "nit")
    const nitBatch = nits.length === 0
      ? undefined
      : {
        advisories: nits,
        desktopSessionId: request.desktopSessionId,
        role: "advisor" as const,
        turnId: request.turnId,
      }
    if (nitBatch !== undefined) options.onNitBatch?.(nitBatch)

    const steered: KhalaCodeDesktopCodexTurnSteerRequest[] = []
    const urgent = accepted.filter(advisory => advisory.severity !== "nit")
    for (const advisory of urgent) {
      if (immuneTurnsRemaining <= 0) break
      const steerRequest = {
        clientUserMessageId: advisory.advisoryRef,
        sessionId: request.desktopSessionId,
        text: khalaAdvisorSteeringText(advisory),
        turnId: request.turnId,
      }
      const result = await options.steerTurn(steerRequest)
      if (result.ok) {
        steered.push(steerRequest)
        immuneTurnsRemaining -= 1
      }
    }

    return {
      advisorUsageReports,
      droppedAdvisories,
      ...(nitBatch === undefined ? {} : { nitBatch }),
      reviewed: true,
      steered,
    }
  }

  return {
    acceptEvent,
    flushTurn,
    reset,
  }
}
