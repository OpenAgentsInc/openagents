import Constants from "expo-constants"

export interface ConfigBaseProps {
  persistNavigation: "always" | "dev" | "prod" | "never"
  catchErrors: "always" | "dev" | "prod" | "never"
  exitRoutes: string[]
  API_URL?: string
  AIUR_API_URL?: string
  GROQ_API_KEY?: string | null
  NEXUS_URL: string
  WS_URL?: string
}

export type PersistNavigationConfig = ConfigBaseProps["persistNavigation"]

const BaseConfig: ConfigBaseProps = {
  // This feature is particularly useful in development mode, but
  // can be used in production as well if you prefer.
  persistNavigation: "dev",

  /**
   * Only enable if we're catching errors in the right environment
   */
  catchErrors: "always",

  /**
   * This is a list of all the route names that will exit the app if the back button
   * is pressed while in that screen. Only affects Android.
   */
  exitRoutes: ["Welcome"],

  /**
   * The API URL for backend services
   */
  API_URL: process.env.API_URL,

  /**
   * The Aiur API URL for OpenAgents.com services
   */
  AIUR_API_URL: process.env.AIUR_API_URL,

  /**
   * The Groq API key for chat completions
   */
  GROQ_API_KEY: Constants.expoConfig?.extra?.GROQ_API_KEY ?? "grrr",

  /**
   * Nexus API URL
   */
  NEXUS_URL: "http://localhost:3000",

  /**
   * WebSocket URL for real-time connections
   */
  WS_URL: process.env.WS_URL,
}

export default BaseConfig
