// Replay clip regression smoke (EPIC #5411, issue #5434).
//
// Runs on owned local/CI/Container render infrastructure, never inside the
// Cloudflare Worker. It renders one curated replay, the same replay with a
// different camera path, and one generated public timeline replay bundle. The
// command fails closed on blank frames, missing WebGL render-surface metadata,
// equal camera hashes, or source/caveat-incomplete clip manifests.
import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { basename, dirname, isAbsolute, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { inflateSync } from 'node:zlib'

import {
  buildManifest,
  objectKeysForRender,
  sha256File,
} from './render-job.mjs'

const here = dirname(fileURLToPath(import.meta.url))
const webRoot = resolve(here, '../..')
const renderClipScript = resolve(here, 'render-clip.mjs')

const defaultGeneratedFrom = '2026-06-18T00:00:00.000Z'
const defaultGeneratedTo = '2026-06-19T00:00:00.000Z'
const defaultOutDir = resolve(tmpdir(), 'openagents-replay-clip-regression')
const defaultPublicHost = 'https://clips.openagents.com'
const pngSignature = Buffer.from([
  0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
])

const usage = `Usage:
  node spike/replay-r1/render-regression-smoke.mjs [options]

Options:
  --out <dir>               Output directory. Default: ${defaultOutDir}
  --origin <url>            Public origin. Default: https://openagents.com
  --public-host <url>       Public clip host used for manifest validation.
                            Default: ${defaultPublicHost}
  --curated-slug <slug>     Curated replay slug. Default: first-real-settlement
  --curated-start <sec>     Curated render start. Default: 32
  --generated-from <iso>    Generated timeline lower bound.
                            Default: ${defaultGeneratedFrom}
  --generated-to <iso>      Generated timeline upper bound.
                            Default: ${defaultGeneratedTo}
  --generated-limit <n>     Generated timeline event limit. Default: 20
  --generated-start <sec>   Generated replay render start. Default: 0
  --duration <sec>          Clip duration. Default: 1
  --fps <n>                 Frames per second. Default: 1
  --width <px>              Render width. Default: 640
  --height <px>             Render height. Default: 360
  --ffmpeg <path>           ffmpeg binary. Default: ffmpeg
  --help                    Show this message.
`

const numericArgNames = new Set([
  'curatedStart',
  'duration',
  'fps',
  'generatedLimit',
  'generatedStart',
  'height',
  'width',
])

export const parseArgs = argv => {
  const args = {
    curatedSlug: 'first-real-settlement',
    curatedStart: 32,
    duration: 1,
    ffmpeg: 'ffmpeg',
    fps: 1,
    generatedFrom: defaultGeneratedFrom,
    generatedLimit: 20,
    generatedStart: 0,
    generatedTo: defaultGeneratedTo,
    height: 360,
    origin: 'https://openagents.com',
    out: defaultOutDir,
    publicHost: defaultPublicHost,
    width: 640,
  }

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    if (arg === '--help' || arg === '-h') {
      args.help = true
      continue
    }
    if (!arg.startsWith('--')) {
      throw new Error(`Unexpected positional argument: ${arg}`)
    }
    const key = arg
      .slice(2)
      .replace(/-([a-z])/g, (_, char) => char.toUpperCase())
    const next = argv[index + 1]
    if (next === undefined || next.startsWith('--')) {
      throw new Error(`Missing value for ${arg}`)
    }
    index += 1
    args[key] = numericArgNames.has(key) ? Number(next) : next
  }

  return args
}

const outputPathFor = value =>
  isAbsolute(value) ? value : resolve(process.cwd(), value)

const assertPositive = (value, label) => {
  if (!Number.isFinite(value) || value <= 0) {
    throw new Error(`${label} must be a positive number`)
  }
  return value
}

const assertNonnegative = (value, label) => {
  if (!Number.isFinite(value) || value < 0) {
    throw new Error(`${label} must be a non-negative number`)
  }
  return value
}

