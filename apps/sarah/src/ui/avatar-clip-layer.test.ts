/**
 * Clip-layer oracles (epic #8610).
 * Contract: sarah.avatar_opens_with_shippable_opener_clip.v1
 *   (oracle avatar_clip_layer.unit)
 *
 * Deterministic fake-video/fake-container units: opener plays immediately and
 * holds its last frame until live media, canned clips fade on end, barge-in
 * drops the clip, autoplay/muted failures reach the typed fallbacks, and
 * teardown always clears the layer.
 */

import { describe, expect, test } from "bun:test"

import {
  AVATAR_CLIP_FADE_MS,
  makeAvatarClipLayer,
  type AvatarClipContainerLike,
  type AvatarClipVideoLike,
} from "./avatar-clip-layer.ts"

type FakeVideo = AvatarClipVideoLike & {
  listeners: Map<string, Array<() => void>>
  playCalls: number
  removed: boolean
  paused: boolean
  emit: (type: string) => void
  playResults: Array<"ok" | "reject">
}

function makeFakeVideo(playResults: Array<"ok" | "reject"> = ["ok"]): FakeVideo {
  const video: FakeVideo = {
    src: "",
    muted: false,
    playsInline: false,
    preload: "",
    className: "",
    dataset: {},
    listeners: new Map(),
    playCalls: 0,
    removed: false,
    paused: false,
    playResults,
    play: () => {
      const result = video.playResults[Math.min(video.playCalls, video.playResults.length - 1)]
      video.playCalls += 1
      return result === "ok" ? Promise.resolve() : Promise.reject(new Error("NotAllowedError"))
    },
    pause: () => {
      video.paused = true
    },
    remove: () => {
      video.removed = true
    },
    addEventListener: (type, listener) => {
      const list = video.listeners.get(type) ?? []
      list.push(listener)
      video.listeners.set(type, list)
    },
    emit: (type) => {
      for (const listener of video.listeners.get(type) ?? []) listener()
    },
  }
  return video
}

function makeFakeContainer(): AvatarClipContainerLike & {
  appended: unknown[]
} {
  const appended: unknown[] = []
  return {
    appended,
    appendChild: (node) => {
      appended.push(node)
      return node
    },
    dataset: {},
  }
}

const tick = () => new Promise((resolve) => setTimeout(resolve, 0))

type Scheduled = { fn: () => void; ms: number }

function makeHarness(options?: {
  videos?: FakeVideo[]
  onUnplayable?: (kind: "opener" | "canned") => void
  onMutedPlayback?: (kind: "opener" | "canned") => void
}) {
  const container = makeFakeContainer()
  const videos = options?.videos ?? [makeFakeVideo()]
  let cursor = 0
  const scheduled: Scheduled[] = []
  const unplayable: string[] = []
  const muted: string[] = []
  const layer = makeAvatarClipLayer({
    container,
    createVideo: () => {
      const video = videos[Math.min(cursor, videos.length - 1)]!
      cursor += 1
      return video
    },
    onUnplayable: options?.onUnplayable ?? ((kind) => unplayable.push(kind)),
    onMutedPlayback: options?.onMutedPlayback ?? ((kind) => muted.push(kind)),
    scheduleTimeout: (fn, ms) => {
      scheduled.push({ fn, ms })
      return 0
    },
  })
  const runScheduled = () => {
    const jobs = scheduled.splice(0)
    for (const job of jobs) job.fn()
  }
  return { layer, container, videos, scheduled, runScheduled, unplayable, muted }
}

describe("opener playback + crossfade to live", () => {
  test("plays immediately, fades in on playing, holds after end, crossfades once live", async () => {
    const { layer, container, videos, scheduled, runScheduled } = makeHarness()
    const video = videos[0]!
    layer.play({ url: "/sarah/api/clips/opener-01-hello", kind: "opener" })
    await tick()

    expect(container.appended).toEqual([video])
    expect(video.src).toBe("/sarah/api/clips/opener-01-hello")
    expect(video.className).toBe("sarah-clip-layer")
    expect(video.playsInline).toBe(true)
    expect(video.muted).toBe(false)
    expect(container.dataset.clip).toBe("playing")
    expect(layer.state()).toBe("playing")

    video.emit("playing")
    expect(video.dataset.clipVisible).toBe("1")

    // Clip ends before live media: hold the final frame — never a void.
    video.emit("ended")
    expect(layer.state()).toBe("holding")
    expect(container.dataset.clip).toBe("holding")
    expect(video.removed).toBe(false)

    // Live media arrives: fade out over the fade window, then clear.
    layer.notifyLiveMedia()
    expect(layer.state()).toBe("fading")
    expect(video.dataset.clipVisible).toBe("0")
    expect(scheduled[0]?.ms).toBe(AVATAR_CLIP_FADE_MS)
    runScheduled()
    expect(video.removed).toBe(true)
    expect(layer.state()).toBe("idle")
    expect(container.dataset.clip).toBeUndefined()
  })

  test("live media before clip end: clip finishes, then fades", async () => {
    const { layer, videos, runScheduled } = makeHarness()
    const video = videos[0]!
    layer.play({ url: "/x", kind: "opener" })
    await tick()
    layer.notifyLiveMedia()
    expect(layer.state()).toBe("playing")
    expect(video.removed).toBe(false)
    video.emit("ended")
    expect(layer.state()).toBe("fading")
    runScheduled()
    expect(video.removed).toBe(true)
  })
})

