// Check deliberate shutdown against real asynchronous WebGPU device loss.
import assert from "node:assert/strict";
import { chromium } from "playwright";
import { createServer } from "vite";
const server = await createServer({
  configFile: false,
  root: new URL("..", import.meta.url).pathname,
  appType: "custom",
  server: { host: "127.0.0.1", port: 0 },
});
server.middlewares.use((req, res, next) => {
  if (req.url !== "/") return next();
  res.setHeader("content-type", "text/html");
  res.end("<title>Renderer shutdown qualification</title>");
});
await server.listen();
const browser = await chromium.connectOverCDP(
    process.env.OPENAGENTS_CHROME_WS ||
      `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
  ),
  context = await browser.newContext();
try {
  const page = await context.newPage(),
    pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.goto(server.resolvedUrls.local[0]);
  const result = await page.evaluate(async () => {
    const { Renderer } = await import("/packages/renderer/renderer.ts"),
      savedInitialize = Renderer.prototype.initialize,
      cases = [];
    try {
      for (const mode of ["startup-failure", "shutdown", "unexpected-loss"]) {
        const reports = [],
          failure = new RangeError("Owned startup failure"),
          canvas = new OffscreenCanvas(2, 2);
        let renderer,
          caught,
          resourceDisposals = 0;
        Renderer.prototype.initialize = async function () {
          renderer = this;
          if (mode === "startup-failure") throw failure;
        };
        try {
          await Renderer.create(canvas, {}, (message) => reports.push(message));
        } catch (error) {
          caught = error;
        }
        if (!renderer) throw Error("The renderer did not acquire a device.");
        if (mode !== "startup-failure") {
          renderer.buffers.add({
            destroy() {
              resourceDisposals++;
            },
          });
          if (mode === "unexpected-loss") renderer.device.destroy();
          else {
            renderer.dispose();
            renderer.dispose();
          }
        }
        const loss = await renderer.device.lost;
        await new Promise((resolve) => setTimeout(resolve, 0));
        if (mode === "unexpected-loss") {
          if (
            reports.length !== 1 ||
            !reports[0].startsWith("Graphics device stopped: ") ||
            !renderer.lost
          )
            throw Error("Unexpected live device loss was hidden.");
          renderer.dispose();
        } else {
          renderer.report("An obsolete loading notification");
          if (reports.length || renderer.lost)
            throw Error("Intentional shutdown reported a graphics failure.");
          if (mode === "startup-failure" && caught !== failure)
            throw Error("Cleanup replaced the original startup exception.");
          if (mode === "shutdown" && resourceDisposals !== 1)
            throw Error("Repeated shutdown disposed resources more than once.");
          if (renderer.render({}, {}, 0) !== false)
            throw Error("A disposed renderer submitted a frame.");
          if (await renderer.replaceShader("No work after shutdown"))
            throw Error("A disposed renderer activated a shader.");
        }
        cases.push({
          mode,
          reason: loss.reason,
          reports,
          resourceDisposals,
          originalFailurePreserved:
            mode !== "startup-failure" || caught === failure,
        });
      }
    } finally {
      Renderer.prototype.initialize = savedInitialize;
    }
    return { cases };
  });
  assert.deepEqual(pageErrors, []);
  assert.equal(result.cases.length, 3);
  console.log(JSON.stringify({ ok: true, ...result }, null, 2));
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
