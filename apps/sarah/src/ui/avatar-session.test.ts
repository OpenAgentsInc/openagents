import { describe, expect, test } from "bun:test"
import {
  AgentEventsEnum,
  SessionEvent,
  type LiveAvatarSession,
} from "@heygen/liveavatar-web-sdk"

import {
  startAvatarSession,
  type AvatarCallbacks,
  type AvatarHandle,
  type AvatarPane,
  type AvatarSessionEnvironment,
} from "./avatar-session.ts"
import {
  applyAvatarCleanupObservation,
  makeAvatarSessionAttemptGate,
} from "./avatar-session-attempt-gate.ts"

class OneSlotAvatarServer {
  readonly maxActiveSessions = 1
  readonly activeSessions = new Set<string>()
  renderer: "liveavatar" | "owned" = "liveavatar"
  offerFails = false
  stopFails = false
  nextSession = 1
  sessionMintRequests = 0
  authoritativeStopPosts = 0
  readonly stopKeepaliveValues: Array<boolean> = []

  fetch: AvatarSessionEnvironment["fetch"] = async (input, init) => {
    const url = String(input)
    if (url.endsWith("/avatar/session")) {
      this.sessionMintRequests += 1
      if (this.activeSessions.size >= this.maxActiveSessions) {
        return Response.json(
          { error: { code: "avatar_session_cap_exceeded" } },
          { status: 429 },
        )
      }
      const sessionId = `session-${this.nextSession++}`
      this.activeSessions.add(sessionId)
      return Response.json({
        renderer: this.renderer,
        sessionToken: `token-${sessionId}`,
        sessionId,
        conversationRef: `conversation-${sessionId}`,
        sandbox: true,
        ...(this.renderer === "owned"
          ? { webrtc: { offer_url: "https://render.test/offer" } }
          : {}),
      })
    }
    if (url.endsWith("/avatar/stop")) {
      this.authoritativeStopPosts += 1
      this.stopKeepaliveValues.push(init?.keepalive === true)
      if (this.stopFails) {
        return Response.json(
          { error: { code: "avatar_stop_unavailable" } },
          { status: 503 },
        )
      }
      const body = JSON.parse(String(init?.body ?? "{}")) as {
        sessionId?: string
      }
      if (body.sessionId) this.activeSessions.delete(body.sessionId)
      return Response.json({ ok: true })
    }
    if (url === "https://render.test/offer") {
      return new Response(this.offerFails ? "" : "v=0\r\n", {
        status: this.offerFails ? 502 : 200,
      })
    }
    if (url.endsWith("/avatar/speak")) return Response.json({ ok: true })
    throw new Error(`unexpected_avatar_test_request:${url}`)
  }

  sendStopBeacon = (_url: string, _body: Blob): boolean => {
    this.authoritativeStopPosts += 1
    this.activeSessions.clear()
    return true
  }

  async expectNextMintIsAdmitted() {
    const response = await this.fetch("/sarah/api/avatar/session", {
      method: "POST",
    })
    expect(response.status).toBe(200)
    expect(this.activeSessions.size).toBe(1)
  }
}

class FakeLiveAvatarSession {
  readonly handlers = new Map<unknown, Array<(event: any) => void>>()
  readonly voiceChat = { start: async () => {} }
  startFailure: Error | null = null
  emitReadyDuringStart = false
  attachThrows = false
  sdkStopCalls = 0

  on(event: unknown, handler: (event: any) => void) {
    const handlers = this.handlers.get(event) ?? []
    handlers.push(handler)
    this.handlers.set(event, handlers)
  }

  emit(event: unknown, value: any = {}) {
    for (const handler of this.handlers.get(event) ?? []) handler(value)
  }

  async start() {
    if (this.emitReadyDuringStart) this.emit(SessionEvent.SESSION_STREAM_READY)
    if (this.startFailure) throw this.startFailure
  }

  attach() {
    if (this.attachThrows) throw new Error("attach_failed")
  }

  async stop() {
    this.sdkStopCalls += 1
  }

  message() {}
}

