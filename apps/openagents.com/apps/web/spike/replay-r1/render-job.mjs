// Replay clip render-box job runner (EPIC #5411, issue #5431).
//
// Promotes the `render-clip.mjs` spike toward a production render-box path:
// it takes a typed clip job (the `openagents.replay_clip_job.v1` shape from
// `@openagentsinc/replay-clips`), drives the existing headless-Chromium +
// ffmpeg renderer, builds a public-safe `openagents.replay_clip_manifest.v1`,
// and uploads the mp4 + manifest to R2 through the S3-compatible API.
//
// RENDER-BOX WORKLOAD ONLY. This must run on owned local/CI/Container
// infrastructure with Node, headless Chromium (Playwright), and ffmpeg. It
// must NEVER run inside the Cloudflare Worker.
//
// NEEDS-OWNER (infra not provisioned here — do not provision without the
// owner):
//   1. An R2 bucket (e.g. `oa-replay-clips`) and a public read host
//      (e.g. `https://clips.openagents.com`) for finished mp4 + manifest
//      objects. Until it exists, this runner renders + builds the manifest
//      locally and reports the upload plan as `needs_owner`.
//   2. Render-box runtime: Node + Playwright Chromium + ffmpeg. The runner
//      preflights these and reports a typed blocker if they are missing.
//   3. A queue/worker process (local/CI/Container) that claims `queued` clip
//      jobs from the Worker's clip-job store (#5432) and runs this module.
//      No new Cloudflare Worker is created here.
//
// Usage (render box):
//   node spike/replay-r1/render-job.mjs --job spike/replay-r1/job.example.json \
//     --out spike/replay-r1/out/clip.mp4 [--upload]
//
// Without --upload (default) the runner renders locally and prints the upload
// plan. With --upload it requires R2 credentials in the environment
// (R2_REPLAY_CLIPS_* — see preflightUpload) and fails closed with a NEEDS-OWNER
// blocker when they are absent.
import { spawn } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFile, stat, writeFile } from 'node:fs/promises'
import { basename, dirname, isAbsolute, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { AwsClient } from 'aws4fetch'

export const here = dirname(fileURLToPath(import.meta.url))
export const renderClipScript = resolve(here, 'render-clip.mjs')

export const REPLAY_CLIP_MANIFEST_SCHEMA_VERSION =
  'openagents.replay_clip_manifest.v1'
export const REPLAY_CLIP_JOB_SCHEMA_VERSION = 'openagents.replay_clip_job.v1'
export const REPLAY_CLIP_CLAIM_SCOPE = 'evidence_presentation_only'

export const RENDERER = {
  pixelFormat: 'yuv420p',
  renderer: 'playwright-chromium-screenshot-plus-ffmpeg',
  rendererVersion: 'replay-r1',
  runLocation:
    'local_or_ci_render_box_with_bun_node_headless_chromium_and_ffmpeg_not_cloudflare_worker',
  videoCodec: 'libx264',
}

// The bounded camera verbs from the #5433 DSL and their render-box camera mode.
export const VERB_TO_MODE = {
  follow: 'follow_actor',
  frame_actor: 'follow_actor',
  frame_settlement: 'zap_focus',
  hold: 'director_track',
  orbit: 'orbit_proof',
}

export const outputPathFor = value =>
  isAbsolute(value) ? value : resolve(process.cwd(), value)

export const parseArgs = argv => {
  const args = { upload: false }
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    if (arg === '--upload') {
      args.upload = true
      continue
    }
    if (arg === '--help' || arg === '-h') {
      args.help = true
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
    args[key] = next
  }
  return args
}

// Validate a clip job against the #5430 contract. This mirrors the Effect
// Schema validation in `@openagentsinc/replay-clips`; the render box stays
// dependency-light so it can run anywhere Node + ffmpeg exist.
export const validateJob = job => {
  if (job === null || typeof job !== 'object') {
    throw new Error('Clip job must be an object')
  }
  if (job.schemaVersion !== REPLAY_CLIP_JOB_SCHEMA_VERSION) {
    throw new Error(
      `Clip job schemaVersion must be ${REPLAY_CLIP_JOB_SCHEMA_VERSION}`,
    )
  }
  const source = job.source
  if (
    source === null ||
    typeof source !== 'object' ||
    (source.kind !== 'replay_bundle' && source.kind !== 'timeline_range')
  ) {
    throw new Error('Clip job source must be replay_bundle or timeline_range')
  }
  const render = job.render
  if (render === null || typeof render !== 'object') {
    throw new Error('Clip job render spec is required')
  }
  for (const [key, min] of [
    ['durationSecond', 0],
    ['fps', 0],
    ['width', 0],
    ['height', 0],
  ]) {
    if (!Number.isFinite(render[key]) || render[key] <= min) {
      throw new Error(`Clip job render.${key} must be greater than ${min}`)
    }
  }
  if (!Number.isFinite(render.startSecond) || render.startSecond < 0) {
    throw new Error('Clip job render.startSecond must be non-negative')
  }
  if (render.outputKind !== 'mp4') {
    throw new Error('Clip job render.outputKind must be mp4')
  }
  if (
    !Array.isArray(job.sourceRefs) ||
    job.sourceRefs.length === 0 ||
    job.sourceRefs.some(ref => typeof ref !== 'string')
  ) {
    throw new Error('Clip job must carry at least one string sourceRef')
  }
  const cameraPath = job.cameraPath
  if (
    cameraPath === null ||
    typeof cameraPath !== 'object' ||
    !Array.isArray(cameraPath.keyframes) ||
    cameraPath.keyframes.length === 0
  ) {
    throw new Error('Clip job cameraPath must carry keyframes')
  }
  return job
}

// Compile the #5433 DSL camera path into the `render-clip.mjs` camera JSON.
export const compileCameraPath = cameraPath => ({
  keyframes: [...cameraPath.keyframes]
    .map(keyframe => {
      const mode = VERB_TO_MODE[keyframe.verb]
      if (mode === undefined) {
        throw new Error(`Unknown camera verb: ${keyframe.verb}`)
      }
      return {
        mode,
        second: keyframe.second,
        ...(keyframe.fov === undefined ? {} : { fov: keyframe.fov }),
      }
    })
    .sort((left, right) => left.second - right.second),
})

export const runCommand = (command, commandArgs, options = {}) =>
  new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(command, commandArgs, {
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

export const sha256File = async path => {
  const bytes = await readFile(path)
  return createHash('sha256').update(bytes).digest('hex')
}

const envValue = (env, key) => {
  const value = env[key]?.trim()
  return value === undefined || value === '' ? undefined : value
}

// R2 credentials are read only from the render-box environment. The runner
// never provisions a bucket and never prints secrets.
export const preflightUpload = (env = process.env) => {
  const bucket = envValue(env, 'R2_REPLAY_CLIPS_BUCKET')
  const publicHost = envValue(env, 'R2_REPLAY_CLIPS_PUBLIC_HOST')
  const accountId = envValue(env, 'R2_REPLAY_CLIPS_ACCOUNT_ID')
  const accessKeyId = envValue(env, 'R2_REPLAY_CLIPS_ACCESS_KEY_ID')
  const secretAccessKey = envValue(env, 'R2_REPLAY_CLIPS_SECRET_ACCESS_KEY')
  const prefix = envValue(env, 'R2_REPLAY_CLIPS_PREFIX') ?? 'replay-clips'
  const missing = [
    ['R2_REPLAY_CLIPS_BUCKET', bucket],
    ['R2_REPLAY_CLIPS_PUBLIC_HOST', publicHost],
    ['R2_REPLAY_CLIPS_ACCOUNT_ID', accountId],
    ['R2_REPLAY_CLIPS_ACCESS_KEY_ID', accessKeyId],
    ['R2_REPLAY_CLIPS_SECRET_ACCESS_KEY', secretAccessKey],
  ]
    .filter(([, value]) => value === undefined)
    .map(([key]) => key)

  if (missing.length > 0) {
    return {
      configured: false,
      blockerRef: 'needs_owner.replay_clip.r2_bucket_not_provisioned',
      detail:
        `Set ${missing.join(', ')} after the owner provisions the R2 bucket.`,
    }
  }
  if (!/^https:\/\//i.test(publicHost)) {
    return {
      configured: false,
      blockerRef: 'config.replay_clip.r2_public_host_invalid',
      detail: 'R2_REPLAY_CLIPS_PUBLIC_HOST must be a public https URL.',
    }
  }
  return {
    configured: true,
    accessKeyId,
    accountId,
    bucket,
    prefix,
    publicHost: publicHost.replace(/\/+$/, ''),
    secretAccessKey,
  }
}

const safeObjectKeySegment = value =>
  String(value)
    .replace(/[^A-Za-z0-9._=-]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .slice(0, 160) || 'local'

export const objectKeysForRender = ({ job, out, prefix = 'replay-clips' }) => {
  const jobRef = job.jobRef ?? `replay_clip_job.${job.source.bundleRef ?? 'local'}`
  const folder = [prefix.replace(/^\/+|\/+$/g, ''), safeObjectKeySegment(jobRef)]
    .filter(part => part !== '')
    .join('/')
  const mp4ObjectKey = `${folder}/${basename(out)}`
  return {
    manifestObjectKey: `${mp4ObjectKey}.clip-manifest.json`,
    mp4ObjectKey,
  }
}

export const publicStorageUrl = (publicHost, objectKey) =>
  `${publicHost.replace(/\/+$/, '')}/${objectKey
    .split('/')
    .map(part => encodeURIComponent(part))
    .join('/')}`

export const buildManifest = ({ artifacts, bundleRef, frameCount, job, publicHost }) => ({
  artifacts: artifacts.map(artifact => ({
    byteSize: artifact.byteSize,
    kind: 'mp4',
    sha256: artifact.sha256,
    storageUrl:
      publicHost === null
        ? `local:${artifact.objectKey}`
        : publicStorageUrl(publicHost, artifact.objectKey),
  })),
  bundleRef,
  cameraPath: job.cameraPath,
  caveatRefs: [
    'Clip is evidence-presentation only and grants no settlement or payout authority.',
    ...(publicHost === null
      ? ['needs_owner.replay_clip.r2_bucket_not_provisioned: storageUrl is a local placeholder until R2 is provisioned.']
      : []),
  ],
  claimScope: REPLAY_CLIP_CLAIM_SCOPE,
  frameCount,
  generatedAt: new Date().toISOString(),
  jobRef: job.jobRef ?? `replay_clip_job.${bundleRef}.local`,
  render: job.render,
  renderer: RENDERER,
  schemaVersion: REPLAY_CLIP_MANIFEST_SCHEMA_VERSION,
  source: job.source,
  sourceRefs: job.sourceRefs,
})

export const r2ObjectEndpoint = (upload, objectKey) =>
  `https://${upload.accountId}.r2.cloudflarestorage.com/${encodeURIComponent(upload.bucket)}/${objectKey
    .split('/')
    .map(part => encodeURIComponent(part))
    .join('/')}`

export const createR2Client = upload =>
  new AwsClient({
    accessKeyId: upload.accessKeyId,
    region: 'auto',
    secretAccessKey: upload.secretAccessKey,
    service: 's3',
  })

export const uploadR2Object = async ({
  body,
  client,
  contentType,
  objectKey,
  upload,
}) => {
  const response = await client.fetch(r2ObjectEndpoint(upload, objectKey), {
    body,
    headers: {
      'content-type': contentType,
      'x-amz-meta-openagents-claim-scope': REPLAY_CLIP_CLAIM_SCOPE,
    },
    method: 'PUT',
  })
  if (!response.ok) {
    const text = await response.text()
    throw new Error(
      `R2 upload failed for ${objectKey}: HTTP ${response.status}${text === '' ? '' : ` ${text.slice(0, 240)}`}`,
    )
  }
  return {
    etag: response.headers.get('etag') ?? null,
    objectKey,
    storageUrl: publicStorageUrl(upload.publicHost, objectKey),
  }
}

export const uploadReplayClipOutputs = async ({
  client = createR2Client(upload),
  manifestObjectKey,
  manifestPath,
  mp4ObjectKey,
  mp4Path,
  upload,
}) => {
  const [mp4Body, manifestBody] = await Promise.all([
    readFile(mp4Path),
    readFile(manifestPath),
  ])
  const [mp4, manifest] = await Promise.all([
    uploadR2Object({
      body: mp4Body,
      client,
      contentType: 'video/mp4',
      objectKey: mp4ObjectKey,
      upload,
    }),
    uploadR2Object({
      body: manifestBody,
      client,
      contentType: 'application/json; charset=utf-8',
      objectKey: manifestObjectKey,
      upload,
    }),
  ])
  return { manifest, mp4 }
}

export const main = async () => {
  const args = parseArgs(process.argv.slice(2))
  if (args.help || args.job === undefined) {
    console.log(
      'Usage: node spike/replay-r1/render-job.mjs --job <job.json> --out <clip.mp4> [--upload]',
    )
    process.exit(args.help ? 0 : 1)
  }

  const jobPath = outputPathFor(args.job)
  const job = validateJob(JSON.parse(await readFile(jobPath, 'utf8')))
  const out = outputPathFor(args.out ?? join(here, 'out', 'clip.mp4'))
  const cameraJson = JSON.stringify(compileCameraPath(job.cameraPath))

  const upload = args.upload ? preflightUpload() : { configured: false }
  if (args.upload && !upload.configured) {
    console.error(
      `[render-job] NEEDS-OWNER: ${upload.blockerRef}\n${upload.detail}`,
    )
    process.exit(2)
  }

  // Drive the existing render-box renderer. render-clip.mjs preflights ffmpeg
  // and Chromium and fails closed if they are missing.
  const renderArgs = [
    renderClipScript,
    '--start',
    String(job.render.startSecond),
    '--duration',
    String(job.render.durationSecond),
    '--fps',
    String(job.render.fps),
    '--width',
    String(job.render.width),
    '--height',
    String(job.render.height),
    '--camera',
    cameraJson,
    '--out',
    out,
  ]
  if (job.source.kind === 'replay_bundle') {
    renderArgs.push('--slug', job.source.bundleRef)
  }
  console.log(`[render-job] rendering job ${job.jobRef ?? '(local)'}`)
  await runCommand(process.execPath, renderArgs)

  const renderManifest = JSON.parse(await readFile(`${out}.render.json`, 'utf8'))
  const fileStat = await stat(out)
  const { manifestObjectKey, mp4ObjectKey } = objectKeysForRender({
    job,
    out,
    prefix: upload.configured ? upload.prefix : 'replay-clips',
  })
  const artifact = {
    byteSize: fileStat.size,
    objectKey: mp4ObjectKey,
    sha256: await sha256File(out),
  }

  const manifest = buildManifest({
    artifacts: [artifact],
    bundleRef:
      renderManifest.bundleRef ?? 'proof_replay_bundle.spike.r1',
    frameCount: renderManifest.frameCount ?? 0,
    job,
    publicHost: upload.configured ? upload.publicHost : null,
  })

  const manifestPath = `${out}.clip-manifest.json`
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`)
  console.log(`[render-job] manifest written: ${manifestPath}`)

  if (!args.upload) {
    console.log(
      '[render-job] upload skipped (pass --upload). Upload plan:\n' +
        `  mp4: ${mp4ObjectKey}\n` +
        `  manifest: ${manifestObjectKey}\n` +
        '  destination: R2 bucket (NEEDS-OWNER: not provisioned in this repo)',
    )
  } else {
    console.log(
      `[render-job] uploading to R2 bucket ${upload.bucket}: ${mp4ObjectKey}, ${manifestObjectKey}`,
    )
    const uploaded = await uploadReplayClipOutputs({
      manifestObjectKey,
      manifestPath,
      mp4ObjectKey,
      mp4Path: out,
      upload,
    })
    console.log(
      `[render-job] upload complete: ${uploaded.mp4.storageUrl} ; ${uploaded.manifest.storageUrl}`,
    )
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main().catch(error => {
    console.error('[render-job] FAILED:', error)
    process.exit(1)
  })
}
