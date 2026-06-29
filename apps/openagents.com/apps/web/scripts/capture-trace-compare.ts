// Headless capture + acceptance proof for the public `/trace/compare/{ids}`
// comparison view (issue #6211 — the real "chill-evals").
//
// Serves the REAL page view (`TraceCompare.view`) through the capture harness
// via Vite (so the actual TS modules + shared UI primitives are exercised),
// renders it in headless Chromium, asserts the comparison table renders >= 2
// sample traces side by side with deltas + per-trace deep links (and that an
// unknown id renders an honest unknown column + an empty list renders an honest
// empty state), and writes screenshots to docs/traces/. Mirrors
// scripts/capture-trace.ts. No app auth / no network.
//
// Run: `bun run scripts/capture-trace-compare.ts`

import { mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { chromium } from 'playwright'
import { createServer } from 'vite'

const here = dirname(fileURLToPath(import.meta.url))
const webRoot = join(here, '..')
const outDir = join(webRoot, 'docs', 'traces')

const HARNESS_HTML = `<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <link href="/src/styles.css" rel="stylesheet" />
    <style>
      html, body { margin: 0; height: 100%; background: #000; }
      #root { min-height: 100vh; }
    </style>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/scripts/capture-trace-compare.harness.ts"></script>
  </body>
</html>`

const main = async (): Promise<void> => {
  const server = await createServer({
    root: webRoot,
    server: { port: 0 },
    plugins: [
      {
        name: 'trace-compare-capture-harness',
        configureServer(s) {
          s.middlewares.use('/__trace-compare-capture', (_req, res) => {
            res.setHeader('Content-Type', 'text/html')
            res.end(HARNESS_HTML)
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
  const errors: string[] = []
  try {
    const capture = async (
      viewName: 'found' | 'unknown' | 'empty',
      file: string,
      assertions: (out: string) => void,
    ): Promise<void> => {
      const page = await browser.newPage({
        viewport: { width: 1280, height: 1000 },
        deviceScaleFactor: 2,
      })
      page.on('console', m => {
        if (m.type() === 'error') errors.push(`[${viewName}] ${m.text()}`)
      })
      page.on('pageerror', e => errors.push(`[${viewName}] ${String(e)}`))

      await page.goto(
        `http://localhost:${port}/__trace-compare-capture?view=${viewName}`,
        { waitUntil: 'load' },
      )
      await page.waitForFunction(
        () => Reflect.get(window, '__traceCompareCaptureMounted') === true,
      )
      await page.waitForSelector('#trace-compare-capture-root')
      await page.waitForTimeout(150)

      const out = await page.content()
      assertions(out)

      mkdirSync(outDir, { recursive: true })
      const path = join(outDir, file)
      await page.screenshot({ path, fullPage: true })
      server.config.logger.info(`[capture-trace-compare] wrote ${path}`)
      await page.close()
    }

    await capture('found', 'trace-compare-screenshot.png', out => {
      const must = [
        'data-component="trace-compare-page"',
        'data-component="trace-compare-table"',
        // >= 2 variants side by side: baseline + at least one variant column.
        'Baseline',
        'Variant 1',
        'Variant 2',
        // Verdicts from each variant's real final_metrics.
        'Verified',
        'Refuted',
        // Per-trace metrics rows.
        'Latency',
        'Steps',
        'Cost',
        '11.5s', // baseline duration
        '13.9s', // mcp-on duration
        // Honest deltas vs baseline.
        'data-component="trace-compare-delta"',
        // Per-trace deep links to the full /trace/{uuid}.
        'data-component="trace-compare-deeplink"',
        '/trace/0e08d2db-2026-4624-9a39-f1efe8000002',
      ]
      for (const token of must) {
        if (!out.includes(token)) {
          throw new Error(`/trace/compare render missing "${token}"`)
        }
      }
    })

    await capture('unknown', 'trace-compare-unknown-screenshot.png', out => {
      const must = [
        'data-component="trace-compare-table"',
        'Unknown id',
        '1 unknown',
      ]
      for (const token of must) {
        if (!out.includes(token)) {
          throw new Error(`/trace/compare unknown render missing "${token}"`)
        }
      }
    })

    await capture('empty', 'trace-compare-empty-screenshot.png', out => {
      const must = [
        'data-component="trace-compare-empty"',
        'Nothing to compare',
      ]
      for (const token of must) {
        if (!out.includes(token)) {
          throw new Error(`/trace/compare empty render missing "${token}"`)
        }
      }
      if (out.includes('data-component="trace-compare-table"')) {
        throw new Error('/trace/compare empty render should NOT include a table')
      }
    })

    if (errors.length > 0) {
      throw new Error(`console errors during render:\n${errors.join('\n')}`)
    }
    server.config.logger.info('[capture-trace-compare] OK')
  } finally {
    await browser.close()
    await server.close()
  }
}

main().catch(error => {
  console.error(error)
  process.exit(1)
})
