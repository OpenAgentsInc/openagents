import { describe, expect, test } from "bun:test"

import {
  describePushToTalkFailure,
  isPushToTalkPressable,
  mergeTranscriptIntoDraft,
  phaseFromAvailability,
  pushToTalkAccessibilityLabel
} from "../src/native/push-to-talk-core"

describe("phaseFromAvailability", () => {
  test("available maps to idle (pressable)", () => {
    expect(phaseFromAvailability({ status: "available" })).toBe("idle")
  })

  test("denied maps to denied (not pressable)", () => {
    expect(phaseFromAvailability({ reason: "denied", status: "denied" })).toBe("denied")
  })

  test("unavailable maps to unavailable (not pressable)", () => {
    expect(phaseFromAvailability({ reason: "no_recognizer", status: "unavailable" })).toBe("unavailable")
  })
})

describe("isPushToTalkPressable", () => {
  test("idle and recording are pressable", () => {
    expect(isPushToTalkPressable("idle")).toBe(true)
    expect(isPushToTalkPressable("recording")).toBe(true)
  })

  test("checking, denied, unavailable, error are not pressable", () => {
    expect(isPushToTalkPressable("checking")).toBe(false)
    expect(isPushToTalkPressable("denied")).toBe(false)
    expect(isPushToTalkPressable("unavailable")).toBe(false)
    expect(isPushToTalkPressable("error")).toBe(false)
  })
})

describe("pushToTalkAccessibilityLabel", () => {
  test("has a distinct label per phase", () => {
    const phases = ["idle", "checking", "recording", "denied", "unavailable", "error"] as const
    const labels = phases.map(pushToTalkAccessibilityLabel)
    expect(new Set(labels).size).toBe(phases.length)
  })
})

describe("describePushToTalkFailure", () => {
  test("wraps an Error message in a user-facing sentence", () => {
    const message = describePushToTalkFailure(
      new Error(
        "The TS-8 module shell is linked, but streaming SFSpeechRecognizer capture still needs the owner device proof pass."
      )
    )
    expect(message).toContain("Dictation is not available on this device yet")
    expect(message).toContain("streaming SFSpeechRecognizer capture")
  })

  test("falls back to a generic sentence for a blank error", () => {
    expect(describePushToTalkFailure(new Error(""))).toBe("Dictation is not available on this device yet.")
  })

  test("handles a non-Error thrown value", () => {
    expect(describePushToTalkFailure("android_stt_runtime_pending")).toContain("android_stt_runtime_pending")
  })
})

describe("mergeTranscriptIntoDraft", () => {
  test("uses the transcript verbatim when the draft is empty", () => {
    expect(mergeTranscriptIntoDraft("", "hello world")).toBe("hello world")
  })

  test("appends with a separating space when the draft has content", () => {
    expect(mergeTranscriptIntoDraft("check the deploy", "then ping me")).toBe("check the deploy then ping me")
  })

  test("does not double a trailing space already on the draft", () => {
    expect(mergeTranscriptIntoDraft("check the deploy ", "then ping me")).toBe("check the deploy then ping me")
  })

  test("does not double a trailing newline already on the draft", () => {
    expect(mergeTranscriptIntoDraft("check the deploy\n", "then ping me")).toBe("check the deploy\nthen ping me")
  })

  test("is a no-op for a blank transcript", () => {
    expect(mergeTranscriptIntoDraft("check the deploy", "   ")).toBe("check the deploy")
  })

  test("trims the transcript's own surrounding whitespace", () => {
    expect(mergeTranscriptIntoDraft("", "  hello  ")).toBe("hello")
  })
})
