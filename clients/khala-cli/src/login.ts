import { Effect, Schema as S } from "effect"
import { mintFreeKey, toKhalaCliError } from "./client.js"
import { readStoredAgentToken, writeStoredAgentToken } from "./token-store.js"
import {
  AGENTS_ME_PATH,
  AgentMeResponse,
  DEFAULT_BASE_URL,
  DeviceAuthStartResponse,
  DeviceAuthStatusResponse,
  KhalaCliError,
  OPENAGENTS_DEVICE_AUTH_START_PATH,
  openAgentsDeviceAuthStatusPath,
} from "./types.js"

// `khala login` — standard OpenAgents device-auth flow (#6363, epic #6359).
//
// The CLI authenticates with its existing agent token (the auto-minted free key
// or an explicit --token / OPENAGENTS_AGENT_TOKEN), starts a device link
// attempt, prints the verification URL + user code (and tries to open the
// browser), then polls the status endpoint at the server-returned interval
// until the browser sign-in links the token to the owner's OpenAgents account.
//
// Because `OPENAGENTS_ADMIN_EMAILS` already includes the owner, signing in as
// the owner makes the linked token the owner token, so `/artanis` works
// afterwards with NO client-side owner logic. Login OVERWRITES the stored token
// with the now-linked credential.

export interface KhalaLoginOptions {
  readonly baseUrl: string
  readonly env: Record<string, string | undefined>
  readonly explicitToken?: string | undefined
  readonly fetch?: typeof fetch | undefined
  readonly openBrowser?: ((url: string) => void) | undefined
  readonly onPrompt: (prompt: KhalaLoginPrompt) => void
  readonly onPending?: (() => void) | undefined
  readonly sleep?: ((ms: number) => Promise<void>) | undefined
  readonly timeoutSeconds?: number | undefined
}

export interface KhalaLoginPrompt {
  readonly userCode: string
  readonly verificationUrl: string
  readonly expiresAt: string | undefined
}

export interface KhalaLoginResult {
  readonly token: string
  readonly tokenPrefix: string | undefined
  readonly displayName: string | undefined
  readonly email: string | undefined
  readonly alreadyLinked: boolean
}

const DEFAULT_TIMEOUT_SECONDS = 600
const DEFAULT_POLL_INTERVAL_SECONDS = 2

const defaultSleep = (ms: number): Promise<void> =>
  new Promise(resolve => setTimeout(resolve, ms))

export async function runKhalaLogin(options: KhalaLoginOptions): Promise<KhalaLoginResult> {
  const baseUrl = options.baseUrl || DEFAULT_BASE_URL
  const fetchImpl = options.fetch ?? fetch
  const sleep = options.sleep ?? defaultSleep
  const timeoutSeconds = options.timeoutSeconds ?? DEFAULT_TIMEOUT_SECONDS

  // Build the device-auth token candidates in priority order, then start the
  // flow with the first candidate the server accepts. An explicit --token or
  // OPENAGENTS_AGENT_TOKEN that is NOT a valid registered agent (e.g. a token
  // from another system) must not block login: we fall through to the stored
  // token and finally to a freshly minted free key, mirroring the Pylon CLI.
  const candidates = await collectLoginTokenCandidates({
    env: options.env,
    explicitToken: options.explicitToken,
  })

  const { token, started } = await startDeviceAuthWithCandidates({
    baseUrl,
    candidates,
    fetch: fetchImpl,
    sleep,
  })

  if (started.status === "linked") {
    await writeStoredAgentToken(options.env, token)
    const identity = await resolveIdentity({ baseUrl, fetch: fetchImpl, token })
    return {
      token,
      tokenPrefix: started.linkedAgent?.tokenPrefix,
      displayName: identity.displayName,
      email: identity.email,
      alreadyLinked: true,
    }
  }

  const attemptId = started.attemptId
  const verificationUrl = started.verificationUrl
  const userCode = started.userCode
  if (attemptId === undefined || verificationUrl === undefined || userCode === undefined) {
    throw new KhalaCliError({
      reason: "OpenAgents device-auth start response was missing the verification URL, user code, or attempt id.",
      code: "schema_mismatch",
    })
  }

  options.onPrompt({ userCode, verificationUrl, expiresAt: started.expiresAt })
  options.openBrowser?.(verificationUrl)

  const intervalMs = Math.max(1, started.intervalSeconds ?? DEFAULT_POLL_INTERVAL_SECONDS) * 1000
  const deadline = Date.now() + timeoutSeconds * 1000

  while (Date.now() <= deadline) {
    await sleep(intervalMs)
    const polled = await Effect.runPromise(
      pollDeviceAuth({ attemptId, baseUrl, fetch: fetchImpl, token }),
    )
    if (polled.status === "linked") {
      await writeStoredAgentToken(options.env, token)
      const identity = await resolveIdentity({ baseUrl, fetch: fetchImpl, token })
      return {
        token,
        tokenPrefix: polled.linkedAgent?.tokenPrefix ?? started.linkedAgent?.tokenPrefix,
        displayName: identity.displayName,
        email: identity.email,
        alreadyLinked: false,
      }
    }
    if (polled.status === "expired") {
      throw new KhalaCliError({
        reason: "The sign-in code expired before it was approved. Run `khala login` again.",
        code: "device_auth_expired",
      })
    }
    options.onPending?.()
  }

  throw new KhalaCliError({
    reason: "Timed out waiting for browser sign-in. Run `khala login` again and approve the code in your browser.",
    code: "device_auth_timeout",
  })
}

