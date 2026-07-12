import { expect, test } from "bun:test"
import { observeProse, stoppedVoiceModel } from "./lifecycle-model"
test("openagents_voice.command_outcome_receipt_authority.v1: prose is never authority", () => {
  const initial = stoppedVoiceModel()
  for (const prose of ["transcript", "assistant text", "TTS", "done", "confirmed"]) {
    expect(observeProse(initial)).toEqual(initial); expect(prose.length).toBeGreaterThan(0)
  }
})
