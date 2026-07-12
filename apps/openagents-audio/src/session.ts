import type { VoiceIdentity } from "@openagentsinc/audio-contract"
import { decodeBinaryMediaFrame } from "./media"
import type { SttAdapter, SttEvent, SttStream } from "./stt"

export const MAX_BUFFERED_AUDIO_BYTES = 256 * 1024
export const ROTATE_AFTER_AUDIO_BYTES = 16_000 * 2 * 240
export type AudioSocket = Readonly<{ sendText: (value: unknown) => boolean; close: (code: number, reason: string) => void }>
const sameIdentity = (a: VoiceIdentity, b: VoiceIdentity) => a.ownerRef === b.ownerRef && a.deviceRef === b.deviceRef && a.threadRef === b.threadRef && a.sessionRef === b.sessionRef && a.generation === b.generation

export class AudioSession {
  private stream: SttStream
  private lastClientSequence = 0
  private serverSequence = 0
  private audioBytes = 0
  private finalIndex = 0
  private closed = false
  private queued: Uint8Array[] = []
  constructor(readonly identity: VoiceIdentity, private readonly adapter: SttAdapter, private readonly socket: AudioSocket) { this.stream = this.openStream() }
  private send(frame: Record<string, unknown>): void {
    if (!this.socket.sendText({ schema: "openagents.audio.v1", identity: this.identity, sequence: ++this.serverSequence, ...frame })) this.fail("backpressure")
  }
  private openStream(): SttStream { return this.adapter.open({ onEvent: (e) => this.onSttEvent(e), onDrain: () => this.flush() }) }
  private onSttEvent(event: SttEvent): void {
    if (this.closed) return
    if (event._tag === "interim") this.send({ _tag: "transcript_interim", utteranceRef: `utterance:${this.identity.sessionRef}:${this.finalIndex + 1}`, text: event.text })
    else if (event._tag === "final") this.send({ _tag: "transcript_final", utteranceRef: `utterance:${this.identity.sessionRef}:${++this.finalIndex}`, text: event.text })
    else if (event._tag === "error") this.send({ _tag: "close", reason: `stt_${event.code}` })
    else this.send({ _tag: event._tag })
  }
  receive(raw: Uint8Array): void {
    if (this.closed) throw new Error("audio_session_closed")
    const frame = decodeBinaryMediaFrame(raw)
    if (!sameIdentity(frame.header.identity, this.identity)) return this.fail("identity_or_generation")
    const sequence = frame.header.sequence
    if (sequence <= this.lastClientSequence) { this.send({ _tag: "ack", acknowledgedClientSequence: this.lastClientSequence }); return }
    if (sequence !== this.lastClientSequence + 1) { this.send({ _tag: "gap", expectedClientSequence: this.lastClientSequence + 1 }); return }
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
  stop(): void { if (!this.closed) { this.closed = true; this.stream.close() } }
  private fail(reason: string): never { this.closed = true; this.stream.cancel(); this.socket.close(4008, reason); throw new Error(`audio_session_${reason}`) }
}
