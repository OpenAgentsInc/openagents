/**
 * SM-3: Sarah email sends go through the monorepo approval-gated CRM rail
 * (openagents.com Worker `dispatchCrmSend` / operator drafts), not a parallel
 * Resend stack.
 *
 * Local file-backed draft + opt-out projection exists only for:
 * - dry-run when no operator bearer is armed
 * - offline smokes / S-8 / S-13 oracles
 * - unsubscribe recording before Worker CRM is the sole live authority
 *
 * Production send remains Worker-owned; Sarah never sends mail itself.
 */

import { mkdir, readFile, writeFile } from "node:fs/promises"
import { dirname, join } from "node:path"
import { z } from "zod"

/** Public Sarah base (path mount — not a separate subdomain). */
export function sarahPublicBaseUrl() {
  return (
    process.env.SARAH_PUBLIC_BASE_URL?.replace(/\/+$/, "") ??
    "https://openagents.com/sarah"
  )
}

export function sarahEmailOptOutUrl(email: string) {
  return `${sarahPublicBaseUrl()}/unsubscribe?email=${encodeURIComponent(email)}`
}

/** AI disclosure + opt-out footer required on every outbound Sarah email draft. */
export function appendEmailComplianceFooter(reply: string, toEmail: string) {
  return [
    reply.trim(),
    "",
    "--",
    "Sarah is an AI sales employee for OpenAgents.",
    "OpenAgents, Inc.",
    `To opt out of Sarah email follow-ups: ${sarahEmailOptOutUrl(toEmail)}`,
  ].join("\n")
}

export type SarahEmailDraftStatus =
  | "pending_approval"
  | "approved_pending_send"
  | "rejected"
  | "sent"
  | "suppressed"
  | "send_failed"
  | "dry_run"
  | "queued_for_approval"

export type SarahEmailDraftRecord = {
  id: string
  status: SarahEmailDraftStatus
  fromEmail: string
  toEmail: string
  subject: string
  inboundText: string
  proposedReply: string
  bodyWithDisclosure: string
  prospectRef: string
  threadId: string
  messageId: string | null
  continuationToken: string
  optOutUrl: string
  createdAt: string
  updatedAt: string
  reviewedAt: string | null
  reviewerRef: string | null
  reviewNote: string | null
  sentAt: string | null
  sendError: string | null
  providerMessageId: string | null
  channel: "crm_operator_rail"
  contactId?: string
  sourceRef?: string
}

type LocalDraftQueue = {
  schema: "sarah.crm_email_rail_queue.v1"
  drafts: Record<string, SarahEmailDraftRecord>
}

type LocalSuppression = {
  email: string
  reason: "unsubscribe" | "operator"
  source: string
  createdAt: string
}

type LocalSuppressionList = {
  schema: "sarah.crm_email_rail_suppression.v1"
  suppressions: Record<string, LocalSuppression>
}

export const sarahEmailDraftSchema = z.object({
  to: z.email().max(320),
  subject: z.string().min(1).max(300),
  bodyText: z.string().min(1).max(20_000),
  prospectRef: z.string().min(1).max(220).optional(),
  contactId: z.string().min(1).max(220).optional(),
  sourceRef: z.string().min(1).max(220).default("sarah.email_draft.v1"),
})

export type SarahEmailDraft = z.infer<typeof sarahEmailDraftSchema>

function openagentsApiBase() {
  return (
    process.env.OPENAGENTS_API_BASE_URL?.replace(/\/$/, "") ||
    process.env.SARAH_OPENAGENTS_API_BASE?.replace(/\/$/, "") ||
    "https://openagents.com"
  )
}

function operatorBearer() {
  return (
    process.env.SARAH_OPERATOR_BEARER ||
    process.env.OPENAGENTS_OPERATOR_TOKEN ||
    null
  )
}

function queuePath() {
  const configured = process.env.SARAH_EMAIL_APPROVAL_QUEUE_PATH
  if (!configured) {
    return join(process.cwd(), ".sarah", "crm-email-rail-queue.json")
  }
  return join(process.cwd(), ".sarah", configured)
}

function suppressionPath() {
  const configured = process.env.SARAH_EMAIL_SUPPRESSION_LIST_PATH
  if (!configured) {
    return join(process.cwd(), ".sarah", "crm-email-rail-suppressions.json")
  }
  return join(process.cwd(), ".sarah", configured)
}

export function normalizeSuppressionEmail(email: string) {
  return email.trim().toLowerCase()
}

async function readQueue(): Promise<LocalDraftQueue> {
  try {
    const raw = await readFile(queuePath(), "utf8")
    const parsed = JSON.parse(raw) as LocalDraftQueue
    if (parsed.schema === "sarah.crm_email_rail_queue.v1") return parsed
  } catch {
    // empty
  }
  return { schema: "sarah.crm_email_rail_queue.v1", drafts: {} }
}

let writeQueue = Promise.resolve()

