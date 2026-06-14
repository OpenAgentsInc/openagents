// CL-59: pure (RN-free) projection helpers for the session-detail screen, kept
// out of session-detail.tsx so they can be unit-tested under `bun test` without
// pulling in the React Native module graph (the repo's existing convention —
// see src/control/session-view-model.ts). session-detail.tsx re-exports
// `verifyText`, so the screen's public surface still owns it.

import type { ControlSessionRow } from "../src/control/control-client"

// Project a session's verify line + tone. Mirrors the inline logic the
// nodes.tsx detail block used before the dedicated screen existed.
export function verifyText(session: ControlSessionRow): { text: string; tone: "ok" | "bad" | "muted" } {
  switch (session.state) {
    case "completed":
      return {
        text: `✓ verify passed${session.artifactRef ? ` · artifact ${session.artifactRef.slice(-12)}` : ""}`,
        tone: "ok",
      }
    case "failed":
      return {
        text: `✗ verify failed${session.errorClass ? ` · ${session.errorClass}` : ""}`,
        tone: "bad",
      }
    case "cancelled":
      return { text: "cancelled", tone: "muted" }
    default:
      return { text: `${session.state}…`, tone: "muted" }
  }
}
