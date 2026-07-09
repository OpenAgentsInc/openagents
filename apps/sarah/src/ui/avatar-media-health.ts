import type { MediaObservation } from "../contracts/fleet-continuity-projection.ts"
import { FC3_FRESHNESS_TIMEOUT_MS } from "../contracts/fleet-continuity-projection.ts"

/**
 * Browser-observable media truth only.
 *
 * This seam does not know whether realtime media was admitted, what it cost,
 * or whether a provider reserved capacity. It can only lease the claim that a
 * local video transport is moving: a decoded frame was observed while a live
 * video track was attached. The lease expires unless another frame renews it.
 */
export const SARAH_AVATAR_FRAME_STALE_AFTER_MS = FC3_FRESHNESS_TIMEOUT_MS
export const SARAH_AVATAR_FRAME_FALLBACK_POLL_MS = 250
export const SARAH_AVATAR_LIVE_EMIT_CADENCE_MS = 5_000
export const SARAH_AVATAR_FRAME_STALE_MAX_MS = 60_000
export const SARAH_AVATAR_FRAME_FALLBACK_POLL_MAX_MS = 5_000
export const SARAH_AVATAR_LIVE_EMIT_CADENCE_MAX_MS = 10_000
export const SARAH_BROWSER_MEDIA_TRANSPORT_LEASE_REF =
  "browser.media.transport.active"

type TimerHandle = ReturnType<typeof setTimeout>

export type AvatarMediaHealthClock = Readonly<{
  now: () => number
  setTimeout: (callback: () => void, delayMs: number) => TimerHandle
  clearTimeout: (handle: TimerHandle) => void
}>

export type AvatarMediaHealthObserverOptions = Readonly<{
  video: HTMLVideoElement
  onObservation: (observation: MediaObservation) => void
  clock?: AvatarMediaHealthClock
  staleAfterMs?: number
  fallbackPollMs?: number
  liveEmitCadenceMs?: number
  transportLeaseRef?: string
}>

export type AvatarMediaHealthObserver = Readonly<{
  mode: "video_frame_callback" | "bounded_time_poll"
  stop: () => void
}>

type VideoFrameMetadataLike = Readonly<{ mediaTime?: number }>
type VideoFrameCapable = HTMLVideoElement & {
  requestVideoFrameCallback?: (
    callback: (now: number, metadata: VideoFrameMetadataLike) => void,
  ) => number
  cancelVideoFrameCallback?: (handle: number) => void
}

const browserClock: AvatarMediaHealthClock = {
  now: () => Date.now(),
  setTimeout: (callback, delayMs) => setTimeout(callback, delayMs),
  clearTimeout: (handle) => clearTimeout(handle),
}

const HAVE_CURRENT_DATA = 2
const MEDIA_PUBLIC_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:-]*$/

const isBoundedPositiveMs = (value: number): boolean =>
  Number.isSafeInteger(value) && value > 0

const mediaStreamVideoTracks = (
  source: HTMLVideoElement["srcObject"],
): ReadonlyArray<MediaStreamTrack> => {
  try {
    if (
      source === null ||
      typeof source !== "object" ||
      !("getVideoTracks" in source) ||
      typeof source.getVideoTracks !== "function"
    ) {
      return []
    }
    const tracks = source.getVideoTracks()
    return Array.isArray(tracks) ? tracks : []
  } catch {
    return []
  }
}

const videoTracks = (video: HTMLVideoElement): ReadonlyArray<MediaStreamTrack> => {
  try {
    return mediaStreamVideoTracks(video.srcObject)
  } catch {
    return []
  }
}

const hasLiveVideoTrack = (video: HTMLVideoElement): boolean =>
  videoTracks(video).some((track) => {
    try {
      return track.readyState === "live" && track.enabled !== false
    } catch {
      return false
    }
  })

/**
 * Observe decoded video movement and project it into FC-3's typed media
 * observation union. `live` is emitted only by `observeFrame`; event names
 * such as `playing` or transport connection state never grant LIVE alone.
 */
