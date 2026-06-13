import { describe, expect, test } from "bun:test"

import {
  MigrationGapError,
  applyPlan,
  planMigrations,
} from "../src/tas/migration"

describe("tas migration planner", () => {
  test("computes pending migrations in ascending version order", () => {
    expect(
      planMigrations(1, [{ version: 3 }, { version: 1 }, { version: 2 }]),
    ).toEqual([{ version: 2 }, { version: 3 }])
  })

  test("idempotently skips already-applied versions", () => {
    const migrations = [{ version: 1 }, { version: 2 }, { version: 3 }]

    expect(applyPlan(0, [1], migrations)).toEqual({
      currentVersion: 3,
      applied: [1, 2, 3],
    })

    expect(applyPlan(3, [1, 2, 3], migrations)).toEqual({
      currentVersion: 3,
      applied: [1, 2, 3],
    })
  })

  test("rejects a missing intermediate migration", () => {
    expect(() => applyPlan(0, [], [{ version: 1 }, { version: 3 }])).toThrow(
      MigrationGapError,
    )
  })

  test("returns an empty pending plan when up to date", () => {
    expect(
      planMigrations(3, [{ version: 1 }, { version: 2 }, { version: 3 }]),
    ).toEqual([])
  })
})
