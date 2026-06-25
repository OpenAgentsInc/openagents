// Headless verification + screenshots for the shared "← OpenAgents" back button
// on /khala and /tassadar. Boots a Vite dev server (transforms the real app
// modules), navigates the real SPA to each route, asserts the back button is
// present, screenshots both, then clicks the /khala back button and asserts the
// SPA navigates home (/). Run: bun run scripts/capture-khala-tassadar-back.ts
import { mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { chromium } from 'playwright'
import { createServer } from 'vite'

const here = dirname(fileURLToPath(import.meta.url))
const webRoot = join(here, '..')
const outDir = join(webRoot, 'docs')

const main = async (): Promise<void> => {
  const server = await createServer({ root: webRoot, server: { port: 0 } })
  await server.listen()
  const address = server.httpServer?.address()
  const port = address && typeof address === 'object' ? address.port : undefined
  if (port === undefined) throw new Error('vite dev server did not bind a port')
  const base = `http://localhost:${port}`

  const browser = await chromium.launch({
    args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader'],
  })
  try {
    const page = await browser.newPage({
      viewport: { width: 1280, height: 800 },
      deviceScaleFactor: 2,
    })
    mkdirSync(outDir, { recursive: true })

    const captureRoute = async (
      route: string,
      backSelector: string,
      file: string,
    ): Promise<void> => {
      await page.goto(`${base}${route}`, { waitUntil: 'load' })
      await page.waitForSelector(backSelector, { timeout: 10000 })
      await page.waitForTimeout(700)
      const label = (await page.locator(backSelector).innerText()).replace(/\s+/g, ' ').trim()
      if (!label.toLowerCase().includes('openagents')) {
        throw new Error(`${route}: back button text unexpected: ${label}`)
      }
      // The accessible name must be present for a screen-reader-usable control.
      const named = await page
        .getByRole('button', { name: 'Back to OpenAgents home' })
        .count()
      if (named < 1) {
        throw new Error(`${route}: no button with accessible name "Back to OpenAgents home"`)
      }
      await page.screenshot({ path: join(outDir, file) })
      server.config.logger.info(`[capture-back] ${route} OK back="${label.replace(/\s+/g, ' ').trim()}"`)
    }

    await captureRoute('/khala', '[data-khala-back="home"]', 'khala-back-button.png')
    await captureRoute('/tassadar', '[data-tassadar-back="home"]', 'tassadar-back-button.png')

    // Click the /khala back button and assert the SPA navigates to home (/).
    await page.goto(`${base}/khala`, { waitUntil: 'load' })
    await page.waitForSelector('[data-khala-back="home"]', { timeout: 10000 })
    await page.click('[data-khala-back="home"]')
    await page.waitForFunction(
      () => window.location.pathname === '/',
      undefined,
      { timeout: 10000 },
    )
    await page.waitForSelector('[data-landing-wordmark="openagents"]', {
      timeout: 10000,
    })
    server.config.logger.info(
      `[capture-back] /khala back -> navigated to ${await page.evaluate(() => window.location.pathname)} (landing wordmark present)`,
    )
    server.config.logger.info('[capture-back] ALL OK')
  } finally {
    await browser.close()
    await server.close()
  }
}

main().catch(error => {
  console.error(error)
  process.exit(1)
})
