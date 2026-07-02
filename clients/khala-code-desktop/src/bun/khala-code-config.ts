import { Config, ConfigProvider, Context, Effect, Layer, Redacted, Schema as S } from "effect"

const StringEnvValue = S.String

export const KhalaCodePlainEnvKeys = [
  "CHAT_ATTACHMENT_TMP_ROOT",
  "CODEX_HOME",
  "CODEX_RPC_TIMEOUT_MS",
  "CODEX_STATE_DB_PATH",
  "CODEX_RATE_LIMIT_RESET_CREDITS_CONSUME_URL",
  "CODEX_RATE_LIMIT_RESET_CREDITS_URL",
  "HOME",
  "INIT_CWD",
  "KHALA_CODE_BUN_BINARY",
  "KHALA_CODE_CODEX_APP_SERVER_FIXTURE",
  "KHALA_CODE_CODEX_APP_SERVER_FIXTURE_PATH",
  "KHALA_CODE_CODEX_APP_SERVER_FIXTURE_SCRIPT",
  "KHALA_CODE_CODEX_APP_SERVER_GAP_DOC_PATH",
  "KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX",
  "KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX_ISSUE",
  "KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX_REFERENCE_COMMIT",
  "KHALA_CODE_CODEX_BINARY",
  "KHALA_CODE_CODEX_COMMAND",
  "KHALA_CODE_CODEX_PARITY_COVERAGE",
  "KHALA_CODE_CODEX_PARITY_REFERENCE_COMMIT",
  "KHALA_CODE_CODEX_PARITY_REFERENCE_LABEL",
  "KHALA_CODE_CODEX_PARITY_REQUIRED_CLIENT_METHODS",
  "KHALA_CODE_CODEX_PARITY_REQUIRED_NOTIFICATIONS",
  "KHALA_CODE_CODEX_PARITY_REQUIRED_SCHEMA_FILES",
  "KHALA_CODE_CODEX_PARITY_REQUIRED_SERVER_REQUESTS",
  "KHALA_CODE_CODEX_PARITY_REQUIRED_THREAD_ITEM_TYPES",
  "KHALA_CODE_CODEX_REFERENCE_ROOT",
  "KHALA_CODE_CODEX_REFERENCE_SCHEMA_DIR",
  "KHALA_CODE_CODEX_STATE_DB_PATH",
  "KHALA_CODE_DEBUG_APP_SERVER",
  "KHALA_CODE_DESKTOP_BACKEND",
  "KHALA_CODE_DESKTOP_BUNDLED_SKILLS",
  "KHALA_CODE_DESKTOP_BUN_COMMAND",
  "KHALA_CODE_DESKTOP_CODEX_PARITY_LIVE_SMOKE",
  "KHALA_CODE_DESKTOP_CODEX_STATE_PATH",
  "KHALA_CODE_DESKTOP_CONTEXT_KEEP_TAIL_COUNT",
  "KHALA_CODE_DESKTOP_CONTEXT_MAX_TOKENS",
  "KHALA_CODE_DESKTOP_FLEET_MCP_BRIDGE",
  "KHALA_CODE_DESKTOP_LEGACY_KHALA_NATIVE_RUNTIME",
  "KHALA_CODE_DESKTOP_OPEN_WINDOW",
  "KHALA_CODE_DESKTOP_PREVIEW_PORT",
  "KHALA_CODE_DESKTOP_PREVIEW_READONLY",
  "KHALA_CODE_DESKTOP_PREVIEW_SERVER",
  "KHALA_CODE_DESKTOP_RUNTIME",
  "KHALA_CODE_DESKTOP_WORKSPACE",
  "KHALA_CODE_HEADLESS_INTERRUPT_AFTER_MS",
  "KHALA_CODE_HOSTED_BYOK_OPENROUTER",
  "KHALA_CODE_MESSAGE_AUDIT_LOCAL_LEDGER_PATH",
  "KHALA_CODE_MESSAGE_TOKEN_AUDIT_LOCAL_LEDGER_PATH",
  "KHALA_CODE_SYSTEM_PROMPT",
  "KHALA_CODE_TOKEN_USAGE_BACKGROUND_SYNC_DISABLED",
  "KHALA_CODE_TOKEN_USAGE_BASE_URL",
  "KHALA_CODE_TOKEN_USAGE_DISABLED",
  "KHALA_CODE_TOKEN_USAGE_LOCAL_LEDGER_PATH",
  "KHALA_CODE_TOKEN_USAGE_REMOTE_DISABLED",
  "KHALA_CODE_TOKEN_USAGE_SECRET_DISABLED",
  "KHALA_CODE_TOKEN_USAGE_SECRET_PATH",
  "KHALA_CODE_TOKEN_USAGE_SYNC_INTERVAL_MS",
  "KHALA_GPT_OSS_BASE_URL",
  "KHALA_GPT_OSS_MODEL",
  "LOG_FORMAT",
  "OPENAGENTS_APPLE_FM_BRIDGE_PATH",
  "OPENAGENTS_BASE_URL",
  "OPENAGENTS_BUN_PATH",
  "OPENAGENTS_PYLON_APP_PATH",
  "OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY",
  "OPENAGENTS_PYLON_CODEX_BUSY",
  "OPENAGENTS_PYLON_CODEX_CONCURRENCY",
  "OPENAGENTS_PYLON_CODEX_QUEUED",
  "OPENAGENTS_PYLON_DISABLE_ASSIGNMENT_PR",
  "OPENAGENTS_PYLON_DISABLE_PR_TITLE_MODEL",
  "OPENAGENTS_REPO_ROOT",
  "PATH",
  "PROBE_OMEGA_BASE_URL",
  "PWD",
  "PYLON_ACCOUNT_HOME_ROOT",
  "PYLON_CONTROL_HOST",
  "PYLON_CONTROL_PORT",
  "PYLON_CONTROL_URL",
  "PYLON_FABLE_HOME",
  "PYLON_HOME",
  "PYLON_OPENAGENTS_BASE_URL",
] as const

