export interface ConfigBaseProps {
  persistNavigation: "always" | "dev" | "prod" | "never"
  catchErrors: "always" | "dev" | "prod" | "never"
  exitRoutes: string[]
  /** Base URL for auth API (WorkOS magic + SSO). Same origin as web app. */
  authApiUrl: string
  /** Convex deployment URL (dev vs prod). Override with EXPO_PUBLIC_CONVEX_URL. */
  convexUrl: string
  /** Enable Khala sync lane for Codex summaries and skip Convex provider boot. */
  khalaSyncEnabled: boolean
  /** Optional override for Khala websocket endpoint. */
  khalaSyncWsUrl: string
}

export type PersistNavigationConfig = ConfigBaseProps["persistNavigation"]

const BaseConfig: ConfigBaseProps = {
  // Disabled so tab bar always shows current routes (e.g. Feed). Re-enable "dev" to restore last screen.
  persistNavigation: "never",

  /**
   * Only enable if we're catching errors in the right environment
   */
  catchErrors: "always",

  /**
   * This is a list of all the route names that will exit the app if the back button
   * is pressed while in that screen. Only affects Android.
   */
  exitRoutes: ["Welcome"],
  authApiUrl: "https://openagents.com",
  // Set in config/index.ts from EXPO_PUBLIC_CONVEX_URL or dev/prod default
  convexUrl: "",
  khalaSyncEnabled: false,
  khalaSyncWsUrl: "",
}

export default BaseConfig
