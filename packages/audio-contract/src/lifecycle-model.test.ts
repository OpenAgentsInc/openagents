import { expect, test } from "vite-plus/test"
import { acceptAudio, acceptRetentionReceipt, acknowledge, connected, mute, observeProse, publishFinal, replayDelivery, start, stop, stoppedVoiceModel } from "./lifecycle-model"
const ids = ["openagents_voice.capture_egress_retention_truth.v1", "openagents_voice.mute_and_stop_fail_closed.v1", "openagents_voice.retention_requires_policy_receipt.v1", "openagents_voice.replay_delivery_only.v1"]
test("lifecycle model falsifiers remain rejected", () => {
  let m = connected(start(stoppedVoiceModel(), 1, "disclosure:1"), 1)
  expect(() => start(m, 2, "disclosure:2")).toThrow()
  expect(() => acceptAudio(m, 2, 1)).toThrow()
  expect(() => acceptAudio(m, 1, 2)).toThrow()
  m = acceptAudio(m, 1, 1)
  expect(() => acknowledge(m, 2)).toThrow(); expect(() => acknowledge({ ...m, ackedClientSeq: 1 }, 0)).toThrow()
  m = publishFinal(m, "utterance:1")
  expect(() => publishFinal(m, "utterance:1")).toThrow()
  expect(replayDelivery(m).sideEffects).toBe(m.sideEffects)
  expect(observeProse(m).sideEffects).toBe(m.sideEffects)
  expect(() => acceptRetentionReceipt(m, 2, "disclosure:1", "receipt:1")).toThrow()
  expect(mute(m)).toMatchObject({ capture: false, egress: false, retention: false })
  const ended = stop(m); expect(() => acceptAudio(ended, 1, 2)).toThrow()
  expect(start(ended, 2, "disclosure:2")).toMatchObject({ capture: false, egress: false, retention: false })
  expect(ids).toHaveLength(4)
})
