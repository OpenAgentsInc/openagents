import {
  AuthRequest,
  exchangeCodeAsync,
  makeRedirectUri,
} from "expo-auth-session"
import type { AuthRequestConfig } from "expo-auth-session"
import * as WebBrowser from "expo-web-browser"
import {
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useRef,
} from "react"
import type { ReactNode } from "react"

import {
  KHALA_OPENAGENTS_API_BASE_URL,
  KHALA_OPENAUTH_BASE_URL,
  KHALA_SYNC_DEMO_BASE_URL,
  KHALA_SYNC_DEMO_OWNER_USER_ID,
  KHALA_SYNC_DEMO_TOKEN,
} from "../config/khala-sync-demo"
import { DEMO_REVIEWER_CREDENTIALS, isDemoToken } from "../demo/demo-fixtures"
import { KhalaAuthContext, type KhalaAuthState, type KhalaAuthStatus } from "./khala-auth-context-value"
import { mobileProblemMessageSafe } from "../network/mobile-problem"
import { unregisterPushNotificationsAsync } from "../push/push-notifications-client"
import { describeAuthSessionFailure } from "./auth-session-failure"
import { resolveVerifiedStoredCredentials } from "./khala-auth-resume-verify-core"
import { clearStoredCredentials, loadStoredCredentials, saveStoredCredentials, type KhalaStoredCredentials } from "./khala-auth-store"
import {
  initialKhalaAuthMachineState,
  reduceKhalaAuthMachine,
} from "./khala-auth-state-machine"
import { validateKhalaCredentials } from "./khala-auth-validate"
import {
  deleteMobileAccount,
  deleteMobileOpenAuthSession,
  fetchMobileSyncSession,
  mobileOpenAuthDiscovery,
  mobileOpenAuthRequestConfig,
  mobileOpenAuthTokenExchangeConfig,
  KHALA_MOBILE_OPENAUTH_REDIRECT_PATH,
  KHALA_MOBILE_OPENAUTH_REDIRECT_SCHEME,
} from "./mobile-openauth"

WebBrowser.maybeCompleteAuthSession()

// The context object + value type live in `./khala-auth-context-value` (a
// native-dep-free module) and are re-exported here for the existing public
// import surface. See that file for why.
export { KhalaAuthContext }
export type { KhalaAuthState, KhalaAuthStatus }

/** The env-var pair only seeds a dev session when BOTH are present. A real
 * distributed build never bakes these in, so a fresh install lands on the
 * GitHub sign-in screen rather than a desktop/Tailnet pairing probe. */
const devEnvCredentials =
  KHALA_SYNC_DEMO_OWNER_USER_ID !== "" && KHALA_SYNC_DEMO_TOKEN !== ""
    ? { ownerUserId: KHALA_SYNC_DEMO_OWNER_USER_ID, token: KHALA_SYNC_DEMO_TOKEN }
    : null

const redirectUri = makeRedirectUri({
  path: KHALA_MOBILE_OPENAUTH_REDIRECT_PATH,
  scheme: KHALA_MOBILE_OPENAUTH_REDIRECT_SCHEME,
})
const discovery = mobileOpenAuthDiscovery(KHALA_OPENAUTH_BASE_URL)

/** TEMP-DIAG-8467: build a public-safe snapshot of the exact auth-session
 * outcome and POST it to the debug sink so we can read device ground truth via
 * `wrangler tail`. Redacts the authorization code + any token before sending.
 * Never throws (fire-and-forget). */
const SIGNIN_DEBUG_BUILD_MARKER = "dbg3"

/** TEMP-DIAG-8467: build a public-safe snapshot of the exact auth-session
 * outcome. Redacts the authorization code + any token. Returned so it can be
 * shown on-screen (the reliable channel — the account's Worker deploys are
 * currently blocked) and best-effort beaconed. */
