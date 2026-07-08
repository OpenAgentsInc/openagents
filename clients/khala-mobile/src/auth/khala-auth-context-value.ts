import { createContext } from "react"

import type { KhalaAuthMachineStatus } from "./khala-auth-state-machine"

/**
 * The auth context object + its value type, deliberately split out of
 * `khala-auth-context.tsx` so consumers that only need to READ the context
 * (e.g. the generic `useKhalaSyncScopeEntities` data-source hook reading
 * `demoMode`) do not transitively pull the provider's heavy native
 * dependencies (`expo-web-browser`, `expo-auth-session`, `expo-secure-store`).
 * That transitive pull broke pure unit tests of the sync hook (native module
 * load failures in the bun test environment); keeping the context object here
 * — with no Expo imports — avoids it.
 */
export type KhalaAuthStatus = KhalaAuthMachineStatus

export type KhalaAuthState = Readonly<{
  status: KhalaAuthStatus
  baseUrl: string
  githubSignInReady: boolean
  ownerUserId: string
  /** GitHub login (username) for the signed-in user, or "" when unavailable
   * (email-provider session, or a Worker deploy predating the greeting
   * change). Used to personalize the onboarding greeting. */
  githubLogin: string
  deleteAccount: () => Promise<void>
  /** True when the active session is the App Store reviewer demo session.
   * Data sources serve hardcoded example data in this mode. */
  demoMode: boolean
  /** Enter the offline reviewer demo session (long-press on the GitHub button).
   * Never touches real GitHub OAuth or a live backend session. */
  enterDemoMode: () => void
  signInErrorMessage: string | null
  signInWithGitHub: () => Promise<void>
  signOut: () => Promise<void>
  token: string
}>

export const KhalaAuthContext = createContext<KhalaAuthState | null>(null)
