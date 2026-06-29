#!/usr/bin/env node

import { existsSync } from 'node:fs'

const defaultBaseUrl = 'https://openagents.com'
const defaultTimeoutMs = 30_000
const defaultProofLimit = 4
const replayBundlePath = '/api/public/tassadar-replays/first-real-settlement'

const viewports = [
  { height: 900, label: 'desktop', width: 1366 },
  { height: 844, label: 'mobile', width: 390 },
]

const routes = [
  {
    canvas: true,
    elementSelector: 'oa-tassadar-proof-replay',
    manifestPath: replayBundlePath,
    name: 'proof_replay_first_real_settlement',
    path: '/tassadar/replay/first-real-settlement',
    readySelector: '[data-proof-replay-webgl-mount]',
  },
  {
    activity: true,
    elementSelector: 'oa-public-activity-timeline',
    name: 'public_activity',
    path: '/activity',
    readySelector: '[data-activity-event]',
  },
]

export const parseArgs = argv => {
  const options = {
    apiBaseUrl: process.env.OPENAGENTS_API_BASE_URL || '',
    baseUrl: process.env.OPENAGENTS_BASE_URL || defaultBaseUrl,
    browserPath: process.env.CHROME_PATH || '',
    headless: true,
    proofLimit: defaultProofLimit,
    timeoutMs: defaultTimeoutMs,
  }

  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index]

    if (value === '--api-base-url' || value === '--apiBaseUrl') {
      options.apiBaseUrl = argv[++index] || options.apiBaseUrl
    } else if (value === '--base-url' || value === '--baseUrl') {
      options.baseUrl = argv[++index] || options.baseUrl
    } else if (value === '--browser-path' || value === '--chrome-path') {
      options.browserPath = argv[++index] || options.browserPath
    } else if (value === '--headed') {
      options.headless = false
    } else if (value === '--proof-limit') {
      options.proofLimit = Number(argv[++index] || options.proofLimit)
    } else if (value === '--timeout-ms') {
      options.timeoutMs = Number(argv[++index] || options.timeoutMs)
    } else if (value === '--help' || value === '-h') {
      options.help = true
    } else {
      throw new Error(`Unknown argument: ${value}`)
    }
  }

  if (!Number.isFinite(options.timeoutMs) || options.timeoutMs < 1_000) {
    throw new Error('--timeout-ms must be a number >= 1000.')
  }
  if (!Number.isFinite(options.proofLimit) || options.proofLimit < 1) {
    throw new Error('--proof-limit must be a number >= 1.')
  }

  return options
}

export const usage = () => `Usage:
  node scripts/visibility-browser-smoke.mjs
  node scripts/visibility-browser-smoke.mjs --base-url http://localhost:5173
  CHROME_PATH=/Applications/Google\\ Chrome.app/Contents/MacOS/Google\\ Chrome node scripts/visibility-browser-smoke.mjs

Options:
  --api-base-url <url>   Optional API origin for local UI smoke runs. Defaults to the page origin.
  --base-url <url>       OpenAgents origin. Defaults to https://openagents.com.
  --browser-path <path>  Chromium/Chrome executable. Defaults to Playwright's browser, then local Chrome.
  --headed              Run with a visible browser window.
  --proof-limit <count>  Max proof drawer URLs to fetch. Defaults to ${defaultProofLimit}.
  --timeout-ms <ms>      Per-route browser timeout. Defaults to ${defaultTimeoutMs}.
`

const assert = (condition, message) => {
  if (!condition) throw new Error(message)
}

const trimBaseUrl = baseUrl =>
  String(baseUrl || defaultBaseUrl).replace(/\/+$/, '')

const absoluteUrl = (baseUrl, pathOrUrl) =>
  new URL(pathOrUrl, trimBaseUrl(baseUrl)).toString()

const okStatus = status => status >= 200 && status < 300

const addCheck = (checks, name, passed, details = {}) => {
  checks.push({ details, name, passed })
}

