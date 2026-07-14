import { Runtime } from "@openagentsinc/runtime-platform"
import { mintAudioGrant } from "../src/auth"

const required = (name: string): string => { const value = process.env[name]; if (!value) throw new Error(`missing ${name}`); return value }
const helper = required("OPENAGENTS_AUDIO_HELPER")
const gateway = required("OPENAGENTS_AUDIO_GATEWAY_URL")
const durationMs = Number(process.env.OPENAGENTS_AUDIO_LONG_DURATION_MS ?? 60_000)
if (!Number.isSafeInteger(durationMs) || durationMs < 10_000 || durationMs > 3_600_000) throw new Error("invalid OPENAGENTS_AUDIO_LONG_DURATION_MS")
const identity = { ownerRef: "smoke:owner", deviceRef: "smoke:long-microphone", threadRef: "smoke:thread", sessionRef: `long:${Date.now()}`, generation: 1 }
const grant = mintAudioGrant({ identity, expiresAtMs: Date.now() + 10 * 60_000 }, required("OPENAGENTS_AUDIO_TOKEN_SECRET"))
const child = Runtime.spawn([helper], { stdin: "pipe", stdout: "pipe", stderr: "pipe" })
child.stdin.write(JSON.stringify({ command: "start", protocol_version: 1, identity, disclosure_ref: "audio-retention.mvp.v1", gateway_url: gateway, application_grant: grant }) + "\n")

let packets = 0; let acks = 0; let live = false; let failure: string | undefined
const connected = Promise.withResolvers<void>()
const readEvents = async () => {
  const reader = child.stdout.getReader(); const decoder = new TextDecoder(); let buffered = ""
  while (true) {
    const result = await reader.read(); if (result.done) break
    buffered += decoder.decode(result.value, { stream: true }); const lines = buffered.split("\n"); buffered = lines.pop() ?? ""
    for (const line of lines) {
      if (!line.trim()) continue
      const event = JSON.parse(line) as { state?: string; generation?: number }
      if (event.state === "live" && event.generation === undefined && !live) { live = true; connected.resolve() }
      if (event.state === "packet") packets++
      if (event.state === "ack") acks++
      if (["crashed", "revoked", "offline", "backpressured"].includes(event.state ?? "")) failure = event.state
    }
  }
}
const reading = readEvents()
const timeout = Runtime.sleep(15_000).then(() => { throw new Error("long microphone connect timeout") })
await Promise.race([connected.promise, timeout])
const firstLegMs = Math.floor(durationMs / 3)
const secondLegMs = durationMs - firstLegMs - 4_000
await Runtime.sleep(firstLegMs)
child.stdin.write(JSON.stringify({ command: "set_capture", enabled: false }) + "\n")
await Runtime.sleep(1_000); const mutedAt = packets; await Runtime.sleep(3_000)
if (packets !== mutedAt) throw new Error("mute did not stop microphone egress")
child.stdin.write(JSON.stringify({ command: "set_capture", enabled: true }) + "\n")
await Runtime.sleep(secondLegMs)
child.stdin.write(JSON.stringify({ command: "set_capture", enabled: false }) + "\n")
const settleBy = Date.now() + 15_000
while (acks < packets && Date.now() < settleBy) await Runtime.sleep(100)
child.stdin.write(JSON.stringify({ command: "stop", reason: "stop" }) + "\n"); child.stdin.end(); await child.exited; await reading
if (!live || failure || packets < Math.floor(durationMs / 200) || acks !== packets) throw new Error(`long microphone run failed: live=${live}, failure=${failure ?? "none"}, packets=${packets}, acks=${acks}`)

const httpBase = gateway.replace(/^wss/u, "https").replace(/\/v1\/stream$/u, "")
const headers = { "content-type": "application/json", "x-openagents-audio-grant": grant }
const invoke = async (operation: string) => {
  const response = await fetch(`${httpBase}/v1/retention/${operation}`, { method: "POST", headers, body: "{}" })
  if (!response.ok) throw new Error(`long retention ${operation} failed (${response.status})`)
  return response.json() as Promise<any>
}
const reconciliation = await invoke("reconcile")
if (reconciliation.missingObjects.length || reconciliation.orphanObjects.length || reconciliation.uncoveredSequences.length) throw new Error("long retention reconciliation failed")
const exported = await invoke("export"); const deleted = await invoke("delete")
if (exported.objectCount !== packets || deleted.receipt?.segmentIds?.length !== packets) throw new Error(`long retention disposition failed: packets=${packets}, exported=${exported.objectCount ?? -1}, deleted=${deleted.receipt?.segmentIds?.length ?? -1}`)
console.log(JSON.stringify({ schema: "openagents.audio.long_fault_smoke.v1", durationSeconds: durationMs / 1_000, realMicrophone: true, muteStoppedEgress: true, packets, acks, reconciliation, exportedObjects: exported.objectCount, deletedSegments: deleted.receipt.segmentIds.length, transcriptLogged: false }))
