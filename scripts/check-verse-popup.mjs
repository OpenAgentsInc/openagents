// Independent CDP interception reproducer retained for #12561 qualification.
// Run with openagents browser run -- node scripts/check-verse-popup.mjs.
import assert from "node:assert/strict";
import { createRequire } from "node:module";
const { chromium } = createRequire(import.meta.url)("playwright");
const browser = await chromium.connectOverCDP(process.env.OPENAGENTS_CHROME_WS);
const context = browser.contexts()[0];
let fulfilled = false;
try {
  await context.route("https://openagents.com/**", async (route) => {
    const pathname = new URL(route.request().url()).pathname;
    await route.fulfill(
      pathname === "/redirect"
        ? { status: 303, headers: { location: "/done" }, body: "" }
        : {
            contentType: "text/html",
            body:
              pathname === "/done"
                ? "Done"
                : `<button onclick="window.open('/redirect','probe','popup,width=560,height=760')">Open</button>`,
          },
    );
    if (pathname === "/redirect") fulfilled = true;
  });
  const page = await context.newPage();
  await page.goto("https://openagents.com/probe");
  const [popup] = await Promise.all([
    context.waitForEvent("page", { timeout: 10000 }),
    page.click("button"),
  ]);
  await popup.waitForURL("**/done");
  assert.equal(await popup.locator("body").innerText(), "Done");
  console.log(
    JSON.stringify({ redirect_fulfilled: fulfilled, popup_completed: true }),
  );
} catch (error) {
  console.log(
    JSON.stringify({
      redirect_fulfilled: fulfilled,
      popup_completed: false,
      error: error.message,
    }),
  );
  process.exitCode = 1;
} finally {
  await context.close();
  await browser.close();
}
