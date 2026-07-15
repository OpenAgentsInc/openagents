import { createHash } from "node:crypto";
import { chromium } from "playwright";

const baseUrl = process.argv[2] ?? "https://openagents.com";
const sampleCount = Number(process.argv[3] ?? 12);
const routes = ["/astro", "/tanstack"];

const median = (values) => {
  const sorted = [...values].sort((left, right) => left - right);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0 ? (sorted[middle - 1] + sorted[middle]) / 2 : sorted[middle];
};

const percentile = (values, fraction) => {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * fraction) - 1)];
};

const summarize = (samples) => {
  const metric = (key) => samples.map((sample) => sample[key]);
  return {
    samples: samples.length,
    ttfbMedianMs: median(metric("ttfbMs")),
    fcpMedianMs: median(metric("fcpMs")),
    fcpP75Ms: percentile(metric("fcpMs"), 0.75),
    lcpMedianMs: median(metric("lcpMs")),
    loadMedianMs: median(metric("loadMs")),
    networkTransferMedianBytes: median(metric("networkTransferBytes")),
    payloadMedianBytes: median(metric("payloadBytes")),
    scriptMedianBytes: median(metric("scriptBytes")),
    cssMedianBytes: median(metric("cssBytes")),
    requestMedianCount: median(metric("requestCount")),
    scriptRequestMedianCount: median(metric("scriptRequestCount")),
  };
};

const browser = await chromium.launch({ headless: true });

const measure = async (route, mode) => {
  const context = await browser.newContext({
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 1,
  });
  const page = await context.newPage();
  await page.addInitScript(() => {
    window.__openagentsLargestContentfulPaint = 0;
    new PerformanceObserver((list) => {
      const latest = list.getEntries().at(-1);
      if (latest !== undefined) {
        window.__openagentsLargestContentfulPaint = latest.startTime;
      }
    }).observe({ type: "largest-contentful-paint", buffered: true });
  });
  const client = await context.newCDPSession(page);
  await client.send("Network.enable");
  await client.send("Network.setCacheDisabled", { cacheDisabled: mode === "cold" });
  await client.send("Emulation.setCPUThrottlingRate", { rate: 4 });

  if (mode === "warm") {
    await page.goto(`${baseUrl}${route}`, { waitUntil: "networkidle" });
  }
  await page.goto(`${baseUrl}${route}`, { waitUntil: "load" });
  await page.waitForTimeout(250);

  const result = await page.evaluate(() => {
    const navigation = performance.getEntriesByType("navigation")[0];
    const resources = performance.getEntriesByType("resource");
    const paints = performance.getEntriesByType("paint");
    const firstContentfulPaint = paints.find((entry) => entry.name === "first-contentful-paint");
    const scripts = resources.filter((entry) => entry.initiatorType === "script");
    const styles = resources.filter(
      (entry) => entry.initiatorType === "css" || entry.name.endsWith(".css"),
    );
    const transferredBytes = (entries) =>
      entries.reduce((total, entry) => total + (entry.transferSize || 0), 0);
    const encodedBytes = (entries) =>
      entries.reduce((total, entry) => total + (entry.encodedBodySize || 0), 0);
    const shell = document.querySelector(".site-shell");
    return {
      ttfbMs: navigation.responseStart - navigation.requestStart,
      fcpMs: firstContentfulPaint?.startTime ?? navigation.loadEventEnd,
      lcpMs:
        window.__openagentsLargestContentfulPaint ||
        firstContentfulPaint?.startTime ||
        navigation.loadEventEnd,
      loadMs: navigation.loadEventEnd,
      networkTransferBytes:
        (navigation.transferSize || 0) + transferredBytes(resources),
      payloadBytes: navigation.encodedBodySize + encodedBytes(resources),
      scriptBytes: encodedBytes(scripts),
      cssBytes: encodedBytes(styles),
      requestCount: resources.length + 1,
      scriptRequestCount: scripts.length,
      shellHtml: shell?.outerHTML ?? "",
      elementCount: shell?.querySelectorAll("*").length ?? 0,
    };
  });
  await context.close();
  return result;
};

const results = {
  cold: Object.fromEntries(routes.map((route) => [route, []])),
  warm: Object.fromEntries(routes.map((route) => [route, []])),
};

for (const mode of ["cold", "warm"]) {
  for (let index = 0; index < sampleCount; index += 1) {
    const orderedRoutes = index % 2 === 0 ? routes : [...routes].reverse();
    for (const route of orderedRoutes) {
      results[mode][route].push(await measure(route, mode));
    }
  }
}

await browser.close();

const firstAstro = results.cold["/astro"][0];
const firstTanStack = results.cold["/tanstack"][0];
const normalizedHash = (html) =>
  createHash("sha256").update(html.replace(/\s+/g, " ")).digest("hex");

const report = {
  baseUrl,
  generatedAt: new Date().toISOString(),
  environment: "Chromium mobile viewport, 4x CPU throttle, production network",
  parity: {
    elementCountEqual: firstAstro.elementCount === firstTanStack.elementCount,
    astroElementCount: firstAstro.elementCount,
    tanstackElementCount: firstTanStack.elementCount,
    normalizedShellHashEqual:
      normalizedHash(firstAstro.shellHtml) === normalizedHash(firstTanStack.shellHtml),
  },
  cold: Object.fromEntries(routes.map((route) => [route, summarize(results.cold[route])])),
  warm: Object.fromEntries(routes.map((route) => [route, summarize(results.warm[route])])),
};

console.log(JSON.stringify(report, null, 2));