const assertCheck = (checks, name, condition, details = {}) => {
  addCheck(checks, name, Boolean(condition), details)
  assert(condition, `${name} failed`)
}

export const canvasProbePassed = probe =>
  probe !== null &&
  probe.samplePixels >= 256 &&
  probe.nonTransparentPixels >= Math.max(64, probe.samplePixels * 0.15) &&
  (probe.distinctColorCount >= 4 ||
    probe.nonUniformPixels >= Math.max(32, probe.samplePixels * 0.01) ||
    probe.nonBlankPixels >= Math.max(32, probe.samplePixels * 0.01))

const hasRefs = value =>
  Array.isArray(value) &&
  value.some(ref => typeof ref === 'string' && ref.trim().length > 0)

const arrayFrom = value => (Array.isArray(value) ? value : [])

const refId = (item, fallback) => {
  for (const key of [
    'eventRef',
    'flowRef',
    'cueRef',
    'captionRef',
    'gapRef',
    'stageRef',
    'actorRef',
  ]) {
    if (typeof item?.[key] === 'string' && item[key].trim().length > 0) {
      return item[key]
    }
  }

  return fallback
}

export const motionSourceRefGaps = bundle => {
  const gaps = []

  if (!hasRefs(arrayFrom(bundle?.sourceRefs).map(source => source?.ref))) {
    gaps.push('bundle.sourceRefs')
  }

  for (const collection of [
    ['events', 'events'],
    ['flows', 'flows'],
    ['cameraCues', 'cameraCues'],
    ['captions', 'captions'],
    ['gaps', 'gaps'],
    ['stages', 'stages'],
  ]) {
    const [field, label] = collection
    arrayFrom(bundle?.[field]).forEach((item, index) => {
      if (!hasRefs(item?.sourceRefs)) {
        gaps.push(`${label}.${refId(item, index)}`)
      }
    })
  }

  const sourceBoundRefs = new Set()
  for (const event of arrayFrom(bundle?.events)) {
    if (!hasRefs(event?.sourceRefs)) continue
    for (const ref of [
      ...arrayFrom(event.actorRefs),
      ...arrayFrom(event.targetRefs),
    ]) {
      if (typeof ref === 'string' && ref.trim().length > 0) {
        sourceBoundRefs.add(ref)
      }
    }
  }
  for (const flow of arrayFrom(bundle?.flows)) {
    if (!hasRefs(flow?.sourceRefs)) continue
    for (const ref of [flow?.fromRef, flow?.toRef]) {
      if (typeof ref === 'string' && ref.trim().length > 0) {
        sourceBoundRefs.add(ref)
      }
    }
  }
  for (const cue of arrayFrom(bundle?.cameraCues)) {
    if (!hasRefs(cue?.sourceRefs)) continue
    for (const ref of arrayFrom(cue.focusRefs)) {
      if (typeof ref === 'string' && ref.trim().length > 0) {
        sourceBoundRefs.add(ref)
      }
    }
  }

  for (const actor of arrayFrom(bundle?.actors)) {
    const actorRef =
      typeof actor?.actorRef === 'string' ? actor.actorRef.trim() : ''
    if (actorRef.length === 0 || !sourceBoundRefs.has(actorRef)) {
      gaps.push(`actors.${actorRef || 'unknown'}`)
    }
  }

  return gaps
}

const localChromeCandidates = () => [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/Applications/Chromium.app/Contents/MacOS/Chromium',
  '/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge',
  '/usr/bin/google-chrome',
  '/usr/bin/google-chrome-stable',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser',
]

const resolveBrowserPath = browserPath => {
  if (browserPath && existsSync(browserPath)) return browserPath
  return localChromeCandidates().find(candidate => existsSync(candidate)) ?? ''
}

const waitForDeepSelector = async (page, selector, timeoutMs) => {
  await page.waitForFunction(
    targetSelector => {
      const find = root => {
        if (root.querySelector?.(targetSelector) !== null) return true
        for (const element of root.querySelectorAll?.('*') ?? []) {
          if (element.shadowRoot !== null && find(element.shadowRoot)) {
            return true
          }
        }
        return false
      }

      return find(document)
    },
    selector,
    { timeout: timeoutMs },
  )
}