const buildSignInDebug = (
  authRequest: { state?: string; codeVerifier?: string; url?: string | null },
  result: { type: string; url?: string; error?: { description?: string | null } | null },
  params: Record<string, string>,
): { line: string; json: string } => {
  const redactUrl = (raw: string | undefined): string =>
    typeof raw === "string"
      ? raw.replace(/(code=)[^&]+/, "$1REDACTED").replace(/(access_token=)[^&]+/, "$1REDACTED")
      : "(none)"
  const snapshot = {
    marker: SIGNIN_DEBUG_BUILD_MARKER,
    resultType: result.type,
    expectedState: authRequest.state ?? "(none)",
    returnedState: params.state ?? "(none)",
    stateMatches: (authRequest.state ?? null) === (params.state ?? null),
    hasCode: typeof params.code === "string" && params.code.length > 0,
    hasCodeVerifier: authRequest.codeVerifier !== undefined,
    oauthError: params.error ?? "(none)",
    expoError: result.error?.description ?? "(none)",
    paramKeys: Object.keys(params).sort().join(","),
    returnedUrl: redactUrl(result.url),
  }
  const line =
    `[${SIGNIN_DEBUG_BUILD_MARKER}] type=${snapshot.resultType} ` +
    `code=${snapshot.hasCode ? "Y" : "N"} verifier=${snapshot.hasCodeVerifier ? "Y" : "N"} ` +
    `stateOK=${snapshot.stateMatches ? "Y" : "N"} exp=${snapshot.expectedState} got=${snapshot.returnedState} ` +
    `keys=${snapshot.paramKeys || "(none)"} url=${snapshot.returnedUrl}`
  return { line, json: JSON.stringify(snapshot) }
}

const beaconSignInDebug = async (json: string): Promise<void> => {
  try {
    await fetch(`${KHALA_OPENAGENTS_API_BASE_URL}/api/mobile/signin-debug`, {
      body: json,
      headers: { "content-type": "application/json" },
      method: "POST",
    })
  } catch {
    // best-effort diagnostics only
  }
}