async function writeApprovalQueue(queue: LocalDraftQueue) {
  const path = queuePath()
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, `${JSON.stringify(queue, null, 2)}\n`)
}

async function readSuppressionList(): Promise<LocalSuppressionList> {
  try {
    const raw = await readFile(suppressionPath(), "utf8")
    const parsed = JSON.parse(raw) as LocalSuppressionList
    if (parsed.schema === "sarah.crm_email_rail_suppression.v1") return parsed
  } catch {
    // empty
  }
  return { schema: "sarah.crm_email_rail_suppression.v1", suppressions: {} }
}

let writeSuppression = Promise.resolve()

async function writeSuppressionList(list: LocalSuppressionList) {
  const path = suppressionPath()
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, `${JSON.stringify(list, null, 2)}\n`)
}

/** Local opt-out projection (CRM rail is still the production authority). */
export async function suppressSarahEmail(input: {
  email: string
  reason: LocalSuppression["reason"]
  source: string
}) {
  const email = normalizeSuppressionEmail(input.email)
  let suppression: LocalSuppression | null = null
  writeSuppression = writeSuppression.then(async () => {
    const list = await readSuppressionList()
    suppression = list.suppressions[email] ?? {
      createdAt: new Date().toISOString(),
      email,
      reason: input.reason,
      source: input.source,
    }
    list.suppressions[email] = suppression
    await writeSuppressionList(list)
  })
  await writeSuppression
  return suppression
}

export async function isSarahEmailSuppressed(email: string) {
  const list = await readSuppressionList()
  return list.suppressions[normalizeSuppressionEmail(email)] ?? null
}

/** Alias for thin callers. */
export async function isEmailSuppressed(email: string): Promise<boolean> {
  return (await isSarahEmailSuppressed(email)) !== null
}

export async function listSarahEmailSuppressions() {
  const list = await readSuppressionList()
  return Object.values(list.suppressions).sort((a, b) =>
    b.createdAt.localeCompare(a.createdAt),
  )
}

function isRichEnqueue(
  input: Record<string, unknown>,
): input is {
  fromEmail: string
  toEmail: string
  subject: string
  inboundText: string
  proposedReply: string
  prospectRef: string
  threadId: string
  messageId: string | null
  continuationToken: string
} {
  return (
    typeof input.toEmail === "string" &&
    typeof input.proposedReply === "string" &&
    typeof input.prospectRef === "string"
  )
}

/**
 * Enqueue a draft on the CRM rail projection.
 * Accepts either the internal follow-up/email-channel shape or the thin
 * operator {to, subject, bodyText} shape.
 */
export async function enqueueSarahEmailDraft(
  input:
    | SarahEmailDraft
    | {
        fromEmail: string
        toEmail: string
        subject: string
        inboundText: string
        proposedReply: string
        prospectRef: string
        threadId: string
        messageId: string | null
        continuationToken: string
        contactId?: string
        sourceRef?: string
      },
): Promise<
  | SarahEmailDraftRecord
  | {
      ok: boolean
      draftRef: string
      channel: "crm_operator_rail"
      status: "queued_for_approval" | "dry_run" | "rejected"
      detail?: string
    }
