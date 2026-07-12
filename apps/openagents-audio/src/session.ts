import type { VoiceIdentity } from "@openagentsinc/audio-contract"
import { createHash } from "node:crypto"
import { decodeBinaryMediaFrame } from "./media"
import type { SttAdapter, SttEvent, SttStream } from "./stt"
import { encodeServerTtsMediaFrame } from "./media"
import { DEFAULT_CHIRP_VOICE, normalizeSpoken, trimLeadingSilencePcm, type TtsAdapter, type TtsChunkStream, type TtsReceipt } from "./tts"

export const MAX_BUFFERED_AUDIO_BYTES = 256 * 1024
export const ROTATE_AFTER_AUDIO_BYTES = 16_000 * 2 * 240
export type AudioSocket = Readonly<{ sendText: (value: unknown) => boolean; sendBinary?: (value: Uint8Array) => boolean; close: (code: number, reason: string) => void }>
export type AcceptedAudioFrame = Readonly<{
  sequence: number
  payload: Uint8Array
  codec: "pcm_s16le" | "opus"
  sampleRateHz: 16_000 | 24_000 | 48_000
  sha256: string
}>
export type AudioRetention = Readonly<{
  accept: (frame: AcceptedAudioFrame) => Promise<void>
  gap: (firstSequence: number, lastSequence: number) => Promise<void>
}>
const sameIdentity = (a: VoiceIdentity, b: VoiceIdentity) => a.ownerRef === b.ownerRef && a.deviceRef === b.deviceRef && a.threadRef === b.threadRef && a.sessionRef === b.sessionRef && a.generation === b.generation

