/**
 * LiveAvatar session wrapper for the Sarah Effect Native surface (#8598).
 *
 * Owns the imperative SDK lifecycle (mint → connect → attach video → voice
 * chat) and the SSE subscription to the brain's event bus, translating both
 * into the typed callbacks the EN surface consumes. The avatar <video> is the
 * Effect Native `MediaVideo` host attach target (catalog `media-video` kind,
 * effect-native#67, vendored v26) — this module never creates or removes the
 * element; it acquires it from the EN media-video driver via the pane handle
 * and only binds/unbinds the live stream. `pane.container` is the chrome
 * region whose `data-state` drives the idle/connecting CSS overlays.
 */

import {
  AgentEventsEnum,
  LiveAvatarSession,
  SessionEvent,
} from "@heygen/liveavatar-web-sdk"

import type { SarahBlueprintDelta } from "../services/avatar-event-bus.ts"
import type { MediaObservation } from "../contracts/fleet-continuity-projection.ts"
import {
  makeAvatarClipLayer,
  type AvatarClipVideoLike,
} from "./avatar-clip-layer.ts"
import {
  observeAvatarMediaHealth,
  type AvatarMediaHealthObserver,
} from "./avatar-media-health.ts"

export type AvatarCallbacks = {
  /** Conversation/session health; independent from decoded media movement. */
  onState: (state: "connecting" | "live" | "ended" | "error") => void
  /** Browser-observed video health; never admission, capacity, or cost truth. */
  onMedia: (observation: MediaObservation) => void
  /** Authoritative server-slot cleanup, independent from local media state. */
  onCleanup: (
    observation: "pending" | "confirmed" | "unconfirmed",
  ) => void
  onTranscript: (role: "user" | "assistant", text: string) => void
  onCard: (card: { title: string; body: string; href?: string }) => void
  onBlueprintDelta?: (delta: SarahBlueprintDelta) => void
}

export type AvatarPane = {
  /** The #sarah-avatar chrome container — `data-state` drives CSS overlays. */
  container: HTMLElement
  /** Resolves the EN media-video attach target once its host driver mounts. */
  acquireVideo: () => Promise<HTMLVideoElement>
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
  /**
   * Epic #8610: pre-rendered shippable opener the browser plays immediately
   * (its own judged audio) while WebRTC warms — present when the mint
   * requested greeting:"client_clip" and a clip is available server-side.
   * Its presence means the server SUPPRESSED the TTS greeting.
   */
  openerClip?: { name: string; url: string; script: string }
}

export type AvatarSessionFetch = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>

export type AvatarSessionEnvironment = Readonly<{
  fetch: AvatarSessionFetch
  createLiveAvatarSession: (sessionToken: string) => LiveAvatarSession
  createPeerConnection: () => RTCPeerConnection
  createEventSource: (url: string) => EventSource
  addBeforeUnload: (listener: () => void) => void
  removeBeforeUnload: (listener: () => void) => void
  sendStopBeacon: (url: string, body: Blob) => boolean
  getSpeechRecognitionConstructor: () => any
  /** Clip-layer <video> factory (epic #8610); defaults to document.createElement. */
  createClipVideoElement?: () => AvatarClipVideoLike
}>

const browserAvatarSessionEnvironment: AvatarSessionEnvironment = {
  fetch: (input, init) => fetch(input, init),
  createLiveAvatarSession: (sessionToken) =>
    new LiveAvatarSession(sessionToken),
  createPeerConnection: () => new RTCPeerConnection(),
  createEventSource: (url) => new EventSource(url),
  addBeforeUnload: (listener) =>
    window.addEventListener("beforeunload", listener),
  removeBeforeUnload: (listener) =>
    window.removeEventListener("beforeunload", listener),
  sendStopBeacon: (url, body) => navigator.sendBeacon?.(url, body) ?? false,
  getSpeechRecognitionConstructor: () =>
    (window as any).SpeechRecognition ?? (window as any).webkitSpeechRecognition,
  createClipVideoElement: () => document.createElement("video"),
}

