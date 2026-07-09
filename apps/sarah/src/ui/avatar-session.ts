/**
 * LiveAvatar session wrapper for the Sarah Effect Native surface (#8598).
 *
 * Owns the imperative SDK lifecycle (mint → connect → attach video → voice
 * chat) and the SSE subscription to the brain's event bus, translating both
 * into the typed callbacks the EN surface consumes. The avatar <video> mounts
 * into a dedicated sibling container (`#sarah-avatar`) — the EN Host kind set
 * is closed (code-editor|terminal|canvas) and a `media-video` host kind is
 * registered as upstream demand in docs/sarah/EN-GAPS.md.
 */

import {
  AgentEventsEnum,
  LiveAvatarSession,
  SessionEvent,
} from "@heygen/liveavatar-web-sdk"

export type AvatarCallbacks = {
  onState: (state: "connecting" | "live" | "ended" | "error") => void
  onTranscript: (role: "user" | "assistant", text: string) => void
  onCard: (card: { title: string; body: string; href?: string }) => void
}

export type AvatarHandle = {
  stop: () => Promise<void>
  /** Send a typed message through the avatar loop — Sarah speaks the reply. */
  message: (text: string) => void
  conversationRef: string
  sandbox: boolean
}

const API = "/sarah/api"

type AvatarMint = {
  /** OAV-4 (#8614) renderer seam; absent/liveavatar → HeyGen SDK path. */
  renderer?: "liveavatar" | "owned"
  sessionToken?: string
  sessionId: string
  conversationRef: string
  sandbox: boolean
  /** Owned renderer only: render-service WebRTC join info. */
  webrtc?: { offer_url?: string }
}

export async function startAvatarSession(
  container: HTMLElement,
  callbacks: AvatarCallbacks,
): Promise<AvatarHandle> {
  callbacks.onState("connecting")
  const mintResponse = await fetch(`${API}/avatar/session`, { method: "POST" })
  if (!mintResponse.ok) {
    const body = (await mintResponse.json().catch(() => ({}))) as {
      error?: { code?: string }
    }
    callbacks.onState("error")
    throw new Error(body.error?.code ?? `avatar_mint_${mintResponse.status}`)
  }
  const mint = (await mintResponse.json()) as AvatarMint

  if (mint.renderer === "owned") {
    return startOwnedRendererSession(container, callbacks, mint)
  }

  const session = new LiveAvatarSession(mint.sessionToken ?? "")

  // One dedupe across BOTH transcript sources (SDK data-channel events and
  // the brain's SSE bus) — the 2026-07-09 live test showed duplicated user
  // lines when both fired.
  const seenTranscripts = new Set<string>()
  const emitTranscript = (role: "user" | "assistant", text: string) => {
    const key = `${role}:${text.trim()}`
    if (!text.trim() || seenTranscripts.has(key)) return
    seenTranscripts.add(key)
    callbacks.onTranscript(role, text.trim())
  }

  const video = document.createElement("video")
  video.autoplay = true
  video.playsInline = true
  video.muted = false
  video.style.width = "100%"
  video.style.height = "100%"
  video.style.objectFit = "cover"
  container.replaceChildren(video)
  container.dataset.state = "connecting"

  session.on(SessionEvent.SESSION_STREAM_READY, () => {
    session.attach(video)
    container.dataset.state = "live"
    callbacks.onState("live")
    void Promise.resolve(session.voiceChat.start()).catch(() => {
      // Mic denied — the avatar still speaks; text input remains available.
    })
  })
  session.on(SessionEvent.SESSION_DISCONNECTED, () => {
    callbacks.onState("ended")
  })
  // Native data-channel transcriptions are the primary transcript source;
  // the SSE bus below adds brain-side cards and covers pre-SDK-event gaps
  // (both paths dedupe in the surface).
  session.on(AgentEventsEnum.USER_TRANSCRIPTION, (event) => {
    if (event.text) emitTranscript("user", event.text)
  })
  session.on(AgentEventsEnum.AVATAR_TRANSCRIPTION, (event) => {
    if (event.text) emitTranscript("assistant", event.text)
  })

  await session.start()

  const events = new EventSource(
    `${API}/avatar/events?ref=${encodeURIComponent(mint.conversationRef)}`,
  )
  events.onmessage = (message) => {
    try {
      const event = JSON.parse(message.data) as {
        type: string
        role?: "user" | "assistant"
        text?: string
        title?: string
        body?: string
        href?: string
      }
      if (event.type === "transcript" && event.role && event.text) {
        emitTranscript(event.role, event.text)
      } else if ((event.type === "card" || event.type === "guard_refusal") && event.title) {
        callbacks.onCard({
          title: event.title,
          body: event.body ?? "",
          ...(event.href ? { href: event.href } : {}),
        })
      }
    } catch {
      // Ignore malformed frames.
    }
  }

  const stop = async () => {
    events.close()
    try {
      await session.stop()
    } catch {
      // Server-side stop below is the authority.
    }
    container.replaceChildren()
    container.dataset.state = "idle"
    await fetch(`${API}/avatar/stop`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ sessionId: mint.sessionId }),
    }).catch(() => {})
    callbacks.onState("ended")
  }

  window.addEventListener("beforeunload", () => {
    navigator.sendBeacon?.(
      `${API}/avatar/stop`,
      new Blob([JSON.stringify({ sessionId: mint.sessionId })], {
        type: "application/json",
      }),
    )
  })

  return {
    stop,
    message: (text: string) => {
      session.message(text)
    },
    conversationRef: mint.conversationRef,
    sandbox: mint.sandbox,
  }
}

