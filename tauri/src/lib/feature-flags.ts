/**
 * Feature flags for gradual migration from Tinyvex to Convex
 */

// Set to true to use Convex for data storage
// Set to false to use Tinyvex (current implementation)
export const USE_CONVEX = import.meta.env.VITE_USE_CONVEX === "true";

// Individual feature flags for granular control
export const FEATURE_FLAGS = {
  // Use Convex for thread list
  CONVEX_THREADS: USE_CONVEX,

  // Use Convex for messages
  CONVEX_MESSAGES: USE_CONVEX,

  // Use Convex for tool calls
  CONVEX_TOOL_CALLS: USE_CONVEX,

  // Use Convex for projects
  CONVEX_PROJECTS: USE_CONVEX,

  // Use Convex for plan entries
  CONVEX_PLANS: USE_CONVEX,

  // Use Convex for thread state
  CONVEX_THREAD_STATE: USE_CONVEX,
} as const;

// Helper to check if Convex is enabled for any feature
export const isConvexEnabled = () => USE_CONVEX;

// Helper to check if Tinyvex should still be used
export const isTinyvexEnabled = () => !USE_CONVEX;

// Log feature flag status on startup
if (typeof window !== "undefined") {
  console.info("[Feature Flags]", {
    USE_CONVEX,
    CONVEX_URL: import.meta.env.VITE_CONVEX_URL,
    flags: FEATURE_FLAGS,
  });
}
