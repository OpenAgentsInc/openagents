import { mintAudioGrant } from "../src/auth"
import { mediaFrame } from "../src/test-support"

const required = (name: string) => { const value = process.env[name]; if (!value) throw new Error(`missing ${name}`); return value }
const pcm = new Uint8Array(await Bun.file(required("OPENAGENTS_AUDIO_SMOKE_PCM")).arrayBuffer())
const identity = { ownerRef: "smoke:owner", deviceRef: "smoke:device", threadRef: "smoke:thread", sessionRef: `smoke:${Date.now()}`, generation: 1 }
const grant = mintAudioGrant({ identity, expiresAtMs: Date.now() + 5 * 60_000 }, required("OPENAGENTS_AUDIO_TOKEN_SECRET"))
const httpBase = required("OPENAGENTS_AUDIO_URL").replace(/\/$/u, "")
const base = httpBase.replace(/^http/, "ws")
const iam = process.env.OPENAGENTS_AUDIO_CLOUD_RUN_ID_TOKEN
const headers = { ...(iam ? { Authorization: `Bearer ${iam}` } : {}), "x-openagents-audio-grant": grant }
const started = Date.now(); let sequence = -1; let finalCount = 0; let gapCount = 0; let ackCount = 0; let sendingDone = false; let finalText = ""
const socket = new WebSocket(`${base}/v1/stream`, { headers } as any)
await new Promise<void>((resolve, reject) => {
  const timeout = setTimeout(() => reject(new Error("live smoke final timeout")), 45_000)
  const settle = () => { if (sendingDone && finalCount === 1 && ackCount === sequence + 1) { clearTimeout(timeout); socket.close(); resolve() } }
  socket.onerror = () => reject(new Error("live smoke websocket error"))
  socket.onopen = async () => {
    for (let offset = 0; offset < pcm.byteLength; offset += 8_000) {
      socket.send(Buffer.from(mediaFrame(++sequence, pcm.slice(offset, offset + 8_000), identity)))
      await Bun.sleep(125)
    }
    for (let n = 0; n < 4; n++) { socket.send(Buffer.from(mediaFrame(++sequence, new Uint8Array(8_000), identity))); await Bun.sleep(125) }
    sendingDone = true; settle()
  }
  socket.onmessage = (message) => {
    const frame = JSON.parse(String(message.data)) as { _tag?: string; text?: string }
    if (frame._tag === "gap") gapCount++
    if (frame._tag === "ack") { ackCount++; settle() }
    if (frame._tag === "transcript_final") {
      finalCount++; finalText = typeof frame.text === "string" ? frame.text : ""
      settle()
    }
  }
})
if (finalCount !== 1 || gapCount !== 0 || ackCount !== sequence + 1) throw new Error(`live smoke receipt invalid: finals=${finalCount}, gaps=${gapCount}, acks=${ackCount}, sent=${sequence + 1}`)
const reconciliationResponse = await fetch(`${httpBase}/v1/retention/reconcile`, { method: "POST", headers: { ...headers, "content-type": "application/json" }, body: "{}" })
if (!reconciliationResponse.ok) throw new Error(`retention reconciliation failed (${reconciliationResponse.status})`)
const reconciliation = await reconciliationResponse.json() as { missingObjects?: unknown[]; orphanObjects?: unknown[]; uncoveredSequences?: unknown[] }
if (reconciliation.missingObjects?.length || reconciliation.orphanObjects?.length || reconciliation.uncoveredSequences?.length) throw new Error(`retention reconciliation was not exact: missing=${reconciliation.missingObjects?.length ?? 0}, orphan=${reconciliation.orphanObjects?.length ?? 0}, uncovered=${reconciliation.uncoveredSequences?.length ?? 0}`)
const retentionOperation = async (operation: "export" | "delete") => {
  const response = await fetch(`${httpBase}/v1/retention/${operation}`, { method: "POST", headers: { ...headers, "content-type": "application/json" }, body: "{}" })
  if (!response.ok) throw new Error(`retention ${operation} failed (${response.status})`)
  return response.json() as Promise<{ objectCount?: number; receipt?: { operation?: string; segmentIds?: unknown[]; remainingLawfulRecords?: unknown[] } }>
}
const exported = await retentionOperation("export")
if (exported.objectCount !== sequence + 1 || exported.receipt?.operation !== "export" || exported.receipt.segmentIds?.length !== sequence + 1) throw new Error("retention export receipt was incomplete")
const deleted = await retentionOperation("delete")
if (deleted.receipt?.operation !== "delete" || deleted.receipt.segmentIds?.length !== sequence + 1 || !deleted.receipt.remainingLawfulRecords?.includes("access_receipt")) throw new Error("retention delete receipt was incomplete")
const expectedAction = process.env.OPENAGENTS_AUDIO_EXPECT_ACTION
let selectedAction: string | undefined
if (expectedAction) {
  const { selectVoiceAction } = await import("../../openagents-desktop/src/renderer/voice-actions")
  selectedAction = selectVoiceAction(finalText).kind
  if (selectedAction !== expectedAction) throw new Error(`live smoke selected ${selectedAction}, expected ${expectedAction}`)
}
console.log(JSON.stringify({ schema: "openagents.audio.stt_smoke.v1", finalCount, gapCount, ackCount, retainedSequenceCount: sequence + 1, reconciliation, exportedObjects: exported.objectCount, deletedSegments: deleted.receipt.segmentIds?.length, remainingLawfulRecords: deleted.receipt.remainingLawfulRecords, audioBytes: pcm.byteLength, latencyMs: Date.now() - started, transcriptLogged: false, ...(selectedAction === undefined ? {} : { selectedAction }) }))
