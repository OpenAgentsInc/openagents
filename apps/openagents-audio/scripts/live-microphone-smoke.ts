import { mintAudioGrant } from "../src/auth"

const required = (name: string): string => { const value = process.env[name]; if (!value) throw new Error(`missing ${name}`); return value }
const helper = required("OPENAGENTS_AUDIO_HELPER")
const fixture = required("OPENAGENTS_AUDIO_MIC_FIXTURE")
const gateway = required("OPENAGENTS_AUDIO_GATEWAY_URL")
const identity = { ownerRef: "smoke:owner", deviceRef: "smoke:real-microphone", threadRef: "smoke:thread", sessionRef: `mic:${Date.now()}`, generation: 1 }
const grant = mintAudioGrant({ identity, expiresAtMs: Date.now() + 5 * 60_000 }, required("OPENAGENTS_AUDIO_TOKEN_SECRET"))
const child = Bun.spawn([helper], { stdin: "pipe", stdout: "pipe", stderr: "pipe" })
child.stdin.write(JSON.stringify({ command: "start", protocol_version: 1, identity, disclosure_ref: "audio-retention.mvp.v1", gateway_url: gateway, application_grant: grant }) + "\n")

let packetCount = 0; let ackCount = 0; let finalCount = 0; let live = false
const deadline = Date.now() + 30_000
const reader = child.stdout.getReader(); const decoder = new TextDecoder(); let buffered = ""
let playback: ReturnType<typeof Bun.spawn> | undefined
try {
  while (Date.now() < deadline && (finalCount === 0 || ackCount < packetCount)) {
    const remaining = deadline - Date.now()
    const result = await Promise.race([reader.read(), Bun.sleep(remaining).then(() => ({ done: true as const, value: undefined }))])
    if (result.done) break
    buffered += decoder.decode(result.value, { stream: true })
    const lines = buffered.split("\n"); buffered = lines.pop() ?? ""
    for (const line of lines) {
      if (!line.trim()) continue
      const event = JSON.parse(line) as { state?: string; final?: boolean; generation?: number }
      // The helper also emits a lifecycle snapshot whose state is `live`.
      // Only the generation-free transport event proves the remote socket.
      if (event.state === "live" && event.generation === undefined && !live) {
        live = true
        child.stdin.write(JSON.stringify({ command: "set_capture", enabled: true }) + "\n")
        await Bun.sleep(500)
        // The OS input device captures this acoustic playback. No PCM bytes
        // are injected into the helper or WebSocket by this smoke.
        playback = Bun.spawn(["/usr/bin/afplay", fixture], { stdout: "ignore", stderr: "ignore" })
      }
      if (event.state === "packet") packetCount++
      if (event.state === "ack") ackCount++
      if (event.state === "transcript" && event.final === true) { finalCount++; child.stdin.write(JSON.stringify({ command: "set_capture", enabled: false }) + "\n") }
      if (["crashed", "revoked", "offline"].includes(event.state ?? "")) throw new Error(`microphone helper ${event.state}`)
    }
  }
} finally {
  child.stdin.write(JSON.stringify({ command: "stop", reason: "stop" }) + "\n")
  child.stdin.end(); await playback?.exited; await child.exited
}
if (!live || packetCount === 0 || ackCount !== packetCount || finalCount !== 1) throw new Error(`real microphone receipt failed: live=${live}, packets=${packetCount}, acks=${ackCount}, finals=${finalCount}`)
console.log(JSON.stringify({ schema: "openagents.audio.real_microphone_smoke.v1", devicePath: "os_default_input", injectedPcm: false, live, packetCount, ackCount, finalCount, transcriptLogged: false }))
