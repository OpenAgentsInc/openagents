import { describe, expect, test } from "bun:test"

import {
  observeAvatarMediaHealth,
  type AvatarMediaHealthClock,
} from "./avatar-media-health.ts"
import {
  fleetContinuityProjection,
  type MediaObservation,
} from "../contracts/fleet-continuity-projection.ts"

type TimerHandle = ReturnType<typeof setTimeout>

class FakeClock implements AvatarMediaHealthClock {
  nowMs = 1_000
  nextId = 1
  tasks = new Map<number, { dueAtMs: number; callback: () => void }>()

  now = () => this.nowMs

  setTimeout = (callback: () => void, delayMs: number): TimerHandle => {
    const id = this.nextId++
    this.tasks.set(id, { dueAtMs: this.nowMs + delayMs, callback })
    return id as unknown as TimerHandle
  }

  clearTimeout = (handle: TimerHandle) => {
    this.tasks.delete(handle as unknown as number)
  }

  advanceBy(ms: number) {
    const targetMs = this.nowMs + ms
    while (true) {
      const next = [...this.tasks.entries()]
        .filter(([, task]) => task.dueAtMs <= targetMs)
        .sort((left, right) => left[1].dueAtMs - right[1].dueAtMs)[0]
      if (!next) break
      const [id, task] = next
      this.tasks.delete(id)
      this.nowMs = task.dueAtMs
      task.callback()
    }
    this.nowMs = targetMs
  }
}

class FakeTrack {
  readyState: MediaStreamTrackState = "live"
  enabled = true
  listeners = new Map<string, Set<EventListenerOrEventListenerObject>>()

  addEventListener(type: string, listener: EventListenerOrEventListenerObject) {
    const listeners = this.listeners.get(type) ?? new Set()
    listeners.add(listener)
    this.listeners.set(type, listeners)
  }

  removeEventListener(type: string, listener: EventListenerOrEventListenerObject) {
    this.listeners.get(type)?.delete(listener)
  }

  dispatch(type: string) {
    for (const listener of this.listeners.get(type) ?? []) {
      if (typeof listener === "function") listener(new Event(type))
      else listener.handleEvent(new Event(type))
    }
  }

  listenerCount(): number {
    return [...this.listeners.values()].reduce(
      (count, listeners) => count + listeners.size,
      0,
    )
  }
}

class FakeVideo {
  srcObject: { getVideoTracks: () => ReadonlyArray<FakeTrack> } | null
  currentTime = 0
  readyState = 2
  listeners = new Map<string, Set<EventListenerOrEventListenerObject>>()
  nextFrameId = 1
  frameCallbacks = new Map<
    number,
    (now: number, metadata: { mediaTime: number }) => void
  >()
  canceledFrameIds: number[] = []

  constructor(
    readonly track: FakeTrack,
    readonly withFrameCallback = true,
  ) {
    this.srcObject = { getVideoTracks: () => [track] }
    if (!withFrameCallback) {
      ;(this as { requestVideoFrameCallback?: unknown }).requestVideoFrameCallback =
        undefined
      ;(this as { cancelVideoFrameCallback?: unknown }).cancelVideoFrameCallback =
        undefined
    }
  }

  addEventListener(type: string, listener: EventListenerOrEventListenerObject) {
    const listeners = this.listeners.get(type) ?? new Set()
    listeners.add(listener)
    this.listeners.set(type, listeners)
  }

  removeEventListener(type: string, listener: EventListenerOrEventListenerObject) {
    this.listeners.get(type)?.delete(listener)
  }

  requestVideoFrameCallback(
    callback: (now: number, metadata: { mediaTime: number }) => void,
  ): number {
    const id = this.nextFrameId++
    this.frameCallbacks.set(id, callback)
    return id
  }

  cancelVideoFrameCallback(id: number) {
    this.canceledFrameIds.push(id)
    this.frameCallbacks.delete(id)
  }

  fireFrame(mediaTime: number) {
    const next = [...this.frameCallbacks.entries()][0]
    if (!next) throw new Error("missing frame callback")
    const [id, callback] = next
    this.frameCallbacks.delete(id)
    this.currentTime = mediaTime
    callback(0, { mediaTime })
  }

  listenerCount(): number {
    return [...this.listeners.values()].reduce(
      (count, listeners) => count + listeners.size,
      0,
    )
  }
}

const startObserver = (
  video: FakeVideo,
  clock: FakeClock,
  observations: MediaObservation[],
) =>
  observeAvatarMediaHealth({
    video: video as unknown as HTMLVideoElement,
    clock,
    staleAfterMs: 1_000,
    fallbackPollMs: 100,
    liveEmitCadenceMs: 200,
    transportLeaseRef: "browser.media.test",
    onObservation: (observation) => observations.push(observation),
  })

