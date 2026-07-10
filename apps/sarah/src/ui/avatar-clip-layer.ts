/**
 * Pre-rendered clip layer for the Sarah avatar pane (epic #8610).
 *
 * Plays the shippable Hallo2 opener/canned clips (served by
 * /sarah/api/clips/:name) in a dedicated <video> element layered OVER the
 * live WebRTC stream inside the #sarah-avatar chrome container, then fades
 * out back to the live tier:
 *
 *   - OPENER: starts immediately at mint (killing cold-start dead air) while
 *     the WebRTC session warms underneath. When the clip ends it HOLDS its
 *     last frame until live media is ready, then fades out (crossfade).
 *   - CANNED (KHS-6): plays over an already-live stream and fades out on end.
 *
 * Honest degradation is the caller's contract: `onUnplayable` fires when the
 * clip cannot start at all (fetch/decode error, or even muted autoplay
 * refused) so the caller can restore the server-side TTS path — never dead
 * air. `onMutedPlayback` fires when audible autoplay was refused but muted
 * visuals proceed (the caller should restore server TTS audio).
 *
 * DOM discipline note: this is imperative MEDIA machinery (like srcObject
 * binding in avatar-session.ts), not UI tree — the element is transient,
 * chrome-scoped, pointer-inert, and never part of the Effect Native tree.
 * Registered as a catalog gap in docs/sarah/EN-GAPS.md.
 */

export const AVATAR_CLIP_FADE_MS = 350

/** Structural slice of HTMLVideoElement so units run without a DOM. */
export type AvatarClipVideoLike = {
  src: string
  muted: boolean
  playsInline?: boolean
  preload?: string
  className: string
  dataset: Partial<Record<string, string>>
  play: () => Promise<void>
  pause: () => void
  remove: () => void
  addEventListener: (type: string, listener: () => void) => void
}

export type AvatarClipContainerLike = {
  /** Structural HTMLElement slice — accepts any node-ish value in units. */
  appendChild: (node: never) => unknown
  dataset: Partial<Record<string, string>>
}

export type AvatarClipKind = "opener" | "canned"

export type AvatarClipLayerState = "idle" | "playing" | "holding" | "fading"

export type AvatarClipLayerOptions = Readonly<{
  container: AvatarClipContainerLike
  createVideo: () => AvatarClipVideoLike
  /** Clip cannot play at all — caller restores the TTS path (no dead air). */
  onUnplayable: (kind: AvatarClipKind) => void
  /** Playing muted only (audible autoplay refused) — caller restores audio. */
  onMutedPlayback?: (kind: AvatarClipKind) => void
  scheduleTimeout?: (fn: () => void, ms: number) => unknown
}>

export type AvatarClipLayer = Readonly<{
  play: (clip: { url: string; kind: AvatarClipKind }) => void
  /** Live WebRTC media is decoding — a held opener may now crossfade out. */
  notifyLiveMedia: () => void
  /** User barge-in — drop the clip immediately. */
  interrupt: () => void
  destroy: () => void
  state: () => AvatarClipLayerState
}>

export function makeAvatarClipLayer(
  options: AvatarClipLayerOptions,
): AvatarClipLayer {
  const schedule = options.scheduleTimeout ?? ((fn, ms) => setTimeout(fn, ms))
  let current: AvatarClipVideoLike | null = null
  let state: AvatarClipLayerState = "idle"
  let liveReady = false

  const clear = () => {
    if (current) {
      try {
        current.pause()
      } catch {
        // already stopped
      }
      try {
        current.remove()
      } catch {
        // already detached
      }
    }
    current = null
    state = "idle"
    delete options.container.dataset.clip
  }

  const fadeOut = (video: AvatarClipVideoLike) => {
    if (current !== video) return
    state = "fading"
    video.dataset.clipVisible = "0"
    schedule(() => {
      if (current === video) clear()
    }, AVATAR_CLIP_FADE_MS)
  }

  const play = (clip: { url: string; kind: AvatarClipKind }) => {
    clear()
    let video: AvatarClipVideoLike
    try {
      video = options.createVideo()
    } catch {
      options.onUnplayable(clip.kind)
      return
    }
    current = video
    state = "playing"
    video.className = "sarah-clip-layer"
    video.playsInline = true
    video.preload = "auto"
    video.src = clip.url
    options.container.dataset.clip = "playing"

    video.addEventListener("playing", () => {
      if (current !== video) return
      video.dataset.clipVisible = "1"
    })
    video.addEventListener("ended", () => {
      if (current !== video) return
      if (clip.kind === "canned" || liveReady) {
        fadeOut(video)
        return
      }
      // Opener before live media: hold the final frame — the held Hallo2
      // frame beats a "Connecting…" void while WebRTC warms.
      state = "holding"
      options.container.dataset.clip = "holding"
    })
    video.addEventListener("error", () => {
      if (current !== video) return
      clear()
      options.onUnplayable(clip.kind)
    })

    try {
      options.container.appendChild(video as never)
    } catch {
      clear()
      options.onUnplayable(clip.kind)
      return
    }

    void Promise.resolve()
      .then(() => video.play())
      .catch(() => {
        if (current !== video) return
        // Audible autoplay refused — degrade to muted visuals + tell the
        // caller so server TTS restores the audio.
        video.muted = true
        void Promise.resolve()
          .then(() => video.play())
          .then(() => {
            if (current !== video) return
            options.onMutedPlayback?.(clip.kind)
          })
          .catch(() => {
            if (current !== video) return
            clear()
            options.onUnplayable(clip.kind)
          })
      })
  }

  return {
    play,
    notifyLiveMedia: () => {
      liveReady = true
      if (current && state === "holding") fadeOut(current)
    },
    interrupt: () => {
      if (current) clear()
    },
    destroy: clear,
    state: () => state,
  }
}
