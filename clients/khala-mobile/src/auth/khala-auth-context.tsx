import {
  exchangeCodeAsync,
  makeRedirectUri,
  useAuthRequest,
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
import { clearStoredCredentials, loadStoredCredentials, saveStoredCredentials } from "./khala-auth-store"
import {
  initialKhalaAuthMachineState,
  reduceKhalaAuthMachine,
  type KhalaAuthMachineStatus,
} from "./khala-auth-state-machine"
import { validateKhalaCredentials } from "./khala-auth-validate"
import {
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
  const [request, , promptAsync] = useAuthRequest(
    mobileOpenAuthRequestConfig(redirectUri) as AuthRequestConfig,
    discovery,
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
    const run = async () => {
      const storedCredentials = await loadStoredCredentials()
      if (cancelled || !mountedRef.current) return
      dispatch({
        devCredentials: devEnvCredentials,
        storedCredentials,
        type: "stored_credentials_loaded",
      })
    }
    void run()
    return () => {
      cancelled = true
    }
  }, [])

  const signInWithGitHub = useCallback(async () => {
    if (request === null || machine.status === "signing_in") return
    dispatch({ type: "github_sign_in_started" })

    try {
      const result = await promptAsync()

      if (result.type !== "success") {
        if (!mountedRef.current) return
        dispatch({ type: "github_sign_in_cancelled" })
        return
      }

      const code = result.params.code
      const codeVerifier = request.codeVerifier

      if (typeof code !== "string" || code.trim() === "" || codeVerifier === undefined) {
        throw new Error("GitHub sign-in did not return a usable authorization code.")
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
  }, [applyCredentials, machine.status, promptAsync, request])

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

  const value = useMemo<KhalaAuthState>(
    () => ({
      baseUrl: KHALA_SYNC_DEMO_BASE_URL,
      githubSignInReady: request !== null && machine.status !== "signing_in",
      ownerUserId: machine.credentials?.ownerUserId ?? "",
      signInErrorMessage: machine.messageSafe,
      signInWithGitHub,
      signOut,
      status: machine.status,
      token: machine.credentials?.token ?? "",
    }),
    [machine, request, signInWithGitHub, signOut],
  )

  return <KhalaAuthContext.Provider value={value}>{children}</KhalaAuthContext.Provider>
}

export const useKhalaAuth = (): KhalaAuthState => {
  const value = useContext(KhalaAuthContext)
  if (value === null) throw new Error("useKhalaAuth must be used within a KhalaAuthProvider")
  return value
}
