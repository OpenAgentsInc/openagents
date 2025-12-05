/**
 * Effuse Mainview Load Tests
 *
 * Tests that the real Effuse-based mainview loads correctly.
 * Uses the actual desktop server on port 8080.
 */

import { test, expect } from "@playwright/test";

test.describe("Effuse Mainview", () => {
  test.beforeEach(async ({ page }) => {
    // Go to the real desktop server
    await page.goto("http://localhost:8080/");
  });

  test("loads without critical JavaScript errors", async ({ page }) => {
    // Collect console errors
    const errors: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        errors.push(msg.text());
      }
    });

    // Wait for page to load and widgets to render
    await page.waitForSelector("#tb-controls-widget", { timeout: 10000 });

    // Check for critical errors (not network/WebSocket errors which are expected in test env)
    const criticalErrors = errors.filter(
      (e) =>
        (e.includes("ReferenceError") ||
          e.includes("TypeError") ||
          e.includes("SyntaxError")) &&
        !e.includes("WebSocket") // Ignore WebSocket errors in test
    );

    expect(criticalErrors).toEqual([]);
  });

  test("renders TB Controls widget", async ({ page }) => {
    // Wait for TB Controls to render
    const tbControls = page.locator("#tb-controls-widget");
    await expect(tbControls).toBeVisible({ timeout: 10000 });

    // Should contain Terminal-Bench header
    await expect(tbControls.getByText("Terminal-Bench")).toBeVisible();
  });

  test("renders Trajectory Pane widget", async ({ page }) => {
    const trajectoryPane = page.locator("#trajectory-pane-widget");
    await expect(trajectoryPane).toBeVisible({ timeout: 10000 });
  });

  test("renders all widget containers", async ({ page }) => {
    // Check all widget containers are present in the DOM
    await expect(page.locator("#tb-controls-widget")).toBeVisible({ timeout: 10000 });
    await expect(page.locator("#trajectory-pane-widget")).toBeVisible();
    await expect(page.locator("#container-panes-widget")).toBeVisible();
    await expect(page.locator("#mc-tasks-widget")).toBeVisible();
  });

  test("page does not show error overlay", async ({ page }) => {
    // Wait for page to settle
    await page.waitForTimeout(2000);

    // Check there's no error overlay shown
    const bodyHtml = await page.locator("body").innerHTML();
    expect(bodyHtml).not.toContain("Effuse Error");
    expect(bodyHtml).not.toContain("ReferenceError:");
    expect(bodyHtml).not.toContain("TypeError:");
  });
});