describe("canned clips over the live stream (KHS-6)", () => {
  test("fades out on end without waiting for live media", async () => {
    const { layer, videos, runScheduled } = makeHarness()
    const video = videos[0]!
    layer.play({ url: "/sarah/api/clips/opener-05-show-you", kind: "canned" })
    await tick()
    video.emit("ended")
    expect(layer.state()).toBe("fading")
    runScheduled()
    expect(video.removed).toBe(true)
  })

  test("a new clip replaces the current one", async () => {
    const first = makeFakeVideo()
    const second = makeFakeVideo()
    const { layer, container } = makeHarness({ videos: [first, second] })
    layer.play({ url: "/a", kind: "canned" })
    await tick()
    layer.play({ url: "/b", kind: "canned" })
    await tick()
    expect(first.removed).toBe(true)
    expect(second.removed).toBe(false)
    expect(container.dataset.clip).toBe("playing")
    // Stale events from the replaced clip are inert.
    first.emit("ended")
    expect(layer.state()).toBe("playing")
  })
})

describe("barge-in and teardown", () => {
  test("interrupt drops the clip immediately", async () => {
    const { layer, container, videos } = makeHarness()
    layer.play({ url: "/a", kind: "opener" })
    await tick()
    layer.interrupt()
    expect(videos[0]!.removed).toBe(true)
    expect(videos[0]!.paused).toBe(true)
    expect(layer.state()).toBe("idle")
    expect(container.dataset.clip).toBeUndefined()
  })

  test("destroy clears the layer (session teardown path)", async () => {
    const { layer, videos } = makeHarness()
    layer.play({ url: "/a", kind: "opener" })
    await tick()
    layer.destroy()
    expect(videos[0]!.removed).toBe(true)
    expect(layer.state()).toBe("idle")
  })
})

describe("honest degradation", () => {
  test("audible autoplay refused → muted playback + typed callback", async () => {
    const video = makeFakeVideo(["reject", "ok"])
    const { layer, muted, unplayable } = makeHarness({ videos: [video] })
    layer.play({ url: "/a", kind: "opener" })
    await tick()
    await tick()
    expect(video.muted).toBe(true)
    expect(video.playCalls).toBe(2)
    expect(muted).toEqual(["opener"])
    expect(unplayable).toEqual([])
    expect(layer.state()).toBe("playing")
  })

  test("muted playback also refused → cleared + onUnplayable", async () => {
    const video = makeFakeVideo(["reject", "reject"])
    const { layer, muted, unplayable } = makeHarness({ videos: [video] })
    layer.play({ url: "/a", kind: "opener" })
    await tick()
    await tick()
    expect(unplayable).toEqual(["opener"])
    expect(muted).toEqual([])
    expect(video.removed).toBe(true)
    expect(layer.state()).toBe("idle")
  })

  test("media error event → cleared + onUnplayable", async () => {
    const video = makeFakeVideo()
    const { layer, unplayable } = makeHarness({ videos: [video] })
    layer.play({ url: "/a", kind: "canned" })
    await tick()
    video.emit("error")
    expect(unplayable).toEqual(["canned"])
    expect(video.removed).toBe(true)
    expect(layer.state()).toBe("idle")
  })

  test("video factory failure never throws — onUnplayable fires", () => {
    const { unplayable, layer } = makeHarness({
      videos: [makeFakeVideo()],
    })
    const throwingLayer = makeAvatarClipLayer({
      container: makeFakeContainer(),
      createVideo: () => {
        throw new Error("no DOM")
      },
      onUnplayable: (kind) => unplayable.push(kind),
    })
    throwingLayer.play({ url: "/a", kind: "opener" })
    expect(unplayable).toEqual(["opener"])
    expect(layer.state()).toBe("idle")
  })
})