export const generatedReplayUrl = ({
  generatedFrom,
  generatedLimit,
  generatedTo,
  origin,
}) => {
  const url = new URL('/api/public/proof-replays', origin)
  url.searchParams.set('mode', 'activity-timeline')
  url.searchParams.set('from', generatedFrom)
  url.searchParams.set('to', generatedTo)
  url.searchParams.set('limit', String(generatedLimit))
  return url.toString()
}

const runCommand = (command, commandArgs, options = {}) =>
  new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(command, commandArgs, {
      cwd: webRoot,
      stdio: ['ignore', 'inherit', 'inherit'],
      ...options,
    })
    child.on('error', rejectPromise)
    child.on('close', code => {
      if (code === 0) {
        resolvePromise()
        return
      }
      rejectPromise(new Error(`${command} exited ${code ?? 'unknown'}`))
    })
  })

const sha256Bytes = bytes => createHash('sha256').update(bytes).digest('hex')

export const sha256Path = async path => sha256Bytes(await readFile(path))

const fetchJson = async url => {
  const response = await fetch(url, { headers: { accept: 'application/json' } })
  if (!response.ok) {
    throw new Error(`Fetch failed (${response.status}) from ${url}`)
  }
  return response.json()
}

const privateMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer\s|cookie|customer[_-]|invoice[_-]?raw|lnbc|lntb|lnbcrt|lno1|lnurl|local[_-]?path|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|invoice|preimage|raw|secret)|payout[_-]?(address|destination|private|target)|preimage|private[_-]?(archive|customer|key|repo|source|trace|wallet)|provider[_-]?(credential|secret|token)|raw[_-]?(artifact|auth|customer|invoice|log|payment|provider|record|repo|runner|source|trace)|secret|seed[_-]?phrase|sk-[a-z0-9]|token[_-]?secret|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed))/i

const assertNoPrivateMaterial = (value, label) => {
  const encoded = typeof value === 'string' ? value : JSON.stringify(value)
  if (privateMaterialPattern.test(encoded)) {
    throw new Error(`${label} contains private/raw material`)
  }
}

const refObjectToString = ref => {
  if (typeof ref === 'string') return ref
  if (ref === null || typeof ref !== 'object') return String(ref)
  if (typeof ref.url === 'string') return ref.url
  if (typeof ref.kind === 'string' && typeof ref.ref === 'string') {
    return `${ref.kind}:${ref.ref}`
  }
  return `public_ref:${sha256Bytes(Buffer.from(JSON.stringify(ref)))}`
}

export const normalizeSourceRefs = refs =>
  [...new Set(refs.map(refObjectToString).filter(ref => ref.length > 0))].map(
    ref => {
      assertNoPrivateMaterial(ref, 'sourceRef')
      return ref
    },
  )

const generatedBundleSourceRefs = (bundle, url) =>
  normalizeSourceRefs([url, ...normalizeArray(bundle.sourceRefs)].slice(0, 64))

const normalizeArray = value => (Array.isArray(value) ? value : [])

const uniqueStrings = values =>
  [...new Set(values.filter(value => typeof value === 'string' && value !== ''))]

const generatedBundleCaveatRefs = bundle =>
  uniqueStrings([
    ...normalizeArray(bundle.caveatRefs),
    ...normalizeArray(bundle.generatedFrom?.caveatRefs),
  ])

const assertGeneratedBundleUsable = (bundle, url) => {
  if (bundle === null || typeof bundle !== 'object') {
    throw new Error('Generated replay bundle must be an object')
  }
  if (!Array.isArray(bundle.events) || bundle.events.length === 0) {
    throw new Error('Generated replay bundle must carry at least one event')
  }
  if (!Array.isArray(bundle.sourceRefs) || bundle.sourceRefs.length === 0) {
    throw new Error('Generated replay bundle must carry sourceRefs')
  }
  if (generatedBundleCaveatRefs(bundle).length === 0) {
    throw new Error('Generated replay bundle must carry caveat refs')
  }
  assertNoPrivateMaterial(generatedBundleSourceRefs(bundle, url), 'sourceRefs')
  return bundle
}

