export const KHALA_CODE_ARCHITECT_CODER_JUDGE_PRESET_ID = "architect-coder-judge"
export const KHALA_CODE_ARCHITECT_CODER_JUDGE_PROMISE_REF = "khala_code.architect_coder_judge.v1"
export const KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH = "openagents.model_roles"

export type KhalaCodeDesktopModelRolePresetId = typeof KHALA_CODE_ARCHITECT_CODER_JUDGE_PRESET_ID
export type KhalaCodeDesktopModelRolePresetJsonValue =
  | null
  | boolean
  | number
  | string
  | readonly KhalaCodeDesktopModelRolePresetJsonValue[]
  | { readonly [key: string]: KhalaCodeDesktopModelRolePresetJsonValue }

export type KhalaCodeDesktopModelRolePresetRole = {
  readonly role: "architect" | "coder" | "judge" | "advisor"
  readonly harness: "codex" | "claude"
  readonly authRail: "user_codex_login" | "user_anthropic_auth"
  readonly authority: "executor" | "advisory"
  readonly enabled: boolean
  readonly effort: "xhigh" | "high" | "medium"
  readonly optional: boolean
}

export type KhalaCodeDesktopModelRolePresetRegistry = {
  readonly schema: "openagents.khala_code.model_roles.v1"
  readonly activePreset: KhalaCodeDesktopModelRolePresetId
  readonly copyGate: "copy_gated_until_end_to_end_verifiable"
  readonly noProxyRails: true
  readonly noResale: true
  readonly promiseRef: typeof KHALA_CODE_ARCHITECT_CODER_JUDGE_PROMISE_REF
  readonly roles: readonly KhalaCodeDesktopModelRolePresetRole[]
}

export type KhalaCodeDesktopModelRolePreset = {
  readonly id: KhalaCodeDesktopModelRolePresetId
  readonly title: string
  readonly description: string
  readonly configKeyPath: typeof KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH
  readonly promiseRef: typeof KHALA_CODE_ARCHITECT_CODER_JUDGE_PROMISE_REF
  readonly noProxyRails: true
  readonly noResale: true
  readonly copyGate: "copy_gated_until_end_to_end_verifiable"
  readonly selected: boolean
  readonly roleSummary: readonly string[]
  readonly registry: KhalaCodeDesktopModelRolePresetJsonValue
}

export const makeKhalaCodeArchitectCoderJudgeRegistry =
  (): KhalaCodeDesktopModelRolePresetRegistry => ({
    schema: "openagents.khala_code.model_roles.v1",
    activePreset: KHALA_CODE_ARCHITECT_CODER_JUDGE_PRESET_ID,
    copyGate: "copy_gated_until_end_to_end_verifiable",
    noProxyRails: true,
    noResale: true,
    promiseRef: KHALA_CODE_ARCHITECT_CODER_JUDGE_PROMISE_REF,
    roles: [
      {
        role: "architect",
        harness: "claude",
        authRail: "user_anthropic_auth",
        authority: "advisory",
        enabled: true,
        effort: "xhigh",
        optional: false,
      },
      {
        role: "coder",
        harness: "codex",
        authRail: "user_codex_login",
        authority: "executor",
        enabled: true,
        effort: "xhigh",
        optional: false,
      },
      {
        role: "judge",
        harness: "claude",
        authRail: "user_anthropic_auth",
        authority: "advisory",
        enabled: true,
        effort: "xhigh",
        optional: false,
      },
      {
        role: "advisor",
        harness: "claude",
        authRail: "user_anthropic_auth",
        authority: "advisory",
        enabled: false,
        effort: "high",
        optional: true,
      },
    ],
  })

export const isKhalaCodeArchitectCoderJudgeRegistry = (
  value: unknown,
): value is KhalaCodeDesktopModelRolePresetRegistry => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) return false
  const record = value as Record<string, unknown>
  return record.schema === "openagents.khala_code.model_roles.v1" &&
    record.activePreset === KHALA_CODE_ARCHITECT_CODER_JUDGE_PRESET_ID &&
    record.noProxyRails === true &&
    record.noResale === true &&
    record.promiseRef === KHALA_CODE_ARCHITECT_CODER_JUDGE_PROMISE_REF
}

export const khalaCodeArchitectCoderJudgePreset = (
  selected: boolean,
): KhalaCodeDesktopModelRolePreset => {
  const registry = makeKhalaCodeArchitectCoderJudgeRegistry()
  return {
    id: KHALA_CODE_ARCHITECT_CODER_JUDGE_PRESET_ID,
    title: "Architect / Coder / Judge",
    description: "Claude plans and judges through your Anthropic auth; Codex codes through your existing login.",
    configKeyPath: KHALA_CODE_MODEL_ROLE_REGISTRY_KEY_PATH,
    promiseRef: KHALA_CODE_ARCHITECT_CODER_JUDGE_PROMISE_REF,
    noProxyRails: true,
    noResale: true,
    copyGate: "copy_gated_until_end_to_end_verifiable",
    selected,
    roleSummary: [
      "Architect: Claude, user Anthropic auth, advisory",
      "Coder: Codex, existing local login, executor",
      "Judge: Claude, user Anthropic auth, advisory",
      "Advisor: Claude, optional and off by default",
    ],
    registry,
  }
}

export const parseKhalaCodeModelRolePresetId = (
  value: string,
): KhalaCodeDesktopModelRolePresetId => {
  if (value === KHALA_CODE_ARCHITECT_CODER_JUDGE_PRESET_ID) return value
  throw new Error(`Unsupported Khala Code preset: ${value}`)
}
