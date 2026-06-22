import { Context, Effect, Layer } from "effect"

export type WorldModerationSurface =
  | "local_chat"
  | "pylon_chat"
  | "forum_reflection_bubble"
  | "user_diagnostic"

export type WorldModerationConfig = Readonly<{
  hardBlockedTokens: ReadonlyArray<string>
  softMaskedTokens: ReadonlyArray<string>
}>

export type WorldModerationActorState = Readonly<{
  strikes: number
  mutedUntil?: string
  lastViolationAt?: string
}>

export type WorldModerationSessionState = Readonly<{
  strikes: number
  mutedUntil?: string
  lastViolationAt?: string
}>

export type WorldModerationState = Readonly<{
  byActor: Readonly<Record<string, WorldModerationActorState>>
  bySession: Readonly<Record<string, WorldModerationSessionState>>
}>

export type WorldModerationSubject = Readonly<{
  actorRef: string
  sessionRef: string
  surface: WorldModerationSurface
}>

export type WorldModerationDecision =
  | Readonly<{
      kind: "allowed"
      text: string
      state: WorldModerationState
    }>
  | Readonly<{
      kind: "blocked"
      publicReasonCode: "hard_token" | "muted" | "empty"
      publicMessage: string
      state: WorldModerationState
      mutedUntil?: string
    }>

export type WorldModerationShape = Readonly<{
  moderateText(input: {
    readonly text: string
    readonly subject: WorldModerationSubject
    readonly state: WorldModerationState
    readonly observedAt: string
  }): Effect.Effect<WorldModerationDecision>
  maskSoftTokens(input: {
    readonly text: string
    readonly enabled: boolean
  }): Effect.Effect<string>
  redactDiagnosticText(input: {
    readonly text: string
  }): Effect.Effect<string>
}>

export class WorldModeration extends Context.Service<
  WorldModeration,
  WorldModerationShape
>()("WorldModeration") {}

export const emptyWorldModerationState: WorldModerationState = {
  byActor: {},
  bySession: {},
}

export const emptyWorldModerationConfig: WorldModerationConfig = {
  hardBlockedTokens: [],
  softMaskedTokens: [],
}

export const makeWorldModeration = (
  config: WorldModerationConfig,
): WorldModerationShape => {
  const hardTokens = normalizeTokenList(config.hardBlockedTokens)
  const softTokens = normalizeTokenList(config.softMaskedTokens)

  return {
    moderateText: input => Effect.sync(() => moderateWorldText(input, hardTokens)),
    maskSoftTokens: input => Effect.sync(() =>
      input.enabled ? maskTokens(input.text, softTokens) : input.text
    ),
    redactDiagnosticText: input => Effect.sync(() => redactUserAuthoredDiagnosticText(input.text)),
  }
}

export const WorldModerationLive = Layer.succeed(
  WorldModeration,
  makeWorldModeration(emptyWorldModerationConfig),
)

export const makeWorldModerationLayer = (
  config: WorldModerationConfig,
) => Layer.succeed(WorldModeration, makeWorldModeration(config))

export const moderationConfigFromEnv = (env: {
  readonly OPENAGENTS_WORLD_MODERATION_HARD_TOKENS_JSON?: string
  readonly OPENAGENTS_WORLD_MODERATION_SOFT_TOKENS_JSON?: string
}): WorldModerationConfig => ({
  hardBlockedTokens: parseTokenSeed(env.OPENAGENTS_WORLD_MODERATION_HARD_TOKENS_JSON),
  softMaskedTokens: parseTokenSeed(env.OPENAGENTS_WORLD_MODERATION_SOFT_TOKENS_JSON),
})

export const moderateTextWithPolicy = (
  input: Parameters<WorldModerationShape["moderateText"]>[0],
  config: WorldModerationConfig,
): WorldModerationDecision =>
  Effect.runSync(Effect.provide(
    Effect.gen(function* () {
      const moderation = yield* WorldModeration
      return yield* moderation.moderateText(input)
    }),
    makeWorldModerationLayer(config),
  ))

export const moderateTextWithDefaultPolicy = (
  input: Parameters<WorldModerationShape["moderateText"]>[0],
): WorldModerationDecision =>
  Effect.runSync(Effect.provide(
    Effect.gen(function* () {
      const moderation = yield* WorldModeration
      return yield* moderation.moderateText(input)
    }),
    WorldModerationLive,
  ))

export const maskSoftTokensWithDefaultPolicy = (
  text: string,
  enabled: boolean,
): string =>
  Effect.runSync(Effect.provide(
    Effect.gen(function* () {
      const moderation = yield* WorldModeration
      return yield* moderation.maskSoftTokens({ text, enabled })
    }),
    WorldModerationLive,
  ))

export const redactDiagnosticTextWithDefaultPolicy = (
  text: string,
): string =>
  Effect.runSync(Effect.provide(
    Effect.gen(function* () {
      const moderation = yield* WorldModeration
      return yield* moderation.redactDiagnosticText({ text })
    }),
    WorldModerationLive,
  ))

export const moderateForumReflectionBubbleWithDefaultPolicy = (input: {
  readonly text: string
  readonly actorRef: string
  readonly sessionRef: string
  readonly state: WorldModerationState
  readonly observedAt: string
}): WorldModerationDecision =>
  moderateTextWithDefaultPolicy({
    text: input.text,
    state: input.state,
    observedAt: input.observedAt,
    subject: {
      actorRef: input.actorRef,
      sessionRef: input.sessionRef,
      surface: "forum_reflection_bubble",
    },
  })

