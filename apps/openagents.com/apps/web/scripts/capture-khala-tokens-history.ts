// Headless screenshot of the "Khala Tokens Served" history chart (#6227).
//
// Boots the REAL `khalaTokensServedPanel` view (live counter + hand-rolled SVG
// per-day history chart) through a minimal Foldkit program in headless
// Chromium, captures the loaded (sample series) and empty states, and asserts
// the chart heading and (for loaded) the accessible per-day text fallback are
// present so the capture proves real render output, not a blank frame.
//
// A tiny Vite dev server transforms the harness TS so the actual view module is
// exercised. Run: `bun run scripts/capture-khala-tokens-history.ts`.

import { mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { chromium } from 'playwright'
import { createServer } from 'vite'

const here = dirname(fileURLToPath(import.meta.url))
const webRoot = join(here, '..')
const outDir = join(webRoot, 'docs')

const harnessHtml = `<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <link rel="stylesheet" href="/src/styles.css" />
    <style>html,body{margin:0;background:#000;}</style>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/scripts/capture-khala-tokens-history.harness.ts"></script>
  </body>
</html>`

const main = async (): Promise<void> => {
  const server = await createServer({
    root: webRoot,
    server: { port: 0 },
    plugins: [
      {
        name: 'khala-history-capture-harness',
        configureServer(s) {
          s.middlewares.use('/__khala-history-capture', (_req, res) => {
            res.setHeader('Content-Type', 'text/html')
            res.end(harnessHtml)
          })
        },
      },
    ],
  })
  await server.listen()
  const address = server.httpServer?.address()
  const port = address && typeof address === 'object' ? address.port : undefined
  if (port === undefined) throw new Error('vite dev server did not bind a port')

  const browser = await chromium.launch()
  try {
    for (const state of ['loaded', 'empty'] as const) {
      const errors: string[] = []
      const page = await browser.newPage({
        viewport: { width: 960, height: 360 },
        deviceScaleFactor: 2,
      })
      page.on('console', message => {
        if (message.type() === 'error') errors.push(message.text())
      })
      page.on('pageerror', error => errors.push(String(error)))

      const url = `http://localhost:${port}/__khala-history-capture?state=${state}`
      await page.goto(url, { waitUntil: 'load' })
      await page.waitForFunction(
        () => Reflect.get(window, '__khalaHistoryMounted') === true,
      )
      await page.waitForSelector(
        '[data-chart="khala-tokens-served-history"]',
        { state: 'attached' },
      )
      await page.waitForTimeout(300)

      const heading = await page
        .locator('text=Tokens Served / Day')
        .count()
      if (heading < 1) throw new Error('chart heading did not render')

      if (state === 'loaded') {
        const bars = await page
          .locator('[data-chart="khala-tokens-served-history"] svg rect')
          .count()
        if (bars < 1) throw new Error('chart bars did not render')
        const fallback = await page
          .locator('text=2026-06-23: 96,250 tokens')
          .count()
        if (fallback < 1) {
          throw new Error('accessible per-day text fallback missing')
        }
      } else {
        const emptyLabel = await page
          .locator('text=No tokens served yet.')
          .count()
        if (emptyLabel < 1) throw new Error('empty state label missing')
      }

      mkdirSync(outDir, { recursive: true })
      const outPath = join(
        outDir,
        `khala-tokens-served-history-${state}.png`,
      )
      await page.screenshot({ path: outPath, fullPage: true })

      if (errors.length > 0) {
        throw new Error(`console errors during render:\n${errors.join('\n')}`)
      }
      server.config.logger.info(
        `[capture-khala-tokens-history] OK ${state} -> ${outPath}`,
      )
      await page.close()
    }
  } finally {
    await browser.close()
    await server.close()
  }
}

main().catch(error => {
  console.error(error)
  process.exit(1)
})