// A just-minted free key can briefly 401 against device/start while the new
// credential propagates through the edge cache. Retry the start a few times on
// unauthorized so a first-time login (auto-minted token) works cleanly.
const MINT_PROPAGATION_RETRIES = 4
const MINT_RETRY_DELAY_MS = 1_500

interface LoginTokenCandidate {
  readonly token: string
  readonly source: "explicit" | "stored" | "minted"
}

async function collectLoginTokenCandidates(input: {
  readonly env: Record<string, string | undefined>
  readonly explicitToken?: string | undefined
}): Promise<ReadonlyArray<LoginTokenCandidate>> {
  const candidates: Array<LoginTokenCandidate> = []
  const seen = new Set<string>()
  const add = (token: string | undefined, source: LoginTokenCandidate["source"]): void => {
    const trimmed = token?.trim()
    if (trimmed === undefined || !trimmed.startsWith("oa_agent_") || seen.has(trimmed)) return
    seen.add(trimmed)
    candidates.push({ token: trimmed, source })
  }

  add(input.explicitToken, "explicit")
  add(await readStoredAgentToken(input.env), "stored")
  return candidates
}

async function startDeviceAuthWithCandidates(input: {
  readonly baseUrl: string
  readonly candidates: ReadonlyArray<LoginTokenCandidate>
  readonly fetch: typeof fetch
  readonly sleep: (ms: number) => Promise<void>
}): Promise<{ readonly token: string; readonly started: DeviceAuthStartResponse }> {
  let lastUnauthorized: KhalaCliError | undefined

  // Try the explicit/stored candidates first, falling through on unauthorized.
  for (const candidate of input.candidates) {
    try {
      const started = await Effect.runPromise(
        startDeviceAuth({ baseUrl: input.baseUrl, fetch: input.fetch, token: candidate.token }),
      )
      return { token: candidate.token, started }
    } catch (error) {
      if (isUnauthorized(error)) {
        lastUnauthorized = error
        continue
      }
      throw error
    }
  }

  // No existing candidate worked: mint a fresh free key and retry through edge
  // propagation. The minted token is the one we will link to the owner account.
  const minted = await Effect.runPromise(mintFreeKey({ baseUrl: input.baseUrl, fetch: input.fetch }))
    .catch(error => {
      throw toKhalaCliError(error, "Could not mint a Khala token for login.")
    })
  const token = minted.credential.token.trim()
  if (!token.startsWith("oa_agent_")) {
    throw new KhalaCliError({
      reason: "Free key response did not include an oa_agent_ token.",
      code: "schema_mismatch",
    })
  }

  for (let attempt = 0; attempt <= MINT_PROPAGATION_RETRIES; attempt += 1) {
    try {
      const started = await Effect.runPromise(
        startDeviceAuth({ baseUrl: input.baseUrl, fetch: input.fetch, token }),
      )
      return { token, started }
    } catch (error) {
      if (isUnauthorized(error) && attempt < MINT_PROPAGATION_RETRIES) {
        await input.sleep(MINT_RETRY_DELAY_MS)
        continue
      }
      if (isUnauthorized(error) && lastUnauthorized !== undefined) {
        // Prefer surfacing the original candidate's unauthorized error so the
        // user understands their provided token was not accepted.
        throw lastUnauthorized
      }
      throw error
    }
  }

  throw (
    lastUnauthorized ??
    new KhalaCliError({ reason: "device-auth start failed: unauthorized", code: "unauthorized" })
  )
}

function isUnauthorized(error: unknown): error is KhalaCliError {
  return (
    error instanceof KhalaCliError &&
    (error.statusCode === 401 || error.code === "unauthorized")
  )
}

