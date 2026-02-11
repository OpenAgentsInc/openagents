import { defineConfig } from "vitest/config"

export default defineConfig({
  test: {
    include: [
      "test/*contract*.test.ts",
      "test/*contracts*.test.ts",
      "test/lnd-contracts.test.ts",
    ],
    environment: "node",
    coverage: {
      enabled: true,
      provider: "v8",
      include: ["src/contracts/lnd.ts", "src/contracts/rpc.ts"],
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