type ServerStopMode = "request" | "beacon"

/** Fixed public-safe proof that browser teardown did not release admission. */
export class AvatarCleanupUnconfirmedError extends Error {
  readonly _tag = "AvatarCleanupUnconfirmedError"

  constructor() {
    super("avatar_cleanup_unconfirmed")
    this.name = "AvatarCleanupUnconfirmedError"
  }
}

export const isAvatarCleanupUnconfirmedError = (
  error: unknown,
): error is AvatarCleanupUnconfirmedError =>
  error instanceof AvatarCleanupUnconfirmedError ||
  (error instanceof Error && error.message === "avatar_cleanup_unconfirmed")

/**
 * One authoritative server-slot release per successful mint. Local browser
 * teardown is deliberately separate: a disconnected SDK or failed peer may
 * already be locally closed when a later handle.stop() still needs to await
 * this exact server stop.
 */
const makeEnsureServerStop = (
  sessionId: string,
  environment: AvatarSessionEnvironment,
) => {
  let serverStop: Promise<void> | null = null

  return (mode: ServerStopMode = "request"): Promise<void> => {
    if (serverStop !== null) return serverStop

    if (mode === "beacon") {
      try {
        const accepted = environment.sendStopBeacon(
          `${API}/avatar/stop`,
          new Blob([JSON.stringify({ sessionId })], {
            type: "application/json",
          }),
        )
        if (accepted) {
          serverStop = Promise.resolve()
          return serverStop
        }
      } catch {
        // A keepalive request below is the unload-compatible fallback.
      }
    }

    serverStop = Promise.resolve()
      .then(() =>
        environment.fetch(`${API}/avatar/stop`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ sessionId }),
          ...(mode === "beacon" ? { keepalive: true } : {}),
        }),
      )
      .then((response) => {
        if (!response.ok) throw new AvatarCleanupUnconfirmedError()
      })
      .catch((error) => {
        if (isAvatarCleanupUnconfirmedError(error)) throw error
        throw new AvatarCleanupUnconfirmedError()
      })
    return serverStop
  }
}