export const KhalaCodeSecretEnvKeys = [
  "KHALA_CODE_DESKTOP_PREVIEW_RPC_TOKEN",
  "KHALA_CODE_HOSTED_BYOK_OPENROUTER_API_KEY",
  "KHALA_CODE_TOKEN_USAGE_BEARER_TOKEN",
  "KHALA_GPT_OSS_API_KEY",
  "OPENAGENTS_ADMIN_API_TOKEN",
  "OPENAGENTS_AGENT_TOKEN",
  "OPENAGENTS_API_KEY",
  "OPENROUTER_API_KEY",
  "PROBE_TOKEN_USAGE_BEARER_TOKEN",
  "PYLON_CONTROL_TOKEN",
] as const

export type KhalaCodePlainEnvKey = typeof KhalaCodePlainEnvKeys[number]
export type KhalaCodeSecretEnvKey = typeof KhalaCodeSecretEnvKeys[number]
export type KhalaCodeEnvKey = KhalaCodePlainEnvKey | KhalaCodeSecretEnvKey

export type KhalaCodePlainConfig = Readonly<Record<KhalaCodePlainEnvKey, string>>
export type KhalaCodeSecretConfig = Readonly<Record<KhalaCodeSecretEnvKey, Redacted.Redacted<string>>>
export type KhalaCodeConfigEnv = NodeJS.ProcessEnv & Partial<Record<KhalaCodeEnvKey, string>>

export type KhalaCodeConfigServiceShape = Readonly<{
  env: KhalaCodeConfigEnv
  plain: KhalaCodePlainConfig
  secrets: KhalaCodeSecretConfig
}>

const emptyRedacted = Redacted.make("")

const plainConfig = Object.fromEntries(
  KhalaCodePlainEnvKeys.map((key) => [
    key,
    Config.schema(StringEnvValue, key).pipe(Config.withDefault("")),
  ]),
) as { readonly [Key in KhalaCodePlainEnvKey]: Config.Config<string> }

