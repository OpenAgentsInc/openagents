// Replay R-1 headless render spike — driver.
//
// Question this answers: can the EXISTING three-effect/proof-replay replay
// scene be rendered to pixels in headless Chrome, driven programmatically (no
// interactive UI, no human)?
//
// What it does:
//   1. boots a vite dev server rooted at the web app (real package resolution)
//   2. opens the spike page in headless Chromium via Playwright
//   3. drives the scene to ONE moment + ONE camera pose through window hooks
//   4. page.screenshot() -> a single PNG
//
// Run from apps/openagents.com/apps/web:
//   bunx playwright install chromium   # one-time
//   node spike/replay-r1/render-one-frame.mjs
//
// NOTE: this is a render-box / CI workload (headless Chrome), NOT a Cloudflare
// Worker workload. The Worker can trigger a render and serve the mp4 from R2;
// it must not render.

import { createServer } from 'vite'
import { chromium } from 'playwright'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'

const here = dirname(fileURLToPath(import.meta.url))
const webRoot = resolve(here, '../..') // apps/openagents.com/apps/web
const outPath = resolve(here, 'frame.png')

const SECOND = 24 // the payment_zap_confirmed beat in the fixture bundle
const CAMERA = 'zap_focus'
const WIDTH = 1280
const HEIGHT = 720

const main = async () => {
  const server = await createServer({
    root: webRoot,
    configFile: resolve(webRoot, 'vite.config.ts'),
    server: { port: 0 },
    logLevel: 'warn',
  })
  await server.listen()
  const address = server.httpServer?.address()
  if (address === null || typeof address !== 'object') {
    throw new Error('vite dev server did not report an address')
  }
  const url = `http://localhost:${address.port}/spike/replay-r1/index.html`
  console.log(`[spike] vite dev server: ${url}`)

  const browser = await chromium.launch({ headless: true })
  const consoleErrors = []
  try {
    const page = await browser.newPage({
      viewport: { width: WIDTH, height: HEIGHT },
      deviceScaleFactor: 1,
    })
    page.on('console', msg => {
      if (msg.type() === 'error') consoleErrors.push(msg.text())
    })
    page.on('pageerror', err => consoleErrors.push(String(err)))

    await page.goto(url, { waitUntil: 'networkidle' })
    await page.waitForFunction(() => window.replaySpikeReady === true, {
      timeout: 15_000,
    })

    // Diagnostic: what controls did the real scene actually render?
    const controlsPresent = await page.evaluate(() => {
      const root = document.getElementById('replay-spike-scene')?.shadowRoot
      if (root == null) return { hasShadow: false }
      return {
        hasShadow: true,
        dataState: root.host.getAttribute('data-state'),
        presentation: root.host.getAttribute('data-replay-presentation'),
        hasScrub: root.querySelector('[data-replay-control="scrub"]') !== null,
        hasCamera: root.querySelector('[data-replay-control="camera"]') !== null,
        controlRefs: Array.from(
          root.querySelectorAll('[data-replay-control]'),
        ).map(el => el.getAttribute('data-replay-control')),
      }
    })
    console.log(
      '[spike] controls present:',
      JSON.stringify(controlsPresent, null, 2),
    )

    // Drive ONE moment + ONE camera pose, programmatically.
    await page.evaluate(
      async ([second, camera]) => {
        if (
          document
            .getElementById('replay-spike-scene')
            ?.shadowRoot?.querySelector('[data-replay-control="camera"]') !=
          null
        ) {
          await window.setCamera(camera)
        }
        await window.setReplaySecond(second)
      },
      [SECOND, CAMERA],
    )
    await page.waitForTimeout(150)

    // What does the scene think its state is? (real scene attributes)
    const sceneState = await page.evaluate(() => {
      const el = document.getElementById('replay-spike-scene')
      return {
        dataState: el?.getAttribute('data-state') ?? null,
        replaySecond: el?.getAttribute('data-replay-second') ?? null,
        cameraMode: el?.getAttribute('data-replay-camera') ?? null,
        cameraPose: window.replaySpikeCameraPose ?? null,
      }
    })

    // Is there a real WebGL canvas anywhere in the rendered scene?
    const renderSurface = await page.evaluate(() => {
      const host = document.getElementById('replay-spike-scene')
      const root = host?.shadowRoot ?? null
      if (root === null) return { hasShadow: false }
      const canvases = Array.from(root.querySelectorAll('canvas'))
      const canvasInfo = canvases.map(c => {
        let gl = null
        try {
          gl =
            c.getContext('webgl2') !== null
              ? 'webgl2'
              : c.getContext('webgl') !== null
                ? 'webgl'
                : c.getContext('2d') !== null
                  ? '2d'
                  : 'none'
        } catch {
          gl = 'error'
        }
        return { width: c.width, height: c.height, context: gl }
      })
      const domNodeCount = root.querySelectorAll('*').length
      const planeNodes = root.querySelectorAll(
        '.plane .stage, .plane .actor, .plane .zap, .plane .marker',
      ).length
      return {
        hasShadow: true,
        canvasCount: canvases.length,
        canvasInfo,
        domNodeCount,
        projectionNodeCount: planeNodes,
      }
    })

    await page.screenshot({ path: outPath })

    // Second capture: the scene's own widget-free "social" presentation mode
    // (?camera=social), the closest thing today to a directed clip frame. This
    // shows the same headless harness can produce a frame without the widget
    // suite the owner does not want — still a DOM projection, not true 3D.
    const socialOut = resolve(here, 'frame-social.png')
    const socialPage = await browser.newPage({
      viewport: { width: WIDTH, height: HEIGHT },
      deviceScaleFactor: 1,
    })
    await socialPage.goto(
      `${url}?camera=social&hud=social&duration=60&start=24`,
      { waitUntil: 'networkidle' },
    )
    await socialPage.waitForFunction(() => window.replaySpikeReady === true, {
      timeout: 15_000,
    })
    await socialPage.waitForTimeout(250)
    await socialPage.screenshot({ path: socialOut })
    await socialPage.close()

    console.log('\n[spike] === RESULT ===')
    console.log('[spike] screenshot written:', outPath)
    console.log('[spike] social-mode screenshot written:', socialOut)
    console.log('[spike] scene state:', JSON.stringify(sceneState, null, 2))
    console.log(
      '[spike] render surface:',
      JSON.stringify(renderSurface, null, 2),
    )
    if (consoleErrors.length > 0) {
      console.log('[spike] page console errors:', consoleErrors)
    }
  } finally {
    await browser.close()
    await server.close()
  }
}

main().catch(err => {
  console.error('[spike] FAILED:', err)
  process.exit(1)
})
