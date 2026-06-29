export type SessionBadgeTone = "running" | "ok" | "warn" | "error" | "idle"

export function sessionBadge(state: string): {
  label: string
  tone: SessionBadgeTone
} {
  if (state === "running") {
    return { label: "running", tone: "running" }
  }

  if (state === "completed") {
    return { label: "completed", tone: "ok" }
  }

  if (state === "failed" || state === "error") {
    return { label: state, tone: "error" }
  }

  if (state === "cancelled") {
    return { label: "cancelled", tone: "warn" }
  }

  return { label: state, tone: "idle" }
}
