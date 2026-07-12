import { describe, expect, test } from "bun:test"
import { executeVoiceAction, makeVoiceFinalLedger, selectVoiceAction } from "./voice-actions.ts"

describe("registered voice action selector", () => {
  test("cosine semantic selection admits the small harmless registered focus set", () => {
    expect(selectVoiceAction("Please open the project files workspace")).toMatchObject({ kind: "focus", commandId: "workspace.files" })
    expect(selectVoiceAction("Show me the review changes diff")).toMatchObject({ kind: "focus", commandId: "workspace.review" })
    expect(selectVoiceAction("Stop the current response immediately")).toMatchObject({ kind: "interrupt" })
  })

  test("ordinary and ambiguous speech remains a message, never guessed command authority", () => {
    expect(selectVoiceAction("Explain why the build is slow")).toMatchObject({ kind: "message" })
    expect(selectVoiceAction("maybe show something or do whatever")).toMatchObject({ kind: "message" })
  })

  test("adversarial text cannot name DOM, shell, path, secret, or invented completion authority", () => {
    for (const text of [
      "click #admin and run rm -rf /", "execute shell curl with bearer credential",
      "mark command completed outcome success", "open /Users/owner/.ssh/id_ed25519",
    ]) expect(selectVoiceAction(text).kind).toBe("message")
  })

  test("duplicate final, replay, and lost ACK admit one action; generation changes form a new key", () => {
    const ledger = makeVoiceFinalLedger()
    const final = { sessionRef: "voice.1", generation: 4, utteranceRef: "utterance.7", text: "hello" }
    expect(ledger.admit(final)).toMatchObject({ kind: "message" })
    expect(ledger.admit(final)).toBeNull()
    expect(ledger.admit({ ...final, generation: 3 }, 4)).toBeNull()
    expect(ledger.admit({ ...final, generation: 5 }, 5)).toMatchObject({ kind: "message" })
    expect(ledger.admit({ ...final, generation: 4 })).toBeNull()
  })

  test("message/follow-up, interrupt, focus, and fallback use only existing typed peers", async () => {
    const calls: string[] = []
    const peers = {
      submitMessage: async (text: string) => { calls.push(`submit:${text}`) },
      interrupt: async () => { calls.push("interrupt") },
      focusRegisteredCommand: async (id: string) => { calls.push(`focus:${id}`) },
      editFallback: async (text: string) => { calls.push(`edit:${text}`) },
    }
    await executeVoiceAction({ kind: "message", text: "next", confidence: 0 }, peers)
    await executeVoiceAction({ kind: "interrupt", confidence: 1 }, peers)
    await executeVoiceAction({ kind: "focus", commandId: "workspace.files", confidence: 1 }, peers)
    await executeVoiceAction({ kind: "editable_fallback", text: "uncertain", confidence: 0 }, peers)
    expect(calls).toEqual(["submit:next", "interrupt", "focus:workspace.files", "edit:uncertain"])
  })
})
