import type { MountedSurface } from "@effect-native/core"
import { type Duration, Effect, Exit, Ref, Scope, Stream } from "effect"
import { applyOps, type CanvasBackend, type FrameTick } from "./backend"
import { diffScene } from "./reconciler"
import type { CanvasScene } from "./scene"

/**
 * Frame scheduling on Effect.
 *
 * A canvas is driven by a `Stream` of {@link CanvasFrame}s. Each frame pairs the
 * scene that should currently be shown with the tick that requested the render.
 * The loop diffs the incoming scene against the previously rendered one, applies
 * the minimal op set to the backend, then calls `renderFrame`. Resource
 * lifetimes live on the backend + `Scope`, so unmount (scope close) stops the
 * loop and disposes GPU/geometry/material resources deterministically.
 */

export interface CanvasFrame {
  readonly scene: CanvasScene
  readonly tick: FrameTick
}

export interface CanvasRunResult {
  readonly framesRendered: number
  readonly lastScene: CanvasScene | undefined
}

export interface CanvasSurface extends MountedSurface {
  readonly framesRendered: Effect.Effect<number>
  readonly lastScene: Effect.Effect<CanvasScene | undefined>
}

interface LoopState {
  readonly prev: Ref.Ref<CanvasScene | undefined>
  readonly count: Ref.Ref<number>
}

const makeLoopState = Effect.gen(function* () {
  const prev = yield* Ref.make<CanvasScene | undefined>(undefined)
  const count = yield* Ref.make(0)
  return { prev, count } satisfies LoopState
})

const runFrame = (backend: CanvasBackend, state: LoopState, frame: CanvasFrame): Effect.Effect<void> =>
  Effect.gen(function* () {
    const prev = yield* Ref.get(state.prev)
    const ops = diffScene(prev, frame.scene)
    yield* applyOps(backend, ops)
    yield* backend.renderFrame(frame.tick)
    yield* Ref.set(state.prev, frame.scene)
    yield* Ref.update(state.count, (n) => n + 1)
  })

/**
 * Drive a backend from a finite (or bounded) `Stream` of frames, running to
 * completion. Ideal for snapshot tests and offline rendering: feed a list of
 * frames, then inspect the backend.
 */
export const drainCanvasFrames = (
  backend: CanvasBackend,
  frames: Stream.Stream<CanvasFrame>
): Effect.Effect<CanvasRunResult> =>
  Effect.gen(function* () {
    const state = yield* makeLoopState
    yield* Stream.runForEach(frames, (frame) => runFrame(backend, state, frame))
    return {
      framesRendered: yield* Ref.get(state.count),
      lastScene: yield* Ref.get(state.prev)
    }
  })

/**
 * Mount a live canvas surface: the frame loop runs on a forked fiber owned by a
 * child `Scope`. `unmount` closes that scope, stopping the loop. The backend's
 * own resources are released when the scope that constructed it closes.
 */
export const mountCanvas = (
  backend: CanvasBackend,
  frames: Stream.Stream<CanvasFrame>
): Effect.Effect<CanvasSurface, never, Scope.Scope> =>
  Effect.gen(function* () {
    const parentScope = yield* Scope.Scope
    const surfaceScope = yield* Scope.fork(parentScope)

    return yield* Scope.provide(surfaceScope)(
      Effect.gen(function* () {
        const state = yield* makeLoopState
        yield* Stream.runForEach(frames, (frame) => runFrame(backend, state, frame)).pipe(Effect.forkScoped)
        return {
          unmount: Scope.close(surfaceScope, Exit.void),
          framesRendered: Ref.get(state.count),
          lastScene: Ref.get(state.prev)
        }
      })
    )
  })

// ---------------------------------------------------------------------------
// Frame tick sources
// ---------------------------------------------------------------------------

/**
 * A live frame clock: an unbounded `Stream` of ticks spaced by `interval`.
 * `frame` increments per tick, `time` is a wall-clock reading, and `delta` is
 * the elapsed time since the previous tick (0 on the first tick).
 */
export const frameClock = (interval: Duration.Input, now: () => number = Date.now): Stream.Stream<FrameTick> =>
  Stream.tick(interval).pipe(
    Stream.mapAccum(
      () => ({ frame: -1, last: undefined as number | undefined }),
      (state) => {
        const time = now()
        const frame = state.frame + 1
        const delta = state.last === undefined ? 0 : time - state.last
        const tick: FrameTick = { frame, time, delta }
        return [{ frame, last: time }, [tick]]
      }
    )
  )

/** Deterministic frame ticks derived from explicit wall-clock timestamps. */
export const frameTicksFromTimes = (times: ReadonlyArray<number>): Stream.Stream<FrameTick> =>
  Stream.fromIterable(
    times.map((time, frame) => ({
      frame,
      time,
      delta: frame === 0 ? 0 : time - (times[frame - 1] ?? time)
    }))
  )

/**
 * Combine a stream of scenes and a stream of ticks into a stream of frames.
 * Rendering is driven by whichever stream advances (latest scene sampled on
 * each tick, latest tick reused when the scene updates) — matching the
 * view/viewport zip used by the headless UI renderer.
 */
export const withFrameTicks = (
  scenes: Stream.Stream<CanvasScene>,
  ticks: Stream.Stream<FrameTick>
): Stream.Stream<CanvasFrame> => ticks.pipe(Stream.zipLatestWith(scenes, (tick, scene) => ({ scene, tick })))

/** Pair each scene in a finite sequence with a synthetic, monotonically increasing tick. */
export const framesFromScenes = (scenes: ReadonlyArray<CanvasScene>, stepMillis = 16): Stream.Stream<CanvasFrame> =>
  Stream.fromIterable(
    scenes.map((scene, frame) => ({
      scene,
      tick: { frame, time: frame * stepMillis, delta: frame === 0 ? 0 : stepMillis }
    }))
  )
