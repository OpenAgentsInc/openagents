/**
 * Mainview Page Object Model for E2E Tests
 *
 * Provides a clean interface for interacting with the OpenAgents mainview.
 */

import { test as base, expect, type Page, type Locator } from "@playwright/test";
import type { HudMessage } from "../../src/hud/protocol.js";
import { TEST_HTTP_PORT } from "../constants.js";

// ============================================================================
// Page Object: MainviewPage
// ============================================================================

export class MainviewPage {
  readonly page: Page;

  constructor(page: Page) {
    this.page = page;
  }

  // Navigation
  async goto(): Promise<void> {
    await this.page.goto(`http://localhost:${TEST_HTTP_PORT}`);
  }

  async waitForReady(): Promise<void> {
    await this.page.waitForSelector("#flow-svg");
    await this.page.waitForSelector("#ws-status.ws-connected", { timeout: 5000 });
  }

  // Locators
  get flowContainer(): Locator {
    return this.page.locator("#flow-container");
  }

  get flowSvg(): Locator {
    return this.page.locator("#flow-svg");
  }

  get flowCanvas(): Locator {
    return this.page.locator("#flow-canvas");
  }

  get flowContent(): Locator {
    return this.page.locator("#flow-content");
  }

  get resetButton(): Locator {
    return this.page.locator("#reset-btn");
  }

  get zoomLevelDisplay(): Locator {
    return this.page.locator("#zoom-level");
  }

  get testIndicator(): Locator {
    return this.page.locator("#test-indicator");
  }

  get wsStatus(): Locator {
    return this.page.locator("#ws-status");
  }

  get errorIndicator(): Locator {
    return this.page.locator("#error-indicator");
  }

  // Queries
  async getZoomLevel(): Promise<string> {
    return (await this.zoomLevelDisplay.textContent()) ?? "100%";
  }

  async getSvgContent(): Promise<string> {
    return (await this.flowSvg.innerHTML()) ?? "";
  }

  async getNodeCount(): Promise<number> {
    return await this.page.locator(".flow-node-group").count();
  }

  async getNode(nodeId: string): Promise<Locator> {
    return this.page.locator(`[data-node-id="${nodeId}"]`);
  }

  async isWsConnected(): Promise<boolean> {
    const cls = await this.wsStatus.getAttribute("class");
    return cls?.includes("ws-connected") ?? false;
  }

  // Interactions
  async pan(deltaX: number, deltaY: number): Promise<void> {
    const box = await this.flowContainer.boundingBox();
    if (!box) throw new Error("Flow container not visible");

    const startX = box.x + box.width / 2;
    const startY = box.y + box.height / 2;

    await this.page.mouse.move(startX, startY);
    await this.page.mouse.down();
    await this.page.mouse.move(startX + deltaX, startY + deltaY, { steps: 5 });
    await this.page.mouse.up();
  }

  async zoom(delta: number): Promise<void> {
    const box = await this.flowContainer.boundingBox();
    if (!box) throw new Error("Flow container not visible");

    const x = box.x + box.width / 2;
    const y = box.y + box.height / 2;

    await this.page.mouse.move(x, y);
    await this.page.mouse.wheel(0, delta);
  }

  async clickReset(): Promise<void> {
    await this.resetButton.click();
  }

  async waitForRender(timeout: number = 500): Promise<void> {
    await this.page.waitForTimeout(timeout);
  }
}

// ============================================================================
// Page Object: HudInjector
// ============================================================================

export class HudInjector {
  private baseUrl: string;
  private clientId: string | undefined;

  constructor(clientId: string | undefined = undefined, port: number = TEST_HTTP_PORT) {
    this.baseUrl = `http://localhost:${port}`;
    this.clientId = clientId;
  }

