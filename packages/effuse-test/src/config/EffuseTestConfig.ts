import { Config, Context, Effect, Layer, Option } from "effect"

export type EffuseTestConfigValue = {
  readonly chromePath?: string
  readonly updateSnapshots: boolean
  readonly e2eBypassSecret?: string
  readonly magicEmail?: string
  readonly magicCode?: string
  readonly childProcessEnv: NodeJS.ProcessEnv
}

export type EffuseTestConfigOverrides = {
  readonly chromePath?: string
  readonly updateSnapshots?: boolean
  readonly e2eBypassSecret?: string
  readonly magicEmail?: string
  readonly magicCode?: string
  readonly childProcessEnv?: NodeJS.ProcessEnv
}

export class EffuseTestConfigError extends Error {
  readonly operation: string
  override readonly cause: unknown

  constructor(operation: string, cause: unknown) {
    const err = cause instanceof Error ? cause : new Error(String(cause))
    super(`[EffuseTestConfig] ${operation}: ${err.message}`)
    this.name = "EffuseTestConfigError"
    this.operation = operation
    this.cause = cause
  }
}

export class EffuseTestConfig extends Context.Tag("@openagentsinc/effuse-test/EffuseTestConfig")<
  EffuseTestConfig,
  EffuseTestConfigValue
>() {}

const normalizeOptionalString = (raw: string | undefined): string | undefined => {
  if (typeof raw !== "string") return undefined
  const trimmed = raw.trim()
  return trimmed.length > 0 ? trimmed : undefined
}

const optionToUndefined = <A>(opt: Option.Option<A>): A | undefined =>
  Option.match(opt, {
    onNone: () => undefined,
    onSome: (value) => value,
  })

const parseBooleanText = (
  key: string,
  raw: string | undefined,
): Effect.Effect<boolean | undefined, EffuseTestConfigError> =>
  Effect.gen(function* () {
    if (raw === undefined) return undefined
    const value = raw.trim().toLowerCase()
    if (value === "1" || value === "true" || value === "yes") return true
    if (value === "0" || value === "false" || value === "no") return false
    return yield* Effect.fail(
      new EffuseTestConfigError(
        `parse boolean ${key}`,
        `Invalid boolean value "${raw}" for ${key}; expected one of: 1,true,yes,0,false,no`,
      ),
    )
  })

export const EffuseTestConfigLive = (
  overrides?: EffuseTestConfigOverrides,
): Layer.Layer<EffuseTestConfig, EffuseTestConfigError> =>
  Layer.effect(
    EffuseTestConfig,
    Effect.gen(function* () {
      const fromEnvChrome = optionToUndefined(
        yield* Config.option(Config.string("EFFUSE_TEST_CHROME_PATH")),
      )
      const fromEnvUpdateSnapshots = optionToUndefined(
        yield* Config.option(Config.string("EFFUSE_TEST_UPDATE_SNAPSHOTS")),
      )
      const fromEnvE2EBypassSecret = optionToUndefined(
        yield* Config.option(Config.string("EFFUSE_TEST_E2E_BYPASS_SECRET")),
      )
      const fromEnvMagicEmail = optionToUndefined(
        yield* Config.option(Config.string("EFFUSE_TEST_MAGIC_EMAIL")),
      )
      const fromEnvMagicCode = optionToUndefined(
        yield* Config.option(Config.string("EFFUSE_TEST_MAGIC_CODE")),
      )

      const updateSnapshots =
        overrides?.updateSnapshots ??
        (yield* parseBooleanText(
          "EFFUSE_TEST_UPDATE_SNAPSHOTS",
          fromEnvUpdateSnapshots,
        )) ??
        false

      const chromePath = normalizeOptionalString(
        overrides?.chromePath ?? fromEnvChrome,
      )
      const e2eBypassSecret = normalizeOptionalString(
        overrides?.e2eBypassSecret ?? fromEnvE2EBypassSecret,
      )
      const magicEmail = normalizeOptionalString(
        overrides?.magicEmail ?? fromEnvMagicEmail,
      )
      const magicCode = normalizeOptionalString(
        overrides?.magicCode ?? fromEnvMagicCode,
      )

      if ((magicEmail && !magicCode) || (!magicEmail && magicCode)) {
        return yield* Effect.fail(
          new EffuseTestConfigError(
            "validate magic login config",
            "EFFUSE_TEST_MAGIC_EMAIL and EFFUSE_TEST_MAGIC_CODE must both be set (or both unset)",
          ),
        )
      }

      return EffuseTestConfig.of({
        ...(chromePath ? { chromePath } : {}),
        updateSnapshots,
        ...(e2eBypassSecret ? { e2eBypassSecret } : {}),
        ...(magicEmail ? { magicEmail } : {}),
        ...(magicCode ? { magicCode } : {}),
        childProcessEnv: overrides?.childProcessEnv ?? process.env,
      })
    }).pipe(
      Effect.mapError((cause) =>
        cause instanceof EffuseTestConfigError
          ? cause
          : new EffuseTestConfigError("load config", cause),
      ),
    ),
  )
