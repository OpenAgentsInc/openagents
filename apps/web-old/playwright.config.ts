import { defineConfig } from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const baseURL = process.env.E2E_BASE_URL ?? 'https://openagents.com';
const moduleDir = path.dirname(fileURLToPath(import.meta.url));
const storageStatePath =
  process.env.E2E_STORAGE_STATE ??
  path.resolve(moduleDir, '.auth', 'storageState.json');
const hasStorageState = fs.existsSync(storageStatePath);

export default defineConfig({
  testDir: './e2e',
  timeout: 90_000,
  expect: { timeout: 10_000 },
  retries: 1,
  use: {
    baseURL,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
    storageState: hasStorageState ? storageStatePath : undefined,
  },
  reporter: [['list'], ['html', { open: 'never' }]],
});