const waitForAnimationFrames = async page => {
  await page.evaluate(
    () =>
      new Promise(resolve => {
        requestAnimationFrame(() => {
          requestAnimationFrame(() => {
            requestAnimationFrame(resolve)
          })
        })
      }),
  )
}

const collectCanvasRects = async page =>
  page.evaluate(() => {
    const canvases = []
    const visit = (root, path) => {
      for (const element of root.querySelectorAll?.('*') ?? []) {
        const tag = element.localName || element.tagName.toLowerCase()
        const nextPath = `${path} > ${tag}`
        if (element instanceof HTMLCanvasElement) {
          canvases.push({ canvas: element, path: nextPath })
        }
        if (element.shadowRoot !== null) visit(element.shadowRoot, nextPath)
      }
    }

    visit(document, 'document')

    return canvases.map(({ canvas, path }) => {
      const rect = canvas.getBoundingClientRect()
      const cssWidth = Math.max(0, Math.round(rect.width))
      const cssHeight = Math.max(0, Math.round(rect.height))
      return {
        cssHeight,
        cssWidth,
        path,
        rect: {
          height: Math.max(0, rect.height),
          width: Math.max(0, rect.width),
          x: Math.max(0, rect.left),
          y: Math.max(0, rect.top),
        },
        sourceHeight: Math.max(0, canvas.height || cssHeight),
        sourceWidth: Math.max(0, canvas.width || cssWidth),
      }
    })
  })

const screenshotProbeForRect = async (page, meta) => {
  const insetX = meta.rect.width * 0.08
  const insetY = meta.rect.height * 0.12
  const clip = {
    height: Math.max(16, Math.floor(meta.rect.height - insetY * 2)),
    width: Math.max(16, Math.floor(meta.rect.width - insetX * 2)),
    x: Math.max(0, Math.floor(meta.rect.x + insetX)),
    y: Math.max(0, Math.floor(meta.rect.y + insetY)),
  }
  const probe = {
    cssHeight: meta.cssHeight,
    cssWidth: meta.cssWidth,
    distinctColorCount: 0,
    error: null,
    nonBlankPixels: 0,
    nonTransparentPixels: 0,
    nonUniformPixels: 0,
    path: meta.path,
    samplePixels: 0,
    sourceHeight: meta.sourceHeight,
    sourceWidth: meta.sourceWidth,
  }

  if (clip.width < 16 || clip.height < 16) {
    probe.error = 'canvas_clip_too_small'
    return probe
  }

  try {
    const screenshot = await page.screenshot({ clip })
    const dataUrl = `data:image/png;base64,${screenshot.toString('base64')}`
    return await page.evaluate(
      async ({ dataUrl: imageUrl, meta: baseProbe }) => {
        const image = new Image()
        image.src = imageUrl
        await image.decode()
        const sampleWidth = Math.min(160, image.naturalWidth)
        const sampleHeight = Math.min(100, image.naturalHeight)
        const sample = document.createElement('canvas')
        sample.width = sampleWidth
        sample.height = sampleHeight
        const context = sample.getContext('2d', { willReadFrequently: true })
        const nextProbe = {
          ...baseProbe,
          samplePixels: sampleWidth * sampleHeight,
        }
        if (context === null) {
          nextProbe.error = '2d_context_unavailable'
          return nextProbe
        }

        context.drawImage(image, 0, 0, sampleWidth, sampleHeight)
        const data = context.getImageData(0, 0, sampleWidth, sampleHeight).data
        const colors = new Set()
        const first = [data[0] ?? 0, data[1] ?? 0, data[2] ?? 0, data[3] ?? 0]
        for (let index = 0; index < data.length; index += 4) {
          const red = data[index] ?? 0
          const green = data[index + 1] ?? 0
          const blue = data[index + 2] ?? 0
          const alpha = data[index + 3] ?? 0
          if (alpha > 8) nextProbe.nonTransparentPixels += 1
          if (alpha > 8 && red + green + blue > 18) {
            nextProbe.nonBlankPixels += 1
          }
          if (
            Math.abs(red - first[0]) +
              Math.abs(green - first[1]) +
              Math.abs(blue - first[2]) +
              Math.abs(alpha - first[3]) >
            12
          ) {
            nextProbe.nonUniformPixels += 1
          }
          colors.add(
            `${Math.round(red / 24)}:${Math.round(green / 24)}:${Math.round(
              blue / 24,
            )}:${Math.round(alpha / 32)}`,
          )
        }
        nextProbe.distinctColorCount = colors.size
        return nextProbe
      },
      { dataUrl, meta: probe },
    )
  } catch (error) {
    probe.error = error instanceof Error ? error.message : String(error)
    return probe
  }
}

