import { describe, expect, test } from "vite-plus/test";

import { SQL } from "./index.ts";

/**
 * `SQL()` is a thin constructor over postgres.js. The only library-independent,
 * side-effect-free behaviour is the input guard: an options object with neither
 * `url` nor `host` must fail closed rather than construct a pool against an
 * unknown target. The success paths open a real connection pool and are not
 * unit-testable here (they belong to an integration test with a live Postgres).
 */
describe("SQL input guard", () => {
  test("throws a TypeError with the exact message when neither url nor host is given", () => {
    expect(() => SQL({})).toThrow(TypeError);
    expect(() => SQL({})).toThrow("Postgres URL or host is required");
  });

  test("still fails closed when other options are present but url/host are absent", () => {
    expect(() => SQL({ max: 4, prepare: false })).toThrow("Postgres URL or host is required");
  });
});
