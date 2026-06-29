// Replay R-3/R-4/R-5 headless clip renderer.
//
// This ports the Remotion pattern, not Remotion: headless Chromium screenshots
// deterministic replay frames, then ffmpeg stitches those PNGs into an mp4.
//
// Run from apps/openagents.com/apps/web:
//   node spike/replay-r1/render-clip.mjs --start 20 --duration 5 --fps 12 --out spike/replay-r1/clip.mp4
//
// Render-box workload only. Do not run this inside a Cloudflare Worker.
import { spawn } from 'node:child_process'
import { mkdir, readFile, readdir, unlink, writeFile } from 'node:fs/promises'
import {
  basename,
  dirname,
  extname,
  isAbsolute,
  join,
  resolve,
} from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium } from 'playwright'
import { createServer } from 'vite'

const here = dirname(fileURLToPath(import.meta.url))
const webRoot = resolve(here, '../..')
const defaultOut = resolve(here, 'clip.mp4')
const cameraModes = new Set([
  'director_track',
  'overview',
  'follow_actor',
  'orbit_proof',
  'zap_focus',
  'free_camera',
])

const usage = `Usage:
  node spike/replay-r1/render-clip.mjs [options]

Options:
  --slug <slug>             Load a public replay bundle from openagents.com.
                            Known: first-real-settlement, launch-recognition-payments.
  --bundle-url <url>        Load a replay bundle JSON from an explicit URL.
  --bundle-file <path>      Load a replay bundle JSON from disk.
  --start <seconds>         Bundle second to start at. Default: 20.
  --duration <seconds>      Clip duration. Default: 4.
  --end <seconds>           End second; overrides --duration.
  --fps <number>            Target frame rate. Default: 12.
  --width <px>              Viewport width. Default: 1280.
  --height <px>             Viewport height. Default: 720.
  --camera <mode|json|file> Camera mode or camera keyframe path.
                            Modes: director_track, overview, follow_actor,
                            orbit_proof, zap_focus, free_camera.
  --presentation <mode>     social or interactive. Default: social.
  --frames-dir <path>       Frame output directory. Default: <out>.frames.
  --out <path>              Output mp4 path. Default: spike/replay-r1/clip.mp4.
  --ffmpeg <path>           ffmpeg binary. Default: ffmpeg.
  --crf <number>            libx264 CRF. Default: 20.
  --preset <name>           libx264 preset. Default: medium.
  --keep-frames             Accepted for explicitness; frame PNGs are retained.
  --help                    Show this message.

Camera path JSON:
  {"keyframes":[
    {"second":0,"mode":"overview"},
    {"second":2,"mode":"zap_focus"},
    {"second":4,"position":{"x":4,"y":3,"z":6},"target":{"x":0,"y":0,"z":0}}
  ]}
`

const parseArgs = argv => {
  const args = {
    camera: 'director_track',
    crf: 20,
    duration: 4,
    ffmpeg: 'ffmpeg',
    fps: 12,
    height: 720,
    keepFrames: true,
    out: defaultOut,
    presentation: 'social',
    preset: 'medium',
    start: 20,
    width: 1280,
  }

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    if (arg === '--help' || arg === '-h') {
      console.log(usage)
      process.exit(0)
    }

    if (arg === '--keep-frames') {
      args.keepFrames = true
      continue
    }

    if (!arg.startsWith('--')) {
      throw new Error(`Unexpected positional argument: ${arg}`)
    }

    const key = arg.slice(2)
    const next = argv[index + 1]
    if (next === undefined || next.startsWith('--')) {
      throw new Error(`Missing value for ${arg}`)
    }
    index += 1

    if (
      key === 'start' ||
      key === 'duration' ||
      key === 'end' ||
      key === 'fps' ||
      key === 'width' ||
      key === 'height' ||
      key === 'crf'
    ) {
      args[key] = Number(next)
      continue
    }

    if (
      key === 'slug' ||
      key === 'bundle-url' ||
      key === 'bundle-file' ||
      key === 'camera' ||
      key === 'presentation' ||
      key === 'frames-dir' ||
      key === 'out' ||
      key === 'ffmpeg' ||
      key === 'preset' ||
      key === 'origin'
    ) {
      args[key.replace(/-([a-z])/g, (_, char) => char.toUpperCase())] = next
      continue
    }

    throw new Error(`Unknown option: ${arg}`)
  }

  return args
}

const finiteNumber = (value, label) => {
  if (!Number.isFinite(value))
    throw new Error(`${label} must be a finite number`)
  return value
}

const positiveNumber = (value, label) => {
  const number = finiteNumber(value, label)
  if (number <= 0) throw new Error(`${label} must be greater than 0`)
  return number
}

