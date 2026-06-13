import { describe, expect, test } from "bun:test"

import {
  WORK_INTENT_TITLE_MAX_LENGTH,
  buildIntentSubmitPayload,
  validateIntentDraft,
  type WorkIntentDraft,
} from "./intent"

describe("intent draft model", () => {
  test("accepts a valid draft", () => {
    expect(validateIntentDraft(validDraft())).toEqual({ ok: true })
  })

  test("rejects an empty title", () => {
    expect(validateIntentDraft({ ...validDraft(), title: "  " })).toEqual({
      ok: false,
      errors: ["Title is required"],
    })
  })

  test("rejects an over-long title", () => {
    expect(validateIntentDraft({ ...validDraft(), title: "a".repeat(WORK_INTENT_TITLE_MAX_LENGTH + 1) })).toEqual({
      ok: false,
      errors: [`Title must be ${WORK_INTENT_TITLE_MAX_LENGTH} characters or fewer`],
    })
  })

  test("rejects an empty body", () => {
    expect(validateIntentDraft({ ...validDraft(), body: "\n\t " })).toEqual({
      ok: false,
      errors: ["Body is required"],
    })
  })

  test("builds a serializable intent submit payload carrying draft and client fields", () => {
    expect(
      buildIntentSubmitPayload(validDraft(), {
        clientRef: "mobile-client-1",
        createdAtMs: 1_764_000_000_000,
        intentId: "intent_fixture",
      }),
    ).toEqual({
      intentId: "intent_fixture",
      title: "Add mobile intent capture",
      body: "Let users compose work requests from the phone.",
      scopeHint: "mobile",
      submittedByClientRef: "mobile-client-1",
      createdAt: 1_764_000_000_000,
    })
  })

  test("derives a deterministic intent id from caller-provided inputs", () => {
    const first = buildIntentSubmitPayload(validDraft(), {
      clientRef: "mobile-client-1",
      createdAtMs: 1_764_000_000_000,
      idInput: "stable-submit-input",
    })
    const second = buildIntentSubmitPayload(validDraft(), {
      clientRef: "mobile-client-1",
      createdAtMs: 1_764_000_000_000,
      idInput: "stable-submit-input",
    })
    const changed = buildIntentSubmitPayload(validDraft(), {
      clientRef: "mobile-client-1",
      createdAtMs: 1_764_000_000_000,
      idInput: "different-submit-input",
    })

    expect(first.intentId).toBe(second.intentId)
    expect(first.intentId).toMatch(/^intent_[0-9a-f]{8}$/)
    expect(first.intentId).not.toBe(changed.intentId)
  })
})

function validDraft(): WorkIntentDraft {
  return {
    title: "Add mobile intent capture",
    body: "Let users compose work requests from the phone.",
    scopeHint: "mobile",
  }
}
