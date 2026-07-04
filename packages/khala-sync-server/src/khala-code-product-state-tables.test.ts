import { describe, expect, test } from "bun:test"

import {
  KHALA_CODE_PRODUCT_STATE_TABLE_SPECS,
  KHALA_CODE_PRODUCT_STATE_TABLES,
  normalizeKhalaCodeProductStateValue,
  scopeChangesForKhalaCodeProductStateRow,
} from "./khala-code-product-state-tables.js"

describe("Khala Code product-state table registry", () => {
  test("every registered table has columns, keys, and an order column", () => {
    for (const table of KHALA_CODE_PRODUCT_STATE_TABLES) {
      const spec = KHALA_CODE_PRODUCT_STATE_TABLE_SPECS[table]
      expect(spec.columns.length).toBeGreaterThan(0)
      expect(spec.keyColumns.length).toBeGreaterThan(0)
      for (const keyColumn of spec.keyColumns) {
        expect(spec.columns).toContain(keyColumn)
      }
      expect(spec.columns).toContain(spec.orderColumn)
    }
  })

  test("team chat rows produce team and thread scope changes", () => {
    const changes = scopeChangesForKhalaCodeProductStateRow(
      "team_chat_messages",
      {
        id: "msg_1",
        team_id: "team_1",
        autopilot_thread_id: "thread_1",
        body: "hello",
      },
    )

    expect(changes.map((change) => String(change.scope))).toEqual([
      "scope.team.team_1",
      "scope.thread.thread_1",
    ])
    expect(changes.map((change) => String(change.entityId))).toEqual([
      "msg_1",
      "msg_1",
    ])
    expect(changes[0]?.postImage).toEqual({
      id: "msg_1",
      team_id: "team_1",
      autopilot_thread_id: "thread_1",
      body: "hello",
    })
  })

  test("membership, file, workspace, and share rows route to their sync scopes", () => {
    expect(
      scopeChangesForKhalaCodeProductStateRow("team_memberships", {
        team_id: "team_1",
        user_id: "user_1",
      }).map((change) => String(change.scope)),
    ).toEqual(["scope.team.team_1"])

    expect(
      scopeChangesForKhalaCodeProductStateRow("thread_files", {
        id: "file_1",
        team_id: "team_1",
        thread_id: "thread_1",
      }).map((change) => String(change.scope)),
    ).toEqual(["scope.team.team_1", "scope.thread.thread_1"])

    expect(
      scopeChangesForKhalaCodeProductStateRow("prefilled_workspaces", {
        id: "workspace_1",
        private_team_id: "team_2",
      }).map((change) => String(change.scope)),
    ).toEqual(["scope.team.team_2"])

    expect(
      scopeChangesForKhalaCodeProductStateRow("share_projections", {
        id: "share_1",
        team_id: "team_3",
      }).map((change) => String(change.scope)),
    ).toEqual(["scope.team.team_3"])
  })

  test("bigint values normalize without precision loss", () => {
    expect(normalizeKhalaCodeProductStateValue(9007199254740993n)).toBe(
      "9007199254740993",
    )
  })
})