export async function startAvatarSession(
  pane: AvatarPane,
  callbacks: AvatarCallbacks,
  environment: AvatarSessionEnvironment = browserAvatarSessionEnvironment,
): Promise<AvatarHandle> {
  callbacks.onState("connecting")
  callbacks.onMedia({ status: "connecting" })
  // greeting:"client_clip" (epic #8610): this surface plays the pre-rendered
  // opener clip itself, so the server suppresses the TTS greeting when (and
  // only when) it returns an openerClip in the mint.
  const mintResponse = await environment.fetch(`${API}/avatar/session`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ greeting: "client_clip" }),
  })
  if (!mintResponse.ok) {
    const body = (await mintResponse.json().catch(() => ({}))) as {
      error?: { code?: string }
    }
    callbacks.onState("error")
    throw new Error(body.error?.code ?? `avatar_mint_${mintResponse.status}`)
  }
  const mint = (await mintResponse.json()) as AvatarMint
  const ensureServerStop = makeEnsureServerStop(mint.sessionId, environment)

  if (mint.renderer === "owned") {
    try {
      return await startOwnedRendererSession(
        pane,
        callbacks,
        mint,
        ensureServerStop,
        environment,
      )
    } catch (error) {
      await ensureServerStop()
      throw error
    }
  }

  let session: LiveAvatarSession
  try {
    session = environment.createLiveAvatarSession(mint.sessionToken ?? "")
  } catch (error) {
    callbacks.onState("error")
    await ensureServerStop()
    throw error instanceof Error
      ? error
      : new Error("avatar_session_construct_failed")
  }

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

  // The EN media-video host driver owns the <video> element lifecycle; this
  // session only binds the SDK stream to it (fit/mute come from typed props).
  let video: HTMLVideoElement
  try {
    video = await pane.acquireVideo()
  } catch (error) {
    callbacks.onState("error")
    await ensureServerStop()
    throw error instanceof Error
      ? error
      : new Error("avatar_video_acquire_failed")
  }
  pane.container.dataset.state = "connecting"
  let mediaObserver: AvatarMediaHealthObserver | null = null
  let events: EventSource | null = null
  let locallyStopped = false
  let sdkStop: Promise<void> | null = null
  let automaticServerStopReported = false

  const ensureSdkStop = (): Promise<void> => {
    if (sdkStop !== null) return sdkStop
    sdkStop = Promise.resolve()
      .then(() => session.stop())
      .then(
        () => undefined,
        () => undefined,
      )
    return sdkStop
  }

  const reportAutomaticServerStop = () => {
    const serverStop = ensureServerStop()
    if (automaticServerStopReported) return serverStop
    automaticServerStopReported = true
    try { callbacks.onCleanup("pending") } catch { /* observer-isolated */ }
    void serverStop.then(
      () => {
        try { callbacks.onCleanup("confirmed") } catch { /* observer-isolated */ }
      },
      () => {
        try { callbacks.onCleanup("unconfirmed") } catch { /* observer-isolated */ }
      },
    )
    return serverStop
  }

  const startMediaObserver = () => {
    mediaObserver?.stop()
    mediaObserver = observeAvatarMediaHealth({
      video,
      onObservation: (observation) => {
        pane.container.dataset.state =
          observation.status === "live" ? "live" : "connecting"
        callbacks.onMedia(observation)
      },
    })
  }

  const teardownLocalMedia = (observation: MediaObservation) => {
    mediaObserver?.stop()
    mediaObserver = null
    events?.close()
    events = null
    environment.removeBeforeUnload(beforeUnload)
    video.srcObject = null
    pane.container.dataset.state = "idle"
    callbacks.onMedia(observation)
  }

  const stopLocalMedia = (observation: MediaObservation): boolean => {
    if (locallyStopped) return false
    locallyStopped = true
    try { teardownLocalMedia(observation) } catch { /* authority continues */ }
    return true
  }

  const beforeUnload = () => {
    void ensureServerStop("beacon").catch(() => {})
    if (stopLocalMedia({ status: "ended" })) callbacks.onState("ended")
  }

  try {
    session.on(SessionEvent.SESSION_STREAM_READY, () => {
      if (locallyStopped) return
      try {
        session.attach(video)
        startMediaObserver()
        callbacks.onState("live")
        void Promise.resolve(session.voiceChat.start()).catch(() => {
          // Mic denied — the avatar still speaks; text input remains available.
        })
      } catch {
        void ensureSdkStop()
        if (stopLocalMedia({ status: "unavailable" })) {
          try { callbacks.onState("error") } catch { /* observer-isolated */ }
        }
        void reportAutomaticServerStop().catch(() => {})
      }
    })
    session.on(SessionEvent.SESSION_DISCONNECTED, () => {
      if (stopLocalMedia({ status: "ended" })) {
        try { callbacks.onState("ended") } catch { /* observer-isolated */ }
      }
      void reportAutomaticServerStop().catch(() => {})
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
  } catch (error) {
    const serverStop = ensureServerStop()
    if (stopLocalMedia({ status: "unavailable" })) {
      callbacks.onState("error")
    }
    await Promise.all([ensureSdkStop(), serverStop])
    throw error instanceof Error ? error : new Error("avatar_session_start_failed")
  }
  if (locallyStopped) {
    await Promise.all([ensureSdkStop(), ensureServerStop()])
    throw new Error("avatar_session_closed_during_start")
  }

  try {
    events = environment.createEventSource(
      `${API}/avatar/events?ref=${encodeURIComponent(mint.conversationRef)}`,
    )
  } catch (error) {
    const serverStop = ensureServerStop()
    if (stopLocalMedia({ status: "unavailable" })) callbacks.onState("error")
    await Promise.all([ensureSdkStop(), serverStop])
    throw error
  }
  events.onmessage = (message) => {
    if (locallyStopped) return
    try {
      const event = JSON.parse(message.data) as {
        type: string
        role?: "user" | "assistant"
        text?: string
        title?: string
        body?: string
        href?: string
        delta?: SarahBlueprintDelta
      }
      if (event.type === "transcript" && event.role && event.text) {
        emitTranscript(event.role, event.text)
      } else if ((event.type === "card" || event.type === "guard_refusal") && event.title) {
        callbacks.onCard({
          title: event.title,
          body: event.body ?? "",
          ...(event.href ? { href: event.href } : {}),
        })
      } else if (event.type === "blueprint_delta" && event.delta) {
        callbacks.onBlueprintDelta?.(event.delta)
      }
    } catch {
      // Ignore malformed frames.
    }
  }

  const stop = async () => {
    const firstLocalStop = stopLocalMedia({ status: "ended" })
    await Promise.all([ensureSdkStop(), ensureServerStop()])
    if (firstLocalStop) callbacks.onState("ended")
  }

  try {
    environment.addBeforeUnload(beforeUnload)
  } catch (error) {
    await stop()
    throw error
  }

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
 * to the speak bridge. The user mic rides the browser's Web Speech API
 * (SpeechRecognition): final utterances post to the speak bridge, so Sarah
 * HEARS on the owned path too (owner-reported failure 2026-07-09; a native
 * owned-ASR lane can replace this without changing the surface contract).
 */
async function startOwnedRendererSession(
  pane: AvatarPane,
  callbacks: AvatarCallbacks,
  mint: AvatarMint,
  ensureServerStop: (mode?: ServerStopMode) => Promise<void>,
  environment: AvatarSessionEnvironment,
): Promise<AvatarHandle> {
  const offerUrl = mint.webrtc?.offer_url
  if (!offerUrl) {
    callbacks.onState("error")
    await ensureServerStop()
    throw new Error("avatar_owned_missing_offer_url")
  }

  const seenTranscripts = new Set<string>()
  const emitTranscript = (role: "user" | "assistant", text: string) => {
    const key = `${role}:${text.trim()}`
    if (!text.trim() || seenTranscripts.has(key)) return
    seenTranscripts.add(key)
    callbacks.onTranscript(role, text.trim())
  }

  // The EN media-video host driver owns the <video>; bind the recvonly
  // WebRTC stream to the acquired attach target.
  let video: HTMLVideoElement
  try {
    video = await pane.acquireVideo()
  } catch (error) {
    callbacks.onState("error")
    await ensureServerStop()
    throw error instanceof Error
      ? error
      : new Error("avatar_video_acquire_failed")
  }
  pane.container.dataset.state = "connecting"

  // --- pre-rendered clip tier (epic #8610) ----------------------------------
  // The opener clip starts NOW — Hallo2 quality on screen immediately, with
  // its own judged audio — while the WebRTC session warms underneath. Canned
  // KHS-6 clips arrive later over SSE and play over the live stream. Failure
  // of any clip falls through to the live/TTS path — never dead air.
  let greetFallbackRequested = false
  const requestServerGreeting = () => {
    if (greetFallbackRequested) return
    greetFallbackRequested = true
    void environment.fetch(`${API}/avatar/greet`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ sessionId: mint.sessionId }),
    }).catch(() => {})
  }
  const clipLayer = makeAvatarClipLayer({
    container: pane.container,
    createVideo:
      environment.createClipVideoElement ??
      (() => document.createElement("video")),
    onUnplayable: (kind) => {
      // The mint suppressed the server TTS greeting expecting the clip to
      // carry it — restore it. Canned clips need nothing: their transcript
      // line already landed and the reply text is on screen.
      if (kind === "opener") requestServerGreeting()
    },
    onMutedPlayback: (kind) => {
      // Audible autoplay refused: muted Hallo2 visuals continue while the
      // server TTS greeting restores the audio through the live track.
      if (kind === "opener") requestServerGreeting()
    },
  })
  if (mint.openerClip?.url) {
    clipLayer.play({ url: mint.openerClip.url, kind: "opener" })
  }

  type SpeechRecognitionLike = {
    lang: string
    continuous: boolean
    interimResults: boolean
    start: () => void
    stop: () => void
    onresult: ((event: any) => void) | null
    onend: (() => void) | null
    onerror: ((event: any) => void) | null
  }
  let mediaObserver: AvatarMediaHealthObserver | null = null
  let events: EventSource | null = null
  let recognition: SpeechRecognitionLike | null = null
  let recognitionActive = false
  let locallyStopped = false
  let automaticServerStopReported = false

  let pc: RTCPeerConnection
  try {
    pc = environment.createPeerConnection()
    pc.addTransceiver("video", { direction: "recvonly" })
    pc.addTransceiver("audio", { direction: "recvonly" })
  } catch (error) {
    clipLayer.destroy()
    callbacks.onState("error")
    await ensureServerStop()
    throw error instanceof Error
      ? error
      : new Error("avatar_owned_peer_construct_failed")
  }

  const startMediaObserver = () => {
    mediaObserver?.stop()
    mediaObserver = observeAvatarMediaHealth({
      video,
      onObservation: (observation) => {
        pane.container.dataset.state =
          observation.status === "live" ? "live" : "connecting"
        callbacks.onMedia(observation)
      },
    })
  }

  const teardownLocalMedia = (observation: MediaObservation) => {
    clipLayer.destroy()
    mediaObserver?.stop()
    mediaObserver = null
    events?.close()
    events = null
    recognitionActive = false
    try { recognition?.stop() } catch { /* already stopped */ }
    recognition = null
    pc.ontrack = null
    pc.onconnectionstatechange = null
    pc.close()
    environment.removeBeforeUnload(beforeUnload)
    video.srcObject = null
    pane.container.dataset.state = "idle"
    callbacks.onMedia(observation)
  }

  const stopLocalMedia = (observation: MediaObservation): boolean => {
    if (locallyStopped) return false
    locallyStopped = true
    try { teardownLocalMedia(observation) } catch { /* authority continues */ }
    return true
  }

  const reportAutomaticServerStop = () => {
    const serverStop = ensureServerStop()
    if (automaticServerStopReported) return serverStop
    automaticServerStopReported = true
    try { callbacks.onCleanup("pending") } catch { /* observer-isolated */ }
    void serverStop.then(
      () => {
        try { callbacks.onCleanup("confirmed") } catch { /* observer-isolated */ }
      },
      () => {
        try { callbacks.onCleanup("unconfirmed") } catch { /* observer-isolated */ }
      },
    )
    return serverStop
  }

  const beforeUnload = () => {
    void ensureServerStop("beacon").catch(() => {})
    if (stopLocalMedia({ status: "ended" })) callbacks.onState("ended")
  }

  pc.ontrack = (event) => {
    const stream = event.streams[0]
    if (stream && video.srcObject !== stream) {
      video.srcObject = stream
      startMediaObserver()
      callbacks.onState("live")
      // A held opener clip may now crossfade out to the live stream.
      clipLayer.notifyLiveMedia()
    }
  }
  pc.onconnectionstatechange = () => {
    if (
      !locallyStopped &&
      (pc.connectionState === "failed" || pc.connectionState === "closed")
    ) {
      if (stopLocalMedia({ status: "ended" })) {
        try { callbacks.onState("ended") } catch { /* observer-isolated */ }
      }
      void reportAutomaticServerStop().catch(() => {})
    }
  }

  try {
    const offer = await pc.createOffer()
    await pc.setLocalDescription(offer)
    const answerResponse = await environment.fetch(offerUrl, {
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
    const serverStop = ensureServerStop()
    if (stopLocalMedia({ status: "unavailable" })) callbacks.onState("error")
    await serverStop
    throw error instanceof Error ? error : new Error("avatar_owned_webrtc_failed")
  }

  try {
    events = environment.createEventSource(
      `${API}/avatar/events?ref=${encodeURIComponent(mint.conversationRef)}`,
    )
  } catch (error) {
    const serverStop = ensureServerStop()
    if (stopLocalMedia({ status: "unavailable" })) callbacks.onState("error")
    await serverStop
    throw error
  }
  events.onmessage = (message) => {
    if (locallyStopped) return
    try {
      const event = JSON.parse(message.data) as {
        type: string
        role?: "user" | "assistant"
        text?: string
        title?: string
        body?: string
        href?: string
        name?: string
        url?: string
        delta?: SarahBlueprintDelta
      }
      if (event.type === "transcript" && event.role && event.text) {
        emitTranscript(event.role, event.text)
      } else if (event.type === "clip" && event.url) {
        // KHS-6 canned clip (epic #8610): play the QA-passed pre-rendered
        // clip over the live stream; the transcript line arrives separately.
        clipLayer.play({ url: event.url, kind: "canned" })
      } else if ((event.type === "card" || event.type === "guard_refusal") && event.title) {
        callbacks.onCard({
          title: event.title,
          body: event.body ?? "",
          ...(event.href ? { href: event.href } : {}),
        })
      } else if (event.type === "blueprint_delta" && event.delta) {
        callbacks.onBlueprintDelta?.(event.delta)
      }
    } catch {
      // Ignore malformed frames.
    }
  }

  // --- user mic: browser speech recognition -> speak bridge ------------------
  let RecognitionCtor: any
  try {
    RecognitionCtor = environment.getSpeechRecognitionConstructor()
  } catch {
    RecognitionCtor = undefined
  }
  let speakInFlight = Promise.resolve()
  if (typeof RecognitionCtor === "function") {
    try {
      recognition = new RecognitionCtor() as SpeechRecognitionLike
      recognition.lang = "en-US"
      recognition.continuous = true
      recognition.interimResults = false
      recognition.onresult = (event: any) => {
        for (let i = event.resultIndex; i < event.results.length; i += 1) {
          const result = event.results[i]
          if (!result.isFinal) continue
          const text = String(result[0]?.transcript ?? "").trim()
          if (!text) continue
          // Barge-in: the user talking over a playing clip drops the clip.
          clipLayer.interrupt()
          // Serialize turns: a fast talker must not interleave speak calls.
          speakInFlight = speakInFlight.then(() =>
            environment.fetch(`${API}/avatar/speak`, {
              method: "POST",
              headers: { "content-type": "application/json" },
              body: JSON.stringify({ sessionId: mint.sessionId, message: text }),
            }).then(() => {}, () => {}),
          )
        }
      }
      recognition.onend = () => {
        // Chrome ends recognition periodically; keep listening for the session.
        if (recognitionActive) {
          try { recognition?.start() } catch { /* already started */ }
        }
      }
      recognition.onerror = (event: any) => {
        if (event?.error === "not-allowed" || event?.error === "service-not-allowed") {
          recognitionActive = false
          callbacks.onCard({
            title: "Microphone unavailable",
            body: "Sarah can't hear you — allow microphone access or type below.",
          })
        }
      }
      recognition.start()
      recognitionActive = true
    } catch {
      recognition = null
    }
  } else {
    callbacks.onCard({
      title: "Voice input not supported here",
      body: "This browser lacks speech recognition — type below and Sarah will speak her replies.",
    })
  }

  const stop = async () => {
    const firstLocalStop = stopLocalMedia({ status: "ended" })
    await ensureServerStop()
    if (firstLocalStop) callbacks.onState("ended")
  }

  try {
    environment.addBeforeUnload(beforeUnload)
  } catch (error) {
    await stop()
    throw error
  }

  return {
    stop,
    message: (text: string) => {
      // Barge-in: a typed turn drops any playing clip before the next reply.
      clipLayer.interrupt()
      // Server-side turn loop: brain → TTS → render-service speak API. The
      // transcript comes back over the SSE bus above.
      void environment.fetch(`${API}/avatar/speak`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ sessionId: mint.sessionId, message: text }),
      }).catch(() => {})
    },
    conversationRef: mint.conversationRef,
    sandbox: mint.sandbox,
  }
}
