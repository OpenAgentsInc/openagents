// Headless capture + acceptance proof for the /pro eval + run pages (issue 6184).
//
// Serves the REAL page views (`evalDetailView` / `runDetailView`) through the
// capture harness via Vite (so the actual TS modules + shared Pro primitives are
// exercised), renders them in headless Chromium, asserts the comparison table +
// per-variant videos + deltas are present, and writes screenshots to
// docs/pro/. Mirrors scripts/capture-landing.ts. No app auth / no network.
//
// Run: `bun run scripts/capture-pro.ts`

import { mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { chromium } from 'playwright'
import { createServer } from 'vite'

const here = dirname(fileURLToPath(import.meta.url))
const webRoot = join(here, '..')
const outDir = join(webRoot, 'docs', 'pro')

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
    <script type="module" src="/scripts/capture-pro-evals.harness.ts"></script>
  </body>
</html>`

const main = async (): Promise<void> => {
  const server = await createServer({
    root: webRoot,
    server: { port: 0 },
    plugins: [
      {
        name: 'pro-capture-harness',
        configureServer(s) {
          s.middlewares.use('/__pro-capture', (_req, res) => {
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
      viewName:
        | 'eval'
        | 'run'
        | 'run-refuted'
        | 'run-multitarget'
        | 'run-multitarget-block',
      file: string,
      assertions: (out: string) => void,
    ): Promise<void> => {
      const page = await browser.newPage({
        viewport: { width: 1280, height: 900 },
        deviceScaleFactor: 2,
      })
      page.on('console', m => {
        if (m.type() === 'error') errors.push(`[${viewName}] ${m.text()}`)
      })
      page.on('pageerror', e => errors.push(`[${viewName}] ${String(e)}`))

      await page.goto(`http://localhost:${port}/__pro-capture?view=${viewName}`, {
        waitUntil: 'load',
      })
      await page.waitForFunction(
        () => Reflect.get(window, '__proCaptureMounted') === true,
      )
      await page.waitForSelector('[data-component="pro-console"]')
      await page.waitForTimeout(150)

      const out = await page.content()
      assertions(out)

      mkdirSync(outDir, { recursive: true })
      const path = join(outDir, file)
      await page.screenshot({ path, fullPage: true })
      server.config.logger.info(`[capture-pro] wrote ${path}`)
      await page.close()
    }

    await capture('eval', 'pro-evals-screenshot.png', out => {
      const must = [
        'pro-eval-comparison',
        'MCP on',
        'MCP off',
        'pass-rate',
        '-100%',
        'pro-video-pane',
        'Illustrative',
        // #6192: the verify verdict renders on the eval page (REFUTED candidate)
        'pro-verdict-pill',
        'REFUTED',
      ]
      for (const token of must) {
        if (!out.includes(token)) {
          throw new Error(`/pro/evals render missing "${token}"`)
        }
      }
    })

    await capture('run', 'pro-run-screenshot.png', out => {
      const must = [
        'pro-step-table',
        'pro-video-pane',
        'pro-status-pill',
        // #6192: the verify verdict renders on the run page (CONFIRMED)
        'pro-verdict-pill',
        'CONFIRMED',
        'pro-verdict-evidence',
      ]
      for (const token of must) {
        if (!out.includes(token)) {
          throw new Error(`/pro/runs render missing "${token}"`)
        }
      }
    })

    // #6192: also capture the REFUTED run page — proof a false claim renders as
    // a refuted verdict (not a fake pass) on /pro.
    await capture('run-refuted', 'pro-run-refuted-screenshot.png', out => {
      const must = ['pro-verdict-pill', 'REFUTED', 'contradicting evidence']
      for (const token of must) {
        if (!out.includes(token)) {
          throw new Error(`/pro/runs refuted render missing "${token}"`)
        }
      }
    })

    // #6190: the MULTI-TARGET run page — proof the SAME scenario's per-target
    // results (dev/staging/prod) show side by side on /pro, with the per-target
    // restriction policy (read-only on prod).
    await capture('run-multitarget', 'pro-run-multitarget-screenshot.png', out => {
      const must = [
        'pro-target-matrix',
        'dev',
        'staging',
        'prod',
        'pro-restriction-badge',
        'read-only',
        'writable',
        'pro-status-pill',
      ]
      for (const token of must) {
        if (!out.includes(token)) {
          throw new Error(`/pro/runs multi-target render missing "${token}"`)
        }
      }
    })

    // #6190: the read-only-BLOCK multi-target run page — proof a read-only target
    // blocks a mutating step honestly (the failure reason renders, not a fake
    // pass) on /pro.
    await capture(
      'run-multitarget-block',
      'pro-run-multitarget-block-screenshot.png',
      out => {
        const must = [
          'pro-target-matrix',
          'pro-target-failure',
          'restriction violation',
          'read-only',
          'never create data',
        ]
        for (const token of must) {
          if (!out.includes(token)) {
            throw new Error(
              `/pro/runs read-only-block render missing "${token}"`,
            )
          }
        }
      },
    )

    if (errors.length > 0) {
      throw new Error(`console errors during render:\n${errors.join('\n')}`)
    }
    server.config.logger.info('[capture-pro] OK')
  } finally {
    await browser.close()
    await server.close()
  }
}

main().catch(error => {
  console.error(error)
  process.exit(1)
})
