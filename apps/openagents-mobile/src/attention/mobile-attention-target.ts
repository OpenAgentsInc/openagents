import type { ConfirmedRuntimeAttentionSnapshot } from "@openagentsinc/khala-sync-client"
import { Schema } from "effect"

export const MobileAttentionTargetSchemaVersion = "openagents.mobile.attention_target.v1"

const Ref = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)

export const MobileAttentionTarget = Schema.Struct({
  schema: Schema.Literal(MobileAttentionTargetSchemaVersion),
  attentionRef: Ref,
  threadRef: Ref,
  turnRef: Ref,
})
export type MobileAttentionTarget = typeof MobileAttentionTarget.Type

export type MobileAttentionCandidate =
  | Readonly<{ source: "in_app"; target: MobileAttentionTarget }>
  | Readonly<{ source: "deep_link"; url: string }>
  | Readonly<{ source: "notification"; payload: unknown }>

export type MobileAttentionResolution =
  | Readonly<{ state: "ready"; source: MobileAttentionCandidate["source"]; target: MobileAttentionTarget }>
  | Readonly<{
      state: "rejected"
      reason: "authority_unavailable" | "projection_invalid" | "invalid_target" |
        "unknown_attention" | "terminal_attention" | "target_mismatch"
      affectedRef: string
    }>

const decodeTarget = Schema.decodeUnknownSync(MobileAttentionTarget)
const rejected = (
  reason: Extract<MobileAttentionResolution, { state: "rejected" }>["reason"],
  affectedRef = "invalid_target",
): MobileAttentionResolution => ({ state: "rejected", reason, affectedRef })

export const decodeMobileAttentionDeepLink = (url: string): MobileAttentionTarget => {
  const parsed = new URL(url)
  if (parsed.protocol !== "openagents:" || parsed.hostname !== "attention") {
    throw new Error("unsupported OpenAgents attention deep link")
  }
  const queryKeys = [...parsed.searchParams.keys()]
  if (queryKeys.length !== 2 || !queryKeys.includes("threadRef") || !queryKeys.includes("turnRef")) {
    throw new Error("invalid OpenAgents attention deep link fields")
  }
  const attentionRef = decodeURIComponent(parsed.pathname.replace(/^\//, ""))
  return decodeTarget({
    schema: MobileAttentionTargetSchemaVersion,
    attentionRef,
    threadRef: parsed.searchParams.get("threadRef"),
    turnRef: parsed.searchParams.get("turnRef"),
  })
}

export const decodeMobileAttentionNotification = (payload: unknown): MobileAttentionTarget => {
  if (typeof payload !== "object" || payload === null || Array.isArray(payload)) {
    throw new Error("invalid OpenAgents attention notification")
  }
  const keys = Object.keys(payload).sort()
  if (keys.join(",") !== "attentionRef,schema,threadRef,turnRef") {
    throw new Error("invalid OpenAgents attention notification fields")
  }
  return decodeTarget(payload)
}

export const resolveMobileAttentionTarget = (
  snapshot: ConfirmedRuntimeAttentionSnapshot,
  candidate: MobileAttentionCandidate,
): MobileAttentionResolution => {
  if (snapshot.status.phase !== "live") return rejected("authority_unavailable")
  if (snapshot.issues.length > 0) return rejected("projection_invalid")
  let target: MobileAttentionTarget
  try {
    target = candidate.source === "in_app"
      ? decodeTarget(candidate.target)
      : candidate.source === "deep_link"
        ? decodeMobileAttentionDeepLink(candidate.url)
        : decodeMobileAttentionNotification(candidate.payload)
  } catch {
    return rejected("invalid_target")
  }
  const pending = snapshot.pending.find(item => item.attentionRef === target.attentionRef)
  if (pending !== undefined) {
    return pending.threadRef === target.threadRef && pending.turnRef === target.turnRef
      ? { state: "ready", source: candidate.source, target }
      : rejected("target_mismatch", target.attentionRef)
  }
  return snapshot.terminal.some(item => item.attentionRef === target.attentionRef)
    ? rejected("terminal_attention", target.attentionRef)
    : rejected("unknown_attention", target.attentionRef)
}
