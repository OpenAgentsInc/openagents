import { defineConfig, devices } from "@playwright/test";

const normalizeBoolean = (value?: string | null) => {
  if (!value) return false;
  return ["1", "true", "yes", "on"].includes(value.toLowerCase());
};

const cliArgs = process.argv.slice(2);
const debugEnabled = normalizeBoolean(process.env.PWDEBUG) || cliArgs.includes("--debug");
const headed =
  normalizeBoolean(process.env.HEADED) ||
  normalizeBoolean(process.env.PLAYWRIGHT_HEADFUL) ||
  cliArgs.includes("--headed") ||
  debugEnabled;

const slowMoValue =
  process.env.SLOWMO ?? process.env.PLAYWRIGHT_SLOWMO ?? (debugEnabled ? "500" : undefined);
const slowMo = slowMoValue ? parseInt(slowMoValue, 10) : 0;

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