const outputPathFor = value =>
  isAbsolute(value) ? value : resolve(process.cwd(), value)

const slugEndpoint = (slug, origin) => {
  const base = origin ?? 'https://openagents.com'
  if (slug === 'first-real-settlement') {
    return `${base}/api/public/tassadar-replays/first-real-settlement`
  }
  return `${base}/api/public/proof-replays?ref=${encodeURIComponent(slug)}`
}

const loadBundle = async args => {
  if (args.bundleFile !== undefined) {
    const path = outputPathFor(args.bundleFile)
    return {
      bundle: JSON.parse(await readFile(path, 'utf8')),
      source: `file:${path}`,
    }
  }

  const bundleUrl =
    args.bundleUrl ??
    (args.slug === undefined ? undefined : slugEndpoint(args.slug, args.origin))
  if (bundleUrl === undefined) {
    return { bundle: null, source: 'fixture:spikeReplayBundle' }
  }

  const response = await fetch(bundleUrl, {
    headers: { accept: 'application/json' },
  })
  if (!response.ok) {
    throw new Error(
      `Bundle fetch failed (${response.status}) from ${bundleUrl}`,
    )
  }
  return { bundle: await response.json(), source: bundleUrl }
}

const parseVector = value => {
  if (value === undefined) return undefined
  if (
    typeof value !== 'object' ||
    value === null ||
    !Number.isFinite(value.x) ||
    !Number.isFinite(value.y) ||
    !Number.isFinite(value.z)
  ) {
    throw new Error(
      'Camera keyframe vectors must be {x:number,y:number,z:number}',
    )
  }
  return { x: value.x, y: value.y, z: value.z }
}

const normalizeCameraKeyframe = value => {
  if (typeof value !== 'object' || value === null) {
    throw new Error('Camera keyframes must be objects')
  }
  const second = Number(value.second ?? 0)
  if (!Number.isFinite(second) || second < 0) {
    throw new Error('Camera keyframe second must be a non-negative number')
  }
  const mode = value.mode
  if (mode !== undefined && !cameraModes.has(mode)) {
    throw new Error(`Unknown camera keyframe mode: ${mode}`)
  }
  const fov = value.fov === undefined ? undefined : Number(value.fov)
  if (fov !== undefined && !Number.isFinite(fov)) {
    throw new Error('Camera keyframe fov must be a finite number')
  }
  return {
    fov,
    mode,
    position: parseVector(value.position),
    second,
    target: parseVector(value.target),
  }
}

const readCameraPlan = async value => {
  if (cameraModes.has(value)) {
    return {
      keyframes: [{ mode: value, second: 0 }],
      source: `mode:${value}`,
    }
  }

  let parsed
  if (value.trim().startsWith('{') || value.trim().startsWith('[')) {
    parsed = JSON.parse(value)
  } else {
    const path = outputPathFor(value)
    parsed = JSON.parse(await readFile(path, 'utf8'))
  }

  const keyframesInput = Array.isArray(parsed) ? parsed : parsed.keyframes
  if (!Array.isArray(keyframesInput) || keyframesInput.length === 0) {
    throw new Error('Camera path must provide a non-empty keyframes array')
  }

  return {
    keyframes: keyframesInput
      .map(normalizeCameraKeyframe)
      .sort((left, right) => left.second - right.second),
    source: value,
  }
}

const lerp = (start, end, t) => start + (end - start) * t

const lerpVector = (start, end, t) =>
  start === undefined || end === undefined
    ? undefined
    : {
        x: lerp(start.x, end.x, t),
        y: lerp(start.y, end.y, t),
        z: lerp(start.z, end.z, t),
      }

const resolveCameraAt = (cameraPlan, relativeSecond) => {
  const keyframes = cameraPlan.keyframes
  const before =
    [...keyframes]
      .reverse()
      .find(keyframe => keyframe.second <= relativeSecond) ?? keyframes[0]
  const after =
    keyframes.find(keyframe => keyframe.second >= relativeSecond) ?? before
  const span = Math.max(0, after.second - before.second)
  const t = span === 0 ? 0 : (relativeSecond - before.second) / span
  const interpolatedPosition = lerpVector(before.position, after.position, t)
  const interpolatedTarget = lerpVector(before.target, after.target, t)
  const hasRequestedPose =
    interpolatedPosition !== undefined || interpolatedTarget !== undefined

  return {
    cameraMode:
      after.mode ??
      before.mode ??
      (hasRequestedPose ? 'free_camera' : 'director_track'),
    requestedPose: !hasRequestedPose
      ? undefined
      : {
          fov:
            before.fov === undefined || after.fov === undefined
              ? (before.fov ?? after.fov)
              : lerp(before.fov, after.fov, t),
          position: interpolatedPosition,
          target: interpolatedTarget,
        },
  }
}

