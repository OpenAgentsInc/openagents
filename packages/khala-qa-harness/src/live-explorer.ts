import { Data, Effect } from "effect"

import {
  createEmptyKhalaCodeQaCoverageLedger,
  type KhalaCodeQaCoverageFrontierReport,
  type KhalaCodeQaCoverageLedger,
} from "./coverage-ledger.js"
import type { KhalaCodeQaDriver } from "./driver.js"
import {
  khalaCodeQaFrontierFromLedger,
  makeKhalaCodeQaDeterministicFixtureBrain,
  makeKhalaCodeQaLiveLlmExplorerBrain,
  type KhalaCodeQaExplorerBrain,
  type KhalaCodeQaExplorerBrainContext,
} from "./explorer-brain.js"
import {
  buildKhalaCodeQaSeededMonkeyPlanEffect,
  replayKhalaCodeQaSeededMonkeyPlan,
  type KhalaCodeQaMonkeyMode,
  type KhalaCodeQaMonkeyRunPlan,
  type KhalaCodeQaMonkeyRunReport,
} from "./monkey-explorer.js"
import type { KhalaCodeQaDriverMode } from "./scenario.js"

export type KhalaCodeQaLiveExplorerBrainMode = "fake_model" | "live_llm"

export type KhalaCodeQaLiveExplorerGoal = {
  readonly frontierRef: string
  readonly prompt: string
}

export type KhalaCodeQaExplorerChatMessage = {
  readonly role: "system" | "user" | "assistant"
  readonly content: string
}

export type KhalaCodeQaExplorerChatClient = {
  readonly complete: (
    messages: ReadonlyArray<KhalaCodeQaExplorerChatMessage>,
  ) => Promise<string>
}

export type KhalaCodeQaOpenAiExplorerConfig = {
  readonly apiKey?: string
  readonly baseUrl: string
  readonly model: string
  readonly timeoutMs: number
  readonly allowKeyless: boolean
}

export class KhalaCodeQaExplorerTransportFailure extends Data.TaggedError(
  "KhalaCodeQaExplorerTransportFailure",
)<{
  readonly cause?: unknown
  readonly message: string
}> {}

export type KhalaCodeQaLiveExplorerSessionReport = {
  readonly schema: "khala_code_qa_live_explorer_session.v1"
  readonly brainMode: KhalaCodeQaLiveExplorerBrainMode
  readonly driverModes: readonly KhalaCodeQaDriverMode[]
  readonly frontier: KhalaCodeQaCoverageFrontierReport
  readonly goals: readonly KhalaCodeQaLiveExplorerGoal[]
  readonly plan: KhalaCodeQaMonkeyRunPlan
  readonly reports: readonly KhalaCodeQaMonkeyRunReport[]
  readonly seed: string
  readonly status: "pass" | "fail"
}

export type KhalaCodeQaLiveExplorerSessionOptions = {
  readonly brain?: KhalaCodeQaExplorerBrain
  readonly brainMode?: KhalaCodeQaLiveExplorerBrainMode
  readonly decide?: (
    context: KhalaCodeQaExplorerBrainContext,
  ) => Effect.Effect<unknown, unknown>
  readonly drivers: readonly KhalaCodeQaDriver[]
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly mode?: KhalaCodeQaMonkeyMode
  readonly previousCoverageLedger?: KhalaCodeQaCoverageLedger
  readonly seed: string
  readonly steps: number
}

const DEFAULT_KHALA_BASE_URL = "https://openagents.com/api/v1"
const DEFAULT_KHALA_MODEL = "openagents/khala"
const DEFAULT_TIMEOUT_MS = 60_000

const isTruthy = (value: string | undefined): boolean => {
  if (value === undefined) return false
  const normalized = value.trim().toLowerCase()
  return normalized === "1" || normalized === "true" || normalized === "yes" || normalized === "on"
}

const uniqueDriverModes = (
  drivers: readonly KhalaCodeQaDriver[],
): readonly KhalaCodeQaDriverMode[] =>
  [...new Set(drivers.map((driver) => driver.mode))].sort()

const parsePositiveInt = (value: string | undefined): number | undefined => {
  if (value === undefined) return undefined
  const parsed = Number.parseInt(value, 10)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : undefined
}

