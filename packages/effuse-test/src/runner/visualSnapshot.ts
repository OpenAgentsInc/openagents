import * as Fs from "node:fs/promises"
import * as Path from "node:path"

import pixelmatch from "pixelmatch"
import { PNG } from "pngjs"
import { Effect } from "effect"

import { EffuseTestConfig } from "../config/EffuseTestConfig.ts"

const toError = (cause: unknown): Error =>
  cause instanceof Error ? cause : new Error(String(cause))

export class VisualSnapshotError extends Error {
  readonly operation: string
  override readonly cause: unknown

  constructor(operation: string, cause: unknown) {
    const err = toError(cause)
    super(`[VisualSnapshot] ${operation}: ${err.message}`)
    this.name = "VisualSnapshotError"
    this.operation = operation
    this.cause = cause
  }
}

const trySnapshotPromise = <A>(operation: string, f: () => Promise<A>) =>
  Effect.tryPromise({
    try: f,
    catch: (cause) => new VisualSnapshotError(operation, cause),
  })

const sanitizeFileName = (value: string): string =>
  value.replaceAll(/[^a-zA-Z0-9._-]+/g, "_").slice(0, 160)

const baselineRoot = (): string =>
  Path.resolve(process.cwd(), "../../apps/openagents.com/tests/visual/storybook")

const readPng = (bytes: Buffer): PNG => PNG.sync.read(bytes)

const writePng = (png: PNG): Buffer => PNG.sync.write(png)

export const snapshotPathForStory = (storyId: string): string => {
  const file = `${sanitizeFileName(storyId)}.png`
  return Path.join(baselineRoot(), file)
}

export const assertPngSnapshot = (input: {
  readonly name: string
  readonly actualPngPath: string
  readonly diffPngPath: string
  readonly baselinePngPath: string
}): Effect.Effect<void, VisualSnapshotError, EffuseTestConfig> =>
  Effect.gen(function* () {
    const config = yield* EffuseTestConfig
    const updateSnapshots = config.updateSnapshots

    const actualBytes = yield* trySnapshotPromise("fs.readFile(actual png)", () =>
      Fs.readFile(input.actualPngPath),
    )

    const baselineExists = yield* trySnapshotPromise("fs.access(baseline png)", () =>
      Fs.access(input.baselinePngPath),
    ).pipe(
      Effect.as(true),
      Effect.catchAll(() => Effect.succeed(false)),
    )

    if (!baselineExists) {
      if (!updateSnapshots) {
        return yield* Effect.fail(
          new VisualSnapshotError(
            "baseline missing",
            `Missing visual snapshot baseline for ${input.name}: ${input.baselinePngPath}\\n` +
            `Run with EFFUSE_TEST_UPDATE_SNAPSHOTS=1 to generate baselines.`,
          ),
        )
      }

      yield* trySnapshotPromise("fs.writeFile(create baseline)", async () => {
        await Fs.mkdir(Path.dirname(input.baselinePngPath), { recursive: true })
        await Fs.writeFile(input.baselinePngPath, actualBytes)
      })
      return
    }

    if (updateSnapshots) {
      yield* trySnapshotPromise("fs.writeFile(update baseline)", async () => {
        await Fs.mkdir(Path.dirname(input.baselinePngPath), { recursive: true })
        await Fs.writeFile(input.baselinePngPath, actualBytes)
      })
      return
    }

    const baselineBytes = yield* trySnapshotPromise("fs.readFile(baseline png)", () =>
      Fs.readFile(input.baselinePngPath),
    )

    const actual = readPng(actualBytes)
    const expected = readPng(baselineBytes)

    if (actual.width !== expected.width || actual.height !== expected.height) {
      return yield* Effect.fail(
        new VisualSnapshotError(
          "compare dimensions",
          `Snapshot size mismatch for ${input.name}: expected ${expected.width}x${expected.height}, got ${actual.width}x${actual.height}`,
        ),
      )
    }

    const diff = new PNG({ width: actual.width, height: actual.height })
    const mismatched = pixelmatch(
      expected.data,
      actual.data,
      diff.data,
      actual.width,
      actual.height,
      { threshold: 0.1 },
    )

    if (mismatched === 0) return

    yield* trySnapshotPromise("fs.writeFile(diff png)", async () => {
      await Fs.mkdir(Path.dirname(input.diffPngPath), { recursive: true })
      await Fs.writeFile(input.diffPngPath, writePng(diff))
    }).pipe(Effect.catchAll(() => Effect.void))

    return yield* Effect.fail(
      new VisualSnapshotError(
        "pixel mismatch",
        `Visual snapshot mismatch for ${input.name}: ${mismatched} pixels differ.\\n` +
        `Baseline: ${input.baselinePngPath}\\n` +
        `Actual: ${input.actualPngPath}\\n` +
        `Diff: ${input.diffPngPath}\\n` +
        `To accept new output, run with EFFUSE_TEST_UPDATE_SNAPSHOTS=1.`,
      ),
    )
  })