const cleanFramesDir = async framesDir => {
  await mkdir(framesDir, { recursive: true })
  const entries = await readdir(framesDir)
  await Promise.all(
    entries
      .filter(name => /^frame_\d{5}\.png$/.test(name))
      .map(name => unlink(join(framesDir, name))),
  )
}

const padFrameIndex = index => String(index).padStart(5, '0')

const runCommand = (command, args, options = {}) =>
  new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(command, args, {
      stdio: ['ignore', 'pipe', 'pipe'],
      ...options,
    })
    let stdout = ''
    let stderr = ''
    child.stdout?.on('data', chunk => {
      stdout += String(chunk)
    })
    child.stderr?.on('data', chunk => {
      stderr += String(chunk)
    })
    child.on('error', rejectPromise)
    child.on('close', code => {
      if (code === 0) {
        resolvePromise({ stdout, stderr })
        return
      }
      rejectPromise(
        new Error(
          `${command} exited ${code ?? 'unknown'}\n${stdout}\n${stderr}`.trim(),
        ),
      )
    })
  })

const probeRenderSurface = async page =>
  page.evaluate(() => {
    const host = document.getElementById('replay-spike-scene')
    const root = host?.shadowRoot ?? null
    if (root === null) return { hasShadow: false }

    const stage = root.querySelector('[data-replay-stage]')
    const canvases = Array.from(root.querySelectorAll('canvas'))
    const projectionNodeCount = root.querySelectorAll(
      '.plane .stage, .plane .actor, .plane .zap, .plane .marker',
    ).length

    return {
      canvasCount: canvases.length,
      canvasInfo: canvases.map(canvas => ({
        camera: canvas.getAttribute('data-proof-replay-camera'),
        height: canvas.height,
        proofReplayWebgl: canvas.getAttribute('data-proof-replay-webgl'),
        second: canvas.getAttribute('data-proof-replay-second'),
        width: canvas.width,
      })),
      hasShadow: true,
      projectionNodeCount,
      stageCamera: stage?.getAttribute('data-camera-mode') ?? null,
      stagePose: stage?.getAttribute('data-camera-pose') ?? null,
      webglState: stage?.getAttribute('data-proof-replay-webgl') ?? null,
    }
  })

const ensureFfmpeg = async ffmpeg => {
  try {
    await runCommand(ffmpeg, ['-version'])
  } catch (error) {
    throw new Error(
      `ffmpeg is required for mp4 encoding. Install it or pass --ffmpeg <path>.\n${String(error)}`,
    )
  }
}

const encodeMp4 = async ({ crf, ffmpeg, fps, framesDir, out, preset }) => {
  await mkdir(dirname(out), { recursive: true })
  const pattern = join(framesDir, 'frame_%05d.png')
  await runCommand(ffmpeg, [
    '-y',
    '-framerate',
    String(fps),
    '-i',
    pattern,
    '-c:v',
    'libx264',
    '-preset',
    preset,
    '-crf',
    String(crf),
    '-pix_fmt',
    'yuv420p',
    '-movflags',
    '+faststart',
    out,
  ])
}

