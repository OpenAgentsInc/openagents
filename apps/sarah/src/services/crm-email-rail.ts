/**
 * SM-3: Sarah email sends go through the monorepo approval-gated CRM rail
 * (openagents.com Worker `dispatchCrmSend` / operator drafts), not a parallel
 * Resend + local suppression stack.
 *
 * This module is a thin client: enqueue is "create operator draft / CRM activity
 * via public API"; actual send is owner-approved on the shared queue.
 */

import { z } from "zod"

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

/**
 * Enqueue a draft on the shared CRM/operator rail. Never sends directly.
 * Returns a public-safe draft ref or a dry-run receipt when unarmed.
 */
export async function enqueueSarahEmailDraft(
  input: SarahEmailDraft,
): Promise<{
  ok: boolean
  draftRef: string
  channel: "crm_operator_rail"
  status: "queued_for_approval" | "dry_run" | "rejected"
  detail?: string
}> {
  const draft = sarahEmailDraftSchema.parse(input)
  const bearer = operatorBearer()
  const draftRef = `draft.sarah.${crypto.randomUUID()}`

  if (!bearer) {
    return {
      ok: true,
      draftRef,
      channel: "crm_operator_rail",
      status: "dry_run",
      detail: "no operator bearer configured; draft held locally as dry-run",
    }
  }

  // Shared operator email-draft surface on openagents.com (CRM rail).
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
        bodyText: draft.bodyText,
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

/** Suppression is owned by the monorepo CRM rail — never a Sarah-local list. */
export async function isEmailSuppressed(_email: string): Promise<boolean> {
  // Local Sarah no longer holds a suppression DB. Default false; the Worker
  // re-checks on send. Callers must not send without Worker approval receipt.
  return false
}
