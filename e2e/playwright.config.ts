import { defineConfig, devices } from "@playwright/test";

// Headed mode options (via env vars)
const headed = !!process.env.HEADED;
const slowMo = process.env.SLOWMO ? parseInt(process.env.SLOWMO, 10) : 0;

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  ...(process.env.CI ? { workers: 1 } : {}),
  reporter: [["html"], ["list"]],

  use: {
    baseURL: "http://localhost:3333",
    trace: "on-first-retry",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
    headless: !headed,
    launchOptions: {
      slowMo,
    },
  },

  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
    // WebKit disabled for local testing - enable when needed
    // {
    //   name: "webkit",
    //   use: { ...devices["Desktop Safari"] },
    // },
  ],

  webServer: {
    command: "bun e2e/test-server.ts",
    url: "http://localhost:3333",
    reuseExistingServer: !process.env.CI,
    timeout: 30000,
    cwd: process.cwd().replace(/\/e2e$/, ""),
  },
});