const renderFrames = async ({
  bundle,
  cameraPlan,
  duration,
  fps,
  framesDir,
  height,
  presentation,
  start,
  width,
}) => {
  const server = await createServer({
    root: webRoot,
    configFile: resolve(webRoot, 'vite.config.ts'),
    logLevel: 'warn',
    server: { port: 0 },
  })
  await server.listen()
  const address = server.httpServer?.address()
  if (address === null || typeof address !== 'object') {
    throw new Error('vite dev server did not report an address')
  }

  const query =
    presentation === 'social'
      ? `?camera=social&hud=social&duration=60&start=${encodeURIComponent(String(start))}`
      : ''
  const url = `http://localhost:${address.port}/spike/replay-r1/index.html${query}`
  console.log(`[clip] vite dev server: ${url}`)

  const browser = await chromium.launch({ headless: true })
  try {
    const page = await browser.newPage({
      deviceScaleFactor: 1,
      viewport: { height, width },
    })
    const consoleErrors = []
    page.on('console', msg => {
      if (msg.type() === 'error') consoleErrors.push(msg.text())
    })
    page.on('pageerror', error => consoleErrors.push(String(error)))

    await page.goto(url, { waitUntil: 'networkidle' })
    await page.waitForFunction(() => window.replaySpikeReady === true, {
      timeout: 15_000,
    })
    if (bundle !== null) {
      await page.evaluate(async loadedBundle => {
        await window.loadReplayBundle(loadedBundle)
      }, bundle)
    }
    await page.evaluate(async () => {
      if ('fonts' in document) await document.fonts.ready
    })

    const frameCount = Math.max(1, Math.ceil(duration * fps))
    const frames = []
    for (let index = 0; index < frameCount; index += 1) {
      const clipSecond = index / fps
      const second = start + clipSecond
      const camera = resolveCameraAt(cameraPlan, clipSecond)
      const computedPose = await page.evaluate(
        async input => window.driveReplayFrame(input),
        {
          cameraMode: camera.cameraMode,
          cameraPose: camera.requestedPose,
          second,
        },
      )
      await page.waitForTimeout(30)
      const renderSurface = await probeRenderSurface(page)
      const path = join(framesDir, `frame_${padFrameIndex(index)}.png`)
      await page.screenshot({
        animations: 'disabled',
        path,
      })
      frames.push({
        cameraMode: camera.cameraMode,
        clipSecond,
        computedPose,
        index,
        path,
        renderSurface,
        requestedPose: camera.requestedPose,
        second,
      })
      if (
        index === 0 ||
        index === frameCount - 1 ||
        index % Math.max(1, fps) === 0
      ) {
        console.log(
          `[clip] frame ${index + 1}/${frameCount} second=${second.toFixed(3)} camera=${camera.cameraMode}`,
        )
      }
    }

    if (consoleErrors.length > 0) {
      console.log('[clip] page console errors:', consoleErrors)
    }

    return frames
  } finally {
    await browser.close()
    await server.close()
  }
}

const main = async () => {
  const args = parseArgs(process.argv.slice(2))
  const fps = positiveNumber(args.fps, 'fps')
  const width = positiveNumber(args.width, 'width')
  const height = positiveNumber(args.height, 'height')
  const start = finiteNumber(args.start, 'start')
  if (start < 0) throw new Error('start must be >= 0')
  const duration =
    args.end === undefined
      ? positiveNumber(args.duration, 'duration')
      : positiveNumber(args.end - start, 'end - start')
  const out = outputPathFor(args.out)
  const framesDir = outputPathFor(
    args.framesDir ??
      join(dirname(out), `${basename(out, extname(out) || '.mp4')}.frames`),
  )

  if (args.presentation !== 'social' && args.presentation !== 'interactive') {
    throw new Error('--presentation must be social or interactive')
  }

  await ensureFfmpeg(args.ffmpeg)
  await cleanFramesDir(framesDir)
  const [bundleSource, cameraPlan] = await Promise.all([
    loadBundle(args),
    readCameraPlan(args.camera),
  ])

  console.log(
    `[clip] rendering ${duration.toFixed(3)}s @ ${fps} fps (${width}x${height})`,
  )
  console.log(`[clip] bundle source: ${bundleSource.source}`)
  console.log(`[clip] camera source: ${cameraPlan.source}`)

  const frames = await renderFrames({
    bundle: bundleSource.bundle,
    cameraPlan,
    duration,
    fps,
    framesDir,
    height,
    presentation: args.presentation,
    start,
    width,
  })
  await encodeMp4({
    crf: args.crf,
    ffmpeg: args.ffmpeg,
    fps,
    framesDir,
    out,
    preset: args.preset,
  })

  const manifest = {
    bundleRef: bundleSource.bundle?.bundleRef ?? 'proof_replay_bundle.spike.r1',
    bundleSource: bundleSource.source,
    cameraPlanSource: cameraPlan.source,
    codec: {
      crf: args.crf,
      ffmpeg: args.ffmpeg,
      pixelFormat: 'yuv420p',
      preset: args.preset,
      videoCodec: 'libx264',
    },
    createdAt: new Date().toISOString(),
    durationSecond: duration,
    frameCount: frames.length,
    frames,
    fps,
    output: out,
    presentation: args.presentation,
    renderer: 'playwright-chromium-screenshot-plus-ffmpeg',
    runLocation:
      'local_or_ci_render_box_with_bun_node_headless_chromium_and_ffmpeg_not_cloudflare_worker',
    startSecond: start,
    viewport: { height, width },
  }
  const manifestPath = `${out}.render.json`
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`)

  console.log(`[clip] mp4 written: ${out}`)
  console.log(`[clip] manifest written: ${manifestPath}`)
  console.log(`[clip] frames: ${framesDir}`)
}

main().catch(error => {
  console.error('[clip] FAILED:', error)
  process.exit(1)
})