export const KhalaAuthProvider = ({ children }: { children: ReactNode }) => {
  const [machine, dispatch] = useReducer(
    reduceKhalaAuthMachine,
    initialKhalaAuthMachineState,
  )
  const mountedRef = useRef(true)

  useEffect(() => () => {
    mountedRef.current = false
  }, [])

  const applyCredentials = useCallback(
    async (credentials: KhalaStoredCredentials, persist: boolean) => {
      if (persist) {
        // A Keychain/SecureStore write hiccup must NEVER bounce a user who
        // already holds a valid mobile session back to an error screen. A
        // throw here surfaced as the generic "GitHub sign-in: request failed"
        // (a plain Error → `unknown` kind) even though the server had returned
        // a valid session. Log in for this session regardless; the next launch
        // re-persists via the resume/validate path. Beacon the failure so a
        // real persist problem is still visible.
        try {
          await saveStoredCredentials(credentials)
        } catch (error) {
          void beaconSignInDebug(
            JSON.stringify({
              errorMessage: (error instanceof Error ? error.message : String(error)).slice(0, 200),
              errorName: error instanceof Error ? error.name : typeof error,
              marker: SIGNIN_DEBUG_BUILD_MARKER,
              stage: "persist",
            }),
          )
        }
      }
      if (!mountedRef.current) return
      dispatch({ credentials, type: "github_sign_in_succeeded" })
    },
    [],
  )

  useEffect(() => {
    let cancelled = false
    let resolved = false
    const run = async () => {
      const storedCredentials = await loadStoredCredentials()

      // A stored credential is not trusted blindly — it may be a leftover
      // token from a prior auth model (e.g. the retired Tailnet-pairing
      // flow) or a since-revoked mobile OpenAuth session that TestFlight
      // build updates otherwise carry forward untouched (Keychain data
      // survives an app update). Re-validate against the server on every
      // launch, exactly like a fresh sign-in does, before ever treating the
      // app as signed in; an invalid credential is cleared so the user
      // lands on the real GitHub sign-in screen instead of a stale session.
      const verifiedStoredCredentials = await resolveVerifiedStoredCredentials(
        storedCredentials,
        {
          clearStoredCredentials,
          validate: credentials =>
            validateKhalaCredentials({
              baseUrl: KHALA_SYNC_DEMO_BASE_URL,
              ownerUserId: credentials.ownerUserId,
              token: credentials.token,
            }),
        },
      )

      if (cancelled || !mountedRef.current) return
      resolved = true
      dispatch({
        devCredentials: devEnvCredentials,
        storedCredentials: verifiedStoredCredentials,
        type: "stored_credentials_loaded",
      })
    }
    void run()

    // Defense-in-depth watchdog: the auth status must NEVER stay "loading"
    // on a bare spinner forever. `validateKhalaCredentials` is already
    // bounded (12s), but SecureStore reads or any future addition to `run`
    // could hang too — if nothing has resolved shortly after that bound,
    // force a decision (treat as no verified stored credential, so the app
    // lands on sign-in or the env dev creds rather than an infinite
    // spinner). Real 2026-07-06 bug: launch stuck on a blank spinner.
    const watchdog = setTimeout(() => {
      if (cancelled || resolved || !mountedRef.current) return
      resolved = true
      dispatch({
        devCredentials: devEnvCredentials,
        storedCredentials: null,
        type: "stored_credentials_loaded",
      })
    }, 15_000)

    return () => {
      cancelled = true
      clearTimeout(watchdog)
    }
  }, [])

  const signInWithGitHub = useCallback(async () => {
    if (machine.status === "signing_in") return
    dispatch({ type: "github_sign_in_started" })

    // TEMP-DIAG-8467: captured after promptAsync, appended to every failure
    // message (incl. the token-exchange catch below) so we always see the
    // exact outcome on-screen.
    let debugLine = `[${SIGNIN_DEBUG_BUILD_MARKER}] (no auth-session result)`
    // Tracks the exact post-callback step in flight so a thrown failure names
    // the stage (owner report, new-account 2026-07-07: the server returned
    // all 200s — /token, /api/mobile/session, /api/sync/bootstrap — yet the app
    // showed "GitHub sign-in: request failed"; without a stage label a plain
    // Error is indistinguishable across the exchange / session / apply steps).
    let stage: "prompt" | "token-exchange" | "mobile-session" | "apply" = "prompt"
    try {
      // Build ONE AuthRequest here, imperatively, and use that exact instance
      // to open the browser AND parse the callback. This is the fix for the
      // real `state_mismatch` failure users hit (#8467, "Cross-Site request
      // verification failed. Cached state and returned state do not match"):
      // `useAuthRequest` reactively rebuilds its request object across
      // renders, so the instance that PARSED the callback could differ from
      // the one that BUILT the authorize URL — different random `state`, so
      // the cross-site check always failed and the code was never exchanged.
      // A single local instance generates one `state` + one PKCE verifier and
      // both opens and validates with it, so they can never diverge.
      const authRequest = new AuthRequest(
        mobileOpenAuthRequestConfig(redirectUri) as AuthRequestConfig,
      )
      // `preferEphemeralSession: true` is the reliable fix for "sign out can't
      // switch accounts" (owner report, 2026-07-07): on iOS this opens
      // a NON-persistent ASWebAuthenticationSession that does not share cookies
      // with system Safari, so the OpenAuth issuer session cookie
      // (auth.openagents.com) and GitHub's own session are never reused. Every
      // login therefore presents a fresh GitHub login / account picker instead
      // of silently re-authenticating the previously signed-in account. Pairs
      // with `signOut`'s server-side session revoke below; together they end the
      // web session so re-auth can choose a different account.
      const result = await authRequest.promptAsync(discovery, {
        preferEphemeralSession: true,
      })

      // Pull the callback query params (present on success AND on error
      // results — `parseReturnUrl` attaches them either way).
      const params = "params" in result ? (result.params ?? {}) : {}
      const code = params.code
      const oauthError = params.error

      // TEMP-DIAG-8467: capture the EXACT outcome. Shown on-screen for the
      // failure paths (the reliable channel) and best-effort beaconed. The
      // code value is redacted. Never blocks sign-in.
      const debug = buildSignInDebug(authRequest, result, params)
      debugLine = debug.line
      void beaconSignInDebug(debug.json)

      // A real OAuth error redirected back by the issuer (?error=...) is a
      // genuine failure — never try to exchange a non-existent code.
      if (typeof oauthError === "string" && oauthError.trim() !== "") {
        if (!mountedRef.current) return
        dispatch({
          messageSafe: `${describeAuthSessionFailure(result)}\n${debug.line}`,
          type: "github_sign_in_failed",
        })
        return
      }

      // No usable code AND no error: the user closed the sheet, or the
      // session ended without a callback. A clean dismissal is silent;
      // anything else surfaces its reason instead of looking like nothing.
      if (typeof code !== "string" || code.trim() === "") {
        if (!mountedRef.current) return
        if (result.type === "cancel" || result.type === "dismiss") {
          dispatch({ type: "github_sign_in_cancelled" })
          return
        }
        dispatch({
          messageSafe: `${describeAuthSessionFailure(result)}\n${debug.line}`,
          type: "github_sign_in_failed",
        })
        return
      }

      // We have a code. Exchange it — even if expo flagged a `state_mismatch`
      // (`result.type === 'error'`). With S256 PKCE the code is cryptographically
      // bound to THIS request's `code_verifier`, which never leaves the device,
      // so PKCE already provides the CSRF protection that `state` duplicates
      // (OAuth 2.1 treats `state` as optional once PKCE is in use). Real bug
      // (#8467): expo's cached-vs-returned `state` compare failed on device and
      // silently blocked every sign-in even though the server had issued a valid
      // code; the `/token` exchange below is the true security gate and rejects
      // any code not minted for our verifier.
      const codeVerifier = authRequest.codeVerifier

      if (codeVerifier === undefined) {
        throw new Error(
          "GitHub sign-in could not complete: PKCE code verifier was missing.",
        )
      }

      stage = "token-exchange"
      const tokenResponse = await exchangeCodeAsync(
        mobileOpenAuthTokenExchangeConfig({
          code,
          codeVerifier,
          redirectUri,
        }),
        discovery,
      )

      stage = "mobile-session"
      const mobileSession = await fetchMobileSyncSession({
        accessToken: tokenResponse.accessToken,
        apiBaseUrl: KHALA_OPENAGENTS_API_BASE_URL,
      })

      // We now hold a VALID mobile session: `fetchMobileSyncSession` only
      // returns on a 200 from /api/mobile/session with a non-empty ownerUserId
      // AND syncToken (it throws otherwise). THAT is the true sign-in gate. A
      // user who reaches here MUST be let in. Previously a second network call
      // — `validateKhalaCredentials` (a /api/sync/bootstrap probe) — gated the
      // login and any hiccup there (or in the SecureStore write) bounced a user
      // who already held a valid session back to an error screen. The server
      // returned all 200s for the new-account attempt yet the app still failed,
      // because a post-session step threw. The bootstrap probe is demoted to an
      // ADVISORY, fire-and-forget check that can never block or revert sign-in;
      // the launch-time re-validation (`resolveVerifiedStoredCredentials`)
      // still clears a genuinely-dead token on the next launch, so
      // defense-in-depth is preserved. Applying credentials no longer throws on
      // a persist failure (see `applyCredentials`).
      stage = "apply"
      await applyCredentials(
        mobileSession.githubLogin !== undefined
          ? {
              ownerUserId: mobileSession.ownerUserId,
              token: mobileSession.syncToken,
              githubLogin: mobileSession.githubLogin,
            }
          : {
              ownerUserId: mobileSession.ownerUserId,
              token: mobileSession.syncToken,
            },
        true,
      )

      // Advisory only — never blocks or reverts the sign-in above. Beacons a
      // real bootstrap/token mismatch for telemetry without failing the user.
      void (async () => {
        try {
          const validation = await validateKhalaCredentials({
            baseUrl: KHALA_SYNC_DEMO_BASE_URL,
            ownerUserId: mobileSession.ownerUserId,
            token: mobileSession.syncToken,
          })
          if (!validation.ok) {
            void beaconSignInDebug(
              JSON.stringify({
                advisoryValidateFailed: validation.messageSafe.slice(0, 200),
                marker: SIGNIN_DEBUG_BUILD_MARKER,
                stage: "advisory-validate",
              }),
            )
          }
        } catch {
          // advisory only
        }
      })()
    } catch (error) {
      if (!mountedRef.current) return
      // Beacon the EXACT stage + error so a genuine pre-session failure
      // (exchange or session) is pinned on the next attempt. Server evidence
      // (all 200s) points at post-session throws, now hardened above.
      void beaconSignInDebug(
        JSON.stringify({
          errorMessage: (error instanceof Error ? error.message : String(error)).slice(0, 300),
          errorName: error instanceof Error ? error.name : typeof error,
          marker: SIGNIN_DEBUG_BUILD_MARKER,
          stage,
        }),
      )
      dispatch({
        messageSafe: `${mobileProblemMessageSafe(error, "GitHub sign-in")}\n${debugLine} stage=${stage}`,
        type: "github_sign_in_failed",
      })
    }
  }, [applyCredentials, machine.status])

  const enterDemoMode = useCallback(() => {
    // Fully synchronous and offline: no OAuth, no token exchange, no backend
    // session. Establishes the synthetic reviewer session that every data
    // source recognizes (via the demo sentinel token) and serves example data.
    dispatch({ credentials: DEMO_REVIEWER_CREDENTIALS, type: "demo_sign_in_started" })
  }, [])

  const signOut = useCallback(async () => {
    const token = machine.credentials?.token

    // The demo session is purely in-app: leaving it must not clear real stored
    // credentials or hit any backend revoke endpoint (the demo token is not a
    // real bearer token).
    if (token !== undefined && isDemoToken(token)) {
      dispatch({ type: "signed_out" })
      return
    }

    await clearStoredCredentials()

    if (token !== undefined && token.trim().length > 0) {
      // Best-effort: unregister this device's push token BEFORE the server
      // revokes the access token below, so the unregister call still
      // authenticates (MM-G1, #8485). Never blocks/fails sign-out.
      await unregisterPushNotificationsAsync({
        apiBaseUrl: KHALA_OPENAGENTS_API_BASE_URL,
        bearerToken: token,
      })

      try {
        await deleteMobileOpenAuthSession({
          accessToken: token,
          apiBaseUrl: KHALA_OPENAGENTS_API_BASE_URL,
        })
      } catch {
        // Local sign-out must complete even if the network revocation attempt
        // fails; the next server call still has to pass bearer validation.
      }
    }

    if (!mountedRef.current) return
    dispatch({ type: "signed_out" })
  }, [machine.credentials?.token])

  const deleteAccount = useCallback(async () => {
    const token = machine.credentials?.token

    // Demo session: "delete account" just exits the offline demo — there is no
    // real account or backend record to delete.
    if (token !== undefined && isDemoToken(token)) {
      dispatch({ type: "signed_out" })
      return
    }

    if (token === undefined || token.trim().length === 0) {
      await clearStoredCredentials()
      if (!mountedRef.current) return
      dispatch({ type: "signed_out" })
      return
    }

    await deleteMobileAccount({
      accessToken: token,
      apiBaseUrl: KHALA_OPENAGENTS_API_BASE_URL,
    })
    await clearStoredCredentials()
    await unregisterPushNotificationsAsync({
      apiBaseUrl: KHALA_OPENAGENTS_API_BASE_URL,
      bearerToken: token,
    })

    if (!mountedRef.current) return
    dispatch({ type: "signed_out" })
  }, [machine.credentials?.token])

  const value = useMemo<KhalaAuthState>(
    () => ({
      baseUrl: KHALA_SYNC_DEMO_BASE_URL,
      deleteAccount,
      demoMode: isDemoToken(machine.credentials?.token ?? ""),
      enterDemoMode,
      // The AuthRequest is now built on demand inside `signInWithGitHub`
      // (discovery is a static module constant), so the button is ready as
      // soon as we are not already mid sign-in.
      githubSignInReady: machine.status !== "signing_in",
      ownerUserId: machine.credentials?.ownerUserId ?? "",
      githubLogin: machine.credentials?.githubLogin ?? "",
      signInErrorMessage: machine.messageSafe,
      signInWithGitHub,
      signOut,
      status: machine.status,
      token: machine.credentials?.token ?? "",
    }),
    [deleteAccount, enterDemoMode, machine, signInWithGitHub, signOut],
  )

  return <KhalaAuthContext.Provider value={value}>{children}</KhalaAuthContext.Provider>
}

export const useKhalaAuth = (): KhalaAuthState => {
  const value = useContext(KhalaAuthContext)
  if (value === null) throw new Error("useKhalaAuth must be used within a KhalaAuthProvider")
  return value
}
