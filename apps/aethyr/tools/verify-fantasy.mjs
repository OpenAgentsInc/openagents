// Private asset review using the production renderer. Captures stay outside Git.
import assert from "node:assert/strict";
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { resolve, sep } from "node:path";
import { createReadStream } from "node:fs";
import { createServer } from "vite";
import { chromium } from "playwright";

const root = resolve(process.env.AF_FANTASY_ASSETS ?? "");
const output = resolve(process.env.AF_CAPTURE_DIR ?? "");
const checkout = resolve(new URL("..", import.meta.url).pathname);
for (const value of [root, output])
  assert.ok(
    value !== checkout && !value.startsWith(checkout + sep),
    "Use private paths outside Git",
  );
assert.ok(process.env.AF_FANTASY_ASSETS && process.env.AF_CAPTURE_DIR);
await mkdir(output, { recursive: true });
const pointer = JSON.parse(
  await readFile(resolve(root, "fantasy-current.json"), "utf8"),
);
const catalog = JSON.parse(
  await readFile(resolve(root, pointer.catalog), "utf8"),
);
const reviewScene = JSON.parse(
  await readFile(resolve(root, pointer.reviewScene), "utf8"),
);
const requests = new Map();
const server = await createServer({
  configFile: false,
  root: checkout,
  appType: "custom",
  server: { host: "127.0.0.1", port: 0 },
});
server.middlewares.use((req, res, next) => {
  if (req.url === "/") {
    res.setHeader("Content-Type", "text/html");
    res.end(
      '<!doctype html><title>Fantasy Props and original models — private review</title><style>body{margin:0;background:#171b1c;color:white;font:16px system-ui}canvas{width:100vw;height:100vh;display:block}#label{position:fixed;bottom:20px;left:24px;text-shadow:0 2px 4px black}</style><canvas></canvas><p id="label"></p>',
    );
  } else if (req.url?.startsWith("/private-assets/")) {
    requests.set(req.url, (requests.get(req.url) ?? 0) + 1);
    const file = resolve(root, req.url.slice("/private-assets/".length));
    if (!file.startsWith(root + sep)) {
      res.statusCode = 403;
      res.end();
      return;
    }
    res.setHeader(
      "Content-Type",
      file.endsWith(".json")
        ? "application/json"
        : file.endsWith(".png")
          ? "image/png"
          : "application/octet-stream",
    );
    const stream = createReadStream(file);
    stream.on("error", () => {
      res.statusCode = 404;
      res.end();
    });
    stream.pipe(res);
  } else next();
});
await server.listen();
const browser = await chromium.connectOverCDP(
  process.env.OPENAGENTS_CHROME_WS ||
    `http://127.0.0.1:${process.env.OPENAGENTS_CHROME_PORT}`,
);
const context = await browser.newContext({
  viewport: { width: 1440, height: 900 },
});
const errors = [];
try {
  const page = await context.newPage();
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto(server.resolvedUrls.local[0]);
  const result = await page.evaluate(
    async ({ pointer, catalog }) => {
      const { Renderer } = await import("/packages/renderer/renderer.ts");
      const { DEFAULT_ENVIRONMENT } =
        await import("/packages/editor-services/protocol.ts");
      const scene = await (
        await fetch("/private-assets/" + pointer.reviewScene)
      ).json();
      const canvas = document.querySelector("canvas");
      const offscreen = canvas.transferControlToOffscreen();
      const messages = [];
      const renderer = await Renderer.create(offscreen, scene, (message) =>
        messages.push(message),
      );
      renderer.resize(1440, 900);
      const env = {
        ...DEFAULT_ENVIRONMENT,
        sunAzimuth: 155,
        sunElevation: 48,
        sunIntensity: 25,
        ambientIntensity: 1,
        fogDensity: 0,
        cloudCoverage: 0,
        bloomStrength: 0,
      };
      window.review = { renderer, scene, env, catalog };
      return {
        messages,
        assetCount: catalog.assets.length,
        vertices: scene.vertexCount,
        adapter: renderer.device.adapterInfo
          ? {
              vendor: renderer.device.adapterInfo.vendor,
              architecture: renderer.device.adapterInfo.architecture,
              device: renderer.device.adapterInfo.device,
              description: renderer.device.adapterInfo.description,
            }
          : undefined,
      };
    },
    { pointer, catalog },
  );
  const shots = [
    {
      name: "contact-sheet",
      eye: [0, 16, 27],
      target: [0, 0, -6],
      label:
        "Fantasy Props · 24 kit assets, five original spooky models, six licensed building modules",
    },
  ];
  for (const id of [
    "custom:haunt:werewolf",
    "custom:haunt:pumpkin",
    "custom:haunt:ghost",
    "custom:haunt:coffin",
    "custom:haunt:gravestone",
    "quaternius:fantasy-standard:cauldron",
    "quaternius:fantasy-standard:chest_wood",
    "medieval-town:wall-4",
  ]) {
    const index = catalog.assets.findIndex((a) => a.id === id),
      a = catalog.assets[index];
    const placement = reviewScene.instances[index].model;
    const x = placement[12] + (a.bounds.min[0] + a.bounds.max[0]) / 2,
      z = placement[14] + (a.bounds.min[2] + a.bounds.max[2]) / 2;
    const h = a.bounds.max[1] - a.bounds.min[1],
      radius = Math.max(1, h) * 1.7;
    shots.push({
      name: id.split(":").at(-1),
      eye: [x + radius * 0.6, h * 0.65, z + radius],
      target: [x, h * 0.48, z],
      label: `${a.name} · ${a.origin}`,
    });
  }
  for (const shot of shots) {
    await page.evaluate(async (shot) => {
      const { renderer, env } = window.review;
      const d = shot.target.map((v, i) => v - shot.eye[i]);
      const camera = {
        eye: shot.eye,
        yaw: Math.atan2(d[0], -d[2]),
        pitch: Math.atan2(d[1], Math.hypot(d[0], d[2])),
        fov: (55 * Math.PI) / 180,
      };
      for (let i = 0; i < 4; i++) {
        renderer.render(camera, env, 0);
        await renderer.device.queue.onSubmittedWorkDone();
      }
      document.querySelector("#label").textContent = shot.label;
    }, shot);
    await page.screenshot({ path: resolve(output, shot.name + ".png") });
  }
  assert.deepEqual(errors, []);
  const graphicsErrors = await page.evaluate(() => window.review.renderer.lost);
  assert.equal(graphicsErrors, false);
  assert.ok(!result.messages.some((s) => /Graphics:|stopped:/.test(s)));
  await page.evaluate(() => window.review.renderer.dispose());
  const evidence = {
    ...result,
    browser: await browser.version(),
    viewport: [1440, 900],
    catalogSha256: pointer.catalogSha256,
    shots: shots.map((s) => s.name),
    errors,
    assetRequests: Object.fromEntries(requests),
  };
  await writeFile(
    resolve(output, "fantasy-browser.json"),
    JSON.stringify(evidence, null, 2) + "\n",
  );
  console.log(
    JSON.stringify({ ...evidence, messages: [evidence.messages.at(-1)] }),
  );
} finally {
  await context.close();
  await browser.close();
  await server.close();
}
