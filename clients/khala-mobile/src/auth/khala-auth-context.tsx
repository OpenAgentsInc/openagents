import {
  AuthRequest,
  exchangeCodeAsync,
  makeRedirectUri,
} from "expo-auth-session"
import type { AuthRequestConfig } from "expo-auth-session"
import * as WebBrowser from "expo-web-browser"
import {
  createContext,
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
import { mobileProblemMessageSafe } from "../network/mobile-problem"
import { unregisterPushNotificationsAsync } from "../push/push-notifications-client"
import { describeAuthSessionFailure } from "./auth-session-failure"
import { resolveVerifiedStoredCredentials } from "./khala-auth-resume-verify-core"
import { clearStoredCredentials, loadStoredCredentials, saveStoredCredentials } from "./khala-auth-store"
import {
  initialKhalaAuthMachineState,
  reduceKhalaAuthMachine,
  type KhalaAuthMachineStatus,
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

export type KhalaAuthStatus = KhalaAuthMachineStatus

export type KhalaAuthState = Readonly<{
  status: KhalaAuthStatus
  baseUrl: string
  githubSignInReady: boolean
  ownerUserId: string
  deleteAccount: () => Promise<void>
  signInErrorMessage: string | null
  signInWithGitHub: () => Promise<void>
  signOut: () => Promise<void>
  token: string
}>

const KhalaAuthContext = createContext<KhalaAuthState | null>(null)

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
    async (credentials: { ownerUserId: string; token: string }, persist: boolean) => {
      if (persist) await saveStoredCredentials(credentials)
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
      const result = await authRequest.promptAsync(discovery)

      // Pull the callback query params (present on success AND on error
      // results — `parseReturnUrl` attaches them either way).
      const params = "params" in result ? (result.params ?? {}) : {}
      const code = params.code
      const oauthError = params.error

      // A real OAuth error redirected back by the issuer (?error=...) is a
      // genuine failure — never try to exchange a non-existent code.
      if (typeof oauthError === "string" && oauthError.trim() !== "") {
        if (!mountedRef.current) return
        dispatch({
          messageSafe: describeAuthSessionFailure(result),
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
          messageSafe: describeAuthSessionFailure(result),
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

      const tokenResponse = await exchangeCodeAsync(
        mobileOpenAuthTokenExchangeConfig({
          code,
          codeVerifier,
          redirectUri,
        }),
        discovery,
      )
      const mobileSession = await fetchMobileSyncSession({
        accessToken: tokenResponse.accessToken,
        apiBaseUrl: KHALA_OPENAGENTS_API_BASE_URL,
      })
      const validation = await validateKhalaCredentials({
        baseUrl: KHALA_SYNC_DEMO_BASE_URL,
        ownerUserId: mobileSession.ownerUserId,
        token: mobileSession.syncToken,
      })

      if (!validation.ok) {
        if (!mountedRef.current) return
        dispatch({
          messageSafe: validation.messageSafe,
          type: "github_sign_in_failed",
        })
        return
      }

      await applyCredentials(
        {
          ownerUserId: mobileSession.ownerUserId,
          token: mobileSession.syncToken,
        },
        true,
      )
    } catch (error) {
      if (!mountedRef.current) return
      dispatch({
        messageSafe: mobileProblemMessageSafe(error, "GitHub sign-in"),
        type: "github_sign_in_failed",
      })
    }
  }, [applyCredentials, machine.status])

  const signOut = useCallback(async () => {
    const token = machine.credentials?.token

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
      // The AuthRequest is now built on demand inside `signInWithGitHub`
      // (discovery is a static module constant), so the button is ready as
      // soon as we are not already mid sign-in.
      githubSignInReady: machine.status !== "signing_in",
      ownerUserId: machine.credentials?.ownerUserId ?? "",
      signInErrorMessage: machine.messageSafe,
      signInWithGitHub,
      signOut,
      status: machine.status,
      token: machine.credentials?.token ?? "",
    }),
    [deleteAccount, machine, signInWithGitHub, signOut],
  )

  return <KhalaAuthContext.Provider value={value}>{children}</KhalaAuthContext.Provider>
}

export const useKhalaAuth = (): KhalaAuthState => {
  const value = useContext(KhalaAuthContext)
  if (value === null) throw new Error("useKhalaAuth must be used within a KhalaAuthProvider")
  return value
}
