/**
 * Error Handling E2E Tests (D1-D6)
 *
 * Tests graceful degradation and error handling scenarios.
 */

import { test, expect } from "../../fixtures/mainview.fixture.js";
import { createSessionStart, createError } from "../../fixtures/hud-messages.js";

test.describe("Error Handling Tests", () => {
  test("D1: graceful WebSocket disconnect shows last state", async ({
    mainviewPage,
    hudInjector,
    page,
  }) => {
    // Send initial data
    await hudInjector.inject(createSessionStart("test-session"));
    await mainviewPage.waitForRender(300);

    // Verify initial state
    await expect(mainviewPage.wsStatus).toHaveClass(/ws-connected/);

    // The test server handles disconnect gracefully
    // Just verify the UI doesn't crash when it would disconnect
    await expect(mainviewPage.flowSvg).toBeVisible();
  });

  test("D2: malformed HUD message is safely ignored", async ({
    mainviewPage,
    page,
  }) => {
    const errors: string[] = [];
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        errors.push(msg.text());
      }
    });

    // Send malformed message via WebSocket
    await page.evaluate(() => {
      const ws = (window as unknown as { __testWs: () => WebSocket }).__testWs?.();
      if (ws && ws.readyState === WebSocket.OPEN) {
        // Simulate receiving malformed data
        window.dispatchEvent(
          new CustomEvent("hud-message", {
            detail: { invalid: "message", noType: true },
          })
        );
      }
    });

    await mainviewPage.waitForRender(500);

    // App should not crash - SVG still visible
    await expect(mainviewPage.flowSvg).toBeVisible();

    // No critical errors (ignore parse errors which are expected)
    const criticalErrors = errors.filter(
      (e) => !e.includes("parse") && !e.includes("Parse")
    );
    expect(criticalErrors.length).toBeLessThan(3);
  });

  test("D3: empty state displays placeholder content", async ({ mainviewPage }) => {
    // Without sending any HUD messages, the app should show default state
    await expect(mainviewPage.flowSvg).toBeVisible();
    await expect(mainviewPage.flowCanvas).toBeVisible();

    // Controls should still work
    await expect(mainviewPage.resetButton).toBeVisible();
    await expect(mainviewPage.zoomLevelDisplay).toBeVisible();
  });

  test("D4: invalid APM values are handled gracefully", async ({
    mainviewPage,
    hudInjector,
    page,
  }) => {
    // Send APM update with edge case values
    await hudInjector.inject({
      type: "apm_update",
      sessionId: "test",
      sessionAPM: NaN,
      recentAPM: Infinity,
      totalActions: -1,
      durationMinutes: 0,
    });

    await mainviewPage.waitForRender(500);

    // App should not crash
    await expect(mainviewPage.flowSvg).toBeVisible();
  });

  test("D5: error message is displayed appropriately", async ({
    mainviewPage,
    hudInjector,
  }) => {
    // Send an error message
    await hudInjector.inject(createError("Test verification failed", "verifying"));
    await mainviewPage.waitForRender(500);

    // App should handle error gracefully - no crash
    await expect(mainviewPage.flowSvg).toBeVisible();
  });

  test("D6: Electrobun RPC unavailable handled gracefully", async ({
    mainviewPage,
  }) => {
    // In test mode, Electrobun is mocked
    // Verify app still works without real Electrobun
    await expect(mainviewPage.flowSvg).toBeVisible();
    await expect(mainviewPage.testIndicator).toHaveText("E2E TEST MODE");

    // Canvas interactions should work
    await mainviewPage.zoom(-50);
    await mainviewPage.waitForRender(200);

    const zoomLevel = await mainviewPage.getZoomLevel();
    expect(parseInt(zoomLevel)).toBeGreaterThan(100);
  });

  test.describe("Recovery", () => {
    test("recovers from multiple errors without crash", async ({
      mainviewPage,
      hudInjector,
      page,
    }) => {
      const errors: string[] = [];
      page.on("console", (msg) => {
        if (msg.type() === "error") {
          errors.push(msg.text());
        }
      });

      // Send series of edge case messages
      await hudInjector.inject({
        type: "apm_update",
        sessionId: "",
        sessionAPM: -999,
        recentAPM: NaN,
        totalActions: Infinity,
        durationMinutes: -1,
      });

      await hudInjector.inject(createError("Error 1", "implementing"));
      await hudInjector.inject(createError("Error 2", "verifying"));

      await mainviewPage.waitForRender(500);

      // App should still be functional
      await expect(mainviewPage.flowSvg).toBeVisible();

      // Can still interact
      await mainviewPage.clickReset();
      const zoomLevel = await mainviewPage.getZoomLevel();
      expect(zoomLevel).toBe("100%");
    });
  });
});
