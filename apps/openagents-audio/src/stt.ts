export type SttEvent =
  | Readonly<{ _tag: "interim"; text: string; stability: number }>
  | Readonly<{ _tag: "final"; text: string }>
  | Readonly<{ _tag: "speech_begin" | "speech_end" }>
  | Readonly<{ _tag: "error"; code: "quota" | "deadline" | "provider" }>
export interface SttStream { write(audio: Uint8Array): boolean; close(): void; cancel(): void }
export interface SttAdapter { open(input: Readonly<{ onEvent: (event: SttEvent) => void; onDrain: () => void }>): SttStream }

export class FakeSttAdapter implements SttAdapter {
  readonly streams: Array<{ events: (event: SttEvent) => void; writes: Uint8Array[]; cancelled: boolean }> = []
  open(input: Readonly<{ onEvent: (event: SttEvent) => void; onDrain: () => void }>): SttStream {
    const state = { events: input.onEvent, writes: [] as Uint8Array[], cancelled: false }; this.streams.push(state)
    return { write: (audio) => { state.writes.push(audio); return true }, close: () => {}, cancel: () => { state.cancelled = true } }
  }
}

export const createGoogleSttAdapter = async (config: Readonly<{ projectId: string; location: string; languageCode: string }>): Promise<SttAdapter> => {
  // Keep the very large generated Google declaration graph out of the repo's
  // normal TypeScript sweep; the runtime module is still a pinned dependency.
  const require = createRequire(import.meta.url)
  const { v2 } = require("@google-cloud/speech") as {
    v2: { SpeechClient: new (input: unknown) => any }
  }
  const client = new v2.SpeechClient({ apiEndpoint: `${config.location}-speech.googleapis.com` })
  return {
    open(input) {
      const stream = client._streamingRecognize()
      stream.on("data", (response: any) => {
        const event = String(response.speechEventType ?? "")
        if (event.includes("BEGIN")) input.onEvent({ _tag: "speech_begin" })
        if (event.includes("END")) input.onEvent({ _tag: "speech_end" })
        for (const result of response.results ?? []) {
          const text = result.alternatives?.[0]?.transcript ?? ""
          if (text === "") continue
          input.onEvent(result.isFinal ? { _tag: "final", text } : { _tag: "interim", text, stability: result.stability ?? 0 })
        }
      })
      stream.on("drain", input.onDrain)
      stream.on("error", (error: { code?: number }) => input.onEvent({ _tag: "error", code: error.code === 8 ? "quota" : error.code === 4 ? "deadline" : "provider" }))
      stream.write({ recognizer: `projects/${config.projectId}/locations/${config.location}/recognizers/_`, streamingConfig: { config: { explicitDecodingConfig: { encoding: "LINEAR16", sampleRateHertz: 16000, audioChannelCount: 1 }, languageCodes: [config.languageCode], model: "chirp_3", features: { enableAutomaticPunctuation: true } }, streamingFeatures: { interimResults: true, enableVoiceActivityEvents: true, endpointingSensitivity: "ENDPOINTING_SENSITIVITY_SHORT" } } })
      return { write: (audio) => stream.write({ audio: Buffer.from(audio) }), close: () => stream.end(), cancel: () => stream.destroy() }
    },
  }
}
import { createRequire } from "node:module"
