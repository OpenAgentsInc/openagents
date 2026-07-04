/**
 * Property tests: for every top-level protocol message schema,
 * arbitrary → encode → JSON wire round-trip → decode → re-encode must be
 * stable. Arbitraries are derived from the schemas via `Schema.toArbitrary`
 * (fast-check integration in effect v4).
 *
 * Runs are deterministic: fixed fast-check seed, no wall-clock input.
 */
import { describe, expect, test } from "bun:test"
import { Schema as S } from "effect"
import * as fc from "fast-check"
import {
  BootstrapEntity,
  BootstrapRequest,
  BootstrapResponse,
  ChangelogEntry,
  LiveFrame,
  LogPage,
  MutationEnvelope,
  MutationResult,
  PushRequest,
  PushResponse,
  SyncCursor,
  SyncError,
} from "./index.js"

const SEED = 20260704
const NUM_RUNS = 128

const schemas: Record<string, S.Codec<any, any, never, never>> = {
  BootstrapEntity,
  BootstrapRequest,
  BootstrapResponse,
  ChangelogEntry,
  LiveFrame,
  LogPage,
  MutationEnvelope,
  MutationResult,
  PushRequest,
  PushResponse,
  SyncCursor,
  SyncError,
}

describe("khala-sync codec stability (property)", () => {
  for (const [name, schema] of Object.entries(schemas)) {
    test(`${name}: arbitrary → encode → JSON → decode → encode is stable`, () => {
      const arbitrary = S.toArbitrary(schema)
      const encode = S.encodeSync(schema)
      const decode = S.decodeUnknownSync(schema)
      fc.assert(
        fc.property(arbitrary, (value) => {
          // Cross the actual wire: serialize the encoded form to a JSON
          // string and parse it back, as a transport would.
          const wire: unknown = JSON.parse(JSON.stringify(encode(value)))
          const decoded = decode(wire)
          expect(encode(decoded)).toEqual(wire)
        }),
        { seed: SEED, numRuns: NUM_RUNS },
      )
    })
  }
})