const collectCanvasProbes = async page => {
  const rects = (await collectCanvasRects(page)).filter(
    rect => rect.cssWidth >= 16 && rect.cssHeight >= 16,
  )
  const probes = []
  for (const rect of rects) {
    probes.push(await screenshotProbeForRect(page, rect))
  }
  return probes
}

const collectTextOverflow = async page =>
  page.evaluate(() => {
    const offenders = []
    const textSelector = [
      'a',
      'button',
      'dd',
      'dt',
      'h1',
      'h2',
      'h3',
      'label',
      'li',
      'p',
      'span',
      'strong',
      'time',
    ].join(',')
    const skip = element =>
      element.closest?.(
        'pre, code, canvas, svg, [data-proof-event-json], [aria-hidden="true"]',
      ) !== null

    const visit = root => {
      for (const element of root.querySelectorAll?.(textSelector) ?? []) {
        if (skip(element)) continue
        const text = element.textContent?.replace(/\s+/g, ' ').trim() ?? ''
        if (text.length === 0) continue
        const style = getComputedStyle(element)
        if (
          style.display === 'none' ||
          style.visibility === 'hidden' ||
          Number(style.opacity) === 0
        ) {
          continue
        }
        const rect = element.getBoundingClientRect()
        if (rect.width < 1 || rect.height < 1) continue
        const horizontalOverflow =
          element.scrollWidth - Math.ceil(element.clientWidth)
        const verticalOverflow =
          style.overflowY !== 'visible'
            ? element.scrollHeight - Math.ceil(element.clientHeight)
            : 0
        if (horizontalOverflow > 2 || verticalOverflow > 3) {
          offenders.push({
            className:
              typeof element.className === 'string' ? element.className : '',
            horizontalOverflow,
            tag: element.localName,
            text: text.slice(0, 120),
            verticalOverflow,
          })
        }
      }

      for (const element of root.querySelectorAll?.('*') ?? []) {
        if (element.shadowRoot !== null) visit(element.shadowRoot)
      }
    }

    visit(document)

    return offenders.slice(0, 12)
  })

const fetchJsonWithPage = async (page, path) =>
  page.evaluate(async targetPath => {
    const response = await fetch(targetPath, {
      headers: { accept: 'application/json' },
    })
    const body = await response.json().catch(() => null)
    return { body, status: response.status }
  }, path)

const smokeReplayManifest = async (page, route, checks) => {
  if (route.manifestPath === undefined) return null

  const manifest = await fetchJsonWithPage(page, route.manifestPath)
  assertCheck(
    checks,
    `${route.name}_manifest_200`,
    okStatus(manifest.status),
    { path: route.manifestPath, status: manifest.status },
  )
  assertCheck(
    checks,
    `${route.name}_manifest_schema`,
    manifest.body?.schemaVersion === 'proof_replay_bundle.v1',
    { schemaVersion: manifest.body?.schemaVersion },
  )

  const sourceGaps = motionSourceRefGaps(manifest.body)
  assertCheck(
    checks,
    `${route.name}_no_anonymous_motion`,
    sourceGaps.length === 0,
    { sourceGaps },
  )

  return {
    bundleRef: manifest.body?.bundleRef,
    eventCount: arrayFrom(manifest.body?.events).length,
    sourceRefCount: arrayFrom(manifest.body?.sourceRefs).length,
  }
}

