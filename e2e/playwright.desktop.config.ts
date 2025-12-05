/**
 * Playwright config for testing against the real desktop server.
 *
 * Usage:
 *   bun e2e:desktop
 *   # or
 *   bunx playwright test --config e2e/playwright.desktop.config.ts
 */

import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/effuse",
  fullyParallel: false, // Run sequentially for stability
  forbidOnly: !!process.env.CI,
  retries: 0,
  workers: 1,
  reporter: [["list"]],
  timeout: 30000,

  use: {
    baseURL: "http://localhost:8080",
    trace: "on-first-retry",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
    headless: true,
  },

  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],

  webServer: {
    command: "bun src/desktop/main.ts",
    url: "http://localhost:8080",
    reuseExistingServer: !process.env.CI,
    timeout: 15000,
    cwd: process.cwd().replace(/\/e2e$/, ""),
  },
});