export const confusableFold = (input: string): string =>
  input
    .normalize("NFKD")
    .toLowerCase()
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[＠@]/g, "a")
    .replace(/[4]/g, "a")
    .replace(/[3]/g, "e")
    .replace(/[1!|]/g, "i")
    .replace(/[0]/g, "o")
    .replace(/[5$]/g, "s")
    .replace(/[7]/g, "t")
    .replace(/[а]/g, "a")
    .replace(/[е]/g, "e")
    .replace(/[о]/g, "o")
    .replace(/[р]/g, "p")
    .replace(/[с]/g, "c")
    .replace(/[у]/g, "y")
    .replace(/[х]/g, "x")

export const tokenizeModerationText = (input: string): ReadonlyArray<string> =>
  confusableFold(input)
    .split(/[^a-z0-9]+/g)
    .filter(token => token.length > 0)

export const redactUserAuthoredDiagnosticText = (input: string): string => {
  if (input.length > 160 || unsafeDiagnosticMaterial(input)) {
    return "User-authored diagnostic text redacted."
  }
  return input.replace(/[\u0000-\u001f\u007f<>]/g, "").trim()
}

const moderateWorldText = (
  input: Parameters<WorldModerationShape["moderateText"]>[0],
  hardTokens: ReadonlySet<string>,
): WorldModerationDecision => {
  const text = redactPlainWorldText(input.text)
  if (text.length === 0) {
    return {
      kind: "blocked",
      publicReasonCode: "empty",
      publicMessage: "Message rejected.",
      state: input.state,
    }
  }

  const actorState = input.state.byActor[input.subject.actorRef] ?? { strikes: 0 }
  const sessionState = input.state.bySession[input.subject.sessionRef] ?? { strikes: 0 }
  if (isMuted(actorState, input.observedAt) || isMuted(sessionState, input.observedAt)) {
    const mutedUntil = laterIso(actorState.mutedUntil, sessionState.mutedUntil)
    return {
      kind: "blocked",
      publicReasonCode: "muted",
      publicMessage: "Message rejected because chat is temporarily muted.",
      state: input.state,
      ...(mutedUntil === undefined ? {} : { mutedUntil }),
    }
  }

  const hasHardToken = tokenizeModerationText(text).some(token => hardTokens.has(token))
  if (!hasHardToken) {
    return {
      kind: "allowed",
      text,
      state: input.state,
    }
  }

  const nextActor = escalateModerationState(actorState, input.observedAt)
  const nextSession = escalateModerationState(sessionState, input.observedAt)
  return {
    kind: "blocked",
    publicReasonCode: "hard_token",
    publicMessage: nextActor.strikes <= 1
      ? "Message rejected by the public world safety policy."
      : "Message rejected and chat is temporarily muted.",
    state: {
      byActor: {
        ...input.state.byActor,
        [input.subject.actorRef]: nextActor,
      },
      bySession: {
        ...input.state.bySession,
        [input.subject.sessionRef]: nextSession,
      },
    },
    ...(nextActor.mutedUntil === undefined ? {} : { mutedUntil: nextActor.mutedUntil }),
  }
}

function normalizeTokenList(tokens: ReadonlyArray<string>): ReadonlySet<string> {
  return new Set(tokens.flatMap(token => tokenizeModerationText(token)).filter(token => token.length > 0))
}

function redactPlainWorldText(value: string): string {
  return value
    .replace(/[\u0000-\u001f\u007f<>]/g, "")
    .replace(/\s+/g, " ")
    .trim()
}

function maskTokens(input: string, softTokens: ReadonlySet<string>): string {
  if (softTokens.size === 0) {
    return input
  }
  return input.split(/(\s+)/g).map(part => {
    const tokens = tokenizeModerationText(part)
    return tokens.some(token => softTokens.has(token)) ? "*".repeat(Math.max(3, part.length)) : part
  }).join("")
}

function escalateModerationState(
  state: WorldModerationActorState,
  observedAt: string,
): WorldModerationActorState {
  const strikes = state.strikes + 1
  return {
    strikes,
    lastViolationAt: observedAt,
    ...(muteDurationMs(strikes) === null
      ? {}
      : { mutedUntil: new Date(Date.parse(observedAt) + muteDurationMs(strikes)!).toISOString() }),
  }
}

function muteDurationMs(strikes: number): number | null {
  if (strikes <= 1) {
    return null
  }
  if (strikes === 2) {
    return 10 * 60 * 1000
  }
  if (strikes === 3) {
    return 60 * 60 * 1000
  }
  return 24 * 60 * 60 * 1000
}

function isMuted(
  state: WorldModerationActorState | WorldModerationSessionState,
  observedAt: string,
): boolean {
  return state.mutedUntil !== undefined && Date.parse(state.mutedUntil) > Date.parse(observedAt)
}

function laterIso(left: string | undefined, right: string | undefined): string | undefined {
  if (left === undefined) return right
  if (right === undefined) return left
  return Date.parse(left) >= Date.parse(right) ? left : right
}

function unsafeDiagnosticMaterial(input: string): boolean {
  return /raw_prompt|raw_shell_log|provider_payload|secret|\/Users\/|sk-[a-z0-9_-]+/i.test(input)
}

function parseTokenSeed(raw: string | undefined): ReadonlyArray<string> {
  if (raw === undefined || raw.trim().length === 0) {
    return []
  }
  try {
    const parsed = JSON.parse(raw) as unknown
    if (!Array.isArray(parsed)) {
      return []
    }
    return parsed
      .filter((value): value is string => typeof value === "string")
      .map(value => value.trim())
      .filter(value => value.length > 0)
      .slice(0, 256)
  } catch {
    return []
  }
}
