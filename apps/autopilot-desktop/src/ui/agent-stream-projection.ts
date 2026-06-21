// VCODE-07 (#5924): public-safe Codex agent stream projection.
//
// This module is intentionally pure: it maps the existing Pylon control
// `session.list` + per-session event tail into compact rows the Verse code dock
// and later panes can render. It never reads raw provider payloads, local files,
// or secrets, and it keeps row keys stable across replayed event batches.

import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"
import type { AccountRow, SessionEventRow } from "../shared/rpc.js"
import { classifyStreamLine } from "./stream-render.js"

export type AgentStreamRowKind =
  | "objective"
  | "plan"
  | "tool"
  | "file"
  | "check"
  | "approval"
  | "error"
  | "done"

export type AgentStreamRow = Readonly<{
  key: string
  kind: AgentStreamRowKind
  sessionRef: string
  eventIndex: number | null
  adapter: string
  accountLabel: string
  accountRefHash: string | null
  title: string
  body: string
  meta: string
}>

export type AgentStreamProjectionInput = Readonly<{
  session: SessionSummary
  events: ReadonlyArray<SessionEventRow>
  accounts?: ReadonlyArray<AccountRow>
}>

const LONG_HEX = /\b[a-f0-9]{24,}\b/gi
const SECRET_TOKEN =
  /\b(?:sk-[A-Za-z0-9_-]{12,}|gh[pousr]_[A-Za-z0-9_]{12,}|xox[baprs]-[A-Za-z0-9-]{12,}|mdk_[A-Za-z0-9_]{12,})\b/g
const LOCAL_PATH =
  /(?:~\/|\/Users\/|\/private\/|\/var\/folders\/|[A-Za-z]:\\)[^\s"'`),;]+/g
const PROVIDER_JSON = /\{[\s\S]{80,}\}/g

export const shortPublicRef = (value: string | null | undefined): string | null => {
  const ref = value?.trim()
  if (!ref) return null
  if (ref.length <= 18) return ref
  const parts = ref.split(".").filter(Boolean)
  const last = parts.at(-1) ?? ref
  if (/^[a-f0-9]{12,}$/i.test(last)) return `#${last.slice(-8)}`
  return `${parts.slice(0, 2).join(".") || "ref"}…${last.slice(-8)}`
}

const shortHash = (value: string | null | undefined): string | null => {
  const hash = value?.trim()
  return hash ? `#${hash.slice(-8)}` : null
}

export const sanitizeAgentStreamText = (text: string): string => {
  const clean = text
    .replace(PROVIDER_JSON, "[provider payload]")
    .replace(SECRET_TOKEN, "[secret]")
    .replace(LOCAL_PATH, "[local path]")
    .replace(LONG_HEX, (match) => `#${match.slice(-8)}`)
    .replace(/\s+/g, " ")
    .trim()
  return clean.length > 140 ? `${clean.slice(0, 137)}...` : clean
}

export const agentStreamAccountLabel = (
  session: Pick<SessionSummary, "adapter" | "accountRefHash">,
  accounts: ReadonlyArray<AccountRow> = [],
): { readonly label: string; readonly hash: string | null } => {
  const hash = shortHash(session.accountRefHash)
  if (session.accountRefHash === null || session.accountRefHash.trim() === "") {
    return { label: `${session.adapter} default`, hash: null }
  }
  const account =
    accounts.find((row) => row.accountRefHash === session.accountRefHash) ?? null
  if (account !== null) {
    const label =
      account.selector === "default_home"
        ? `${account.provider} default`
        : `${account.provider} ${account.accountRef ?? hash ?? "account"}`
    return { label, hash }
  }
  return { label: `${session.adapter} ${hash ?? "account"}`, hash }
}

const classifyAgentStreamEvent = (
  event: SessionEventRow,
): { readonly kind: AgentStreamRowKind; readonly title: string; readonly body: string } | null => {
  const source = event.detail.trim()
  const lower = source.toLowerCase()
  if (event.phase === "decision_requested") {
    return { kind: "approval", title: "Permission", body: "approval requested" }
  }
  if (event.phase === "decision_resolved" || event.phase === "decision_cancelled") {
    return { kind: "approval", title: "Permission", body: "approval resolved" }
  }
  if (event.phase === "completed" || event.state === "completed") {
    return { kind: "done", title: "Done", body: "session completed" }
  }
  if (event.phase === "failed" || event.state === "failed" || lower.startsWith("error:")) {
    return { kind: "error", title: "Error", body: sanitizeAgentStreamText(source || "session failed") }
  }
  if (event.phase === "cancelled" || event.state === "cancelled") {
    return { kind: "done", title: "Cancelled", body: "session cancelled" }
  }
  if (/^(thinking|plan|planning|agent):/i.test(source)) {
    const classified = classifyStreamLine(source)
    return {
      kind: "plan",
      title: classified.kind === "reasoning" ? "Thinking" : "Plan",
      body: sanitizeAgentStreamText(classified.text || source),
    }
  }
  if (
    /\b(edit|edited|create|created|update|updated|delete|deleted|modify|modified|patch|diff)\b/i
      .test(source)
  ) {
    return { kind: "file", title: "Files", body: sanitizeAgentStreamText(source) }
  }
  if (/\b(test|verify|typecheck|lint|check|exit\s+\d+|passed|failed)\b/i.test(source)) {
    return { kind: "check", title: "Check", body: sanitizeAgentStreamText(source) }
  }
  const classified = classifyStreamLine(source)
  if (classified.kind === "noise" || classified.text.trim() === "") return null
  if (classified.kind === "error") {
    return { kind: "error", title: "Error", body: sanitizeAgentStreamText(classified.text) }
  }
  if (classified.kind === "tool") {
    return { kind: "tool", title: "Tool", body: sanitizeAgentStreamText(classified.text) }
  }
  return { kind: "plan", title: "Plan", body: sanitizeAgentStreamText(classified.text) }
}

export const projectAgentStreamRows = (
  input: AgentStreamProjectionInput,
): ReadonlyArray<AgentStreamRow> => {
  const account = agentStreamAccountLabel(input.session, input.accounts ?? [])
  const base = {
    sessionRef: input.session.sessionRef,
    adapter: input.session.adapter,
    accountLabel: account.label,
    accountRefHash: account.hash,
  }
  const objectiveRef = shortPublicRef(input.session.objectiveRef)
  const rows: AgentStreamRow[] = [
    {
      ...base,
      key: `${input.session.sessionRef}:objective`,
      kind: "objective",
      eventIndex: null,
      title: "Objective",
      body: objectiveRef === null ? "objective accepted" : `objective ${objectiveRef}`,
      meta: `${input.session.adapter} · ${account.label}`,
    },
  ]
  for (const event of input.events) {
    const classified = classifyAgentStreamEvent(event)
    if (classified === null) continue
    rows.push({
      ...base,
      key: `${input.session.sessionRef}:event:${event.eventIndex}`,
      kind: classified.kind,
      eventIndex: event.eventIndex,
      title: classified.title,
      body: classified.body,
      meta: `${event.phase} · #${event.eventIndex}`,
    })
  }
  return rows
}
