// Headless capture + acceptance proof for the `/landing` glow grid.
//
// Renders the REAL landing scene (`mountLandingSquares` — the same dense grid
// of blue HDR-emissive squares routed through the EffectComposer ->
// UnrealBloomPass -> OutputPass chain that the live page uses) plus the centred
// "OpenAgents" wordmark in headless Chromium, then:
//   - writes the screenshot to docs/landing-page-screenshot.png
//   - asserts the canvas mounted, the wordmark is centred, the bloom signal is
//     present (bright BLUE pixels), and no console errors fired.
//
// HONEST CAVEAT: this is headless Chromium WebGL (SwiftShader, software GL), not
// a GPU. It proves the composer/bloom pass runs and the squares carry HDR blue
// signal that blooms on black; it does not prove the exact on-device GPU look.
//
// A tiny Vite dev server transforms the scene TS so we exercise the actual
// module, not a re-implementation. Run: `bun run scripts/capture-landing.ts`.

import { mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { inflateSync } from 'node:zlib'

import { chromium } from 'playwright'
import { createServer } from 'vite'

// Minimal PNG -> RGBA decoder (no external deps): parse IHDR + IDAT, inflate,
// and undo the per-scanline filters. Sufficient for the 8-bit RGBA screenshots
// Playwright produces. Mirrors the inline decoder in autopilot-desktop's
// headless-pixel helper so the landing capture has no new dependencies.
type DecodedPng = Readonly<{
  channels: number
  data: Uint8Array
  width: number
  height: number
}>

const decodePng = (png: Buffer): DecodedPng => {
  if (png.readUInt32BE(0) !== 0x89504e47) throw new Error('not a PNG')
  let offset = 8
  let width = 0
  let height = 0
  let channels = 4
  const idat: Buffer[] = []
  while (offset < png.length) {
    const length = png.readUInt32BE(offset)
    const type = png.toString('ascii', offset + 4, offset + 8)
    const body = png.subarray(offset + 8, offset + 8 + length)
    if (type === 'IHDR') {
      width = body.readUInt32BE(0)
      height = body.readUInt32BE(4)
      const bitDepth = body.readUInt8(8)
      const colorType = body.readUInt8(9)
      // colorType 2 = RGB (3 channels), 6 = RGBA (4 channels).
      channels = colorType === 2 ? 3 : 4
      if (bitDepth !== 8 || (colorType !== 6 && colorType !== 2)) {
        throw new Error(`unsupported PNG (bitDepth=${bitDepth} colorType=${colorType})`)
      }
    } else if (type === 'IDAT') {
      idat.push(body)
    } else if (type === 'IEND') {
      break
    }
    offset += 12 + length
  }
  const raw = inflateSync(Buffer.concat(idat))
  const stride = width * channels
  const out = new Uint8Array(width * height * channels)
  let prev = new Uint8Array(stride)
  let pos = 0
  for (let y = 0; y < height; y += 1) {
    const filter = raw[pos]
    pos += 1
    const line = raw.subarray(pos, pos + stride)
    pos += stride
    const cur = new Uint8Array(stride)
    for (let x = 0; x < stride; x += 1) {
      const a = x >= channels ? cur[x - channels] : 0
      const b = prev[x]
      const c = x >= channels ? prev[x - channels] : 0
      let value = line[x]
      switch (filter) {
        case 1:
          value = (value + a) & 0xff
          break
        case 2:
          value = (value + b) & 0xff
          break
        case 3:
          value = (value + ((a + b) >> 1)) & 0xff
          break
        case 4: {
          const p = a + b - c
          const pa = Math.abs(p - a)
          const pb = Math.abs(p - b)
          const pc = Math.abs(p - c)
          const pr = pa <= pb && pa <= pc ? a : pb <= pc ? b : c
          value = (value + pr) & 0xff
          break
        }
        default:
          break
      }
      cur[x] = value
    }
    out.set(cur, y * stride)
    prev = cur
  }
  return { channels, data: out, width, height }
}

const here = dirname(fileURLToPath(import.meta.url))
const webRoot = join(here, '..')
const outPath = join(webRoot, 'docs', 'landing-page-screenshot.png')

// A minimal harness page that mounts the real scene module and the centred
// wordmark. Served by Vite so the `.ts` import is transformed on the fly.
const HARNESS_HTML = `<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <style>
      html, body { margin: 0; height: 100%; background: #000; overflow: hidden; }
      #root { position: absolute; inset: 0; }
      #scene { position: absolute; inset: 0; }
      #wordmark {
        position: absolute; inset: 0; z-index: 10; display: flex;
        align-items: center; justify-content: center; pointer-events: none;
      }
      #wordmark span {
        color: #fff; font-weight: 600; letter-spacing: -0.02em;
        font-family: ui-sans-serif, system-ui, -apple-system, sans-serif;
        font-size: clamp(48px, 9vw, 128px); user-select: none;
      }
    </style>
  </head>
  <body>
    <div id="root">
      <div id="scene"></div>
      <div id="wordmark"><span>OpenAgents</span></div>
    </div>
    <script type="module">
      import { mountLandingSquares } from '/src/scene/landingSquares.ts'
      mountLandingSquares(document.getElementById('scene'))
      window.__landingMounted = true
    </script>
  </body>
</html>`

const main = async (): Promise<void> => {
  const server = await createServer({
    root: webRoot,
    server: { port: 0 },
    // Serve our harness HTML at /__landing-capture instead of index.html.
    plugins: [
      {
        name: 'landing-capture-harness',
        configureServer(s) {
          s.middlewares.use('/__landing-capture', (_req, res) => {
            res.setHeader('Content-Type', 'text/html')
            res.end(HARNESS_HTML)
          })
        },
      },
    ],
  })
  await server.listen()
  const address = server.httpServer?.address()
  const port =
    address && typeof address === 'object' ? address.port : undefined
  if (port === undefined) throw new Error('vite dev server did not bind a port')
  const url = `http://localhost:${port}/__landing-capture`

  const browser = await chromium.launch({
    args: ['--use-gl=swiftshader', '--enable-unsafe-swiftshader'],
  })
  const errors: string[] = []
  try {
    const page = await browser.newPage({
      viewport: { width: 1280, height: 800 },
      deviceScaleFactor: 2,
    })
    page.on('console', message => {
      if (message.type() === 'error') errors.push(message.text())
    })
    page.on('pageerror', error => errors.push(String(error)))

    await page.goto(url, { waitUntil: 'load' })
    await page.waitForFunction(
      () => Reflect.get(window, '__landingMounted') === true,
    )
    // Let a few animation frames run so the composer renders a glowing frame.
    await page.waitForTimeout(800)

    // Assert: canvas mounted, wordmark centred.
    const canvasCount = await page.locator('#scene canvas').count()
    if (canvasCount < 1) throw new Error('landing canvas did not mount')
    const wordmark = await page.locator('#wordmark span').first().innerText()
    if (wordmark.trim() !== 'OpenAgents') {
      throw new Error(`unexpected wordmark text: ${wordmark}`)
    }

    mkdirSync(dirname(outPath), { recursive: true })
    const buffer = await page.screenshot({ path: outPath })

    // Bloom signal: decode the captured frame and count bright BLUE pixels (the
    // HDR-emissive squares glowing on near-black). Blue must clearly dominate
    // red/green and be bright. We read the PNG the composer actually produced,
    // not the WebGL canvas (which clears its buffer after compositing).
    const { channels, data } = decodePng(buffer)
    let brightBlue = 0
    let maxBlue = 0
    for (let i = 0; i < data.length; i += channels) {
      const r = data[i]
      const g = data[i + 1]
      const b = data[i + 2]
      if (b > maxBlue) maxBlue = b
      // A glowing blue pixel: clearly bright blue, blue dominates red/green.
      if (b >= 90 && b > r + 25 && b > g + 10) brightBlue += 1
    }
    const probe = { brightBlue, maxBlue, sampled: (data.length / channels) | 0 }

    server.config.logger.info(
      `[capture-landing] canvas=${canvasCount} brightBluePixels=${probe.brightBlue} maxBlue=${probe.maxBlue} consoleErrors=${errors.length}`,
    )

    if (errors.length > 0) {
      throw new Error(`console errors during render:\n${errors.join('\n')}`)
    }
    // ~384 small glowing squares on a 2560x1600 frame produce tens of
    // thousands of bright-blue pixels; require a clear floor so a black or
    // bloom-broken frame fails loudly.
    if (probe.brightBlue < 5000) {
      throw new Error(
        `bloom signal too weak: only ${probe.brightBlue} bright-blue pixels (maxBlue=${probe.maxBlue})`,
      )
    }

    server.config.logger.info(
      `[capture-landing] OK — wrote ${outPath} (${buffer.length} bytes)`,
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