  async inject(message: HudMessage): Promise<void> {
    const headers: Record<string, string> = { "Content-Type": "application/json" };
    if (this.clientId) headers["X-Client-ID"] = this.clientId;

    const response = await fetch(`${this.baseUrl}/api/inject-hud`, {
      method: "POST",
      headers,
      body: JSON.stringify(message),
    });

    if (!response.ok) {
      const error = await response.text();
      throw new Error(`Failed to inject HUD message: ${error}`);
    }
  }

  async injectSequence(
    messages: HudMessage[],
    delayMs: number = 200
  ): Promise<void> {
    for (const msg of messages) {
      await this.inject(msg);
      await new Promise((r) => setTimeout(r, delayMs));
    }
  }

  /**
   * Inject raw/malformed data directly to WebSocket clients
   */
  async injectRaw(rawData: string): Promise<void> {
    const headers: Record<string, string> = {};
    if (this.clientId) headers["X-Client-ID"] = this.clientId;

    const response = await fetch(`${this.baseUrl}/api/inject-raw`, {
      method: "POST",
      headers,
      body: rawData,
    });

    if (!response.ok) {
      const error = await response.text();
      throw new Error(`Failed to inject raw message: ${error}`);
    }
  }

  /**
   * Disconnect all WebSocket clients (for testing resilience)
   */
  async disconnectAll(): Promise<number> {
    const response = await fetch(`${this.baseUrl}/api/disconnect-ws`, {
      method: "POST",
    });

    if (!response.ok) {
      const error = await response.text();
      throw new Error(`Failed to disconnect WebSocket clients: ${error}`);
    }

    const result = await response.json();
    return result.disconnected;
  }
}

// ============================================================================
// SVG Assertions Helper
// ============================================================================

export class SVGFlowAssertions {
  readonly page: Page;

  constructor(page: Page) {
    this.page = page;
  }

  get svg(): Locator {
    return this.page.locator("#flow-svg");
  }

  getNode(nodeId: string): Locator {
    return this.svg.locator(`[data-node-id="${nodeId}"]`);
  }

  getNodesByType(type: string): Locator {
    return this.svg.locator(`.flow-node-${type}`);
  }

  get apmWidget(): Locator {
    return this.svg.locator(".apm-widget");
  }

  getAPMValue(): Locator {
    return this.apmWidget.locator(".apm-value");
  }

  // Assertions
  async expectNodeExists(nodeId: string): Promise<void> {
    await expect(this.getNode(nodeId)).toBeVisible();
  }

  async expectNodeCount(count: number): Promise<void> {
    await expect(this.svg.locator(".flow-node-group")).toHaveCount(count);
  }

  async expectAPMVisible(): Promise<void> {
    await expect(this.apmWidget).toBeVisible();
  }

  async expectAPMValue(value: number): Promise<void> {
    const apmText = await this.getAPMValue().textContent();
    expect(apmText).toContain(`APM: ${value.toFixed(1)}`);
  }

  async waitForRender(timeout: number = 500): Promise<void> {
    await this.page.waitForTimeout(timeout);
  }
}

// ============================================================================
// Custom Test Fixture
// ============================================================================

type MainviewFixtures = {
  mainviewPage: MainviewPage;
  hudInjector: HudInjector;
  svgAssertions: SVGFlowAssertions;
};

export const test = base.extend<MainviewFixtures>({
  mainviewPage: async ({ page }, use) => {
    const mainview = new MainviewPage(page);
    await mainview.goto();
    await mainview.waitForReady();
    await use(mainview);
  },

  hudInjector: async ({ page, mainviewPage }, use) => {
    // Ensure mainview is loaded so __clientId is available
    await mainviewPage.waitForRender(0);
    const clientId = await page.evaluate(() => (window as unknown as { __clientId?: string }).__clientId);
    if (!clientId) {
      throw new Error("Failed to resolve clientId for HUD injector");
    }
    await use(new HudInjector(clientId));
  },

  svgAssertions: async ({ page }, use) => {
    await use(new SVGFlowAssertions(page));
  },
});

export { expect } from "@playwright/test";