const renderClip = async ({
  bundleUrl,
  camera,
  duration,
  ffmpeg,
  fps,
  height,
  out,
  slug,
  start,
  width,
}) => {
  const args = [
    renderClipScript,
    '--start',
    String(start),
    '--duration',
    String(duration),
    '--fps',
    String(fps),
    '--width',
    String(width),
    '--height',
    String(height),
    '--camera',
    camera,
    '--out',
    out,
    '--ffmpeg',
    ffmpeg,
  ]
  if (slug !== undefined) {
    args.push('--slug', slug)
  }
  if (bundleUrl !== undefined) {
    args.push('--bundle-url', bundleUrl)
  }

  await runCommand(process.execPath, args)
  return JSON.parse(await readFile(`${out}.render.json`, 'utf8'))
}

const paeth = (left, up, upLeft) => {
  const p = left + up - upLeft
  const pa = Math.abs(p - left)
  const pb = Math.abs(p - up)
  const pc = Math.abs(p - upLeft)
  if (pa <= pb && pa <= pc) return left
  return pb <= pc ? up : upLeft
}

const channelsForColorType = colorType => {
  if (colorType === 0) return 1
  if (colorType === 2) return 3
  if (colorType === 4) return 2
  if (colorType === 6) return 4
  throw new Error(`Unsupported PNG color type: ${colorType}`)
}

