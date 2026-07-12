import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import path from "node:path"

const app = path.resolve(import.meta.dir, "..")
const read = (file: string) => readFileSync(path.join(app, file), "utf8")
const code = (source: string) => source.replace(/\/\*[\s\S]*?\*\//gu, "").replace(/^\s*\/\/.*$/gmu, "")
describe("voice media authority boundary", () => {
  test("preload and renderer never receive raw media, sockets, credentials, paths, or generic ports", () => {
    const sources = [read("src/preload.cts"), ...["boot.ts", "runtime-conversation.ts", "shell.ts"].map(file => read(`src/renderer/${file}`))].map(code)
    for (const source of sources) for (const forbidden of ["getUserMedia", "AudioContext", "WebSocket", "MessagePort", "voiceCredential", "audioObjectPath", "rawAudioFrame"]) expect(source).not.toContain(forbidden)
  })
  test("gateway voice contract is lifecycle-only", () => {
    const source = read("src/runtime-gateway-contract.ts")
    for (const allowed of ["voice.start", "voice.stop", "voice.mute", "voice.unmute", "voice.revoke", "voice.state"]) expect(source).toContain(allowed)
    for (const forbidden of ["audioBytes", "pcmPayload", "socketHandle", "objectPath", "providerFrame"]) expect(source).not.toContain(forbidden)
  })
})
