// Headless capture + acceptance proof for the homepage "Khala Tokens Served"
// top-left pill (#6273 follow-up).
//
// Boots the REAL Foldkit app from `index.html` (the same `entry.ts` runtime the
// live page uses) in headless Chromium and:
//   - intercepts the public tokens-served endpoint so the live counter seeds a
//     deterministic total (no backend required), then
//   - navigates to `/` and asserts the top-left pill shows
//     "Khala Tokens Served: 1,250,000", links to /khala, and that the back
//     button is ABSENT, screenshotting to docs/khala-tokens-pill-home.png
//   - clicks the pill and asserts client-side navigation lands on /khala
//   - navigates to `/khala` and asserts the "← OpenAgents" back button occupies
//     the SAME top-left slot with the pill ABSENT, screenshotting to
//     docs/khala-tokens-pill-khala.png
//
// HONEST CAVEAT: this is headless Chromium with the tokens-served fetch stubbed,
// so it proves the pill wiring, formatting, slot exclusivity, and navigation —
// not the production sync-stream tick (that is covered by the subscription unit
// tests gating Landing as a live tokens-served surface).
//
// A tiny Vite dev server transforms the app TS so we exercise the actual
// modules. Run: `bun run scripts/capture-khala-tokens-pill.ts`.

import { mkdirSync, readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { chromium } from 'playwright'
import { createServer } from 'vite'

const here = dirname(fileURLToPath(import.meta.url))
const webRoot = join(here, '..')
const homeOut = join(webRoot, 'docs', 'khala-tokens-pill-home.png')
const khalaOut = join(webRoot, 'docs', 'khala-tokens-pill-khala.png')

const SEED_TOKENS = 1_250_000
const SEED_DISPLAY = '1,250,000'

const main = async (): Promise<void> => {
  const indexHtml = readFileSync(join(webRoot, 'index.html'), 'utf8')

  const server = await createServer({
    root: webRoot,
    server: { port: 0 },
    appType: 'custom',
    // SPA fallback: serve the transformed index.html for any non-asset path so
    // deep links like /khala boot the client-routed app.
    plugins: [
      {
        name: 'khala-pill-spa-fallback',
        configureServer(s) {
          s.middlewares.use(async (req, res, next) => {
            const url = req.url ?? '/'
            if (
              url.startsWith('/@') ||
              url.startsWith('/src/') ||
              url.startsWith('/node_modules/') ||
              url.includes('.')
            ) {
              next()
              return
            }
            try {
              const html = await s.transformIndexHtml(url, indexHtml)
              res.setHeader('Content-Type', 'text/html')
              res.end(html)
            } catch (error) {
              next(error)
            }
          })
        },
      },
    ],
  })
  await server.listen()
  const address = server.httpServer?.address()
  const port = address && typeof address === 'object' ? address.port : undefined
  if (port === undefined) throw new Error('vite dev server did not bind a port')
  const base = `http://localhost:${port}`

  const browser = await chromium.launch({
    args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader'],
  })
  const errors: string[] = []
  try {
    const page = await browser.newPage({
      viewport: { width: 1280, height: 800 },
      deviceScaleFactor: 2,
    })
    // Ignore the expected resource 404s from the deliberately-unstubbed public
    // endpoints (snapshot/sync/home JSON) in this backend-less harness; only
    // real script/runtime errors should fail the proof.
    const isExpectedNoise = (text: string): boolean =>
      /Failed to load resource/i.test(text) ||
      /the server responded with a status of 404/i.test(text)
    page.on('console', message => {
      if (message.type() === 'error' && !isExpectedNoise(message.text())) {
        errors.push(message.text())
      }
    })
    page.on('pageerror', error => {
      if (!isExpectedNoise(String(error))) errors.push(String(error))
    })

    // Seed the live counter deterministically: the scalar endpoint feeds the
    // SAME `publicKhalaTokensServed` model the pill and /khala counter read.
    await page.route('**/api/public/khala-tokens-served', route =>
      route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          tokensServed: SEED_TOKENS,
          generatedAt: '2026-06-24T12:00:00.000Z',
        }),
      }),
    )
    // The snapshot + sync stream are not needed for this proof; let the snapshot
    // 404 so the scalar value seeds the displayed total, and fail the WS fast.
    await page.route('**/api/sync/**', route =>
      route.fulfill({ status: 404, body: 'not found' }),
    )

    // ---- Homepage `/`: pill present, back button absent ----
    await page.goto(`${base}/`, { waitUntil: 'load' })
    const pill = page.locator('[data-landing-khala-tokens-pill="home"]')
    await pill.waitFor({ state: 'attached', timeout: 10_000 })
    await page.waitForFunction(
      display => {
        const el = document.querySelector(
          '[data-landing-khala-tokens-pill="home"]',
        )
        return el !== null && el.textContent?.includes(display) === true
      },
      SEED_DISPLAY,
      { timeout: 10_000 },
    )

    // The shared pill styling uppercases the label via CSS, so compare on the
    // accessible label (the source text) and check the rendered total verbatim.
    const pillText = (await pill.innerText()).replace(/\s+/g, ' ').trim()
    if (!/khala tokens served:/i.test(pillText)) {
      throw new Error(`pill missing label, got: ${pillText}`)
    }
    if (!pillText.includes(SEED_DISPLAY)) {
      throw new Error(`pill missing live total ${SEED_DISPLAY}, got: ${pillText}`)
    }
    const accessibleName = await pill.getAttribute('aria-label')
    if (accessibleName !== 'Khala tokens served — open Khala') {
      throw new Error(`unexpected pill aria-label: ${accessibleName}`)
    }
    const backOnHome = await page
      .locator('[data-khala-back="home"], [data-tassadar-back="home"]')
      .count()
    if (backOnHome !== 0) {
      throw new Error('back button must NOT render on the homepage slot')
    }

    await page.waitForTimeout(600)
    mkdirSync(dirname(homeOut), { recursive: true })
    await page.screenshot({ path: homeOut })

    // ---- Click the pill -> client-side navigate to /khala ----
    await pill.click()
    await page.waitForFunction(
      () => window.location.pathname === '/khala',
      undefined,
      { timeout: 10_000 },
    )

    // ---- /khala: back button present, pill absent ----
    await page.goto(`${base}/khala`, { waitUntil: 'load' })
    const back = page.locator('[data-khala-back="home"]')
    await back.waitFor({ state: 'attached', timeout: 10_000 })
    const pillOnKhala = await page
      .locator('[data-landing-khala-tokens-pill="home"]')
      .count()
    if (pillOnKhala !== 0) {
      throw new Error('pill must NOT render on the /khala slot')
    }
    await page.waitForTimeout(600)
    await page.screenshot({ path: khalaOut })

    if (errors.length > 0) {
      throw new Error(`console errors during render:\n${errors.join('\n')}`)
    }

    server.config.logger.info(
      `[capture-khala-tokens-pill] OK — pill="${pillText}" -> wrote ${homeOut} and ${khalaOut}`,
    )
  } finally {
    await browser.close()
    await server.close()
  }
}

main().catch(error => {
  console.error(error)
  process.exit(1)
})
