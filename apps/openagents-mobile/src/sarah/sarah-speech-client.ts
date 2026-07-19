import { Schema as S } from "effect"

export const SARAH_SPEECH_MOBILE_PATH = "/api/mobile/sarah/speech"
export const SARAH_SPEECH_REQUEST_SCHEMA = "openagents.sarah.speech.request.v1"
export const SARAH_SPEECH_MAX_CHARACTERS = 4_096
const SARAH_SPEECH_MAX_AUDIO_BYTES = 16 * 1024 * 1024

const SarahSpeechError = S.Struct({ error: S.String })

export type SarahSpeechClientResult = Readonly<
  | { state: "ready"; audio: Uint8Array<ArrayBuffer> }
  | { state: "unauthorized" | "forbidden" | "too_long" | "unavailable"; message: string }
>

export const fetchSarahSpeech = async (input: Readonly<{
  baseUrl: string
  accessToken: string
  threadRef: string
  messageRef: string
  text: string
  fetch?: typeof globalThis.fetch
}>): Promise<SarahSpeechClientResult> => {
  const text = input.text.trim()
  if (text.length === 0) {
    return { state: "unavailable", message: "Sarah has no completed reply to read yet." }
  }
  if (text.length > SARAH_SPEECH_MAX_CHARACTERS) {
    return {
      state: "too_long",
      message: "This reply is too long to voice in one clip.",
    }
  }
  try {
    const response = await (input.fetch ?? globalThis.fetch)(
      `${input.baseUrl.replace(/\/$/, "")}${SARAH_SPEECH_MOBILE_PATH}`,
      {
        method: "POST",
        headers: {
          accept: "audio/mpeg, application/json",
          authorization: `Bearer ${input.accessToken}`,
          "content-type": "application/json",
        },
        body: JSON.stringify({
          schema: SARAH_SPEECH_REQUEST_SCHEMA,
          threadRef: input.threadRef,
          messageRef: input.messageRef,
          text,
        }),
      },
    )
    if (response.status === 401) {
      return { state: "unauthorized", message: "Sign in again to listen to Sarah." }
    }
    if (response.status === 403) {
      return { state: "forbidden", message: "Sarah voice is private to the owner." }
    }
    if (!response.ok) {
      const decoded = S.decodeUnknownOption(SarahSpeechError)(await response.json().catch(() => null))
      return {
        state: "unavailable",
        message: decoded._tag === "Some" && decoded.value.error === "sarah_speech_unavailable"
          ? "Sarah voice is not configured yet."
          : "Sarah voice is unavailable right now.",
      }
    }
    if (!(response.headers.get("content-type") ?? "").toLowerCase().startsWith("audio/mpeg")) {
      return { state: "unavailable", message: "Sarah voice returned an invalid audio response." }
    }
    const buffer = await response.arrayBuffer()
    if (buffer.byteLength === 0 || buffer.byteLength > SARAH_SPEECH_MAX_AUDIO_BYTES) {
      return { state: "unavailable", message: "Sarah voice returned an invalid audio response." }
    }
    return { state: "ready", audio: new Uint8Array(buffer) }
  } catch {
    return { state: "unavailable", message: "Sarah voice is unavailable right now." }
  }
}