const waitForCanvasProbes = async (page, timeoutMs) => {
  const startedAt = Date.now()
  let probes = []
  while (Date.now() - startedAt < timeoutMs) {
    await waitForAnimationFrames(page)
    probes = await collectCanvasProbes(page)
    if (probes.find(canvasProbePassed) !== undefined) return probes
    await page.waitForTimeout(500)
  }

  return probes
}

const smokeCanvas = async (page, route, checks, timeoutMs) => {
  await waitForDeepSelector(page, 'canvas', timeoutMs)
  const probes = await waitForCanvasProbes(page, timeoutMs)
  const passing = probes.find(canvasProbePassed) ?? null
  assertCheck(checks, `${route.name}_canvas_present`, probes.length > 0, {
    canvasCount: probes.length,
  })
  assertCheck(
    checks,
    `${route.name}_canvas_nonblank`,
    passing !== null,
    { probes },
  )
  return { passing, probes }
}

const smokeActivityDrawer = async (
  page,
  route,
  checks,
  proofLimit,
  apiBaseUrl,
) => {
  await page.evaluate(() => {
    const find = root => {
      const button = root.querySelector?.('[data-activity-event]')
      if (button !== null) return button
      for (const element of root.querySelectorAll?.('*') ?? []) {
        if (element.shadowRoot !== null) {
          const found = find(element.shadowRoot)
          if (found !== null) return found
        }
      }
      return null
    }

    find(document)?.click()
  })
  await waitForDeepSelector(page, '[data-proof-drawer]', 5_000)
  const proofUrls = await page.evaluate(limit => {
    const links = []
    const visit = root => {
      for (const element of root.querySelectorAll?.('[data-proof-url]') ?? []) {
        if (element instanceof HTMLAnchorElement) links.push(element.href)
      }
      for (const element of root.querySelectorAll?.('*') ?? []) {
        if (element.shadowRoot !== null) visit(element.shadowRoot)
      }
    }

    visit(document)
    return [...new Set(links)].slice(0, limit)
  }, proofLimit)

  assertCheck(
    checks,
    `${route.name}_proof_drawer_links_present`,
    proofUrls.length > 0,
    { proofUrls },
  )

  const linked = []
  for (const url of proofUrls) {
    const proofUrl = new URL(url)
    const targetUrl =
      apiBaseUrl.length === 0
        ? url
        : absoluteUrl(apiBaseUrl, `${proofUrl.pathname}${proofUrl.search}`)
    const response = await page.request.get(targetUrl, {
      headers: { accept: 'application/json,text/html;q=0.9,*/*;q=0.8' },
      timeout: 10_000,
    })
    const passed = okStatus(response.status())
    addCheck(checks, `${route.name}_proof_drawer_public_route_200`, passed, {
      status: response.status(),
      url: targetUrl,
    })
    assert(
      passed,
      `${route.name}_proof_drawer_public_route_200 failed: ${targetUrl}`,
    )
    linked.push({ status: response.status(), url: targetUrl })
  }

  return { linked, proofUrlCount: proofUrls.length }
}

