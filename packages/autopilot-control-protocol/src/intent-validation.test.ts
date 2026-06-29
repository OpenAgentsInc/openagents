import { describe, expect, test } from "bun:test"

import { validateIntentDraft } from "./intent-validation.js"

describe("intent draft validation", () => {
  test("accepts valid strings and trims both fields", () => {
    expect(validateIntentDraft({
      title: "  Ship mobile capture  ",
      body: "  Keep the capture flow fast.  ",
    })).toEqual({
      ok: true,
      title: "Ship mobile capture",
      body: "Keep the capture flow fast.",
      errors: [],
    })
  })

  test("accepts an empty body after trimming", () => {
    expect(validateIntentDraft({
      title: "Capture intent",
      body: "   ",
    })).toEqual({
      ok: true,
      title: "Capture intent",
      body: "",
      errors: [],
    })
  })

  test("rejects an empty title after trimming", () => {
    expect(validateIntentDraft({
      title: "   ",
      body: "Body is optional.",
    })).toEqual({
      ok: false,
      title: "",
      body: "Body is optional.",
      errors: ["title is required"],
    })
  })

  test("rejects titles longer than 120 characters", () => {
    const title = "a".repeat(121)

    expect(validateIntentDraft({
      title,
      body: "",
    })).toEqual({
      ok: false,
      title,
      body: "",
      errors: ["title must be 120 characters or fewer"],
    })
  })

  test("accepts title and body at their maximum lengths", () => {
    const title = "t".repeat(120)
    const body = "b".repeat(4000)

    expect(validateIntentDraft({ title, body })).toEqual({
      ok: true,
      title,
      body,
      errors: [],
    })
  })

  test("rejects bodies longer than 4000 characters", () => {
    const body = "b".repeat(4001)

    expect(validateIntentDraft({
      title: "Capture intent",
      body,
    })).toEqual({
      ok: false,
      title: "Capture intent",
      body,
      errors: ["body must be 4000 characters or fewer"],
    })
  })

  test("reports non-string title and body without coercing values", () => {
    expect(validateIntentDraft({
      title: 42,
      body: null,
    })).toEqual({
      ok: false,
      title: "",
      body: "",
      errors: ["title must be a string", "body must be a string"],
    })
  })

  test("collects type and length errors together", () => {
    const body = "b".repeat(4001)

    expect(validateIntentDraft({
      title: undefined,
      body,
    })).toEqual({
      ok: false,
      title: "",
      body,
      errors: ["title must be a string", "body must be 4000 characters or fewer"],
    })
  })
})