/**
 * OAV-4 (#8614): owned render-service session — a plain RTCPeerConnection
 * wrapper (no vendor SDK). The browser attaches recvonly audio/video via the
 * WHEP-style `webrtc.offer_url` from the mint; transcripts and cards arrive on
 * the same SSE bus, fed by the server-side owned turn loop; typed messages go
 * to the speak bridge. Honest v1 scope: text-driven speech only — the user-mic
 * path stays LiveAvatar-only until a later lane adds owned ASR.
 */
async function startOwnedRendererSession(
  container: HTMLElement,
  callbacks: AvatarCallbacks,
  mint: AvatarMint,
): Promise<AvatarHandle> {
  const offerUrl = mint.webrtc?.offer_url
  if (!offerUrl) {
    callbacks.onState("error")
    throw new Error("avatar_owned_missing_offer_url")
  }

  const seenTranscripts = new Set<string>()
  const emitTranscript = (role: "user" | "assistant", text: string) => {
    const key = `${role}:${text.trim()}`
    if (!text.trim() || seenTranscripts.has(key)) return
    seenTranscripts.add(key)
    callbacks.onTranscript(role, text.trim())
  }

  const video = document.createElement("video")
  video.autoplay = true
  video.playsInline = true
  video.muted = false
  video.style.width = "100%"
  video.style.height = "100%"
  video.style.objectFit = "cover"
  container.replaceChildren(video)
  container.dataset.state = "connecting"

  const pc = new RTCPeerConnection()
  pc.addTransceiver("video", { direction: "recvonly" })
  pc.addTransceiver("audio", { direction: "recvonly" })
  pc.ontrack = (event) => {
    const stream = event.streams[0]
    if (stream && video.srcObject !== stream) {
      video.srcObject = stream
      container.dataset.state = "live"
      callbacks.onState("live")
    }
  }
  pc.onconnectionstatechange = () => {
    if (pc.connectionState === "failed" || pc.connectionState === "closed") {
      callbacks.onState("ended")
    }
  }

  try {
    const offer = await pc.createOffer()
    await pc.setLocalDescription(offer)
    const answerResponse = await fetch(offerUrl, {
      method: "POST",
      headers: { "content-type": "application/sdp" },
      body: offer.sdp ?? "",
    })
    if (!answerResponse.ok) {
      throw new Error(`avatar_owned_offer_${answerResponse.status}`)
    }
    await pc.setRemoteDescription({
      type: "answer",
      sdp: await answerResponse.text(),
    })
  } catch (error) {
    pc.close()
    container.replaceChildren()
    container.dataset.state = "idle"
    callbacks.onState("error")
    throw error instanceof Error ? error : new Error("avatar_owned_webrtc_failed")
  }

  const events = new EventSource(
    `${API}/avatar/events?ref=${encodeURIComponent(mint.conversationRef)}`,
  )
  events.onmessage = (message) => {
    try {
      const event = JSON.parse(message.data) as {
        type: string
        role?: "user" | "assistant"
        text?: string
        title?: string
        body?: string
        href?: string
      }
      if (event.type === "transcript" && event.role && event.text) {
        emitTranscript(event.role, event.text)
      } else if ((event.type === "card" || event.type === "guard_refusal") && event.title) {
        callbacks.onCard({
          title: event.title,
          body: event.body ?? "",
          ...(event.href ? { href: event.href } : {}),
        })
      }
    } catch {
      // Ignore malformed frames.
    }
  }

  const stop = async () => {
    events.close()
    pc.close()
    container.replaceChildren()
    container.dataset.state = "idle"
    await fetch(`${API}/avatar/stop`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ sessionId: mint.sessionId }),
    }).catch(() => {})
    callbacks.onState("ended")
  }

  window.addEventListener("beforeunload", () => {
    navigator.sendBeacon?.(
      `${API}/avatar/stop`,
      new Blob([JSON.stringify({ sessionId: mint.sessionId })], {
        type: "application/json",
      }),
    )
  })

  return {
    stop,
    message: (text: string) => {
      // Server-side turn loop: brain → TTS → render-service speak API. The
      // transcript comes back over the SSE bus above.
      void fetch(`${API}/avatar/speak`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ sessionId: mint.sessionId, message: text }),
      }).catch(() => {})
    },
    conversationRef: mint.conversationRef,
    sandbox: mint.sandbox,
  }
}
