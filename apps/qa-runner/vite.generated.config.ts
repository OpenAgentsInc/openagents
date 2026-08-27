import "vite-plus/test/config";

import { defineConfig } from "vite-plus";

/** Runs review-approved generated scenarios against an explicitly selected target. */
export default defineConfig({
  root: import.meta.dirname,
  test: {
    environment: "node",
    include: ["generated/*.e2e.test.ts"],
    testTimeout: 120_000,
  },
});
