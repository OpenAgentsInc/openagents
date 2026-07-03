#!/usr/bin/env bun
// Site Speed Lane — Mode L lab harness for the openagents.com landing page.
// Spec: docs/fable/2026-07-02-site-speed-lane-spec.md (schema
// openagents.site_speed.run_report.v1). Read-only against the target: issues
// only visitor-shaped GETs; blocking experiments abort requests client-side.
//
// Usage:
//   bun apps/openagents.com/scripts/site-speed-landing.ts \
//     [--origin https://openagents.com] [--route-set landing|funnel|all] [--runs 3] [--settle-ms 9000] \
//     [--profiles desktop-fast,mobile-mid] [--variants baseline,block-counter,block-thirdparty] \
//     [--out var/site-speed] [--fail-on-budget]

import { mkdir, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { chromium, type Browser, type BrowserContext, type Page } from "playwright"

type ProfileName = "desktop-fast" | "mobile-mid"
type VariantName = "baseline" | "block-counter" | "block-thirdparty"
type RouteSetName = "landing" | "funnel" | "all"
type RouteTargetName = "landing-home" | "business" | "business-preview"

type CliOptions = {
  readonly origin: string
  readonly routeSet: RouteSetName
  readonly routeTargets: readonly RouteTarget[]
  readonly runs: number
  readonly settleMs: number
  readonly profiles: readonly ProfileName[]
  readonly variants: readonly VariantName[]
  readonly outDir: string
  readonly failOnBudget: boolean
}

export type RouteTarget = {
  readonly name: RouteTargetName
  readonly path: string
  readonly class: "landing" | "funnel"
  readonly budgetProfile: "landing" | "funnel"
}

export const SITE_SPEED_ROUTE_TARGETS: Record<RouteTargetName, RouteTarget> = {
  "landing-home": {
    name: "landing-home",
    path: "/",
    class: "landing",
    budgetProfile: "landing",
  },
  business: {
    name: "business",
    path: "/business",
    class: "funnel",
    budgetProfile: "funnel",
  },
  "business-preview": {
    name: "business-preview",
    path: "/business-new",
    class: "funnel",
    budgetProfile: "funnel",
  },
}

const ROUTE_SETS: Record<RouteSetName, readonly RouteTargetName[]> = {
  landing: ["landing-home"],
  funnel: ["business"],
  all: ["landing-home", "business"],
}

export const parseArgs = (argv: readonly string[]): CliOptions => {
  const get = (flag: string): string | undefined => {
    const index = argv.indexOf(flag)
    return index === -1 ? undefined : argv[index + 1]
  }
  const list = <T extends string>(raw: string | undefined, fallback: readonly T[]): readonly T[] =>
    raw === undefined ? fallback : raw.split(",").map((entry) => entry.trim()).filter(Boolean) as T[]
  const origin = get("--origin") ?? get("--url") ?? "https://openagents.com/"
  const routeSet = (get("--route-set") ?? "landing") as RouteSetName
  const routeNames = list<RouteTargetName>(get("--routes"), ROUTE_SETS[routeSet] ?? ROUTE_SETS.landing)
  return {
    origin,
    routeSet,
    routeTargets: routeNames.map((name) => SITE_SPEED_ROUTE_TARGETS[name]),
    runs: Number(get("--runs") ?? "3"),
    settleMs: Number(get("--settle-ms") ?? "9000"),
    profiles: list<ProfileName>(get("--profiles"), ["desktop-fast", "mobile-mid"]),
    variants: list<VariantName>(get("--variants"), ["baseline", "block-counter", "block-thirdparty"]),
    outDir: get("--out") ?? "var/site-speed",
    failOnBudget: argv.includes("--fail-on-budget"),
  }
}

type Profile = {
  readonly name: ProfileName
  readonly viewport: { width: number; height: number }
  readonly userAgent?: string
  readonly deviceScaleFactor?: number
  readonly isMobile?: boolean
  readonly cpuThrottle: number
  readonly network: { downloadKbps: number; uploadKbps: number; latencyMs: number } | null
}

const PROFILES: Record<ProfileName, Profile> = {
  "desktop-fast": {
    name: "desktop-fast",
    viewport: { width: 1440, height: 900 },
    cpuThrottle: 1,
    network: null,
  },
  "mobile-mid": {
    name: "mobile-mid",
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 3,
    isMobile: true,
    userAgent:
      "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
    cpuThrottle: 4,
    network: { downloadKbps: 9000, uploadKbps: 1500, latencyMs: 170 },
  },
}

// Variant block-lists. `block-counter` answers the owner's question directly:
// the page with the token-total data paths stalled. `block-thirdparty` removes
// Google Fonts + Fathom.
const VARIANT_BLOCKED_URL_PARTS: Record<VariantName, readonly string[]> = {
  baseline: [],
  "block-counter": [
    "/api/public/khala-tokens-served",
    "khala-tokens-served/stream",
    "khala-tokens-served-sync",
  ],
  "block-thirdparty": ["fonts.googleapis.com", "fonts.gstatic.com", "usefathom.com"],
}

type BudgetStatus = "pass" | "fail" | "not_measured"

type BudgetEvaluation = {
  readonly budgetId: string
  readonly status: BudgetStatus
  readonly actual: number | null
  readonly threshold: number
  readonly unit: "ms" | "score" | "kb" | "count"
  readonly comparator: "<" | "<="
}

type BudgetDefinition = {
  readonly budgetId: string
  readonly metric: keyof CellReport["medians"]
  readonly thresholds: Record<ProfileName, number>
  readonly unit: BudgetEvaluation["unit"]
  readonly comparator: BudgetEvaluation["comparator"]
}

export const SITE_SPEED_BUDGETS: readonly BudgetDefinition[] = [
  {
    budgetId: "ttfb.doc",
    comparator: "<",
    metric: "ttfbMs",
    thresholds: { "desktop-fast": 250, "mobile-mid": 400 },
    unit: "ms",
  },
  {
    budgetId: "fcp",
    comparator: "<",
    metric: "fcpMs",
    thresholds: { "desktop-fast": 1000, "mobile-mid": 2500 },
    unit: "ms",
  },
  {
    budgetId: "lcp",
    comparator: "<",
    metric: "lcpMs",
    thresholds: { "desktop-fast": 1800, "mobile-mid": 3500 },
    unit: "ms",
  },
  {
    budgetId: "cls",
    comparator: "<",
    metric: "clsMilli",
    thresholds: { "desktop-fast": 50, "mobile-mid": 50 },
    unit: "score",
  },
  {
    budgetId: "tbt",
    comparator: "<",
    metric: "tbtMs",
    thresholds: { "desktop-fast": 200, "mobile-mid": 600 },
    unit: "ms",
  },
  {
    budgetId: "long_task.max",
    comparator: "<",
    metric: "longTaskMaxMs",
    thresholds: { "desktop-fast": 200, "mobile-mid": 350 },
    unit: "ms",
  },
  {
    budgetId: "js.wire",
    comparator: "<",
    metric: "jsTransferKb",
    thresholds: { "desktop-fast": 350, "mobile-mid": 350 },
    unit: "kb",
  },
  {
    budgetId: "counter.value_rendered",
    comparator: "<",
    metric: "counterValueRenderedMs",
    thresholds: { "desktop-fast": 1200, "mobile-mid": 3000 },
    unit: "ms",
  },
  {
    budgetId: "request.count",
    comparator: "<=",
    metric: "requestCount",
    thresholds: { "desktop-fast": 18, "mobile-mid": 18 },
    unit: "count",
  },
]

type ResourceClassSummary = Record<string, { count: number; transferBytes: number; maxDurationMs: number }>

type RunSample = {
  readonly ok: boolean
  readonly error?: string
  readonly navStart?: number
  readonly ttfbMs?: number
  readonly fcpMs?: number
  readonly lcpMs?: number
  readonly lcpElement?: string
  readonly cls?: number
  readonly clsSources?: readonly string[]
  readonly tbtMs?: number
  readonly longTasks?: { count: number; totalMs: number; maxMs: number }
  readonly counterValueRenderedMs?: number | null
  readonly counterText?: string | null
  readonly webSocketConnectMs?: number | null
  readonly webSocketUrl?: string | null
  readonly domContentLoadedMs?: number
  readonly loadEventMs?: number
  readonly scriptDurationMs?: number
  readonly layoutDurationMs?: number
  readonly jsHeapUsedBytes?: number
  readonly transferTotalBytes?: number
  readonly requestCount?: number
  readonly resourceClasses?: ResourceClassSummary
  readonly blockedRequests?: readonly string[]
}

const classifyUrl = (url: string, resourceType: string): string => {
  if (url.includes("fonts.googleapis") || url.includes("fonts.gstatic")) return "font-thirdparty"
  if (url.includes("usefathom")) return "analytics-thirdparty"
  if (url.includes("/api/public/khala-tokens-served")) return "api-counter"
  if (url.includes("/api/")) return "api"
  if (url.endsWith(".glb") || url.includes(".glb?")) return "model"
  if (resourceType === "document") return "doc"
  if (resourceType === "script") return "js"
  if (resourceType === "stylesheet") return "css"
  if (resourceType === "font") return "font"
  if (resourceType === "image") return "image"
  if (resourceType === "websocket") return "ws"
  return resourceType || "other"
}

const INIT_SCRIPT = `(() => {
  const perf = { lcp: [], cls: [], clsSources: [], paint: {}, longTasks: [], counterValueRenderedMs: null, counterText: null };
  window.__oaPerf = perf;
  try {
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        perf.lcp.push({ t: entry.startTime, size: entry.size, tag: entry.element ? entry.element.tagName + (entry.element.id ? "#" + entry.element.id : "") + (entry.element.className && typeof entry.element.className === "string" ? "." + entry.element.className.split(" ")[0] : "") : "?" });
      }
    }).observe({ type: "largest-contentful-paint", buffered: true });
  } catch {}
  try {
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        if (!entry.hadRecentInput) {
          perf.cls.push(entry.value);
          for (const source of entry.sources ?? []) {
            const node = source.node;
            if (node && node.tagName) perf.clsSources.push(node.tagName + (node.id ? "#" + node.id : "") + " @" + Math.round(entry.startTime));
          }
        }
      }
    }).observe({ type: "layout-shift", buffered: true });
  } catch {}
  try {
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) perf.paint[entry.name] = entry.startTime;
    }).observe({ type: "paint", buffered: true });
  } catch {}
  try {
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) perf.longTasks.push({ t: entry.startTime, d: entry.duration });
    }).observe({ type: "longtask", buffered: true });
  } catch {}
  const checkCounter = () => {
    if (perf.counterValueRenderedMs !== null) return true;
    // Primary: the home hero counter node. Secondary: the landing pill, which
    // carries no data attributes — detect the first comma-grouped >=7-digit
    // number anywhere in the body (the token total is the only such number on
    // the landing fold).
    const node = document.querySelector('[data-counter-display="khala-tokens-served"]');
    const target = node ? (node.textContent ?? "") : (document.body ? document.body.innerText : "");
    const match = target.match(/\\d{1,3}(?:,\\d{3}){2,}/);
    if (match) {
      perf.counterValueRenderedMs = performance.now();
      perf.counterText = match[0].slice(0, 40);
      return true;
    }
    return false;
  };
  const counterPoll = setInterval(() => { if (checkCounter()) clearInterval(counterPoll); }, 50);
  setTimeout(() => clearInterval(counterPoll), 30000);
})();`

async function runOnce(
  browser: Browser,
  profile: Profile,
  variant: VariantName,
  url: string,
  options: CliOptions,
): Promise<RunSample> {
  const context: BrowserContext = await browser.newContext({
    viewport: profile.viewport,
    userAgent: profile.userAgent,
    deviceScaleFactor: profile.deviceScaleFactor,
    isMobile: profile.isMobile,
    serviceWorkers: "block",
  })
  const blockedRequests: string[] = []
  const blockedParts = VARIANT_BLOCKED_URL_PARTS[variant]
  if (blockedParts.length > 0) {
    await context.route("**/*", async (route) => {
      const url = route.request().url()
      if (blockedParts.some((part) => url.includes(part))) {
        blockedRequests.push(url.slice(0, 120))
        await route.abort("connectionrefused")
        return
      }
      await route.continue()
    })
  }
  const page: Page = await context.newPage()
  try {
    const cdp = await context.newCDPSession(page)
    if (profile.network !== null) {
      await cdp.send("Network.enable")
      await cdp.send("Network.emulateNetworkConditions", {
        offline: false,
        latency: profile.network.latencyMs,
        downloadThroughput: (profile.network.downloadKbps * 1024) / 8,
        uploadThroughput: (profile.network.uploadKbps * 1024) / 8,
      })
    }
    if (profile.cpuThrottle > 1) {
      await cdp.send("Emulation.setCPUThrottlingRate", { rate: profile.cpuThrottle })
    }
    // WebSocket blocking for the counter variant rides CDP (page.route cannot
    // intercept WS upgrades).
    if (variant === "block-counter") {
      await cdp.send("Network.enable")
      await cdp.send("Network.setBlockedURLs", { urls: ["*khala-tokens-served*"] })
    }
    await cdp.send("Performance.enable")
    await page.addInitScript(INIT_SCRIPT)

    let webSocketConnectMs: number | null = null
    let webSocketUrl: string | null = null
    const navStartedAt = Date.now()
    page.on("websocket", (socket) => {
      if (webSocketConnectMs === null) {
        webSocketConnectMs = Date.now() - navStartedAt
        webSocketUrl = socket.url().slice(0, 120)
      }
    })

    await page.goto(url, { waitUntil: "domcontentloaded", timeout: 60_000 })
    await page.waitForTimeout(options.settleMs)

    const inPage = await page.evaluate(() => {
      const perf = (window as unknown as { __oaPerf: any }).__oaPerf
      const nav = performance.getEntriesByType("navigation")[0] as PerformanceNavigationTiming | undefined
      const resources = performance.getEntriesByType("resource") as PerformanceResourceTiming[]
      return {
        perf,
        nav: nav
          ? {
              ttfb: nav.responseStart,
              domContentLoaded: nav.domContentLoadedEventEnd,
              loadEvent: nav.loadEventEnd,
            }
          : null,
        resources: resources.map((entry) => ({
          url: entry.name.slice(0, 200),
          type: entry.initiatorType,
          transfer: entry.transferSize,
          duration: entry.duration,
        })),
      }
    })

    const metrics = await cdp.send("Performance.getMetrics")
    const metric = (name: string): number | undefined =>
      metrics.metrics.find((entry) => entry.name === name)?.value

    const fcp = inPage.perf.paint["first-contentful-paint"] as number | undefined
    const lcpEntries = inPage.perf.lcp as Array<{ t: number; tag: string }>
    const lastLcp = lcpEntries.at(-1)
    const longTasks = inPage.perf.longTasks as Array<{ t: number; d: number }>
    const tbt = longTasks
      .filter((task) => task.t >= (fcp ?? 0))
      .reduce((total, task) => total + Math.max(0, task.d - 50), 0)

    const resourceClasses: ResourceClassSummary = {}
    let transferTotal = 0
    for (const resource of inPage.resources) {
      const cls = classifyUrl(resource.url, resource.type)
      const bucket = (resourceClasses[cls] ??= { count: 0, transferBytes: 0, maxDurationMs: 0 })
      bucket.count += 1
      bucket.transferBytes += resource.transfer
      bucket.maxDurationMs = Math.max(bucket.maxDurationMs, Math.round(resource.duration))
      transferTotal += resource.transfer
    }

    return {
      ok: true,
      ttfbMs: inPage.nav?.ttfb,
      fcpMs: fcp,
      lcpMs: lastLcp?.t,
      lcpElement: lastLcp?.tag,
      cls: (inPage.perf.cls as number[]).reduce((total, value) => total + value, 0),
      clsSources: (inPage.perf.clsSources as string[]).slice(0, 8),
      tbtMs: Math.round(tbt),
      longTasks: {
        count: longTasks.length,
        totalMs: Math.round(longTasks.reduce((total, task) => total + task.d, 0)),
        maxMs: Math.round(longTasks.reduce((max, task) => Math.max(max, task.d), 0)),
      },
      counterValueRenderedMs: inPage.perf.counterValueRenderedMs === null
        ? null
        : Math.round(inPage.perf.counterValueRenderedMs),
      counterText: inPage.perf.counterText,
      webSocketConnectMs,
      webSocketUrl,
      domContentLoadedMs: inPage.nav?.domContentLoaded,
      loadEventMs: inPage.nav?.loadEvent,
      scriptDurationMs: metric("ScriptDuration") === undefined ? undefined : Math.round((metric("ScriptDuration") ?? 0) * 1000),
      layoutDurationMs: metric("LayoutDuration") === undefined ? undefined : Math.round((metric("LayoutDuration") ?? 0) * 1000),
      jsHeapUsedBytes: metric("JSHeapUsedSize"),
      transferTotalBytes: transferTotal,
      requestCount: inPage.resources.length,
      resourceClasses,
      blockedRequests: blockedRequests.slice(0, 10),
    }
  } catch (error) {
    return { ok: false, error: String(error).slice(0, 300) }
  } finally {
    await context.close()
  }
}

const median = (values: readonly number[]): number | null => {
  const sorted = values.filter((value) => Number.isFinite(value)).sort((left, right) => left - right)
  if (sorted.length === 0) return null
  return Math.round(sorted[Math.floor(sorted.length / 2)]!)
}

type CellReport = {
  readonly route: RouteTarget
  readonly url: string
  readonly profile: ProfileName
  readonly variant: VariantName
  readonly runs: readonly RunSample[]
  readonly medians: Record<string, number | null>
  readonly budgetEvaluations: readonly BudgetEvaluation[]
}

export const evaluateSiteSpeedBudgets = (
  profile: ProfileName,
  medians: CellReport["medians"],
): readonly BudgetEvaluation[] =>
  SITE_SPEED_BUDGETS.map((budget) => {
    const actual = medians[budget.metric]
    const threshold = budget.thresholds[profile]
    const passes = actual === null
      ? false
      : budget.comparator === "<"
        ? actual < threshold
        : actual <= threshold
    return {
      budgetId: budget.budgetId,
      status: actual === null ? "not_measured" : passes ? "pass" : "fail",
      actual,
      threshold,
      unit: budget.unit,
      comparator: budget.comparator,
    }
  })

const routeUrl = (origin: string, route: RouteTarget): string =>
  new URL(route.path, origin).toString()

export async function main(): Promise<void> {
  const options = parseArgs(process.argv.slice(2))
  const browser = await chromium.launch()
  const cells: CellReport[] = []
  for (const route of options.routeTargets) {
    const url = routeUrl(options.origin, route)
    for (const profileName of options.profiles) {
      const profile = PROFILES[profileName]
      for (const variant of options.variants) {
        const runs: RunSample[] = []
        for (let index = 0; index < options.runs; index += 1) {
          process.stderr.write(`run ${route.name} ${profileName}/${variant} ${index + 1}/${options.runs}\n`)
          runs.push(await runOnce(browser, profile, variant, url, options))
        }
        const okRuns = runs.filter((run) => run.ok)
        const num = (pick: (run: RunSample) => number | null | undefined): number | null =>
          median(okRuns.map((run) => pick(run) ?? Number.NaN))
        const medians = {
          ttfbMs: num((run) => run.ttfbMs),
          fcpMs: num((run) => run.fcpMs),
          lcpMs: num((run) => run.lcpMs),
          clsMilli: num((run) => (run.cls === undefined ? undefined : run.cls * 1000)),
          tbtMs: num((run) => run.tbtMs),
          longTaskMaxMs: num((run) => run.longTasks?.maxMs),
          jsTransferKb: num((run) => ((run.resourceClasses?.js?.transferBytes ?? 0) / 1024)),
          counterValueRenderedMs: num((run) => run.counterValueRenderedMs),
          webSocketConnectMs: num((run) => run.webSocketConnectMs),
          scriptDurationMs: num((run) => run.scriptDurationMs),
          transferTotalKb: num((run) => (run.transferTotalBytes ?? Number.NaN) / 1024),
          requestCount: num((run) => run.requestCount),
          jsHeapUsedMb: num((run) => (run.jsHeapUsedBytes ?? Number.NaN) / (1024 * 1024)),
        }
        cells.push({
          route,
          url,
          profile: profileName,
          variant,
          runs,
          medians,
          budgetEvaluations: variant === "baseline" ? evaluateSiteSpeedBudgets(profileName, medians) : [],
        })
      }
    }
  }
  await browser.close()

  const report = {
    schema: "openagents.site_speed.run_report.v1",
    origin: options.origin,
    routeSet: options.routeSet,
    routes: options.routeTargets,
    generatedAt: new Date().toISOString(),
    runsPerCell: options.runs,
    settleMs: options.settleMs,
    cells,
  }
  const stamp = new Date().toISOString().replaceAll(/[:.]/g, "-")
  const outDir = join(options.outDir, stamp)
  await mkdir(outDir, { recursive: true })
  const outPath = join(outDir, "report.json")
  await writeFile(outPath, `${JSON.stringify(report, null, 2)}\n`)

  for (const cell of cells) {
    process.stdout.write(`\n== ${cell.route.name} ${cell.profile} / ${cell.variant} (medians of ${options.runs}) ==\n`)
    process.stdout.write(`${JSON.stringify(cell.medians)}\n`)
    if (cell.budgetEvaluations.length > 0) {
      const failed = cell.budgetEvaluations.filter((budget) => budget.status !== "pass")
      process.stdout.write(`budgets: ${failed.length === 0 ? "pass" : `fail ${failed.map((budget) => budget.budgetId).join(",")}`}\n`)
    }
  }
  process.stdout.write(`\nreport: ${outPath}\n`)

  if (
    options.failOnBudget &&
    cells.some((cell) => cell.budgetEvaluations.some((budget) => budget.status !== "pass"))
  ) {
    process.exitCode = 1
  }
}

if (import.meta.main) {
  await main()
}
