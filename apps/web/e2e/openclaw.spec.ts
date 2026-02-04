import { test, expect } from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';

const storageStatePath =
  process.env.E2E_STORAGE_STATE ??
  path.resolve(process.cwd(), '.auth', 'storageState.json');

test.describe('openclaw', () => {
  test.skip(!fs.existsSync(storageStatePath), 'No auth storage state present');

  test('openclaw instance route responds', async ({ page }) => {
    await page.goto('/openclaw/instance');
    await expect(page).toHaveURL(/\/openclaw\/instance/);
    await expect(page.locator('body')).not.toContainText('not authenticated');
  });
});