const smokeRoute = async ({
  apiBaseUrl,
  baseUrl,
  browser,
  proofLimit,
  route,
  timeoutMs,
  viewport,
}) => {
  const page = await browser.newPage({
    viewport: { height: viewport.height, width: viewport.width },
  })
  const checks = []

  try {
    if (apiBaseUrl.length > 0) {
      await page.route('**/api/**', async route => {
        const requestUrl = new URL(route.request().url())
        const targetUrl = absoluteUrl(
          apiBaseUrl,
          `${requestUrl.pathname}${requestUrl.search}`,
        )
        const response = await route.fetch({ url: targetUrl })
        await route.fulfill({ response })
      })
    }

    const url = absoluteUrl(baseUrl, route.path)
    const response = await page.goto(url, {
      timeout: timeoutMs,
      waitUntil: 'domcontentloaded',
    })
    assertCheck(
      checks,
      `${route.name}_route_200`,
      response !== null && okStatus(response.status()),
      { status: response?.status() ?? null, url },
    )
    assertCheck(
      checks,
      `${route.name}_route_not_redirected`,
      new URL(page.url()).pathname === route.path,
      { finalUrl: page.url(), expectedPath: route.path },
    )
    await waitForDeepSelector(page, route.elementSelector, timeoutMs)
    addCheck(checks, `${route.name}_custom_element_present`, true, {
      selector: route.elementSelector,
    })
    await waitForDeepSelector(page, route.readySelector, timeoutMs)
    addCheck(checks, `${route.name}_ready_selector_present`, true, {
      selector: route.readySelector,
    })

    const canvas = route.canvas
      ? await smokeCanvas(page, route, checks, timeoutMs)
      : null
    const manifest = await smokeReplayManifest(page, route, checks)
    const activity = route.activity
      ? await smokeActivityDrawer(page, route, checks, proofLimit, apiBaseUrl)
      : null
    const overflow = await collectTextOverflow(page)
    assertCheck(
      checks,
      `${route.name}_text_fits_${viewport.label}`,
      overflow.length === 0,
      { overflow, viewport },
    )

    return {
      activity,
      canvas,
      checks,
      manifest,
      name: route.name,
      ok: checks.every(check => check.passed),
      path: route.path,
      viewport,
    }
  } catch (error) {
    addCheck(checks, `${route.name}_smoke_completed`, false, {
      error: error instanceof Error ? error.message : String(error),
    })
    return {
      activity: null,
      canvas: null,
      checks,
      error: error instanceof Error ? error.message : String(error),
      manifest: null,
      name: route.name,
      ok: false,
      path: route.path,
      viewport,
    }
  } finally {
    await page.close()
  }
}

export const runVisibilityBrowserSmoke = async ({
  apiBaseUrl = '',
  baseUrl = defaultBaseUrl,
  browserPath = '',
  headless = true,
  proofLimit = defaultProofLimit,
  timeoutMs = defaultTimeoutMs,
} = {}) => {
  const { chromium } = await import('playwright')
  const executablePath = resolveBrowserPath(browserPath)
  const browser = await chromium.launch({
    ...(executablePath.length === 0 ? {} : { executablePath }),
    headless,
  })
  const origin = trimBaseUrl(baseUrl)
  const apiOrigin =
    typeof apiBaseUrl === 'string' && apiBaseUrl.trim().length > 0
      ? trimBaseUrl(apiBaseUrl)
      : ''
  const results = []

  try {
    for (const viewport of viewports) {
      for (const route of routes) {
        results.push(
          await smokeRoute({
            apiBaseUrl: apiOrigin,
            baseUrl: origin,
            browser,
            proofLimit,
            route,
            timeoutMs,
            viewport,
          }),
        )
      }
    }
  } finally {
    await browser.close()
  }

  const checks = results.flatMap(result => result.checks)
  return {
    apiBaseUrl: apiOrigin || origin,
    baseUrl: origin,
    browserPath: executablePath || 'playwright-default',
    checks,
    ok: checks.every(check => check.passed),
    results,
  }
}

const main = async () => {
  const options = parseArgs(process.argv.slice(2))
  if (options.help) {
    console.log(usage())
    return
  }

  const output = await runVisibilityBrowserSmoke(options)
  console.log(JSON.stringify(output, null, 2))
  if (!output.ok) process.exitCode = 1
}

if (import.meta.url === `file://${process.argv[1]}`) {
  main().catch(error => {
    console.error(error instanceof Error ? error.message : String(error))
    process.exitCode = 1
  })
}
