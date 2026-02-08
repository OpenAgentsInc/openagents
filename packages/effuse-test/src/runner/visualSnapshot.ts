import * as Fs from "node:fs/promises"
import * as Path from "node:path"

import pixelmatch from "pixelmatch"
import { PNG } from "pngjs"
import { Effect } from "effect"

const sanitizeFileName = (value: string): string =>
  value.replaceAll(/[^a-zA-Z0-9._-]+/g, "_").slice(0, 160)

const baselineRoot = (): string =>
  // Runner currently supports only apps/web, and `effuse-test` is executed from `packages/effuse-test`.
  Path.resolve(process.cwd(), "../../apps/web/tests/visual/storybook")

const shouldUpdateSnapshots = (): boolean => {
  const raw = process.env.EFFUSE_TEST_UPDATE_SNAPSHOTS
  return raw === "1" || raw === "true" || raw === "yes"
}

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
}): Effect.Effect<void, Error> =>
  Effect.gen(function* () {
    const actualBytes = yield* Effect.tryPromise({
      try: () => Fs.readFile(input.actualPngPath),
      catch: (e) => (e instanceof Error ? e : new Error(String(e))),
    })

    const baselineExists = yield* Effect.promise(() =>
      Fs.access(input.baselinePngPath)
        .then(() => true)
        .catch(() => false),
    )

    if (!baselineExists) {
      if (!shouldUpdateSnapshots()) {
        return yield* Effect.fail(
          new Error(
            `Missing visual snapshot baseline for ${input.name}: ${input.baselinePngPath}\\n` +
              `Run with EFFUSE_TEST_UPDATE_SNAPSHOTS=1 to generate baselines.`,
          ),
        )
      }

      yield* Effect.tryPromise({
        try: async () => {
          await Fs.mkdir(Path.dirname(input.baselinePngPath), { recursive: true })
          await Fs.writeFile(input.baselinePngPath, actualBytes)
        },
        catch: (e) => (e instanceof Error ? e : new Error(String(e))),
      })
      return
    }

    if (shouldUpdateSnapshots()) {
      yield* Effect.tryPromise({
        try: async () => {
          await Fs.mkdir(Path.dirname(input.baselinePngPath), { recursive: true })
          await Fs.writeFile(input.baselinePngPath, actualBytes)
        },
        catch: (e) => (e instanceof Error ? e : new Error(String(e))),
      })
      return
    }

    const baselineBytes = yield* Effect.tryPromise({
      try: () => Fs.readFile(input.baselinePngPath),
      catch: (e) => (e instanceof Error ? e : new Error(String(e))),
    })

    const actual = readPng(actualBytes)
    const expected = readPng(baselineBytes)

    if (actual.width !== expected.width || actual.height !== expected.height) {
      return yield* Effect.fail(
        new Error(
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

    yield* Effect.tryPromise({
      try: async () => {
        await Fs.mkdir(Path.dirname(input.diffPngPath), { recursive: true })
        await Fs.writeFile(input.diffPngPath, writePng(diff))
      },
      catch: (e) => (e instanceof Error ? e : new Error(String(e))),
    }).pipe(Effect.catchAll(() => Effect.void))

    return yield* Effect.fail(
      new Error(
        `Visual snapshot mismatch for ${input.name}: ${mismatched} pixels differ.\\n` +
          `Baseline: ${input.baselinePngPath}\\n` +
          `Actual: ${input.actualPngPath}\\n` +
          `Diff: ${input.diffPngPath}\\n` +
          `To accept new output, run with EFFUSE_TEST_UPDATE_SNAPSHOTS=1.`,
      ),
    )
  })