export class AudioSession {
  private stream: SttStream
  // AUDIO-1 media generations are zero-based (the Rust golden corpus and
  // helper both emit sequence 0 first). -1 is internal-only and is never put
  // on the wire.
  private lastClientSequence = -1
  private serverSequence = 0
  private audioBytes = 0
  private finalIndex = 0
  private closed = false
  private queued: Uint8Array[] = []
  private activeSpeech: Readonly<{ turnRef: string; speechRef: string; stream: TtsChunkStream; startedAt: number }> | null = null
  private speechEpoch = 0
  private speechBeganAt: number | null = null
  constructor(readonly identity: VoiceIdentity, private readonly adapter: SttAdapter, private readonly socket: AudioSocket, private readonly tts?: Readonly<{
    adapter: TtsAdapter; voiceRef?: string; languageCode?: string; now?: () => number
    receipt?: (receipt: TtsReceipt) => void
    onBargeIn?: (input: Readonly<{ identity: VoiceIdentity; turnRef: string; speechRef: string }>) => Promise<string>
  }>, private readonly retention?: AudioRetention) { this.stream = this.openStream() }
  private send(frame: Record<string, unknown>): void {
    if (!this.socket.sendText({ schema: "openagents.audio.v1", identity: this.identity, sequence: ++this.serverSequence, ...frame })) this.fail("backpressure")
  }
  announceRetention(receipt: Record<string, unknown>): void { this.send({ _tag: "retention_receipt", receipt }) }
  private openStream(): SttStream { return this.adapter.open({ onEvent: (e) => this.onSttEvent(e), onDrain: () => this.flush() }) }
  private onSttEvent(event: SttEvent): void {
    if (this.closed) return
    if (event._tag === "interim") {
      this.send({ _tag: "transcript_interim", utteranceRef: `utterance:${this.identity.sessionRef}:${this.finalIndex + 1}`, text: event.text })
      if (this.speechBeganAt !== null && event.text.trim().length >= 3 && (this.tts?.now?.() ?? Date.now()) - this.speechBeganAt <= 5_000) void this.bargeIn()
    }
    else if (event._tag === "final") { if (this.speechBeganAt !== null && event.text.trim().length >= 3) void this.bargeIn(); this.speechBeganAt = null; this.send({ _tag: "transcript_final", utteranceRef: `utterance:${this.identity.sessionRef}:${++this.finalIndex}`, text: event.text }) }
    else if (event._tag === "error") this.send({ _tag: "close", reason: `stt_${event.code}` })
    else { if (event._tag === "speech_begin") this.speechBeganAt = this.tts?.now?.() ?? Date.now(); this.send({ _tag: event._tag }) }
  }
  private async bargeIn(): Promise<void> {
    const active = this.activeSpeech
    if (active === null) return
    this.speechBeganAt = null; this.speechEpoch++; active.stream.cancel(); this.activeSpeech = null
    const outcomeRef = await this.tts?.onBargeIn?.({ identity: this.identity, turnRef: active.turnRef, speechRef: active.speechRef }) ?? `outcome:barge:${active.speechRef}`
    this.send({ _tag: "playback_cancel", turnRef: active.turnRef, speechRef: active.speechRef, outcomeRef, reason: "qualified_barge_in" })
    this.send({ _tag: "tts_state", turnRef: active.turnRef, speechRef: active.speechRef, state: "canceled" })
  }
  async speak(input: Readonly<{ turnRef: string; speechRef: string; messageRef: string; text: string }>): Promise<TtsReceipt> {
    const now = this.tts?.now ?? Date.now
    const startedAt = now(); const normalized = normalizeSpoken(input.text)
    this.send({ _tag: "assistant_text", messageRef: input.messageRef, turnRef: input.turnRef, speechRef: input.speechRef, text: input.text })
    if (this.tts === undefined || normalized === "" || this.socket.sendBinary === undefined) {
      const receipt = { schema: "openagents.audio.tts_receipt.v1" as const, adapterRef: this.tts?.adapter.adapterRef ?? "unavailable", voiceRef: this.tts?.voiceRef ?? DEFAULT_CHIRP_VOICE, charsIn: input.text.length, synthTtfbMs: null, totalMs: now() - startedAt, chunksOut: 0, bytesOut: 0, outcome: normalized === "" ? "empty" as const : "provider_error" as const }
      this.tts?.receipt?.(receipt); this.send({ _tag: "tts_state", turnRef: input.turnRef, speechRef: input.speechRef, state: receipt.outcome }); return receipt
    }
    this.activeSpeech?.stream.cancel(); const epoch = ++this.speechEpoch
    const stream = this.tts.adapter.synthesize({ text: normalized, voiceRef: this.tts.voiceRef ?? DEFAULT_CHIRP_VOICE, languageCode: this.tts.languageCode ?? "en-US" })
    this.activeSpeech = { turnRef: input.turnRef, speechRef: input.speechRef, stream, startedAt }
    this.send({ _tag: "tts_state", turnRef: input.turnRef, speechRef: input.speechRef, state: "started" })
    let firstAt: number | null = null; let chunksOut = 0; let bytesOut = 0; let first = true; let outcome: TtsReceipt["outcome"] = "completed"
    try {
      for await (const raw of stream) {
        if (epoch !== this.speechEpoch || this.activeSpeech?.speechRef !== input.speechRef) { outcome = "canceled"; break }
        let chunk = first ? trimLeadingSilencePcm(raw) : raw; first = false
        for (let offset = 0; offset < chunk.byteLength; offset += 24_000) {
          const payload = chunk.subarray(offset, Math.min(offset + 24_000, chunk.byteLength)); if (payload.byteLength === 0) continue
          if (firstAt === null) firstAt = now()
          this.send({ _tag: "tts_chunk", turnRef: input.turnRef, speechRef: input.speechRef, payloadLength: payload.byteLength, sha256: createHash("sha256").update(payload).digest("hex") })
          const sequence = ++this.serverSequence
          if (!this.socket.sendBinary(encodeServerTtsMediaFrame({ identity: this.identity, sequence, turnRef: input.turnRef, speechRef: input.speechRef, payload }))) throw new Error("backpressure")
          chunksOut++; bytesOut += payload.byteLength
        }
      }
      if (epoch !== this.speechEpoch) outcome = "canceled"
    } catch { outcome = epoch === this.speechEpoch ? "provider_error" : "canceled" }
    if (epoch === this.speechEpoch && outcome !== "completed") this.activeSpeech = null
    if (epoch === this.speechEpoch && outcome === "completed" && bytesOut > 0) {
      const audibleWindowMs = Math.ceil(bytesOut / (24_000 * 2) * 1_000) + 1_000
      setTimeout(() => { if (this.activeSpeech?.speechRef === input.speechRef) this.activeSpeech = null }, audibleWindowMs)
    }
    const receipt = { schema: "openagents.audio.tts_receipt.v1" as const, adapterRef: this.tts.adapter.adapterRef, voiceRef: this.tts.voiceRef ?? DEFAULT_CHIRP_VOICE, charsIn: input.text.length, synthTtfbMs: firstAt === null ? null : firstAt - startedAt, totalMs: now() - startedAt, chunksOut, bytesOut, outcome }
    this.tts.receipt?.(receipt); this.send({ _tag: "tts_state", turnRef: input.turnRef, speechRef: input.speechRef, state: outcome }); return receipt
  }
  async receive(raw: Uint8Array): Promise<void> {
    if (this.closed) throw new Error("audio_session_closed")
    const frame = decodeBinaryMediaFrame(raw)
    if (!sameIdentity(frame.header.identity, this.identity)) return this.fail("identity_or_generation")
    const sequence = frame.header.sequence
    if (sequence <= this.lastClientSequence) { this.send({ _tag: "ack", acknowledgedClientSequence: this.lastClientSequence }); return }
    if (sequence !== this.lastClientSequence + 1) {
      await this.retention?.gap(this.lastClientSequence + 1, sequence - 1)
      this.send({ _tag: "gap", expectedClientSequence: this.lastClientSequence + 1 }); return
    }
    // The ACK is a durable-acceptance receipt, not merely a socket-read
    // receipt. Retention must commit the encrypted object and SQL manifest
    // before STT or the client watermark advances.
    await this.retention?.accept({ sequence, payload: frame.payload, codec: frame.header.codec, sampleRateHz: frame.header.sampleRateHz, sha256: frame.header.sha256 })
    this.lastClientSequence = sequence
    if (this.audioBytes + frame.payload.byteLength >= ROTATE_AFTER_AUDIO_BYTES) {
      this.stream.close(); this.stream = this.openStream(); this.audioBytes = 0
    }
    this.audioBytes += frame.payload.byteLength
    if (!this.stream.write(frame.payload)) {
      this.queued.push(frame.payload)
      if (this.queued.reduce((n, p) => n + p.byteLength, 0) > MAX_BUFFERED_AUDIO_BYTES) return this.fail("backpressure")
    }
    this.send({ _tag: "ack", acknowledgedClientSequence: sequence })
  }
  private flush(): void { while (this.queued.length > 0 && this.stream.write(this.queued[0]!)) this.queued.shift() }
  revoke(): void { this.fail("revoked") }
  stop(): void { if (!this.closed) { this.closed = true; this.activeSpeech?.stream.cancel(); this.activeSpeech = null; this.stream.close() } }
  private fail(reason: string): never { this.closed = true; this.stream.cancel(); this.socket.close(4008, reason); throw new Error(`audio_session_${reason}`) }
}
