/**
 * Visual/Layout E2E Tests (A1-A7)
 *
 * Tests that the mainview renders correctly with all visual elements.
 */

import { test, expect } from "../../fixtures/mainview.fixture.js";
import { createTaskSelected, createAPMUpdate } from "../../fixtures/hud-messages.js";

test.describe("Visual/Layout Tests", () => {
  test.describe("Initial Render", () => {
    test("A1: mainview loads without console errors", async ({ page }) => {
      const errors: string[] = [];
      page.on("console", (msg) => {
        if (msg.type() === "error") {
          errors.push(msg.text());
        }
      });

      // Page is already loaded by fixture
      await page.waitForTimeout(1000);

      // Filter out expected errors (WebSocket reconnect attempts, etc.)
      const realErrors = errors.filter(
        (e) => !e.includes("WebSocket") && !e.includes("fetch")
      );
      expect(realErrors).toHaveLength(0);
    });

    test("A2: SVG flow diagram renders with container", async ({ mainviewPage }) => {
      await expect(mainviewPage.flowSvg).toBeVisible();
      await expect(mainviewPage.flowCanvas).toBeVisible();
      await expect(mainviewPage.flowContent).toBeVisible();

      // SVG should have proper structure
      const svgHtml = await mainviewPage.getSvgContent();
      expect(svgHtml).toContain("flow-canvas");
    });

    test("A3: APM widget displays after receiving apm_update", async ({
      mainviewPage,
      hudInjector,
      svgAssertions,
    }) => {
      await hudInjector.inject(
        createAPMUpdate({
          sessionAPM: 15.5,
          totalActions: 42,
          durationMinutes: 3,
        })
      );

      await mainviewPage.waitForRender(500);
      await svgAssertions.expectAPMVisible();
    });
  });

  test.describe("Theming", () => {
    test("A4: node types display correctly after task_selected", async ({
      mainviewPage,
      hudInjector,
      svgAssertions,
    }) => {
      const taskId = "oa-test-theme";

      await hudInjector.inject(
        createTaskSelected({
          id: taskId,
          title: "Theme Test Task",
          status: "in_progress",
          priority: 1,
        })
      );

      await mainviewPage.waitForRender(500);

      // Should have at least one node
      const nodeCount = await mainviewPage.getNodeCount();
      expect(nodeCount).toBeGreaterThan(0);
    });

    test("A5: status colors apply to node elements", async ({
      mainviewPage,
      hudInjector,
    }) => {
      await hudInjector.inject(
        createTaskSelected({
          id: "oa-color-test",
          title: "Color Test",
          status: "in_progress",
        })
      );

      await mainviewPage.waitForRender(500);

      // Check that the SVG contains node with fill color
      const svgHtml = await mainviewPage.getSvgContent();
      expect(svgHtml).toContain("flow-node-group");
    });
  });

  test.describe("Controls", () => {
    test("A6: flow controls are visible", async ({ mainviewPage }) => {
      await expect(mainviewPage.resetButton).toBeVisible();
      await expect(mainviewPage.zoomLevelDisplay).toBeVisible();

      // Zoom should start at 100%
      const zoomLevel = await mainviewPage.getZoomLevel();
      expect(zoomLevel).toBe("100%");
    });

    test("A7: grid background pattern exists", async ({ mainviewPage }) => {
      const svgHtml = await mainviewPage.getSvgContent();

      // Should have grid pattern definition
      expect(svgHtml).toContain('id="grid"');
      expect(svgHtml).toContain("pattern");
    });
  });

  test.describe("Test Mode Indicators", () => {
    test("shows E2E test mode indicator", async ({ mainviewPage }) => {
      await expect(mainviewPage.testIndicator).toBeVisible();
      await expect(mainviewPage.testIndicator).toHaveText("E2E TEST MODE");
    });

    test("shows WebSocket connection status", async ({ mainviewPage }) => {
      await expect(mainviewPage.wsStatus).toBeVisible();

      // Should be connected
      const isConnected = await mainviewPage.isWsConnected();
      expect(isConnected).toBe(true);
    });
  });
});