class FakePeerConnection {
  connectionState: RTCPeerConnectionState = "new"
  ontrack: ((event: RTCTrackEvent) => any) | null = null
  onconnectionstatechange: ((event: Event) => any) | null = null
  localDescription: RTCSessionDescription | null = null
  remoteDescriptionFails = false

  addTransceiver() {}
  async createOffer(): Promise<RTCSessionDescriptionInit> {
    return { type: "offer", sdp: "v=0\r\n" }
  }
  async setLocalDescription(description: RTCLocalSessionDescriptionInit) {
    this.localDescription = description as RTCSessionDescription
  }
  async setRemoteDescription(_description: RTCSessionDescriptionInit) {
    if (this.remoteDescriptionFails) {
      throw new Error("remote_description_failed")
    }
  }
  close() {
    this.connectionState = "closed"
  }
  fail() {
    this.connectionState = "failed"
    this.onconnectionstatechange?.(new Event("connectionstatechange"))
  }
}

const makeCallbacks = (
  onCleanup: AvatarCallbacks["onCleanup"] = () => {},
): AvatarCallbacks => ({
  onState: () => {},
  onMedia: () => {},
  onCleanup,
  onTranscript: () => {},
  onCard: () => {},
})

const makePane = (acquireThrows = false): AvatarPane => ({
  container: { dataset: {} } as unknown as HTMLElement,
  acquireVideo: async () => {
    if (acquireThrows) throw new Error("video_acquire_failed")
    return { srcObject: null } as unknown as HTMLVideoElement
  },
})

type EnvironmentOptions = Readonly<{
  liveConstructorThrows?: boolean
  peerConstructorThrows?: boolean
  eventSourceThrows?: boolean
  beacon?: "accept" | "false" | "throw"
}>

const makeEnvironment = (
  server: OneSlotAvatarServer,
  liveSession: FakeLiveAvatarSession,
  peer: FakePeerConnection,
  options: EnvironmentOptions = {},
) => {
  let beforeUnload: (() => void) | null = null
  const environment: AvatarSessionEnvironment = {
    fetch: server.fetch,
    createLiveAvatarSession: () => {
      if (options.liveConstructorThrows) {
        throw new Error("live_constructor_failed")
      }
      return liveSession as unknown as LiveAvatarSession
    },
    createPeerConnection: () => {
      if (options.peerConstructorThrows) {
        throw new Error("peer_constructor_failed")
      }
      return peer as unknown as RTCPeerConnection
    },
    createEventSource: () => {
      if (options.eventSourceThrows) throw new Error("event_source_failed")
      return { onmessage: null, close: () => {} } as unknown as EventSource
    },
    addBeforeUnload: (listener) => {
      beforeUnload = listener
    },
    removeBeforeUnload: (listener) => {
      if (beforeUnload === listener) beforeUnload = null
    },
    sendStopBeacon:
      options.beacon === "false"
        ? () => false
        : options.beacon === "throw"
          ? () => { throw new Error("beacon_failed") }
          : server.sendStopBeacon,
    getSpeechRecognitionConstructor: () => undefined,
  }
  return {
    environment,
    fireBeforeUnload: () => beforeUnload?.(),
  }
}

const expectReleasedOnceAndRemint = async (
  server: OneSlotAvatarServer,
) => {
  expect(server.authoritativeStopPosts).toBe(1)
  expect(server.activeSessions.size).toBe(0)
  await server.expectNextMintIsAdmitted()
}

const expectCleanupUnconfirmedAndNoRemint = async (
  server: OneSlotAvatarServer,
  terminal: Promise<unknown>,
) => {
  await expect(terminal).rejects.toThrow("avatar_cleanup_unconfirmed")
  expect(server.authoritativeStopPosts).toBe(1)
  expect(server.activeSessions.size).toBe(1)
  expect(server.sessionMintRequests).toBe(1)
  const refused = await server.fetch("/sarah/api/avatar/session", {
    method: "POST",
  })
  expect(refused.status).toBe(429)
}

const cleanupGateCallbacks = () => {
  const gate = makeAvatarSessionAttemptGate()
  gate.nextAttempt()
  const observations: Array<"pending" | "confirmed" | "unconfirmed"> = []
  const callbacks = makeCallbacks((observation) => {
    observations.push(observation)
    applyAvatarCleanupObservation(gate, observation)
  })
  return { gate, observations, callbacks }
}

