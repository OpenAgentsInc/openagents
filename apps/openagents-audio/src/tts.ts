import { createRequire } from "node:module"

export const TTS_SAMPLE_RATE_HZ = 24_000
export const TTS_CHANNELS = 1
export const TTS_SAMPLE_WIDTH_BYTES = 2
export const DEFAULT_CHIRP_VOICE = "en-US-Chirp3-HD-Sulafat"

export type TtsChunkStream = AsyncIterable<Uint8Array> & Readonly<{ cancel: () => void }>
export interface TtsAdapter {
  readonly adapterRef: string
  synthesize(input: Readonly<{ text: string; voiceRef: string; languageCode: string }>): TtsChunkStream
}

const spokenLexicon = new Map([
  ["AI", "A.I."], ["API", "A.P.I."], ["CRM", "C.R.M."], ["URL", "U.R.L."],
  ["TTS", "T.T.S."], ["QA", "Q.A."], ["MVP", "M.V.P."], ["openagents.com", "open agents dot com"],
])
export const normalizeSpoken = (input: string): string => {
  let text = input.trim().slice(0, 16_384)
  for (const [written, spoken] of spokenLexicon) {
    const escaped = written.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
    text = text.replace(new RegExp(`\\b${escaped}\\b`, written === "openagents.com" ? "giu" : "gu"), spoken)
  }
  return text.replace(/(?<!\.)\.\.(?!\.)/gu, ".")
}

export const trimLeadingSilencePcm = (chunk: Uint8Array, threshold = 24): Uint8Array => {
  let offset = 0
  while (offset + 1 < chunk.byteLength) {
    const sample = Math.abs(new DataView(chunk.buffer, chunk.byteOffset + offset, 2).getInt16(0, true))
    if (sample > threshold) break
    offset += 2
  }
  return chunk.subarray(offset)
}

export type TtsReceipt = Readonly<{
  schema: "openagents.audio.tts_receipt.v1"
  adapterRef: string; voiceRef: string; charsIn: number
  synthTtfbMs: number | null; totalMs: number; chunksOut: number; bytesOut: number
  outcome: "completed" | "canceled" | "provider_error" | "empty"
}>

export class FakeTtsAdapter implements TtsAdapter {
  readonly adapterRef = "fake-tts"
  readonly requests: Array<{ text: string; voiceRef: string; languageCode: string; canceled: boolean }> = []
  constructor(private readonly chunks: ReadonlyArray<Uint8Array> = []) {}
  synthesize(input: Readonly<{ text: string; voiceRef: string; languageCode: string }>): TtsChunkStream {
    const request = { ...input, canceled: false }; this.requests.push(request)
    const chunks = this.chunks
    return Object.assign((async function* () { for (const chunk of chunks) { if (request.canceled) return; yield chunk } })(), { cancel: () => { request.canceled = true } })
  }
}

export const createGoogleChirpTtsAdapter = (): TtsAdapter => {
  const require = createRequire(import.meta.url)
  const { TextToSpeechClient } = require("@google-cloud/text-to-speech") as { TextToSpeechClient: new () => any }
  const client = new TextToSpeechClient()
  return {
    adapterRef: "google-chirp3-hd-streaming",
    synthesize(input) {
      const stream = client.streamingSynthesize()
      stream.write({ streamingConfig: { voice: { name: input.voiceRef, languageCode: input.languageCode }, streamingAudioConfig: { audioEncoding: "PCM", sampleRateHertz: TTS_SAMPLE_RATE_HZ } } })
      stream.write({ input: { text: normalizeSpoken(input.text) } })
      stream.end()
      const iterable = (async function* () {
        for await (const response of stream) {
          const bytes = response.audioContent
          if (bytes?.length) yield new Uint8Array(bytes)
        }
      })()
      return Object.assign(iterable, { cancel: () => stream.destroy() })
    },
  }
}
