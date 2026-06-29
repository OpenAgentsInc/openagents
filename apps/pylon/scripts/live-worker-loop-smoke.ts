#!/usr/bin/env bun

import {
  buildLiveWorkerLoopSmokeOptions,
  redactSmokeText,
  runLiveWorkerLoopSmoke,
} from "../src/live-worker-loop-smoke"

try {
  const options = buildLiveWorkerLoopSmokeOptions()
  const result = await runLiveWorkerLoopSmoke(options)

  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)

  if (result.status !== "passed") {
    process.exitCode = 2
  }
} catch (error) {
  const message = error instanceof Error ? error.message : String(error)
  process.stderr.write(`${redactSmokeText(message)}\n`)
  process.exitCode = 1
}