export const analyzePngBytes = (bytes, label = 'frame') => {
  if (!bytes.subarray(0, pngSignature.length).equals(pngSignature)) {
    throw new Error(`${label} is not a PNG`)
  }

  let offset = pngSignature.length
  let width
  let height
  let bitDepth
  let colorType
  let interlace
  const idatChunks = []

  while (offset < bytes.length) {
    const length = bytes.readUInt32BE(offset)
    const type = bytes.subarray(offset + 4, offset + 8).toString('ascii')
    const data = bytes.subarray(offset + 8, offset + 8 + length)
    offset += 12 + length

    if (type === 'IHDR') {
      width = data.readUInt32BE(0)
      height = data.readUInt32BE(4)
      bitDepth = data[8]
      colorType = data[9]
      interlace = data[12]
    } else if (type === 'IDAT') {
      idatChunks.push(data)
    } else if (type === 'IEND') {
      break
    }
  }

  if (
    width === undefined ||
    height === undefined ||
    bitDepth === undefined ||
    colorType === undefined
  ) {
    throw new Error(`${label} is missing PNG IHDR metadata`)
  }
  if (bitDepth !== 8) {
    throw new Error(`${label} must be an 8-bit PNG`)
  }
  if (interlace !== 0) {
    throw new Error(`${label} must be a non-interlaced PNG`)
  }

  const channels = channelsForColorType(colorType)
  const bytesPerPixel = channels
  const rowLength = width * bytesPerPixel
  const inflated = inflateSync(Buffer.concat(idatChunks))
  let sourceOffset = 0
  let previous = Buffer.alloc(rowLength)

  const totalPixels = width * height
  const sampleStep = Math.max(1, Math.floor(totalPixels / 20_000))
  const lumaBuckets = new Set()
  const colorBuckets = new Set()
  let sampledPixels = 0
  let nonNearBlackPixels = 0
  let nonTransparentPixels = 0
  let lumaMin = Number.POSITIVE_INFINITY
  let lumaMax = 0
  let lumaSum = 0
  let lumaSquares = 0

  for (let y = 0; y < height; y += 1) {
    const filter = inflated[sourceOffset]
    sourceOffset += 1
    const row = Buffer.alloc(rowLength)

    for (let x = 0; x < rowLength; x += 1) {
      const raw = inflated[sourceOffset]
      sourceOffset += 1
      const left = x >= bytesPerPixel ? row[x - bytesPerPixel] : 0
      const up = previous[x] ?? 0
      const upLeft = x >= bytesPerPixel ? previous[x - bytesPerPixel] : 0

      let value
      if (filter === 0) value = raw
      else if (filter === 1) value = raw + left
      else if (filter === 2) value = raw + up
      else if (filter === 3) value = raw + Math.floor((left + up) / 2)
      else if (filter === 4) value = raw + paeth(left, up, upLeft)
      else throw new Error(`${label} has unsupported PNG filter ${filter}`)
      row[x] = value & 0xff
    }

    for (let x = 0; x < width; x += 1) {
      const pixelIndex = y * width + x
      if (pixelIndex % sampleStep !== 0) continue
      const pixelOffset = x * channels
      let red
      let green
      let blue
      let alpha = 255
      if (colorType === 0) {
        red = green = blue = row[pixelOffset]
      } else if (colorType === 2) {
        red = row[pixelOffset]
        green = row[pixelOffset + 1]
        blue = row[pixelOffset + 2]
      } else if (colorType === 4) {
        red = green = blue = row[pixelOffset]
        alpha = row[pixelOffset + 1]
      } else {
        red = row[pixelOffset]
        green = row[pixelOffset + 1]
        blue = row[pixelOffset + 2]
        alpha = row[pixelOffset + 3]
      }

      const luma = 0.2126 * red + 0.7152 * green + 0.0722 * blue
      sampledPixels += 1
      if (alpha > 0) nonTransparentPixels += 1
      if (alpha > 0 && luma > 8) nonNearBlackPixels += 1
      lumaMin = Math.min(lumaMin, luma)
      lumaMax = Math.max(lumaMax, luma)
      lumaSum += luma
      lumaSquares += luma * luma
      lumaBuckets.add(Math.floor(luma / 8))
      colorBuckets.add(`${red >> 4},${green >> 4},${blue >> 4}`)
    }

    previous = row
  }

  const lumaMean = sampledPixels === 0 ? 0 : lumaSum / sampledPixels
  const lumaVariance =
    sampledPixels === 0 ? 0 : lumaSquares / sampledPixels - lumaMean * lumaMean

  return {
    bitDepth,
    colorType,
    distinctColorBuckets: colorBuckets.size,
    distinctLumaBuckets: lumaBuckets.size,
    height,
    lumaMax,
    lumaMean,
    lumaMin: Number.isFinite(lumaMin) ? lumaMin : 0,
    lumaStddev: Math.sqrt(Math.max(0, lumaVariance)),
    nonNearBlackPixels,
    nonTransparentPixels,
    sampleStep,
    sampledPixels,
    width,
  }
}

export const analyzePngFrame = async path => ({
  path,
  ...analyzePngBytes(await readFile(path), path),
})

export const assertFrameNonblank = metrics => {
  const minimumLitPixels = Math.max(
    1,
    Math.min(20, Math.ceil(metrics.sampledPixels * 0.005)),
  )
  if (metrics.width <= 0 || metrics.height <= 0) {
    throw new Error(`${metrics.path ?? 'frame'} has invalid dimensions`)
  }
  if (metrics.nonTransparentPixels < metrics.sampledPixels * 0.95) {
    throw new Error(`${metrics.path ?? 'frame'} is mostly transparent`)
  }
  if (metrics.nonNearBlackPixels < minimumLitPixels) {
    throw new Error(`${metrics.path ?? 'frame'} appears blank or black`)
  }
  if (metrics.distinctLumaBuckets < 4 && metrics.distinctColorBuckets < 4) {
    throw new Error(`${metrics.path ?? 'frame'} has too little pixel variety`)
  }
  if (metrics.lumaStddev < 2) {
    throw new Error(`${metrics.path ?? 'frame'} has too little contrast`)
  }
}

