export type IntentDraftInput = {
  title: unknown
  body: unknown
}

export type IntentDraftValidation = {
  ok: boolean
  title: string
  body: string
  errors: string[]
}

export function validateIntentDraft(input: IntentDraftInput): IntentDraftValidation {
  const errors: string[] = []
  let title = ""
  let body = ""

  if (typeof input.title !== "string") {
    errors.push("title must be a string")
  } else {
    title = input.title.trim()
    if (title.length === 0) errors.push("title is required")
    if (title.length > 120) errors.push("title must be 120 characters or fewer")
  }

  if (typeof input.body !== "string") {
    errors.push("body must be a string")
  } else {
    body = input.body.trim()
    if (body.length > 4000) errors.push("body must be 4000 characters or fewer")
  }

  return {
    ok: errors.length === 0,
    title,
    body,
    errors,
  }
}
