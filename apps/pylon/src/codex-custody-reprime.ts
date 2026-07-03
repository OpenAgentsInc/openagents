import type { ResolvedPylonAccountSelection } from "./account-registry.js"

export const CODEX_CUSTODY_REPRIME_PRE_EXPIRY_BUFFER_MS = 5 * 60 * 1000
export const PYLON_CODEX_CUSTODY_ASSIGNMENT_BLOCKER_REF =
  "blocker.assignment.codex_agent_custody_unavailable"

export type PylonCodexCustodyReprimeBlockerReason =
  | "agent_token_missing"
  | "auth_material_expiring"
  | "auth_material_unavailable"
  | "invalid_auth_material"
  | "network"

export type PylonCodexCustodyReprimeResult =
  | {
      status: "not_applicable"
      env: Record<string, string | undefined>
      blockerRefs: []
    }
  | {
      status: "reprimed"
      env: Record<string, string | undefined>
      expiresAt: number
      blockerRefs: []
    }
  | {
      status: "blocked"
      env: Record<string, string | undefined>
      reason: PylonCodexCustodyReprimeBlockerReason
      blockerRefs: string[]
    }

type ProviderAccountCodexAuthMaterial = {
  authContentEnv: "OPENCODE_AUTH_CONTENT"
  authContentJson: string
}

const endpointPath =
  "/api/pylon/provider-accounts/chatgpt-codex/auth-material"

const blockerRefForReason = (
  reason: PylonCodexCustodyReprimeBlockerReason,
): string => `blocker.pylon.codex_custody.${reason}`

const trimmed = (value: string | undefined): string | undefined => {
  const result = value?.trim()
  return result === undefined || result === "" ? undefined : result
}

const openAgentsBaseUrl = (
  input: {
    baseUrl?: string | undefined
    env: Record<string, string | undefined>
  },
): string =>
  trimmed(input.baseUrl) ??
  trimmed(input.env.PYLON_OPENAGENTS_BASE_URL) ??
  trimmed(input.env.OPENAGENTS_BASE_URL) ??
  "https://openagents.com"

const endpointUrl = (baseUrl: string): string => {
  const normalized = baseUrl.endsWith("/") ? baseUrl : `${baseUrl}/`
  return new URL(endpointPath, normalized).toString()
}

const blocked = (
  env: Record<string, string | undefined>,
  reason: PylonCodexCustodyReprimeBlockerReason,
): Extract<PylonCodexCustodyReprimeResult, { status: "blocked" }> => ({
  status: "blocked",
  env,
  reason,
  blockerRefs: [blockerRefForReason(reason)],
})

const authMaterialFromResponse = (
  value: unknown,
): ProviderAccountCodexAuthMaterial | null => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return null
  }
  const material = (value as { authMaterial?: unknown }).authMaterial
  if (material === null || typeof material !== "object" || Array.isArray(material)) {
    return null
  }
  const authContentEnv = (material as { authContentEnv?: unknown }).authContentEnv
  const authContentJson = (material as { authContentJson?: unknown }).authContentJson
  return authContentEnv === "OPENCODE_AUTH_CONTENT" && typeof authContentJson === "string"
    ? { authContentEnv, authContentJson }
    : null
}

const authMaterialExpiresAt = (
  material: ProviderAccountCodexAuthMaterial,
): number | null => {
  let parsed: unknown
  try {
    parsed = JSON.parse(material.authContentJson) as unknown
  } catch {
    return null
  }
  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
    return null
  }
  const openai = (parsed as { openai?: unknown }).openai
  if (openai === null || typeof openai !== "object" || Array.isArray(openai)) {
    return null
  }
  const record = openai as Record<string, unknown>
  if (typeof record.access !== "string" || record.access.trim() === "") {
    return null
  }
  if (Object.hasOwn(record, "refresh")) {
    return null
  }
  return typeof record.expires === "number" && Number.isFinite(record.expires)
    ? record.expires
    : null
}

export async function reprimePylonCodexAccountAuthFromCustody(input: {
  account: ResolvedPylonAccountSelection | null | undefined
  agentToken?: string | undefined
  baseUrl?: string | undefined
  env: Record<string, string | undefined>
  fetcher?: typeof fetch | undefined
  now?: Date | undefined
}): Promise<PylonCodexCustodyReprimeResult> {
  const env = { ...input.env }
  const providerAccountRef = input.account?.openAgentsProviderAccountRef?.trim()
  if (
    input.account === null ||
    input.account === undefined ||
    input.account.provider !== "codex" ||
    providerAccountRef === undefined ||
    providerAccountRef === ""
  ) {
    return { status: "not_applicable", env, blockerRefs: [] }
  }

  const agentToken = trimmed(input.agentToken) ?? trimmed(env.OPENAGENTS_AGENT_TOKEN)
  if (agentToken === undefined) {
    return blocked(env, "agent_token_missing")
  }

  let response: Response
  try {
    response = await (input.fetcher ?? fetch)(endpointUrl(openAgentsBaseUrl(input)), {
      method: "POST",
      headers: {
        authorization: `Bearer ${agentToken}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({
        accountRef: input.account.accountRef,
        providerAccountRef,
      }),
    })
  } catch {
    return blocked(env, "network")
  }

  if (!response.ok) {
    return blocked(
      env,
      response.status === 404 || response.status === 409
        ? "auth_material_unavailable"
        : "network",
    )
  }

  const body = await response.json().catch((): unknown => null)
  const material = authMaterialFromResponse(body)
  if (material === null) {
    return blocked(env, "invalid_auth_material")
  }

  const expiresAt = authMaterialExpiresAt(material)
  const nowMs = (input.now ?? new Date()).getTime()
  if (expiresAt === null) {
    return blocked(env, "invalid_auth_material")
  }
  if (expiresAt - nowMs <= CODEX_CUSTODY_REPRIME_PRE_EXPIRY_BUFFER_MS) {
    return blocked(env, "auth_material_expiring")
  }

  return {
    status: "reprimed",
    env: {
      ...env,
      [material.authContentEnv]: material.authContentJson,
    },
    expiresAt,
    blockerRefs: [],
  }
}
