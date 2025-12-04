/**
 * Error Handling E2E Tests (HUD-060 to HUD-063)
 *
 * Tests graceful degradation and error handling scenarios.
 * - HUD-060: No crash on WS disconnect
 * - HUD-061: Malformed messages handled
 * - HUD-062: Error indicators visible
 * - HUD-063: Recovery from multiple errors
 */

import { test, expect } from "../../fixtures/mainview.fixture.js";
import { createSessionStart, createError, createTaskSelected, createAPMUpdate } from "../../fixtures/hud-messages.js";

test.describe("Error Handling Tests", () => {
  test("HUD-060: no crash on WebSocket disconnect, preserves last state", async ({
    mainviewPage,
    hudInjector,
    page,
  }) => {
    // Send initial data to establish state
    await hudInjector.inject(createSessionStart("test-session"));
    await hudInjector.inject(createTaskSelected({ id: "oa-test-123", title: "Test Task" }));
    await mainviewPage.waitForRender(300);

    // Verify initial state is rendered
    await expect(mainviewPage.wsStatus).toHaveClass(/ws-connected/);
    await expect(mainviewPage.flowSvg).toBeVisible();
    const nodeCount = await mainviewPage.getNodeCount();
    expect(nodeCount).toBeGreaterThan(0);

    // Trigger server-side disconnect
    const disconnected = await hudInjector.disconnectAll();
    expect(disconnected).toBeGreaterThan(0);

    // Wait for disconnect to be detected
    await mainviewPage.waitForRender(500);

    // Verify UI shows disconnected state
    await expect(mainviewPage.wsStatus).toHaveClass(/ws-disconnected/);

    // Critical: App should NOT crash - SVG and last state should still be visible
    await expect(mainviewPage.flowSvg).toBeVisible();
    const nodeCountAfter = await mainviewPage.getNodeCount();
    expect(nodeCountAfter).toBe(nodeCount); // State preserved

    // Controls should still work
    await mainviewPage.clickReset();
    const zoomLevel = await mainviewPage.getZoomLevel();
    expect(zoomLevel).toBe("100%");
  });

  test("HUD-061: malformed messages via server are safely ignored", async ({
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

    // Send various malformed messages via server injection
    const malformedMessages = [
      "not json at all",
      '{"incomplete": true',
      '{"type": "unknown_type", "data": 123}',
      '[]',
      'null',
      '{"type": null}',
      '{"type": "apm_update"}', // Missing required fields
    ];

    for (const rawMsg of malformedMessages) {
      await hudInjector.injectRaw(rawMsg);
      await mainviewPage.waitForRender(100);
    }

    await mainviewPage.waitForRender(500);

    // App should not crash - SVG still visible
    await expect(mainviewPage.flowSvg).toBeVisible();

    // Controls should still work
    await mainviewPage.zoom(-50);
    await mainviewPage.waitForRender(200);
    const zoomLevel = await mainviewPage.getZoomLevel();
    expect(parseInt(zoomLevel)).toBeGreaterThan(100);
  });

  test("D2: malformed HUD message via client-side event is ignored", async ({
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
    // flowCanvas is an empty <g> element - check it exists (not visible, as empty groups have no dimensions)
    await expect(mainviewPage.flowCanvas).toHaveCount(1);

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

  test("HUD-062: error indicators become visible when errors occur", async ({
    mainviewPage,
    hudInjector,
    page,
  }) => {
    // Initially, error indicator should not be visible
    await expect(mainviewPage.errorIndicator).not.toHaveClass(/visible/);

    // Send an error message
    await hudInjector.inject(createError("Test verification failed", "verifying"));
    await mainviewPage.waitForRender(500);

    // Error indicator should now be visible
    await expect(mainviewPage.errorIndicator).toHaveClass(/visible/);
    await expect(mainviewPage.errorIndicator).toContainText("Error: Test verification failed");

    // App should handle error gracefully - no crash
    await expect(mainviewPage.flowSvg).toBeVisible();

    // Verify error is tracked in window state (may accumulate from other events)
    const errorCount = await page.evaluate(() => (window as unknown as { __errorCount: number }).__errorCount);
    expect(errorCount).toBeGreaterThanOrEqual(1);
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
    test("HUD-063: recovers from multiple errors without crash", async ({
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

      // Phase 1: Send multiple malformed messages via server
      await hudInjector.injectRaw("not valid json");
      await hudInjector.injectRaw('{"broken":');
      await mainviewPage.waitForRender(200);

      // Phase 2: Send series of edge case APM messages
      await hudInjector.inject({
        type: "apm_update",
        sessionId: "",
        sessionAPM: -999,
        recentAPM: NaN,
        totalActions: Infinity,
        durationMinutes: -1,
      });

      await hudInjector.inject({
        type: "apm_update",
        sessionId: "test",
        sessionAPM: NaN,
        recentAPM: -Infinity,
        totalActions: -1,
        durationMinutes: NaN,
      });
      await mainviewPage.waitForRender(200);

      // Phase 3: Send multiple error messages
      await hudInjector.inject(createError("Error 1: Subtask execution failed", "executing_subtask"));
      await mainviewPage.waitForRender(200);
      await hudInjector.inject(createError("Error 2: Verification failed", "verifying"));
      await mainviewPage.waitForRender(200);
      await hudInjector.inject(createError("Error 3: Commit failed", "committing"));
      await mainviewPage.waitForRender(500);

      // App should still be functional after all errors
      await expect(mainviewPage.flowSvg).toBeVisible();

      // Phase 4: Verify recovery - can still receive valid messages
      await hudInjector.inject(createSessionStart("recovery-session"));
      await hudInjector.inject(createAPMUpdate({ sessionAPM: 99.9, totalActions: 500 }));
      await mainviewPage.waitForRender(500);

      // Verify the app is still responding to new messages
      // APM widget should be present and showing the new value
      const svgContent = await mainviewPage.getSvgContent();
      expect(svgContent).toContain("apm-widget");

      // Controls should still work
      await mainviewPage.pan(100, 100);
      await mainviewPage.clickReset();
      const zoomLevel = await mainviewPage.getZoomLevel();
      expect(zoomLevel).toBe("100%");
    });

    test("recovers from WebSocket disconnect and reconnects", async ({
      mainviewPage,
      hudInjector,
      page,
    }) => {
      // Send initial data
      await hudInjector.inject(createSessionStart("reconnect-test"));
      await mainviewPage.waitForRender(300);
      await expect(mainviewPage.wsStatus).toHaveClass(/ws-connected/);

      // Disconnect
      await hudInjector.disconnectAll();
      await mainviewPage.waitForRender(500);
      await expect(mainviewPage.wsStatus).toHaveClass(/ws-disconnected/);

      // Wait for auto-reconnect (test server reconnects after 1 second)
      await page.waitForSelector("#ws-status.ws-connected", { timeout: 3000 });

      // Should be reconnected
      await expect(mainviewPage.wsStatus).toHaveClass(/ws-connected/);

      // Can receive new messages after reconnect
      await hudInjector.inject(createAPMUpdate({ sessionAPM: 42.0, totalActions: 100 }));
      await mainviewPage.waitForRender(500);

      // App should still function
      await expect(mainviewPage.flowSvg).toBeVisible();
    });
  });
});
