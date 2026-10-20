// Run against build-web-demo.sh --with-page output under openagents browser run.
const assert = require("node:assert/strict");
const { chromium } = require("playwright");

(async () => {
  const browser = await chromium.connectOverCDP(process.env.OPENAGENTS_CHROME_WS);
  try {
    const page = await browser.contexts()[0].newPage();
    await page.setViewportSize({ width: 1440, height: 1000 });
    const errors = [];
    const downloads = [];
    page.on("pageerror", error => errors.push(error.message));
    page.on("request", request => {
      if (request.url().includes("/visuals/") && request.url().endsWith(".wasm")) downloads.push(request.url());
    });
    await page.goto(process.env.OPENAGENTS_VISUALS_URL);
    await page.waitForSelector("canvas");
    assert.equal(downloads.length, 0, "a chat without visuals does not fetch the renderer");
    // The recorded demo's Agent visuals chat is the last sidebar row.
    await page.mouse.click(100, 930);
    await page.waitForFunction(() => document.querySelectorAll("canvas[data-oa-visual]").length >= 3, null, { timeout: 120000 });
    const slot = page.locator('div[id^=oa-visual-][aria-label="Latency dashboard"]');
    await page.waitForFunction(() => [...document.querySelectorAll("div[id^=oa-visual-]")].some(e => e.offsetHeight > 400));
    const before = await slot.boundingBox();
    assert(before && before.height > 400, "the dashboard reports its content height");
    // The chart range and table toggle are the desktop's actual controls.
    await page.mouse.click(before.x + 63, before.y + 82);
    await page.mouse.click(before.x + 16, before.y + before.height - 45);
    await page.waitForFunction(height => [...document.querySelectorAll("div[id^=oa-visual-]")].some(e => e.offsetHeight > height + 100), before.height);
    assert.equal(downloads.length, 1, "all rows share one downloaded visual module");
    if (process.env.OPENAGENTS_VISUALS_CAPTURE) await page.screenshot({ path: process.env.OPENAGENTS_VISUALS_CAPTURE });
    // Scrolling over the chart forwards an unused wheel event to the transcript.
    await page.mouse.move(before.x + 100, before.y + 100);
    await page.mouse.wheel(0, -1000);
    await page.waitForTimeout(500);
    await page.setViewportSize({ width: 1100, height: 900 });
    await page.waitForTimeout(500);
    await page.mouse.click(100, 140);
    await page.waitForFunction(() => document.querySelectorAll("canvas[data-oa-visual]").length === 0);
    assert.equal(await page.locator("input").count(), 1, "closing inline windows removes their hidden inputs");
    assert.deepEqual(errors, [], "no callback outlives a closed visual window");
    console.log("PASS: lazy download, multiple visual rows, controls, content height, scrolling, resizing, and chat cleanup");
  } finally {
    await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
