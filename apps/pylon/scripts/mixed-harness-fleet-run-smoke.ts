#!/usr/bin/env bun
/**
 * Mixed two-harness FleetRun exit receipt (MH-2, issue #8583).
 *
 * Runs one FleetRun (`workerKind: "auto"`) that dispatches two work units to
 * two DIFFERENT concrete harnesses — Codex on one, Claude on the other —
 * under one claim registry, and prints the receipt proving claim uniqueness
 * under mixed kinds and both closeouts receipted.
 *
 *   bun run --cwd apps/pylon smoke:mixed-harness-fleet-run
 *
 * CI-safe by construction (real executors + real in-memory claim registry, mock
 * SDK runners, no key/network/spend).
 */
import { runMixedHarnessFleetRunCiSmoke } from "../src/mixed-harness-fleet-run-smoke"

const result = await runMixedHarnessFleetRunCiSmoke()
process.stdout.write(`${JSON.stringify(result, null, 2)}\n`)
process.exit(result.ok ? 0 : 1)