const labeledValues = (
  label: string,
  values: readonly string[],
): readonly KhalaCodeQaLiveExplorerGoal[] =>
  values.map((value) => ({
    frontierRef: `${label}:${value}`,
    prompt: `Exercise uncovered ${label.slice(0, -1)} "${value}".`,
  }))

export const khalaCodeQaLiveExplorerGoalsFromFrontier = (
  frontier: KhalaCodeQaCoverageFrontierReport,
  limit = 8,
): readonly KhalaCodeQaLiveExplorerGoal[] => {
  const groups = [
    labeledValues("rpcMethods", frontier.missing.rpcMethods),
    labeledValues("hotbarPanels", frontier.missing.hotbarPanels),
    labeledValues("slashCommands", frontier.missing.slashCommands),
    labeledValues("selectors", frontier.missing.selectors),
    labeledValues("settingsKeys", frontier.missing.settingsKeys),
    labeledValues("approvalDecisionKinds", frontier.missing.approvalDecisionKinds),
    labeledValues("threadItemVariants", frontier.missing.threadItemVariants),
  ]
  const goals: KhalaCodeQaLiveExplorerGoal[] = []
  for (let index = 0; goals.length < limit; index += 1) {
    let added = false
    for (const group of groups) {
      const goal = group[index]
      if (goal !== undefined) {
        goals.push(goal)
        added = true
        if (goals.length >= limit) break
      }
    }
    if (!added) break
  }
  return goals
}

export const resolveKhalaCodeQaOpenAiExplorerConfig = (options: {
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly allowKeyless?: boolean
  readonly timeoutMs?: number
} = {}): KhalaCodeQaOpenAiExplorerConfig => {
  const env = options.env ?? (typeof process === "undefined" ? {} : process.env)
  const apiKey = env.KHALA_QA_EXPLORER_API_KEY ?? env.QA_API_KEY ?? env.OPENAI_API_KEY
  const allowKeyless = options.allowKeyless ?? isTruthy(env.KHALA_QA_EXPLORER_ALLOW_KEYLESS)
  if (!allowKeyless && (apiKey === undefined || apiKey.length === 0)) {
    throw new Error("Live QA explorer requires KHALA_QA_EXPLORER_API_KEY, QA_API_KEY, or OPENAI_API_KEY; use fake_model for CI")
  }
  return {
    ...(apiKey === undefined ? {} : { apiKey }),
    allowKeyless,
    baseUrl: env.KHALA_QA_EXPLORER_BASE_URL ?? env.QA_BASE_URL ?? env.OPENAI_BASE_URL ?? DEFAULT_KHALA_BASE_URL,
    model: env.KHALA_QA_EXPLORER_MODEL ?? env.QA_MODEL ?? env.OPENAI_MODEL ?? DEFAULT_KHALA_MODEL,
    timeoutMs: options.timeoutMs ?? parsePositiveInt(env.KHALA_QA_EXPLORER_TIMEOUT_MS) ?? DEFAULT_TIMEOUT_MS,
  }
}

export const makeKhalaCodeQaOpenAiExplorerChatClient = (
  config: KhalaCodeQaOpenAiExplorerConfig,
): KhalaCodeQaExplorerChatClient => ({
  complete: async (messages) => {
    const controller = new AbortController()
    const timer = setTimeout(() => controller.abort(), config.timeoutMs)
    try {
      const response = await fetch(`${config.baseUrl.replace(/\/$/, "")}/chat/completions`, {
        body: JSON.stringify({
          max_tokens: 512,
          messages,
          model: config.model,
          temperature: 0,
        }),
        headers: {
          "content-type": "application/json",
          ...(config.apiKey === undefined ? {} : { authorization: `Bearer ${config.apiKey}` }),
        },
        method: "POST",
        signal: controller.signal,
      })
      if (!response.ok) {
        const body = await response.text().catch(() => "")
        throw new Error(`QA explorer chat/completions HTTP ${response.status}: ${body.slice(0, 300)}`)
      }
      const json = await response.json() as {
        readonly choices?: readonly { readonly message?: { readonly content?: string } }[]
      }
      return json.choices?.[0]?.message?.content ?? ""
    } finally {
      clearTimeout(timer)
    }
  },
})

