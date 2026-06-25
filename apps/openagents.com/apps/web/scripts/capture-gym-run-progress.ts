// Headless capture + acceptance proof for the public `/gym` live run
// follow-along view (#6261).
//
// Serves the REAL `Gym.view` through the capture harness via Vite (so the actual
// TS modules + shared UI primitives + three-effect mount are exercised), renders
// it in headless Chromium, asserts the live run follow-along panel is present
// (in-progress label + three-effect run field + accessible text/table mirror with
// counts / pass-rate / denominator / freshness) and that no raw benchmark content
// leaks, then writes a screenshot to apps/web/docs/gym/ (mirroring the
// /trace capture under apps/web/docs/traces/). No app auth / no network.
//
// Run: `bun run scripts/capture-gym-run-progress.ts`

import { mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { chromium } from 'playwright'
import { createServer } from 'vite'

const here = dirname(fileURLToPath(import.meta.url))
const webRoot = join(here, '..')
const outDir = join(webRoot, 'docs', 'gym')

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
    <script type="module" src="/scripts/capture-gym-run-progress.harness.ts"></script>
  </body>
</html>`

const main = async (): Promise<void> => {
  const server = await createServer({
    root: webRoot,
    server: { port: 0 },
    plugins: [
      {
        name: 'gym-run-progress-capture-harness',
        configureServer(s) {
          s.middlewares.use('/__gym-run-progress-capture', (_req, res) => {
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
    const page = await browser.newPage({
      viewport: { width: 1280, height: 1600 },
      deviceScaleFactor: 2,
    })
    page.on('console', m => {
      if (m.type() === 'error') errors.push(m.text())
    })
    page.on('pageerror', e => errors.push(String(e)))

    await page.goto(`http://localhost:${port}/__gym-run-progress-capture`, {
      waitUntil: 'load',
    })
    await page.waitForFunction(
      () => Reflect.get(window, '__gymRunProgressCaptureMounted') === true,
    )
    await page.waitForSelector('[data-gym-run-progress-panel]')
    // Give the three-effect canvas a moment to size/paint.
    await page.waitForTimeout(600)

    const out = await page.content()
    const must = [
      'data-gym-run-progress-panel',
      'Live Gym run follow-along',
      'data-three-effect-scene="gym-run-progress"',
      'data-gym-run-progress-in-progress="true"',
      'data-gym-run-progress-decision-grade="false"',
      'data-gym-run-progress-accessible-mirror',
      'Completed of official denominator',
      '41 of 89',
      'Pass rate over completed tasks',
      'Last updated',
    ]
    for (const token of must) {
      if (!out.includes(token)) {
        throw new Error(`/gym run-progress render missing "${token}"`)
      }
    }
    const forbidden = ['private_openai_compat', 'Bearer ', 'sk-live']
    for (const token of forbidden) {
      if (out.includes(token)) {
        throw new Error(`/gym run-progress render leaked "${token}"`)
      }
    }

    mkdirSync(outDir, { recursive: true })
    const path = join(outDir, '2026-06-25-gym-run-progress-followalong.png')
    await page.screenshot({ path, fullPage: true })
    server.config.logger.info(`[capture-gym-run-progress] wrote ${path}`)

    // A focused screenshot of just the follow-along panel for the proof.
    const panel = await page.$('[data-gym-run-progress-panel]')
    if (panel !== null) {
      const panelPath = join(
        outDir,
        '2026-06-25-gym-run-progress-followalong-panel.png',
      )
      await panel.screenshot({ path: panelPath })
      server.config.logger.info(`[capture-gym-run-progress] wrote ${panelPath}`)
    }
    await page.close()

    if (errors.length > 0) {
      throw new Error(`console errors during render:\n${errors.join('\n')}`)
    }
    server.config.logger.info('[capture-gym-run-progress] OK')
  } finally {
    await browser.close()
    await server.close()
  }
}

main().catch(error => {
  console.error(error)
  process.exit(1)
})
