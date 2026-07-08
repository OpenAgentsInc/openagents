/**
 * Pinned self-test workload loader for Bun/Node runtimes (Pylon). Kept
 * out of the package root export so Cloudflare Worker bundles that
 * import lane constants never pull in `node:fs`. The workload is the
 * committed PoC fixture: a psionic-compiled, digest-pinned numeric
 * model whose Rust trace digest the TS executor must reproduce
 * byte-for-byte.
 */
import { readFileSync } from "node:fs"

import type { TassadarSelfTestWorkload } from "./capability-envelope.js"
import {
  runTassadarExecutorSelfTest,
  type TassadarExecutorSelfTestReceipt,
} from "./capability-envelope.js"

export const loadPinnedTassadarSelfTestWorkload =
  (): TassadarSelfTestWorkload => {
    const fixture = JSON.parse(
      readFileSync(
        new URL("../fixtures/tassadar-poc-loop-sum-v1.json", import.meta.url),
        "utf8",
      ),
    ) as TassadarSelfTestWorkload
    return {
      expectedModelDigest: fixture.expectedModelDigest,
      expectedTraceDigest: fixture.expectedTraceDigest,
      fixtureId: fixture.fixtureId,
      model: fixture.model,
      steps: fixture.steps,
    }
  }

/** Runs the real executor self-test against the pinned committed fixture. */
export const runPinnedTassadarExecutorSelfTest = async (
  input: Readonly<{ observedAt?: string }> = {},
): Promise<TassadarExecutorSelfTestReceipt> =>
  runTassadarExecutorSelfTest({
    workload: loadPinnedTassadarSelfTestWorkload(),
    ...(input.observedAt === undefined
      ? {}
      : { observedAt: input.observedAt }),
  })
