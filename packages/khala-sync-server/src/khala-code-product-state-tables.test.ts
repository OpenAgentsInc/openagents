import { describe, expect, test } from "bun:test"

import {
  KHALA_CODE_PRODUCT_STATE_TABLE_SPECS,
  KHALA_CODE_PRODUCT_STATE_TABLES,
  normalizeKhalaCodeProductStateValue,
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

  test("bigint values normalize without precision loss", () => {
    expect(normalizeKhalaCodeProductStateValue(9007199254740993n)).toBe(
      "9007199254740993",
    )
  })
})
