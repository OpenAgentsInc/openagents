import { describe, expect, test } from "vite-plus/test"

import {
  SARAH_SPEECH_MOBILE_PATH,
  SARAH_SPEECH_REQUEST_SCHEMA,
  fetchSarahSpeech,
} from "../src/sarah/sarah-speech-client"

const input = {
  baseUrl: "https://openagents.com/",
  accessToken: "owner-token",
  threadRef: "thread.sarah.owner",
  messageRef: "event.sarah.reply",
  text: "Hello from Sarah.",
} as const

describe("Sarah speech mobile client", () => {
  test("posts the exact owner message and accepts MP3", async () => {
    let url = ""
    let authorization = ""
    let body: Record<string, unknown> = {}
    const result = await fetchSarahSpeech({
      ...input,
      fetch: async (requestedUrl, init) => {
        url = String(requestedUrl)
        authorization = new Headers(init?.headers).get("authorization") ?? ""
        body = JSON.parse(String(init?.body)) as Record<string, unknown>
        return new Response(new Uint8Array([73, 68, 51]), {
          headers: { "content-type": "audio/mpeg" },
        })
      },
    })

    expect(url).toBe(`https://openagents.com${SARAH_SPEECH_MOBILE_PATH}`)
    expect(authorization).toBe("Bearer owner-token")
    expect(body).toEqual({
      schema: SARAH_SPEECH_REQUEST_SCHEMA,
      threadRef: input.threadRef,
      messageRef: input.messageRef,
      text: input.text,
    })
    expect(result.state).toBe("ready")
  })

  test("keeps auth and provider failures typed and non-throwing", async () => {
    expect((await fetchSarahSpeech({
      ...input,
      fetch: async () => new Response(null, { status: 401 }),
    })).state).toBe("unauthorized")
    expect((await fetchSarahSpeech({
      ...input,
      fetch: async () => new Response(null, { status: 403 }),
    })).state).toBe("forbidden")
    expect((await fetchSarahSpeech({
      ...input,
      fetch: async () => new Response("not audio"),
    })).state).toBe("unavailable")
  })
})