const compactFrontier = (frontier: KhalaCodeQaCoverageFrontierReport): unknown => ({
  missing: {
    approvalDecisionKinds: frontier.missing.approvalDecisionKinds.slice(0, 12),
    hotbarPanels: frontier.missing.hotbarPanels.slice(0, 12),
    rpcMethods: frontier.missing.rpcMethods.slice(0, 24),
    selectors: frontier.missing.selectors.slice(0, 12),
    settingsKeys: frontier.missing.settingsKeys.slice(0, 12),
    slashCommands: frontier.missing.slashCommands.slice(0, 12),
    threadItemVariants: frontier.missing.threadItemVariants.slice(0, 12),
  },
})

export const makeKhalaCodeQaOpenAiExplorerDecider = (options: {
  readonly chat?: KhalaCodeQaExplorerChatClient
  readonly config?: KhalaCodeQaOpenAiExplorerConfig
  readonly env?: Readonly<Record<string, string | undefined>>
} = {}) => {
  const chat = options.chat ?? makeKhalaCodeQaOpenAiExplorerChatClient(
    options.config ?? resolveKhalaCodeQaOpenAiExplorerConfig({
      ...(options.env === undefined ? {} : { env: options.env }),
    }),
  )
  return (context: KhalaCodeQaExplorerBrainContext): Effect.Effect<unknown, unknown> =>
    Effect.tryPromise({
      catch: (cause) =>
        new KhalaCodeQaExplorerTransportFailure({
          ...(cause === undefined ? {} : { cause }),
          message: cause instanceof Error ? cause.message : String(cause),
        }),
      try: async () => {
        const content = await chat.complete([
          {
            content: [
              "You are the Khala Code QA explorer brain.",
              "Choose exactly one JSON decision with shape:",
              "{\"action\":<one action from actionSpace>,\"frontierRef\":\"optional matching frontier ref\",\"rationale\":\"short reason\",\"tier\":\"live_llm\"}",
              "Prefer actions that exercise missing coverage frontier refs. Output JSON only.",
            ].join("\n"),
            role: "system",
          },
          {
            content: JSON.stringify({
              actionLog: context.actionLog.slice(-12),
              actionSpace: context.actionSpace,
              frontier: compactFrontier(context.frontier),
              prngState: context.prngState,
              stepIndex: context.stepIndex,
            }),
            role: "user",
          },
        ])
        return JSON.parse(content)
      },
    })
}

const brainForSession = (
  options: KhalaCodeQaLiveExplorerSessionOptions,
): KhalaCodeQaExplorerBrain => {
  if (options.brain !== undefined) return options.brain
  if ((options.brainMode ?? "fake_model") === "fake_model") {
    return makeKhalaCodeQaDeterministicFixtureBrain()
  }
  return makeKhalaCodeQaLiveLlmExplorerBrain({
    decide: options.decide ?? makeKhalaCodeQaOpenAiExplorerDecider({
      ...(options.env === undefined ? {} : { env: options.env }),
    }),
    ...(options.env === undefined ? {} : { env: options.env as Record<string, string | undefined> }),
  })
}

export const runKhalaCodeQaLiveExplorerSession = (
  options: KhalaCodeQaLiveExplorerSessionOptions,
): Effect.Effect<KhalaCodeQaLiveExplorerSessionReport, never> => Effect.gen(function* () {
  const mode = options.mode ?? "fixture_smoke"
  const previousCoverageLedger = options.previousCoverageLedger ?? createEmptyKhalaCodeQaCoverageLedger()
  const frontier = khalaCodeQaFrontierFromLedger(previousCoverageLedger)
  const brainMode = options.brainMode ?? "fake_model"
  const plan = yield* buildKhalaCodeQaSeededMonkeyPlanEffect({
    brain: brainForSession(options),
    options: {
      mode,
      previousCoverageLedger,
      seed: options.seed,
      steps: options.steps,
    },
  })
  const driverModes = uniqueDriverModes(options.drivers)
  const replayPlan = {
    ...plan,
    scenario: {
      ...plan.scenario,
      modes: driverModes,
    },
  }
  const reports = yield* Effect.all(
    options.drivers.map((driver) => replayKhalaCodeQaSeededMonkeyPlan({ driver, plan: replayPlan })),
    { concurrency: 1 },
  )
  return {
    brainMode,
    driverModes,
    frontier,
    goals: khalaCodeQaLiveExplorerGoalsFromFrontier(frontier),
    plan: replayPlan,
    reports,
    schema: "khala_code_qa_live_explorer_session.v1",
    seed: options.seed,
    status: reports.length > 0 && reports.every((report) => report.status === "pass") ? "pass" : "fail",
  }
})