const secretConfig = Object.fromEntries(
  KhalaCodeSecretEnvKeys.map((key) => [
    key,
    Config.redacted(key).pipe(Config.withDefault(emptyRedacted)),
  ]),
) as { readonly [Key in KhalaCodeSecretEnvKey]: Config.Config<Redacted.Redacted<string>> }

export const KhalaCodeConfigSchema = Config.all({
  plain: Config.all(plainConfig),
  secrets: Config.all(secretConfig),
})

const definedEnvEntries = (
  env: Readonly<Record<string, string | undefined>>,
): Record<string, string> =>
  Object.fromEntries(
    Object.entries(env).filter((entry): entry is [string, string] => entry[1] !== undefined),
  )

export const khalaCodeConfigProviderFromEnv = (
  env: Readonly<Record<string, string | undefined>>,
): ConfigProvider.ConfigProvider =>
  ConfigProvider.fromEnv({ env: definedEnvEntries(env) })

export const makeKhalaCodeConfig = (input: {
  sourceEnv?: Readonly<Record<string, string | undefined>>
  readonly plain: KhalaCodePlainConfig
  readonly secrets: KhalaCodeSecretConfig
}): KhalaCodeConfigServiceShape => {
  const service = {
    plain: input.plain,
    secrets: input.secrets,
  } as KhalaCodeConfigServiceShape

  Object.defineProperty(service, "env", {
    enumerable: false,
    get: () => {
      // Spawn-interop surface: children must inherit the FULL runtime env
      // (TMPDIR, SSH_AUTH_SOCK, GH_TOKEN, proxy/CA vars, ...), with the
      // declared keys overlaid. Narrowing to declared keys alone broke
      // git-over-SSH and gh auth inside Codex sessions (review finding).
      const env: Record<string, string> = {}
      for (const [key, value] of Object.entries(input.sourceEnv ?? {})) {
        if (typeof value === "string") env[key] = value
      }
      for (const key of KhalaCodePlainEnvKeys) {
        const value = input.plain[key]
        if (value.length > 0) env[key] = value
      }
      for (const key of KhalaCodeSecretEnvKeys) {
        const value = Redacted.value(input.secrets[key])
        if (value.length > 0) env[key] = value
      }
      return env as KhalaCodeConfigEnv
    },
  })

  return service
}

export const khalaCodeConfigFromEnv = (
  env: Readonly<Record<string, string | undefined>>,
): KhalaCodeConfigServiceShape =>
  Effect.runSync(
    KhalaCodeConfigSchema.pipe(
      Effect.map((parsed) => makeKhalaCodeConfig({ ...parsed, sourceEnv: env })),
      Effect.provideService(ConfigProvider.ConfigProvider, khalaCodeConfigProviderFromEnv(env)),
    ),
  )

export const khalaCodeConfigFromRuntimeEnv = (): KhalaCodeConfigServiceShape =>
  khalaCodeConfigFromEnv(typeof Bun === "undefined" ? process.env : Bun.env)

export class KhalaCodeConfig extends Context.Service<
  KhalaCodeConfig,
  KhalaCodeConfigServiceShape
>()("@openagentsinc/khala-code-desktop/KhalaCodeConfig") {
  static readonly fromEnv = (
    env: Readonly<Record<string, string | undefined>>,
  ) =>
    Layer.effect(
      KhalaCodeConfig,
      KhalaCodeConfigSchema.pipe(
        Effect.map((parsed) => makeKhalaCodeConfig({ ...parsed, sourceEnv: env })),
        Effect.provideService(ConfigProvider.ConfigProvider, khalaCodeConfigProviderFromEnv(env)),
      ),
    )

  static readonly testProfile = (
    env: Readonly<Record<string, string | undefined>> = {},
  ): Layer.Layer<KhalaCodeConfig> =>
    Layer.succeed(KhalaCodeConfig, khalaCodeConfigFromEnv(env))
}

export const KhalaCodeConfigLive = Layer.effect(
  KhalaCodeConfig,
  KhalaCodeConfigSchema.pipe(
    Effect.map((parsed) =>
      makeKhalaCodeConfig({
        ...parsed,
        sourceEnv: typeof Bun === "undefined" ? process.env : Bun.env,
      }),
    ),
  ),
)
