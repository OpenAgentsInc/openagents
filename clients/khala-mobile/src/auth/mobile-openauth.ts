import { Schema as S } from "effect"

import { mobileProblemMessageSafe, readOkMobileJsonResponse } from "../network/mobile-problem"

export const KHALA_MOBILE_OPENAUTH_CLIENT_ID = "openagents-khala-mobile"
export const KHALA_MOBILE_OPENAUTH_PROVIDER = "github"
export const KHALA_MOBILE_OPENAUTH_REDIRECT_SCHEME = "khala"
export const KHALA_MOBILE_OPENAUTH_REDIRECT_PATH = "auth"

export type MobileOpenAuthDiscovery = Readonly<{
  authorizationEndpoint: string
  tokenEndpoint: string
}>

export type MobileOpenAuthRequestConfig = Readonly<{
  clientId: string
  codeChallengeMethod: "S256"
  extraParams: Readonly<{ provider: typeof KHALA_MOBILE_OPENAUTH_PROVIDER }>
  redirectUri: string
  responseType: "code"
  usePKCE: true
}>

export type MobileOpenAuthTokenExchangeConfig = Readonly<{
  clientId: string
  code: string
  extraParams: Readonly<{ code_verifier: string }>
  redirectUri: string
}>

export type MobileSyncSession = Readonly<{
  ownerUserId: string
  syncToken: string
}>

export type FetchLike = (
  url: string,
  init: RequestInit,
) => Promise<Pick<Response, "json" | "ok" | "status" | "statusText">>

const MobileSyncSessionSchema = S.Struct({
  ownerUserId: S.String,
  syncToken: S.String,
})

export const normalizeHttpsBaseUrl = (value: string, label: string): string => {
  const trimmed = value.trim()

  if (trimmed.length === 0) {
    throw new Error(`${label} must be a non-empty HTTPS URL`)
  }

  const parsed = new URL(trimmed)

  if (parsed.protocol !== "https:") {
    throw new Error(`${label} must use https`)
  }

  parsed.pathname = parsed.pathname.replace(/\/$/, "")
  parsed.search = ""
  parsed.hash = ""

  return parsed.toString().replace(/\/$/, "")
}

export const mobileOpenAuthDiscovery = (
  authBaseUrl: string,
): MobileOpenAuthDiscovery => {
  const base = normalizeHttpsBaseUrl(authBaseUrl, "OpenAuth base URL")

  return {
    authorizationEndpoint: `${base}/authorize`,
    tokenEndpoint: `${base}/token`,
  }
}

export const mobileOpenAuthRequestConfig = (
  redirectUri: string,
): MobileOpenAuthRequestConfig => ({
  clientId: KHALA_MOBILE_OPENAUTH_CLIENT_ID,
  codeChallengeMethod: "S256",
  extraParams: { provider: KHALA_MOBILE_OPENAUTH_PROVIDER },
  redirectUri,
  responseType: "code",
  usePKCE: true,
})

export const mobileOpenAuthTokenExchangeConfig = (input: {
  code: string
  codeVerifier: string
  redirectUri: string
}): MobileOpenAuthTokenExchangeConfig => ({
  clientId: KHALA_MOBILE_OPENAUTH_CLIENT_ID,
  code: input.code,
  extraParams: { code_verifier: input.codeVerifier },
  redirectUri: input.redirectUri,
})

const decodeMobileSyncSession = (input: unknown): MobileSyncSession => {
  const decoded = S.decodeUnknownSync(MobileSyncSessionSchema)(input)
  const ownerUserId = decoded.ownerUserId.trim()
  const syncToken = decoded.syncToken.trim()

  if (ownerUserId.length === 0 || syncToken.length === 0) {
    throw new Error("Mobile session response was missing owner or sync token")
  }

  return { ownerUserId, syncToken }
}

export const fetchMobileSyncSession = async (input: {
  accessToken: string
  apiBaseUrl: string
  fetchImpl?: FetchLike
}): Promise<MobileSyncSession> => {
  const base = normalizeHttpsBaseUrl(input.apiBaseUrl, "OpenAgents API base URL")
  const response = await (input.fetchImpl ?? fetch)(`${base}/api/mobile/session`, {
    headers: { authorization: `Bearer ${input.accessToken}` },
    method: "POST",
  })
  const body = await readOkMobileJsonResponse(response, "mobile session")

  return decodeMobileSyncSession(body)
}

export const deleteMobileOpenAuthSession = async (input: {
  accessToken: string
  apiBaseUrl: string
  fetchImpl?: FetchLike
}): Promise<void> => {
  const base = normalizeHttpsBaseUrl(input.apiBaseUrl, "OpenAgents API base URL")

  try {
    const response = await (input.fetchImpl ?? fetch)(
      `${base}/api/mobile/auth/session`,
      {
        headers: { authorization: `Bearer ${input.accessToken}` },
        method: "DELETE",
      },
    )
    await readOkMobileJsonResponse(response, "mobile sign-out")
  } catch (error) {
    throw new Error(mobileProblemMessageSafe(error, "mobile sign-out"))
  }
}

export const KHALA_ACCOUNT_DELETION_POLICY_COPY =
  "Deleting your Khala account permanently removes your GitHub sign-in link, your chat threads and turn history, and your device's push notification registration. Any remaining credit balance is forfeited and is not refunded — credits are non-transferable and have no cash value."

export const deleteMobileAccount = async (input: {
  accessToken: string
  apiBaseUrl: string
  fetchImpl?: FetchLike
}): Promise<void> => {
  const base = normalizeHttpsBaseUrl(input.apiBaseUrl, "OpenAgents API base URL")

  try {
    const response = await (input.fetchImpl ?? fetch)(
      `${base}/api/mobile/account`,
      {
        headers: { authorization: `Bearer ${input.accessToken}` },
        method: "DELETE",
      },
    )
    await readOkMobileJsonResponse(response, "account deletion")
  } catch (error) {
    throw new Error(mobileProblemMessageSafe(error, "account deletion"))
  }
}