describe("Sarah authoritative avatar server-slot release", () => {
  test("SDK start rejection releases the only slot exactly once", async () => {
    const server = new OneSlotAvatarServer()
    const live = new FakeLiveAvatarSession()
    live.startFailure = new Error("sdk_start_failed")
    const { environment } = makeEnvironment(
      server,
      live,
      new FakePeerConnection(),
    )

    await expect(
      startAvatarSession(makePane(), makeCallbacks(), environment),
    ).rejects.toThrow("sdk_start_failed")

    expect(live.sdkStopCalls).toBe(1)
    await expectReleasedOnceAndRemint(server)
  })

  test("post-handle LiveAvatar attach failure and later stop release the only slot exactly once", async () => {
    const server = new OneSlotAvatarServer()
    const live = new FakeLiveAvatarSession()
    live.attachThrows = true
    const { environment } = makeEnvironment(
      server,
      live,
      new FakePeerConnection(),
    )

    const handle = await startAvatarSession(
      makePane(),
      makeCallbacks(),
      environment,
    )
    live.emit(SessionEvent.SESSION_STREAM_READY)
    await handle.stop()
    await handle.stop()

    expect(live.sdkStopCalls).toBe(1)
    await expectReleasedOnceAndRemint(server)
  })

  test("post-handle SDK disconnect and later stop coalesce on one server stop", async () => {
    const server = new OneSlotAvatarServer()
    const live = new FakeLiveAvatarSession()
    const { environment } = makeEnvironment(
      server,
      live,
      new FakePeerConnection(),
    )
    const cleanup = cleanupGateCallbacks()
    const handle = await startAvatarSession(
      makePane(),
      cleanup.callbacks,
      environment,
    )

    live.emit(SessionEvent.SESSION_DISCONNECTED)
    await handle.stop()
    await handle.stop()

    expect(live.sdkStopCalls).toBe(1)
    expect(cleanup.observations).toEqual(["pending", "confirmed"])
    expect(cleanup.gate.tryBeginReplacementTransition()).toBe(true)
    await expectReleasedOnceAndRemint(server)
  })

  test("owned offer failure releases the only slot exactly once", async () => {
    const server = new OneSlotAvatarServer()
    server.renderer = "owned"
    server.offerFails = true
    const { environment } = makeEnvironment(
      server,
      new FakeLiveAvatarSession(),
      new FakePeerConnection(),
    )

    await expect(
      startAvatarSession(makePane(), makeCallbacks(), environment),
    ).rejects.toThrow("avatar_owned_offer_502")

    await expectReleasedOnceAndRemint(server)
  })

  test("owned remote-description failure releases the only slot exactly once", async () => {
    const server = new OneSlotAvatarServer()
    server.renderer = "owned"
    const peer = new FakePeerConnection()
    peer.remoteDescriptionFails = true
    const { environment } = makeEnvironment(
      server,
      new FakeLiveAvatarSession(),
      peer,
    )

    await expect(
      startAvatarSession(makePane(), makeCallbacks(), environment),
    ).rejects.toThrow("remote_description_failed")

    await expectReleasedOnceAndRemint(server)
  })

  test("owned peer failure and later stop coalesce on one server stop", async () => {
    const server = new OneSlotAvatarServer()
    server.renderer = "owned"
    const peer = new FakePeerConnection()
    const { environment } = makeEnvironment(
      server,
      new FakeLiveAvatarSession(),
      peer,
    )
    const handle: AvatarHandle = await startAvatarSession(
      makePane(),
      makeCallbacks(),
      environment,
    )

    peer.fail()
    await handle.stop()
    await handle.stop()

    await expectReleasedOnceAndRemint(server)
  })

  test("beforeunload uses one beacon-compatible stop joined by later cleanup", async () => {
    const server = new OneSlotAvatarServer()
    const live = new FakeLiveAvatarSession()
    const { environment, fireBeforeUnload } = makeEnvironment(
      server,
      live,
      new FakePeerConnection(),
    )
    const handle = await startAvatarSession(
      makePane(),
      makeCallbacks(),
      environment,
    )

    fireBeforeUnload()
    await handle.stop()

    expect(live.sdkStopCalls).toBe(1)
    await expectReleasedOnceAndRemint(server)
  })

  for (const scenario of [
    {
      name: "LiveAvatar constructor",
      renderer: "liveavatar" as const,
      options: { liveConstructorThrows: true },
      expected: "live_constructor_failed",
    },
    {
      name: "LiveAvatar video acquire",
      renderer: "liveavatar" as const,
      acquireThrows: true,
      expected: "video_acquire_failed",
    },
    {
      name: "LiveAvatar EventSource",
      renderer: "liveavatar" as const,
      options: { eventSourceThrows: true },
      expected: "event_source_failed",
    },
    {
      name: "owned video acquire",
      renderer: "owned" as const,
      acquireThrows: true,
      expected: "video_acquire_failed",
    },
    {
      name: "owned peer constructor",
      renderer: "owned" as const,
      options: { peerConstructorThrows: true },
      expected: "peer_constructor_failed",
    },
    {
      name: "owned EventSource",
      renderer: "owned" as const,
      options: { eventSourceThrows: true },
      expected: "event_source_failed",
    },
  ]) {
    test(`${scenario.name} failure releases admission when cleanup succeeds`, async () => {
      const server = new OneSlotAvatarServer()
      server.renderer = scenario.renderer
      const { environment } = makeEnvironment(
        server,
        new FakeLiveAvatarSession(),
        new FakePeerConnection(),
        scenario.options,
      )

      await expect(
        startAvatarSession(
          makePane(scenario.acquireThrows === true),
          makeCallbacks(),
          environment,
        ),
      ).rejects.toThrow(scenario.expected)
      await expectReleasedOnceAndRemint(server)
    })
  }

  type CleanupFailureScenario = Readonly<{
    name: string
    renderer: "liveavatar" | "owned"
    acquireThrows?: boolean
    options?: EnvironmentOptions
    prepare?: (
      server: OneSlotAvatarServer,
      live: FakeLiveAvatarSession,
      peer: FakePeerConnection,
    ) => void
    exercise?: (
      pane: AvatarPane,
      callbacks: AvatarCallbacks,
      environment: AvatarSessionEnvironment,
      live: FakeLiveAvatarSession,
      peer: FakePeerConnection,
    ) => Promise<unknown>
  }>

  const cleanupFailureScenarios: ReadonlyArray<CleanupFailureScenario> = [
    {
      name: "LiveAvatar constructor",
      renderer: "liveavatar",
      options: { liveConstructorThrows: true },
    },
    {
      name: "LiveAvatar video acquire",
      renderer: "liveavatar",
      acquireThrows: true,
    },
    {
      name: "LiveAvatar SDK start",
      renderer: "liveavatar",
      prepare: (_server, live) => {
        live.startFailure = new Error("sdk_start_failed")
      },
    },
    {
      name: "LiveAvatar attach",
      renderer: "liveavatar",
      prepare: (_server, live) => {
        live.attachThrows = true
        live.emitReadyDuringStart = true
      },
    },
    {
      name: "LiveAvatar EventSource",
      renderer: "liveavatar",
      options: { eventSourceThrows: true },
    },
    {
      name: "owned video acquire",
      renderer: "owned",
      acquireThrows: true,
    },
    {
      name: "owned peer constructor",
      renderer: "owned",
      options: { peerConstructorThrows: true },
    },
    {
      name: "owned offer",
      renderer: "owned",
      prepare: (server) => {
        server.offerFails = true
      },
    },
    {
      name: "owned remote description",
      renderer: "owned",
      prepare: (_server, _live, peer) => {
        peer.remoteDescriptionFails = true
      },
    },
    {
      name: "owned EventSource",
      renderer: "owned",
      options: { eventSourceThrows: true },
    },
    {
      name: "owned peer terminal",
      renderer: "owned",
      exercise: async (pane, callbacks, environment, _live, peer) => {
        const handle = await startAvatarSession(pane, callbacks, environment)
        peer.fail()
        return handle.stop()
      },
    },
  ]

  for (const terminal of [
    "LiveAvatar attach",
    "LiveAvatar disconnect",
    "owned peer terminal",
  ] as const) {
    test(`post-handle ${terminal} stop 503 closes the client replacement gate without another mint`, async () => {
      const server = new OneSlotAvatarServer()
      server.stopFails = true
      server.renderer = terminal === "owned peer terminal" ? "owned" : "liveavatar"
      const live = new FakeLiveAvatarSession()
      const peer = new FakePeerConnection()
      if (terminal === "LiveAvatar attach") live.attachThrows = true
      const { environment } = makeEnvironment(server, live, peer)
      const cleanup = cleanupGateCallbacks()
      const handle = await startAvatarSession(
        makePane(),
        cleanup.callbacks,
        environment,
      )

      if (terminal === "LiveAvatar attach") {
        live.emit(SessionEvent.SESSION_STREAM_READY)
      } else if (terminal === "LiveAvatar disconnect") {
        live.emit(SessionEvent.SESSION_DISCONNECTED)
      } else {
        peer.fail()
      }
      await expect(handle.stop()).rejects.toThrow("avatar_cleanup_unconfirmed")

      expect(cleanup.observations).toEqual(["pending", "unconfirmed"])
      expect(cleanup.gate.tryBeginReplacementTransition()).toBe(false)
      expect(server.sessionMintRequests).toBe(1)
      expect(server.activeSessions.size).toBe(1)
    })
  }

  for (const scenario of cleanupFailureScenarios) {
    test(`${scenario.name} plus stop 503 is typed cleanup-unconfirmed and forbids remint`, async () => {
      const server = new OneSlotAvatarServer()
      server.renderer = scenario.renderer
      server.stopFails = true
      const live = new FakeLiveAvatarSession()
      const peer = new FakePeerConnection()
      scenario.prepare?.(server, live, peer)
      const { environment } = makeEnvironment(
        server,
        live,
        peer,
        scenario.options,
      )
      const pane = makePane(scenario.acquireThrows === true)
      const terminal = scenario.exercise === undefined
        ? startAvatarSession(pane, makeCallbacks(), environment)
        : scenario.exercise(
            pane,
            makeCallbacks(),
            environment,
            live,
            peer,
          )

      await expectCleanupUnconfirmedAndNoRemint(server, terminal)
    })
  }

  for (const beacon of ["false", "throw"] as const) {
    test(`beforeunload beacon ${beacon} uses exactly one keepalive fallback`, async () => {
      const server = new OneSlotAvatarServer()
      const live = new FakeLiveAvatarSession()
      const { environment, fireBeforeUnload } = makeEnvironment(
        server,
        live,
        new FakePeerConnection(),
        { beacon },
      )
      const handle = await startAvatarSession(
        makePane(),
        makeCallbacks(),
        environment,
      )

      fireBeforeUnload()
      await handle.stop()

      expect(server.stopKeepaliveValues).toEqual([true])
      await expectReleasedOnceAndRemint(server)
    })
  }

  test("a failed authoritative stop remains one failed promise and keeps admission closed", async () => {
    const server = new OneSlotAvatarServer()
    const live = new FakeLiveAvatarSession()
    const { environment } = makeEnvironment(
      server,
      live,
      new FakePeerConnection(),
    )
    const handle = await startAvatarSession(
      makePane(),
      makeCallbacks(),
      environment,
    )
    server.stopFails = true

    await expect(handle.stop()).rejects.toThrow("avatar_cleanup_unconfirmed")
    await expect(handle.stop()).rejects.toThrow("avatar_cleanup_unconfirmed")
    expect(server.authoritativeStopPosts).toBe(1)
    expect(server.activeSessions.size).toBe(1)
    const refused = await server.fetch("/sarah/api/avatar/session", {
      method: "POST",
    })
    expect(refused.status).toBe(429)
  })

  test("event names used by the fake remain the SDK contract values", () => {
    expect(SessionEvent.SESSION_STREAM_READY).toBeTruthy()
    expect(SessionEvent.SESSION_DISCONNECTED).toBeTruthy()
    expect(AgentEventsEnum.USER_TRANSCRIPTION).toBeTruthy()
  })
})