export function observeAvatarMediaHealth(
  options: AvatarMediaHealthObserverOptions,
): AvatarMediaHealthObserver {
  const clock = options.clock ?? browserClock
  const staleAfterMs = options.staleAfterMs ?? SARAH_AVATAR_FRAME_STALE_AFTER_MS
  const fallbackPollMs =
    options.fallbackPollMs ?? SARAH_AVATAR_FRAME_FALLBACK_POLL_MS
  const liveEmitCadenceMs =
    options.liveEmitCadenceMs ?? SARAH_AVATAR_LIVE_EMIT_CADENCE_MS
  const transportLeaseRef =
    options.transportLeaseRef ?? SARAH_BROWSER_MEDIA_TRANSPORT_LEASE_REF
  if (
    !isBoundedPositiveMs(staleAfterMs) ||
    staleAfterMs > SARAH_AVATAR_FRAME_STALE_MAX_MS ||
    !isBoundedPositiveMs(fallbackPollMs) ||
    fallbackPollMs > SARAH_AVATAR_FRAME_FALLBACK_POLL_MAX_MS ||
    fallbackPollMs >= staleAfterMs ||
    !isBoundedPositiveMs(liveEmitCadenceMs) ||
    liveEmitCadenceMs > SARAH_AVATAR_LIVE_EMIT_CADENCE_MAX_MS ||
    liveEmitCadenceMs >= staleAfterMs ||
    !transportLeaseRef ||
    transportLeaseRef.length > 256 ||
    !MEDIA_PUBLIC_REF_PATTERN.test(transportLeaseRef)
  ) {
    throw new Error("sarah_avatar_media_health_invalid_options")
  }

  const video = options.video as VideoFrameCapable
  const mode =
    typeof video.requestVideoFrameCallback === "function" &&
    typeof video.cancelVideoFrameCallback === "function"
      ? "video_frame_callback"
      : "bounded_time_poll"
  let stopped = false
  let lastFrameAtMs: number | null = null
  let lastLiveEmissionAtMs: number | null = null
  let publiclyLive = false
  const readCurrentTime = (): number => {
    try {
      return Number.isFinite(video.currentTime) && video.currentTime >= 0
        ? video.currentTime
        : 0
    } catch {
      return 0
    }
  }
  const safeNow = (): number | null => {
    try {
      const nowMs = clock.now()
      return Number.isSafeInteger(nowMs) &&
        nowMs >= 0 &&
        nowMs <= Number.MAX_SAFE_INTEGER - staleAfterMs
        ? nowMs
        : null
    } catch {
      return null
    }
  }
  let lastMediaTime = readCurrentTime()
  let staleTimer: TimerHandle | null = null
  let projectionTimer: TimerHandle | null = null
  let fallbackTimer: TimerHandle | null = null
  let frameCallbackHandle: number | null = null
  let trackCleanups: Array<() => void> = []
  let videoCleanups: Array<() => void> = []
  let pendingProjectionFrameAtMs: number | null = null

  const emit = (observation: MediaObservation) => {
    if (stopped) return
    try {
      options.onObservation(observation)
    } catch {
      // A host listener cannot break frame rescheduling or lease expiry.
    }
  }

  const clearStaleTimer = () => {
    if (staleTimer === null) return
    try { clock.clearTimeout(staleTimer) } catch { /* already cleared */ }
    staleTimer = null
  }

  const clearProjectionTimer = () => {
    if (projectionTimer !== null) {
      try { clock.clearTimeout(projectionTimer) } catch { /* already cleared */ }
      projectionTimer = null
    }
    pendingProjectionFrameAtMs = null
  }

  const emitLiveProjection = (
    frameAtMs: number,
    emissionAtMs: number,
  ) => {
    publiclyLive = true
    lastLiveEmissionAtMs = emissionAtMs
    emit({
      status: "live",
      lease: {
        transportLeaseRef,
        transportExpiresAtMs: frameAtMs + staleAfterMs,
        lastFrameAtMs: frameAtMs,
      },
    })
  }

  const scheduleLiveProjection = (frameAtMs: number) => {
    pendingProjectionFrameAtMs = frameAtMs
    if (projectionTimer !== null || lastLiveEmissionAtMs === null) return
    const delayMs = Math.max(
      1,
      liveEmitCadenceMs - (frameAtMs - lastLiveEmissionAtMs),
    )
    try {
      projectionTimer = clock.setTimeout(() => {
        projectionTimer = null
        const latestFrameAtMs = pendingProjectionFrameAtMs
        pendingProjectionFrameAtMs = null
        const emissionAtMs = safeNow()
        if (
          stopped ||
          !publiclyLive ||
          latestFrameAtMs === null ||
          emissionAtMs === null ||
          emissionAtMs - latestFrameAtMs >= staleAfterMs
        ) {
          return
        }
        emitLiveProjection(latestFrameAtMs, emissionAtMs)
      }, delayMs)
    } catch {
      projectionTimer = null
      pendingProjectionFrameAtMs = null
    }
  }

  const emitTransportLoss = () => {
    clearStaleTimer()
    clearProjectionTimer()
    publiclyLive = false
    if (lastFrameAtMs === null) {
      emit({ status: "unavailable" })
      return
    }
    emit({ status: "stale", lastFrameAtMs })
  }

  const renewStaleTimer = (observedAtMs: number) => {
    clearStaleTimer()
    try {
      staleTimer = clock.setTimeout(() => {
        staleTimer = null
        if (stopped || lastFrameAtMs !== observedAtMs) return
        clearProjectionTimer()
        publiclyLive = false
        emit({ status: "stale", lastFrameAtMs: observedAtMs })
      }, staleAfterMs)
    } catch {
      staleTimer = null
      emit({ status: "unavailable" })
    }
  }

  const armInitialFrameDeadline = () => {
    clearStaleTimer()
    try {
      staleTimer = clock.setTimeout(() => {
        staleTimer = null
        if (stopped || lastFrameAtMs !== null) return
        clearProjectionTimer()
        publiclyLive = false
        emit({ status: "unavailable" })
      }, staleAfterMs)
    } catch {
      staleTimer = null
      emit({ status: "unavailable" })
    }
  }

  const observeFrame = () => {
    if (stopped || !hasLiveVideoTrack(video)) return
    const observedAtMs = safeNow()
    if (observedAtMs === null) return
    lastFrameAtMs = observedAtMs
    const shouldEmitLive =
      !publiclyLive ||
      lastLiveEmissionAtMs === null ||
      observedAtMs - lastLiveEmissionAtMs >= liveEmitCadenceMs
    if (shouldEmitLive) {
      clearProjectionTimer()
      emitLiveProjection(observedAtMs, observedAtMs)
    } else {
      scheduleLiveProjection(observedAtMs)
    }
    renewStaleTimer(observedAtMs)
  }

  const clearTrackListeners = () => {
    for (const cleanup of trackCleanups.splice(0)) {
      try { cleanup() } catch { /* detached track */ }
    }
  }

  const refreshTrackListeners = () => {
    clearTrackListeners()
    for (const track of videoTracks(video)) {
      const onEnded = () => emitTransportLoss()
      try {
        track.addEventListener("ended", onEnded)
        trackCleanups.push(() => track.removeEventListener("ended", onEnded))
      } catch {
        // The frame+live-track check remains authoritative without listeners.
      }
    }
  }

  const addVideoListener = (type: string, listener: EventListener) => {
    try {
      video.addEventListener(type, listener)
      videoCleanups.push(() => video.removeEventListener(type, listener))
    } catch {
      // A hostile host cannot grant LIVE; frame callbacks/polling still decide.
    }
  }

  const onSourceReady = () => refreshTrackListeners()
  const onTerminalMediaEvent = () => emitTransportLoss()
  addVideoListener("loadedmetadata", onSourceReady)
  addVideoListener("loadeddata", onSourceReady)
  addVideoListener("emptied", onTerminalMediaEvent)
  addVideoListener("ended", onTerminalMediaEvent)
  addVideoListener("error", onTerminalMediaEvent)
  refreshTrackListeners()

  const scheduleVideoFrameCallback = () => {
    if (stopped || mode !== "video_frame_callback") return
    try {
      frameCallbackHandle = video.requestVideoFrameCallback!((_now, metadata) => {
        frameCallbackHandle = null
        if (stopped) return
        try {
          if (
            typeof metadata.mediaTime === "number" &&
            Number.isFinite(metadata.mediaTime) &&
            metadata.mediaTime >= 0
          ) {
            lastMediaTime = metadata.mediaTime
          }
          observeFrame()
        } catch {
          emit({ status: "unavailable" })
        } finally {
          scheduleVideoFrameCallback()
        }
      })
    } catch {
      frameCallbackHandle = null
      emit({ status: "unavailable" })
    }
  }

  const scheduleFallbackPoll = () => {
    if (stopped || mode !== "bounded_time_poll") return
    try {
      fallbackTimer = clock.setTimeout(() => {
        fallbackTimer = null
        if (stopped) return
        try {
          const nextMediaTime = readCurrentTime()
          const decodedFrameAvailable =
            nextMediaTime > lastMediaTime &&
            video.readyState >= HAVE_CURRENT_DATA
          if (decodedFrameAvailable) {
            lastMediaTime = nextMediaTime
            observeFrame()
          }
        } catch {
          emit({ status: "unavailable" })
        } finally {
          scheduleFallbackPoll()
        }
      }, fallbackPollMs)
    } catch {
      fallbackTimer = null
      emit({ status: "unavailable" })
    }
  }

  emit({ status: "connecting" })
  armInitialFrameDeadline()
  if (mode === "video_frame_callback") scheduleVideoFrameCallback()
  else scheduleFallbackPoll()

  return {
    mode,
    stop: () => {
      if (stopped) return
      stopped = true
      clearStaleTimer()
      clearProjectionTimer()
      if (fallbackTimer !== null) {
        try { clock.clearTimeout(fallbackTimer) } catch { /* already cleared */ }
        fallbackTimer = null
      }
      if (frameCallbackHandle !== null) {
        try {
          video.cancelVideoFrameCallback?.(frameCallbackHandle)
        } catch {
          // The host already discarded the callback.
        }
        frameCallbackHandle = null
      }
      for (const cleanup of videoCleanups.splice(0)) {
        try { cleanup() } catch { /* detached video */ }
      }
      clearTrackListeners()
    },
  }
}
