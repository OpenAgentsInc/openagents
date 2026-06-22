import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  emptyWorldModerationState,
  makeWorldModeration,
  maskSoftTokensWithDefaultPolicy,
  moderateForumReflectionBubbleWithDefaultPolicy,
  redactDiagnosticTextWithDefaultPolicy,
  tokenizeModerationText,
} from "./moderation"

const observedAt = "2026-06-22T00:00:00.000Z"
const subject = {
  actorRef: "actor.public.alice",
  sessionRef: "session.alice.1",
  surface: "local_chat" as const,
}

describe("world moderation policy", () => {
  test("empty hard-list allows ordinary chat and keeps soft masking client/user controlled", () => {
    const moderation = makeWorldModeration({
      hardBlockedTokens: [],
      softMaskedTokens: ["spoiler"],
    })
    const decision = Effect.runSync(moderation.moderateText({
      text: "this spoiler remains visible on the server",
      subject,
      state: emptyWorldModerationState,
      observedAt,
    }))

    expect(decision.kind).toBe("allowed")
    if (decision.kind === "allowed") {
      expect(decision.text).toBe("this spoiler remains visible on the server")
    }
    expect(maskSoftTokensWithDefaultPolicy("spoiler", false)).toBe("spoiler")
  })

  test("hard-list matching is whole-token and avoids false positives", () => {
    const moderation = makeWorldModeration({
      hardBlockedTokens: ["ass"],
      softMaskedTokens: [],
    })
    const classy = Effect.runSync(moderation.moderateText({
      text: "class assignment was despicable",
      subject,
      state: emptyWorldModerationState,
      observedAt,
    }))
    const isolated = Effect.runSync(moderation.moderateText({
      text: "ass",
      subject,
      state: emptyWorldModerationState,
      observedAt,
    }))

    expect(classy.kind).toBe("allowed")
    expect(isolated.kind).toBe("blocked")
  })

  test("confusable folding catches whole-token hard-list variants", () => {
    const moderation = makeWorldModeration({
      hardBlockedTokens: ["badword"],
      softMaskedTokens: [],
    })
    const folded = tokenizeModerationText("b@dw0rd")
    const decision = Effect.runSync(moderation.moderateText({
      text: "b@dw0rd",
      subject,
      state: emptyWorldModerationState,
      observedAt,
    }))

    expect(folded).toEqual(["badword"])
    expect(decision.kind).toBe("blocked")
  })

  test("strikes escalate warning to timed mute without leaking raw message bodies", () => {
    const moderation = makeWorldModeration({
      hardBlockedTokens: ["badword"],
      softMaskedTokens: [],
    })
    const first = Effect.runSync(moderation.moderateText({
      text: "badword with private body",
      subject,
      state: emptyWorldModerationState,
      observedAt,
    }))
    if (first.kind !== "blocked") throw new Error("expected first strike")
    const second = Effect.runSync(moderation.moderateText({
      text: "badword again with private body",
      subject,
      state: first.state,
      observedAt: "2026-06-22T00:00:05.000Z",
    }))
    if (second.kind !== "blocked") throw new Error("expected second strike")
    const muted = Effect.runSync(moderation.moderateText({
      text: "normal text while muted",
      subject,
      state: second.state,
      observedAt: "2026-06-22T00:00:06.000Z",
    }))

    expect(first.mutedUntil).toBeUndefined()
    expect(second.mutedUntil).toBe("2026-06-22T00:10:05.000Z")
    expect(muted.kind).toBe("blocked")
    if (muted.kind === "blocked") {
      expect(muted.publicMessage).not.toContain("normal text")
      expect(muted.publicMessage).not.toContain("private body")
    }
  })

  test("user-authored diagnostic text is redacted before public diagnostics", () => {
    expect(redactDiagnosticTextWithDefaultPolicy("secret sk-test-123 /Users/example")).toBe(
      "User-authored diagnostic text redacted.",
    )
    expect(redactDiagnosticTextWithDefaultPolicy("ordinary player report")).toBe("ordinary player report")
  })

  test("forum-reflection bubbles have an explicit moderation gate before future deltas", () => {
    const decision = moderateForumReflectionBubbleWithDefaultPolicy({
      text: "forum reflection",
      actorRef: "actor.public.forum",
      sessionRef: "session.forum.1",
      state: emptyWorldModerationState,
      observedAt,
    })

    expect(decision.kind).toBe("allowed")
  })
})