> {
  // Thin operator / HTTP shape
  if (!isRichEnqueue(input as Record<string, unknown>)) {
    const draft = sarahEmailDraftSchema.parse(input)
    const bearer = operatorBearer()
    const draftRef = `draft.sarah.${crypto.randomUUID()}`
    const bodyWithDisclosure = appendEmailComplianceFooter(
      draft.bodyText,
      draft.to,
    )
    const suppressed = await isSarahEmailSuppressed(draft.to)
    if (suppressed) {
      const record = await persistLocalDraft({
        fromEmail: "sarah@openagents.com",
        toEmail: draft.to,
        subject: draft.subject,
        inboundText: "",
        proposedReply: draft.bodyText,
        prospectRef: draft.prospectRef ?? `email:${draft.to}`,
        threadId: draft.prospectRef ?? `email:${draft.to}`,
        messageId: null,
        continuationToken: draftRef,
        contactId: draft.contactId,
        sourceRef: draft.sourceRef,
        forcedStatus: "suppressed",
        reviewNote: `Suppressed by ${suppressed.reason} at ${suppressed.createdAt}.`,
      })
      return {
        ok: false,
        draftRef: record.id,
        channel: "crm_operator_rail",
        status: "rejected",
        detail: "email suppressed on CRM rail projection",
      }
    }

    if (!bearer) {
      const record = await persistLocalDraft({
        fromEmail: "sarah@openagents.com",
        toEmail: draft.to,
        subject: draft.subject,
        inboundText: "",
        proposedReply: draft.bodyText,
        prospectRef: draft.prospectRef ?? `email:${draft.to}`,
        threadId: draft.prospectRef ?? `email:${draft.to}`,
        messageId: null,
        continuationToken: draftRef,
        contactId: draft.contactId,
        sourceRef: draft.sourceRef,
        forcedStatus: "pending_approval",
      })
      return {
        ok: true,
        draftRef: record.id,
        channel: "crm_operator_rail",
        status: "dry_run",
        detail:
          "no operator bearer configured; draft held on local CRM rail projection",
      }
    }

    const url = `${openagentsApiBase()}/api/operator/business/email-drafts`
    try {
      const response = await fetch(url, {
        method: "POST",
        headers: {
          authorization: `Bearer ${bearer}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({
          to: draft.to,
          subject: draft.subject,
          bodyText: bodyWithDisclosure,
          prospectRef: draft.prospectRef ?? null,
          contactId: draft.contactId ?? null,
          sourceRef: draft.sourceRef,
          requiresApproval: true,
          draftRef,
        }),
      })
      if (!response.ok) {
        const text = await response.text()
        return {
          ok: false,
          draftRef,
          channel: "crm_operator_rail",
          status: "rejected",
          detail: `crm rail HTTP ${response.status}: ${text.slice(0, 200)}`,
        }
      }
      return {
        ok: true,
        draftRef,
        channel: "crm_operator_rail",
        status: "queued_for_approval",
      }
    } catch (error) {
      return {
        ok: false,
        draftRef,
        channel: "crm_operator_rail",
        status: "rejected",
        detail: error instanceof Error ? error.message : String(error),
      }
    }
  }

  // Rich internal shape (follow-ups, email channel, smokes)
  return persistLocalDraft({
    ...input,
    forcedStatus: undefined,
  })
}

async function persistLocalDraft(input: {
  fromEmail: string
  toEmail: string
  subject: string
  inboundText: string
  proposedReply: string
  prospectRef: string
  threadId: string
  messageId: string | null
  continuationToken: string
  contactId?: string
  sourceRef?: string
  forcedStatus?: SarahEmailDraftStatus
  reviewNote?: string | null
}): Promise<SarahEmailDraftRecord> {
  const id = `sarah_email_draft.${crypto.randomUUID()}`
  const now = new Date().toISOString()
  const suppressed = await isSarahEmailSuppressed(input.toEmail)
  const status: SarahEmailDraftStatus =
    input.forcedStatus ??
    (suppressed === null ? "pending_approval" : "suppressed")

  const draft: SarahEmailDraftRecord = {
    id,
    status,
    fromEmail: input.fromEmail,
    toEmail: input.toEmail,
    subject: input.subject,
    inboundText: input.inboundText,
    proposedReply: input.proposedReply,
    bodyWithDisclosure: appendEmailComplianceFooter(
      input.proposedReply,
      input.toEmail,
    ),
    prospectRef: input.prospectRef,
    threadId: input.threadId,
    messageId: input.messageId,
    continuationToken: input.continuationToken,
    optOutUrl: sarahEmailOptOutUrl(input.toEmail),
    createdAt: now,
    updatedAt: now,
    reviewedAt: null,
    reviewerRef: null,
    reviewNote:
      input.reviewNote ??
      (suppressed
        ? `Suppressed by ${suppressed.reason} at ${suppressed.createdAt}.`
        : null),
    sentAt: null,
    sendError: null,
    providerMessageId: null,
    channel: "crm_operator_rail",
    contactId: input.contactId,
    sourceRef: input.sourceRef,
  }

  writeQueue = writeQueue.then(async () => {
    const queue = await readQueue()
    queue.drafts[id] = draft
    await writeApprovalQueue(queue)
  })
  await writeQueue
  return draft
}

export async function listSarahEmailDrafts() {
  const queue = await readQueue()
  return Object.values(queue.drafts).sort((a, b) =>
    b.createdAt.localeCompare(a.createdAt),
  )
}

export async function reviewSarahEmailDraft(input: {
  draftId: string
  decision: "approve" | "reject"
  reviewerRef: string
  note: string | null
}): Promise<SarahEmailDraftRecord | null> {
  let result: SarahEmailDraftRecord | null = null
  writeQueue = writeQueue.then(async () => {
    const queue = await readQueue()
    const draft = queue.drafts[input.draftId]
    if (!draft) {
      result = null
      return
    }
    if (draft.status === "suppressed" || draft.status === "sent") {
      result = draft
      return
    }
    const now = new Date().toISOString()
    // Sarah never sends; approval stays pending send until CRM rail executes.
    const nextStatus: SarahEmailDraftStatus =
      input.decision === "approve" ? "approved_pending_send" : "rejected"
    const reviewed: SarahEmailDraftRecord = {
      ...draft,
      reviewedAt: now,
      reviewerRef: input.reviewerRef,
      reviewNote: input.note,
      status: nextStatus,
      updatedAt: now,
    }
    queue.drafts[input.draftId] = reviewed
    await writeApprovalQueue(queue)
    result = reviewed
  })
  await writeQueue
  return result
}