export const assertWebglSurface = (renderManifest, label) => {
  const firstFrame = renderManifest.frames?.[0]
  const surface = firstFrame?.renderSurface
  if (surface === undefined || surface.hasShadow !== true) {
    throw new Error(`${label} render manifest is missing WebGL surface metadata`)
  }
  if (surface.webglState !== 'available') {
    throw new Error(`${label} WebGL state is ${surface.webglState ?? 'missing'}`)
  }
  if (!Number.isFinite(surface.canvasCount) || surface.canvasCount < 1) {
    throw new Error(`${label} did not render a canvas`)
  }
  if (
    !surface.canvasInfo?.some(
      canvas => canvas.proofReplayWebgl === 'available',
    )
  ) {
    throw new Error(`${label} did not report an available proof replay canvas`)
  }
  return surface
}

export const assertClipManifestComplete = manifest => {
  if (manifest.schemaVersion !== 'openagents.replay_clip_manifest.v1') {
    throw new Error('Clip manifest schemaVersion is invalid')
  }
  if (manifest.claimScope !== 'evidence_presentation_only') {
    throw new Error('Clip manifest claimScope is invalid')
  }
  if (!Array.isArray(manifest.sourceRefs) || manifest.sourceRefs.length === 0) {
    throw new Error('Clip manifest must carry source refs')
  }
  if (!Array.isArray(manifest.caveatRefs) || manifest.caveatRefs.length === 0) {
    throw new Error('Clip manifest must carry caveat refs')
  }
  if (!Array.isArray(manifest.artifacts) || manifest.artifacts.length === 0) {
    throw new Error('Clip manifest must carry artifacts')
  }
  if (!Number.isFinite(manifest.frameCount) || manifest.frameCount <= 0) {
    throw new Error('Clip manifest frameCount must be positive')
  }
  for (const artifact of manifest.artifacts) {
    if (!/^https:\/\//i.test(artifact.storageUrl)) {
      throw new Error('Clip manifest artifact storageUrl must be public https')
    }
    if (!/^[a-f0-9]{64}$/i.test(artifact.sha256)) {
      throw new Error('Clip manifest artifact sha256 must be a hex digest')
    }
    if (!Number.isFinite(artifact.byteSize) || artifact.byteSize <= 0) {
      throw new Error('Clip manifest artifact byteSize must be positive')
    }
  }
  assertNoPrivateMaterial(manifest, 'clip manifest')
  return manifest
}

