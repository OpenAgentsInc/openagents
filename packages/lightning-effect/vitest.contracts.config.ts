import { defineConfig } from "vitest/config"

export default defineConfig({
  test: {
    include: [
      "test/*contract*.test.ts",
      "test/*contracts*.test.ts",
    ],
    environment: "node",
    coverage: {
      enabled: true,
      provider: "v8",
      include: ["src/contracts/seller.ts"],
      reporter: ["text", "json-summary"],
      thresholds: {
        branches: 100,
        functions: 100,
        lines: 100,
        statements: 100,
      },
    },
  },
})