function startDeviceAuth(input: {
  readonly baseUrl: string
  readonly fetch: typeof fetch
  readonly token: string
}): Effect.Effect<DeviceAuthStartResponse, KhalaCliError> {
  return Effect.gen(function* () {
    const response = yield* requestJson({
      fetch: input.fetch,
      url: urlFor(input.baseUrl, OPENAGENTS_DEVICE_AUTH_START_PATH),
      init: {
        method: "POST",
        headers: {
          accept: "application/json",
          authorization: `Bearer ${input.token}`,
        },
      },
      label: "device-auth start",
    })
    return yield* Effect.try({
      try: () => S.decodeUnknownSync(DeviceAuthStartResponse)(response),
      catch: error => decodeError(error, "device-auth start"),
    })
  })
}

function pollDeviceAuth(input: {
  readonly attemptId: string
  readonly baseUrl: string
  readonly fetch: typeof fetch
  readonly token: string
}): Effect.Effect<DeviceAuthStatusResponse, KhalaCliError> {
  return Effect.gen(function* () {
    const response = yield* requestJson({
      fetch: input.fetch,
      url: urlFor(input.baseUrl, openAgentsDeviceAuthStatusPath(input.attemptId)),
      init: {
        method: "GET",
        headers: {
          accept: "application/json",
          authorization: `Bearer ${input.token}`,
        },
      },
      label: "device-auth status",
    })
    return yield* Effect.try({
      try: () => S.decodeUnknownSync(DeviceAuthStatusResponse)(response),
      catch: error => decodeError(error, "device-auth status"),
    })
  })
}

async function resolveIdentity(input: {
  readonly baseUrl: string
  readonly fetch: typeof fetch
  readonly token: string
}): Promise<{ readonly displayName: string | undefined; readonly email: string | undefined }> {
  // Best-effort identity read so login can print "Signed in as <name>." Never
  // fails the login: the token is already linked at this point.
  try {
    const me = await Effect.runPromise(
      Effect.gen(function* () {
        const response = yield* requestJson({
          fetch: input.fetch,
          url: urlFor(input.baseUrl, AGENTS_ME_PATH),
          init: {
            method: "GET",
            headers: {
              accept: "application/json",
              authorization: `Bearer ${input.token}`,
            },
          },
          label: "agent identity",
        })
        return yield* Effect.try({
          try: () => S.decodeUnknownSync(AgentMeResponse)(response),
          catch: error => decodeError(error, "agent identity"),
        })
      }),
    )
    return {
      displayName: me.agent?.user?.displayName ?? undefined,
      email: me.agent?.user?.primaryEmail ?? undefined,
    }
  } catch {
    return { displayName: undefined, email: undefined }
  }
}

function requestJson(input: {
  readonly fetch: typeof fetch
  readonly url: string
  readonly init: RequestInit
  readonly label: string
}): Effect.Effect<unknown, KhalaCliError> {
  return Effect.tryPromise({
    try: async () => {
      const response = await input.fetch(input.url, input.init)
      if (!response.ok) {
        const detail = await readErrorDetail(response)
        throw new KhalaCliError({
          reason: `${input.label} failed: ${detail}`,
          statusCode: response.status,
          ...(response.status === 401 ? { code: "unauthorized" } : {}),
        })
      }
      return await response.json()
    },
    catch: error => toKhalaCliError(error, `${input.label} request failed.`),
  })
}

function decodeError(error: unknown, label: string): KhalaCliError {
  return new KhalaCliError({
    reason: `Unexpected ${label} response: ${String(error)}`,
    code: "schema_mismatch",
  })
}

async function readErrorDetail(response: Response): Promise<string> {
  try {
    const text = await response.text()
    if (text.trim().length === 0) return `HTTP ${response.status}`
    try {
      const payload = JSON.parse(text) as { readonly error?: unknown; readonly reason?: unknown }
      if (typeof payload.reason === "string") return payload.reason
      if (typeof payload.error === "string") return payload.error
    } catch {
      // fall through to raw text
    }
    return text.trim().slice(0, 200)
  } catch {
    return `HTTP ${response.status}`
  }
}

function urlFor(baseUrl: string, path: string): string {
  return `${baseUrl.replace(/\/+$/, "")}${path}`
}

// Best-effort browser open. Non-fatal: the URL + code are always printed.
export function openVerificationUrl(url: string): void {
  try {
    const platform = process.platform
    const command =
      platform === "darwin" ? "open" : platform === "win32" ? "cmd" : "xdg-open"
    const args = platform === "win32" ? ["/c", "start", "", url] : [url]
    const child = Bun.spawn([command, ...args], {
      stdin: "ignore",
      stdout: "ignore",
      stderr: "ignore",
    })
    child.unref?.()
  } catch {
    // Ignore: printing the URL is the reliable path.
  }
}
