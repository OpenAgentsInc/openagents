// Headless capture + acceptance proof for the public `/trace/{uuid}` page
// (issue #6209).
//
// Serves the REAL page view (`Trace.view`) through the capture harness via Vite
// (so the actual TS modules + shared UI primitives are exercised), renders it in
// headless Chromium, asserts the header + step timeline + tool calls +
// observations + video + final metrics are present (and that an unknown uuid
// renders an honest 404), and writes screenshots to docs/traces/. Mirrors
// scripts/capture-pro.ts. No app auth / no network.
//
// Run: `bun run scripts/capture-trace.ts`

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
    <script type="module" src="/scripts/capture-trace.harness.ts"></script>
  </body>
</html>`

const main = async (): Promise<void> => {
  const server = await createServer({
    root: webRoot,
    server: { port: 0 },
    plugins: [
      {
        name: 'trace-capture-harness',
        configureServer(s) {
          s.middlewares.use('/__trace-capture', (_req, res) => {
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
      viewName: 'found' | 'not-found' | 'skeleton',
      file: string,
      assertions: (out: string) => void,
    ): Promise<void> => {
      const page = await browser.newPage({
        viewport: { width: 1280, height: 1100 },
        deviceScaleFactor: 2,
      })
      page.on('console', m => {
        if (m.type() === 'error') errors.push(`[${viewName}] ${m.text()}`)
      })
      page.on('pageerror', e => errors.push(`[${viewName}] ${String(e)}`))

      await page.goto(
        `http://localhost:${port}/__trace-capture?view=${viewName}`,
        { waitUntil: 'load' },
      )
      await page.waitForFunction(
        () => Reflect.get(window, '__traceCaptureMounted') === true,
      )
      await page.waitForSelector('#trace-capture-root')
      await page.waitForTimeout(150)

      const out = await page.content()
      assertions(out)

      mkdirSync(outDir, { recursive: true })
      const path = join(outDir, file)
      await page.screenshot({ path, fullPage: true })
      server.config.logger.info(`[capture-trace] wrote ${path}`)
      await page.close()
    }

    await capture('found', 'trace-page-screenshot.png', out => {
      const must = [
        'data-component="trace-page"',
        // Header: agent, model, verdict, duration, cost.
        'openagents-qa-runner',
        'openagents/khala',
        'Verified',
        '11.5s',
        '$0.00',
        // Timeline + goal + steps.
        'data-component="trace-timeline"',
        'Verify the login page works on this site',
        'Step 2',
        'Step 7',
        // Tool calls + arguments.
        'data-component="trace-tool-call"',
        'navigate()',
        'done()',
        '/login',
        // Observations.
        'data-component="trace-observation"',
        'verification_class=test_passed',
        // Video.
        'data-component="pro-video-pane"',
        '/pro-assets/sample-session.webm',
        // Final metrics.
        'data-component="trace-final-metrics"',
        // Per-step deep-link anchors.
        'id="step-2"',
        'Copy link to this step',
      ]
      for (const token of must) {
        if (!out.includes(token)) {
          throw new Error(`/trace render missing "${token}"`)
        }
      }
    })

    await capture('not-found', 'trace-not-found-screenshot.png', out => {
      const must = [
        'data-component="trace-not-found"',
        'No trace at this link',
      ]
      for (const token of must) {
        if (!out.includes(token)) {
          throw new Error(`/trace 404 render missing "${token}"`)
        }
      }
      if (out.includes('data-component="trace-timeline"')) {
        throw new Error('/trace 404 render should NOT include a timeline')
      }
    })

    await capture('skeleton', 'trace-skeleton-screenshot.png', out => {
      const must = ['data-component="trace-skeleton"', 'animate-pulse']
      for (const token of must) {
        if (!out.includes(token)) {
          throw new Error(`/trace skeleton render missing "${token}"`)
        }
      }
    })

    if (errors.length > 0) {
      throw new Error(`console errors during render:\n${errors.join('\n')}`)
    }
    server.config.logger.info('[capture-trace] OK')
  } finally {
    await browser.close()
    await server.close()
  }
}

main().catch(error => {
  console.error(error)
  process.exit(1)
})