describe("Sarah browser avatar media health", () => {
  test("rejects hostile bounds and public-ref inputs with a fixed error", () => {
    const clock = new FakeClock()
    const video = new FakeVideo(new FakeTrack())
    const invalidOptions = [
      { staleAfterMs: 60_001 },
      { staleAfterMs: 1_000, fallbackPollMs: 1_000 },
      { staleAfterMs: 1_000, liveEmitCadenceMs: 1_000 },
      { transportLeaseRef: "not a public ref" },
    ]

    for (const invalid of invalidOptions) {
      expect(() =>
        observeAvatarMediaHealth({
          video: video as unknown as HTMLVideoElement,
          clock,
          onObservation: () => {},
          ...invalid,
        }),
      ).toThrow("sarah_avatar_media_health_invalid_options")
    }
  })

  test("never reports LIVE without a frame and expires the initial wait to unavailable", () => {
    const clock = new FakeClock()
    const video = new FakeVideo(new FakeTrack())
    const observations: MediaObservation[] = []
    const observer = startObserver(video, clock, observations)

    clock.advanceBy(999)
    expect(observations).toEqual([{ status: "connecting" }])
    clock.advanceBy(1)
    expect(observations).toEqual([
      { status: "connecting" },
      { status: "unavailable" },
    ])
    expect(observations.some((observation) => observation.status === "live")).toBe(false)

    observer.stop()
  })

  test("leases a fresh frame, expires it to stale, and recovers on the next frame", () => {
    const clock = new FakeClock()
    const video = new FakeVideo(new FakeTrack())
    const observations: MediaObservation[] = []
    const observer = startObserver(video, clock, observations)

    video.fireFrame(0.1)
    expect(observations.at(-1)).toEqual({
      status: "live",
      lease: {
        transportLeaseRef: "browser.media.test",
        transportExpiresAtMs: 2_000,
        lastFrameAtMs: 1_000,
      },
    })

    clock.advanceBy(999)
    expect(observations.at(-1)?.status).toBe("live")
    clock.advanceBy(1)
    expect(observations.at(-1)).toEqual({
      status: "stale",
      lastFrameAtMs: 1_000,
    })

    video.fireFrame(0.2)
    expect(observations.at(-1)).toMatchObject({
      status: "live",
      lease: { lastFrameAtMs: 2_000, transportExpiresAtMs: 3_000 },
    })

    observer.stop()
  })

  test("burst frames renew expiry without projecting LIVE at frame rate", () => {
    const clock = new FakeClock()
    const video = new FakeVideo(new FakeTrack())
    const observations: MediaObservation[] = []
    const observer = startObserver(video, clock, observations)

    for (let frame = 1; frame <= 100; frame += 1) {
      video.fireFrame(frame / 60)
      clock.advanceBy(10)
    }

    const liveObservations = observations.filter(
      (observation) => observation.status === "live",
    )
    // First frame plus one refresh per 200ms cadence over this one-second burst.
    expect(liveObservations.length).toBeLessThanOrEqual(6)
    expect(liveObservations.length).toBeGreaterThanOrEqual(5)
    expect(observations.some((observation) => observation.status === "stale")).toBe(false)

    clock.advanceBy(989)
    expect(observations.at(-1)?.status).toBe("live")
    const latestPublicLive = [...observations]
      .reverse()
      .find((observation) => observation.status === "live")
    expect(latestPublicLive).toBeDefined()
    expect(
      fleetContinuityProjection(
        {
          conversation: { status: "text_live" },
          media: latestPublicLive!,
          progress: { status: "not_started" },
        },
        clock.nowMs,
      ).media.status,
    ).toBe("live")
    clock.advanceBy(1)
    expect(observations.at(-1)?.status).toBe("stale")

    observer.stop()
  })

  test("bounded fallback grants LIVE only when media time advances", () => {
    const clock = new FakeClock()
    const video = new FakeVideo(new FakeTrack(), false)
    const observations: MediaObservation[] = []
    const observer = startObserver(video, clock, observations)

    expect(observer.mode).toBe("bounded_time_poll")
    clock.advanceBy(100)
    expect(observations).toEqual([{ status: "connecting" }])
    video.currentTime = 0.25
    clock.advanceBy(100)
    expect(observations.at(-1)?.status).toBe("live")

    observer.stop()
  })

  test("a non-live track cannot grant LIVE", () => {
    const clock = new FakeClock()
    const track = new FakeTrack()
    track.readyState = "ended"
    const video = new FakeVideo(track)
    const observations: MediaObservation[] = []
    const observer = startObserver(video, clock, observations)

    video.fireFrame(0.1)
    expect(observations).toEqual([{ status: "connecting" }])

    observer.stop()
  })

  test("contains a hostile clock and listener without emitting an invalid lease or losing frame rescheduling", () => {
    const clock = new FakeClock()
    const video = new FakeVideo(new FakeTrack())
    const observations: MediaObservation[] = []
    let hostileClock = true
    clock.now = () => {
      if (hostileClock) throw new Error("hostile clock detail")
      return clock.nowMs
    }
    const observer = startObserver(video, clock, observations)

    expect(() => video.fireFrame(0.1)).not.toThrow()
    expect(observations).toEqual([{ status: "connecting" }])
    expect(video.frameCallbacks.size).toBe(1)

    hostileClock = false
    video.fireFrame(0.2)
    expect(observations.at(-1)?.status).toBe("live")
    observer.stop()

    const throwingVideo = new FakeVideo(new FakeTrack())
    const throwingObserver = observeAvatarMediaHealth({
      video: throwingVideo as unknown as HTMLVideoElement,
      clock: new FakeClock(),
      staleAfterMs: 1_000,
      fallbackPollMs: 100,
      liveEmitCadenceMs: 200,
      onObservation: () => {
        throw new Error("host listener detail")
      },
    })
    expect(() => throwingVideo.fireFrame(0.1)).not.toThrow()
    expect(throwingVideo.frameCallbacks.size).toBe(1)
    throwingObserver.stop()
  })

  test("stop cancels callbacks, timers, video listeners, and track listeners", () => {
    const clock = new FakeClock()
    const track = new FakeTrack()
    const video = new FakeVideo(track)
    const observations: MediaObservation[] = []
    const observer = startObserver(video, clock, observations)

    video.fireFrame(0.1)
    const observationCount = observations.length
    observer.stop()

    expect(clock.tasks.size).toBe(0)
    expect(video.frameCallbacks.size).toBe(0)
    expect(video.canceledFrameIds).toHaveLength(1)
    expect(video.listenerCount()).toBe(0)
    expect(track.listenerCount()).toBe(0)
    track.dispatch("ended")
    clock.advanceBy(5_000)
    expect(observations).toHaveLength(observationCount)
  })
})
