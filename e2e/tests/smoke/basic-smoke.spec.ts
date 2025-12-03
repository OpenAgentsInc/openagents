/**
 * Smoke coverage for core HUD P0 stories:
 * HUD-001 (app launch), HUD-002 (flow renders),
 * HUD-010 (canvas pan), HUD-012 (reset view).
 */

import { test, expect } from "../../fixtures/mainview.fixture.js";

test.describe("HUD Smoke (HUD-001/002/010/012)", () => {
  test("loads HUD and supports pan/reset", async ({ mainviewPage }) => {
    // App launch + render
    await expect(mainviewPage.flowSvg).toBeVisible();
    await expect(mainviewPage.flowCanvas).toBeVisible();
    await expect(mainviewPage.flowContent).toBeVisible();
    await expect(mainviewPage.resetButton).toBeVisible();

    // WebSocket should be connected for HUD updates
    expect(await mainviewPage.isWsConnected()).toBe(true);

    // Canvas pan
    const initialTransform = await mainviewPage.flowCanvas.getAttribute("transform");
    await mainviewPage.pan(80, 40);
    await mainviewPage.waitForRender(150);
    const afterPanTransform = await mainviewPage.flowCanvas.getAttribute("transform");
    expect(afterPanTransform).not.toBe(initialTransform);

    // Reset view
    await mainviewPage.clickReset();
    await mainviewPage.waitForRender(150);
    expect(await mainviewPage.getZoomLevel()).toBe("100%");
    expect(await mainviewPage.flowCanvas.getAttribute("transform")).toBe(initialTransform);
  });
});
