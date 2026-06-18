// Replay clip render-box job runner (EPIC #5411, issue #5431 — SCAFFOLD).
//
// Promotes the `render-clip.mjs` spike toward a production render-box path:
// it takes a typed clip job (the `openagents.replay_clip_job.v1` shape from
// `@openagentsinc/replay-clips`), drives the existing headless-Chromium +
// ffmpeg renderer, builds a public-safe `openagents.replay_clip_manifest.v1`,
// and computes the R2 upload plan.
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
//      locally and reports the upload plan as `needs_owner`, never uploading.
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

const here = dirname(fileURLToPath(import.meta.url))
const renderClipScript = resolve(here, 'render-clip.mjs')

const REPLAY_CLIP_MANIFEST_SCHEMA_VERSION =
  'openagents.replay_clip_manifest.v1'
const REPLAY_CLIP_JOB_SCHEMA_VERSION = 'openagents.replay_clip_job.v1'
const REPLAY_CLIP_CLAIM_SCOPE = 'evidence_presentation_only'

const RENDERER = {
  pixelFormat: 'yuv420p',
  renderer: 'playwright-chromium-screenshot-plus-ffmpeg',
  rendererVersion: 'replay-r1',
  runLocation:
    'local_or_ci_render_box_with_bun_node_headless_chromium_and_ffmpeg_not_cloudflare_worker',
  videoCodec: 'libx264',
}

// The bounded camera verbs from the #5433 DSL and their render-box camera mode.
const VERB_TO_MODE = {
  follow: 'follow_actor',
  frame_actor: 'follow_actor',
  frame_settlement: 'zap_focus',
  hold: 'director_track',
  orbit: 'orbit_proof',
}

const outputPathFor = value =>
  isAbsolute(value) ? value : resolve(process.cwd(), value)

const parseArgs = argv => {
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
const validateJob = job => {
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
const compileCameraPath = cameraPath => ({
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

const runCommand = (command, commandArgs, options = {}) =>
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

const sha256File = async path => {
  const bytes = await readFile(path)
  return createHash('sha256').update(bytes).digest('hex')
}

// NEEDS-OWNER: R2 is not provisioned here. This preflight reports whether the
// render box has R2 credentials configured. It never invents a bucket.
const preflightUpload = () => {
  const bucket = process.env.R2_REPLAY_CLIPS_BUCKET
  const publicHost = process.env.R2_REPLAY_CLIPS_PUBLIC_HOST
  const accountId = process.env.R2_REPLAY_CLIPS_ACCOUNT_ID
  const accessKeyId = process.env.R2_REPLAY_CLIPS_ACCESS_KEY_ID
  const secretAccessKey = process.env.R2_REPLAY_CLIPS_SECRET_ACCESS_KEY
  if (
    bucket === undefined ||
    publicHost === undefined ||
    accountId === undefined ||
    accessKeyId === undefined ||
    secretAccessKey === undefined
  ) {
    return {
      configured: false,
      blockerRef: 'needs_owner.replay_clip.r2_bucket_not_provisioned',
      detail:
        'Set R2_REPLAY_CLIPS_BUCKET, R2_REPLAY_CLIPS_PUBLIC_HOST, R2_REPLAY_CLIPS_ACCOUNT_ID, R2_REPLAY_CLIPS_ACCESS_KEY_ID, R2_REPLAY_CLIPS_SECRET_ACCESS_KEY after the owner provisions the R2 bucket.',
    }
  }
  return { configured: true, bucket, publicHost }
}

const buildManifest = ({ artifacts, bundleRef, frameCount, job, publicHost }) => ({
  artifacts: artifacts.map(artifact => ({
    byteSize: artifact.byteSize,
    kind: 'mp4',
    sha256: artifact.sha256,
    storageUrl:
      publicHost === null
        ? `local:${artifact.objectKey}`
        : `${publicHost.replace(/\/$/, '')}/${artifact.objectKey}`,
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

const main = async () => {
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
  const objectKey = basename(out)
  const artifact = {
    byteSize: fileStat.size,
    objectKey,
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
        `  object: ${objectKey}\n` +
        '  destination: R2 bucket (NEEDS-OWNER: not provisioned in this repo)',
    )
  } else {
    // NEEDS-OWNER: actual R2 PutObject is intentionally not implemented until
    // the owner provisions the bucket + credentials. The preflight above
    // already gated on credentials; wiring S3/R2 PutObject is the owner step.
    console.log(
      `[render-job] R2 upload target: ${upload.bucket}/${objectKey} -> ${manifest.artifacts[0].storageUrl}`,
    )
    console.log(
      '[render-job] NEEDS-OWNER: R2 PutObject wiring is deferred to the owner-provisioned bucket step (#5431).',
    )
  }
}

main().catch(error => {
  console.error('[render-job] FAILED:', error)
  process.exit(1)
})
