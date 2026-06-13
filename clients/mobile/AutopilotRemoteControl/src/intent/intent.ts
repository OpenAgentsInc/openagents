export type WorkIntentDraft = {
  title: string
  body: string
  scopeHint?: string
}

export type IntentDraftValidationResult = { ok: true } | { ok: false; errors: string[] }

export type IntentSubmitPayload = {
  intentId: string
  title: string
  body: string
  scopeHint?: string
  submittedByClientRef: string
  createdAt: number
}

export type IntentSubmitPayloadMeta = {
  clientRef: string
  createdAtMs: number
  intentId?: string
  idInput?: string
}

export const WORK_INTENT_TITLE_MAX_LENGTH = 120

export function validateIntentDraft(draft: WorkIntentDraft): IntentDraftValidationResult {
  const errors: string[] = []

  if (draft.title.trim().length === 0) errors.push("Title is required")
  if (draft.title.length > WORK_INTENT_TITLE_MAX_LENGTH) {
    errors.push(`Title must be ${WORK_INTENT_TITLE_MAX_LENGTH} characters or fewer`)
  }
  if (draft.body.trim().length === 0) errors.push("Body is required")

  return errors.length === 0 ? { ok: true } : { ok: false, errors }
}

export function buildIntentSubmitPayload(draft: WorkIntentDraft, meta: IntentSubmitPayloadMeta): IntentSubmitPayload {
  const payload: IntentSubmitPayload = {
    intentId: meta.intentId ?? buildIntentId(meta.idInput ?? stableIntentIdInput(draft, meta)),
    title: draft.title,
    body: draft.body,
    submittedByClientRef: meta.clientRef,
    createdAt: meta.createdAtMs,
  }

  if (draft.scopeHint !== undefined) payload.scopeHint = draft.scopeHint

  return payload
}

function stableIntentIdInput(draft: WorkIntentDraft, meta: IntentSubmitPayloadMeta): string {
  return JSON.stringify({
    title: draft.title,
    body: draft.body,
    scopeHint: draft.scopeHint ?? null,
    clientRef: meta.clientRef,
    createdAtMs: meta.createdAtMs,
  })
}

function buildIntentId(input: string): string {
  return `intent_${fnv1a32(input)}`
}

function fnv1a32(input: string): string {
  let hash = 0x811c9dc5

  for (let index = 0; index < input.length; index += 1) {
    hash ^= input.charCodeAt(index)
    hash = Math.imul(hash, 0x01000193)
  }

  return (hash >>> 0).toString(16).padStart(8, "0")
}
