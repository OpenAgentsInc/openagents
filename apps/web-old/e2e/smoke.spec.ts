import { test, expect } from '@playwright/test';

test('home loads', async ({ page }) => {
  await page.goto('/');
  await expect(page).toHaveTitle(/OpenAgents/i);
});

test('docs route loads', async ({ page }) => {
  await page.goto('/docs');
  await expect(page).toHaveURL(/\/docs/);
});