const buildRegressionManifest = async ({
  extraCaveatRefs = [],
  job,
  out,
  publicHost,
  renderManifest,
}) => {
  const { mp4ObjectKey } = objectKeysForRender({
    job,
    out,
    prefix: 'regression/replay-clips',
  })
  const fileStat = await stat(out)
  const manifest = buildManifest({
    artifacts: [
      {
        byteSize: fileStat.size,
        objectKey: mp4ObjectKey,
        sha256: await sha256File(out),
      },
    ],
    bundleRef: renderManifest.bundleRef ?? job.source.bundleRef ?? 'unknown',
    frameCount: renderManifest.frameCount ?? 0,
    job,
    publicHost,
  })
  manifest.caveatRefs = uniqueStrings([
    ...manifest.caveatRefs,
    ...extraCaveatRefs,
  ])
  assertClipManifestComplete(manifest)

  const manifestPath = `${out}.clip-manifest.json`
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`)
  return { manifest, manifestPath }
}

const cameraPathDsl = {
  schemaVersion: 'openagents.replay_camera_path.v1',
  keyframes: [
    { second: 0, verb: 'hold' },
    { second: 0.5, verb: 'orbit', fov: 54 },
    { second: 1, verb: 'frame_settlement' },
  ],
}

const clipJobFor = ({ jobRef, render, source, sourceRefs }) => ({
  cameraPath: cameraPathDsl,
  jobRef,
  render: { ...render, outputKind: 'mp4' },
  schemaVersion: 'openagents.replay_clip_job.v1',
  source,
  sourceRefs,
})

const collectRenderOutput = async ({
  extraCaveatRefs,
  job,
  label,
  out,
  publicHost,
  renderManifest,
}) => {
  const surface = assertWebglSurface(renderManifest, label)
  const firstFrame = renderManifest.frames?.[0]?.path
  if (typeof firstFrame !== 'string') {
    throw new Error(`${label} render manifest does not name its first frame`)
  }
  const frameAnalysis = await analyzePngFrame(firstFrame)
  assertFrameNonblank(frameAnalysis)
  const frameSha256 = await sha256Path(firstFrame)
  const { manifest, manifestPath } = await buildRegressionManifest({
    extraCaveatRefs,
    job,
    out,
    publicHost,
    renderManifest,
  })

  return {
    bundleRef: renderManifest.bundleRef,
    clipManifest: manifestPath,
    clipManifestArtifactUrl: manifest.artifacts[0]?.storageUrl,
    firstFrame,
    frameAnalysis,
    frameSha256,
    mp4: out,
    renderManifest: `${out}.render.json`,
    renderSurface: surface,
    sourceRefs: manifest.sourceRefs,
  }
}

export const main = async () => {
  const args = parseArgs(process.argv.slice(2))
  if (args.help) {
    console.log(usage)
    return
  }
  assertPositive(args.duration, 'duration')
  assertPositive(args.fps, 'fps')
  assertPositive(args.width, 'width')
  assertPositive(args.height, 'height')
  assertPositive(args.generatedLimit, 'generatedLimit')
  assertNonnegative(args.curatedStart, 'curatedStart')
  assertNonnegative(args.generatedStart, 'generatedStart')
  if (!/^https:\/\//i.test(args.publicHost)) {
    throw new Error('--public-host must be a public https URL')
  }

  const outDir = outputPathFor(args.out)
  await mkdir(outDir, { recursive: true })

  const generatedUrl = generatedReplayUrl(args)
  console.log(`[regression] fetching generated bundle: ${generatedUrl}`)
  const generatedBundle = assertGeneratedBundleUsable(
    await fetchJson(generatedUrl),
    generatedUrl,
  )
  const generatedCaveats = generatedBundleCaveatRefs(generatedBundle)

  const render = {
    durationSecond: args.duration,
    fps: args.fps,
    height: args.height,
    startSecond: 0,
    width: args.width,
  }
  const curatedRender = { ...render, startSecond: args.curatedStart }
  const generatedRender = { ...render, startSecond: args.generatedStart }
  const curatedSourceRef = `${args.origin.replace(/\/+$/, '')}/api/public/tassadar-replays/${encodeURIComponent(args.curatedSlug)}`
  const generatedSourceRefs = generatedBundleSourceRefs(
    generatedBundle,
    generatedUrl,
  )

  const curatedMainOut = resolve(outDir, 'curated-main.mp4')
  const curatedAltOut = resolve(outDir, 'curated-alt-camera.mp4')
  const generatedOut = resolve(outDir, 'generated-timeline.mp4')
  const primaryCamera = resolve(here, 'camera-path.example.json')
  const alternateCamera = resolve(here, 'camera-path-alt.example.json')

  console.log('[regression] rendering curated replay')
  const curatedMainRender = await renderClip({
    camera: primaryCamera,
    duration: args.duration,
    ffmpeg: args.ffmpeg,
    fps: args.fps,
    height: args.height,
    out: curatedMainOut,
    slug: args.curatedSlug,
    start: args.curatedStart,
    width: args.width,
  })

  console.log('[regression] rendering curated replay with alternate camera')
  const curatedAltRender = await renderClip({
    camera: alternateCamera,
    duration: args.duration,
    ffmpeg: args.ffmpeg,
    fps: args.fps,
    height: args.height,
    out: curatedAltOut,
    slug: args.curatedSlug,
    start: args.curatedStart,
    width: args.width,
  })

  console.log('[regression] rendering generated timeline replay')
  const generatedRenderManifest = await renderClip({
    bundleUrl: generatedUrl,
    camera: primaryCamera,
    duration: args.duration,
    ffmpeg: args.ffmpeg,
    fps: args.fps,
    height: args.height,
    out: generatedOut,
    start: args.generatedStart,
    width: args.width,
  })

  const curatedMain = await collectRenderOutput({
    extraCaveatRefs: [],
    job: clipJobFor({
      jobRef: `replay_clip_job.regression.${args.curatedSlug}.main`,
      render: curatedRender,
      source: { kind: 'replay_bundle', bundleRef: args.curatedSlug },
      sourceRefs: [curatedSourceRef],
    }),
    label: 'curated main',
    out: curatedMainOut,
    publicHost: args.publicHost,
    renderManifest: curatedMainRender,
  })
  const curatedAlt = await collectRenderOutput({
    extraCaveatRefs: [],
    job: clipJobFor({
      jobRef: `replay_clip_job.regression.${args.curatedSlug}.alt_camera`,
      render: curatedRender,
      source: { kind: 'replay_bundle', bundleRef: args.curatedSlug },
      sourceRefs: [curatedSourceRef],
    }),
    label: 'curated alternate camera',
    out: curatedAltOut,
    publicHost: args.publicHost,
    renderManifest: curatedAltRender,
  })
  const generated = await collectRenderOutput({
    extraCaveatRefs: generatedCaveats,
    job: clipJobFor({
      jobRef: 'replay_clip_job.regression.generated_public_activity',
      render: generatedRender,
      source: {
        fromCursor: `${args.generatedFrom}:public_activity_timeline:regression.from`,
        kind: 'timeline_range',
        toCursor: `${args.generatedTo}:public_activity_timeline:regression.to`,
      },
      sourceRefs: generatedSourceRefs,
    }),
    label: 'generated timeline',
    out: generatedOut,
    publicHost: args.publicHost,
    renderManifest: generatedRenderManifest,
  })

  if (curatedMain.frameSha256 === curatedAlt.frameSha256) {
    throw new Error(
      'Curated primary and alternate camera paths produced the same first-frame hash',
    )
  }

  const summary = {
    checks: {
      curatedAlternateCameraDiffers: true,
      generatedManifestHasCaveats: generatedCaveats.length > 0,
      generatedManifestHasSourceRefs: generated.sourceRefs.length > 0,
      nonblankFrames: true,
      webglFrames: true,
    },
    claimScope: 'evidence_presentation_only',
    createdAt: new Date().toISOString(),
    generatedReplay: {
      bundleRef: generatedRenderManifest.bundleRef,
      caveatRefs: generatedCaveats,
      eventCount: generatedBundle.events.length,
      url: generatedUrl,
    },
    outputs: {
      curatedAlt,
      curatedMain,
      generated,
    },
    render: {
      curatedStartSecond: args.curatedStart,
      durationSecond: args.duration,
      fps: args.fps,
      generatedStartSecond: args.generatedStart,
      height: args.height,
      width: args.width,
    },
    schemaVersion: 'openagents.replay_clip_regression.v1',
  }

  const summaryPath = resolve(outDir, 'regression-summary.json')
  await writeFile(summaryPath, `${JSON.stringify(summary, null, 2)}\n`)
  console.log(`[regression] summary written: ${summaryPath}`)
  console.log(
    `[regression] camera frame hashes differ: ${basename(curatedMain.firstFrame)}=${curatedMain.frameSha256.slice(0, 12)} ${basename(curatedAlt.firstFrame)}=${curatedAlt.frameSha256.slice(0, 12)}`,
  )
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch(error => {
    console.error('[regression] FAILED:', error)
    process.exit(1)
  })
}
