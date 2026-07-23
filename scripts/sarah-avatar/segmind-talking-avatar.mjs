#!/usr/bin/env node
// Sarah talking-avatar generator over the Segmind AI Gateway (docs/sarah/).
//
// Programmatic replacement for the manual "animate a Midjourney image with a
// hosted video-gen platform" step used for Episode 260. Input is one portrait
// image (as a URL) plus a spoken line — either a text script (built-in TTS) or
// a pre-recorded audio URL — and the output is an MP4 of that person speaking.
//
// This is a thin, dependency-free runner (Node built-in fetch only). It uses
// the Segmind Async Inference (V2) API, which is the reliable path for video
// models: submit -> poll status -> download the result.
//
// The API key is NEVER hardcoded. It is read from the environment variable
// SEGMIND_API_KEY, or from a KEY=VALUE file given by SEGMIND_ENV_FILE
// (default: <workspace>/.secrets/segmind.env, which is gitignored).
//
// The image MUST be a public URL. Segmind fetches it server-side; base64 is not
// accepted by these avatar models. Host the portrait on our own Google Cloud
// Storage and pass a short-lived signed URL — see the runbook
// docs/sarah/2026-07-22-segmind-talking-avatar-pipeline.md.
//
// Usage:
//   node scripts/sarah-avatar/segmind-talking-avatar.mjs \
//     --image "<https url to portrait>" \
//     --script "Hi! I'm Sarah. Shall we begin?" \
//     --out ./sarah-intro.mp4
//
// Flags:
//   --model <slug>       default p-video-avatar (Pruna P Video Avatar).
//                        Also supported: kling-v2-standard-avatar (needs --audio).
//   --image <url>        required. Public URL of the portrait.
//   --script "<text>"    text to speak (built-in TTS). p-video-avatar only.
//   --audio <url>        public URL of a voice recording. Overrides --script.
//   --voice <name>       TTS voice (p-video-avatar), default "Zephyr (Female)".
//   --voice-prompt "..." tone/pace/emotion hint.
//   --video-prompt "..." framing/motion hint.
//   --resolution <r>     "720p" or "1080p" (p-video-avatar), default "1080p".
//   --seed <n>           integer seed, default 4242.
//   --out <path>         output MP4 path, default ./sarah-avatar-<model>.mp4.
//   --timeout-min <n>    max minutes to poll, default 15.

import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const args = process.argv.slice(2)
const flag = (name, fallback = undefined) => {
  const i = args.indexOf(`--${name}`)
  return i >= 0 && i + 1 < args.length ? args[i + 1] : fallback
}

// Canonical Sarah direction. Applied to every generation so the owner's
// director notes (voice register, expression, framing) stay consistent across
// all future videos. An explicit --flag overrides one field for one call.
// Point elsewhere with --direction <path>, or --direction none to disable.
const loadDirection = () => {
  const chosen = flag('direction')
  if (chosen === 'none') return {}
  const path =
    chosen ||
    resolve(dirname(fileURLToPath(import.meta.url)), 'sarah-direction.json')
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch {
    return {}
  }
}
const direction = loadDirection()

const resolveApiKey = () => {
  if (process.env.SEGMIND_API_KEY) return process.env.SEGMIND_API_KEY.trim()
  const envFile =
    process.env.SEGMIND_ENV_FILE ||
    resolve(process.env.HOME || '', 'work/.secrets/segmind.env')
  try {
    const match = readFileSync(envFile, 'utf8').match(/^SEGMIND_API_KEY=(\S+)/m)
    if (match) return match[1]
  } catch {
    // fall through
  }
  return undefined
}

const KEY = resolveApiKey()
if (!KEY) {
  console.error(
    'error: no Segmind API key. Set SEGMIND_API_KEY or SEGMIND_ENV_FILE ' +
      '(default ~/work/.secrets/segmind.env).',
  )
  process.exit(1)
}

const model = flag('model', 'p-video-avatar')
const image = flag('image')
if (!image || !/^https?:\/\//.test(image)) {
  console.error('error: --image must be a public http(s) URL to a portrait.')
  process.exit(1)
}
const script = flag('script')
const audio = flag('audio')
const out = resolve(flag('out', `./sarah-avatar-${model}.mp4`))
const timeoutMin = Number(flag('timeout-min', '15'))

// Model-specific input mapping. Each avatar model on Segmind names its inputs
// differently; keep the caller-facing flags stable and translate here.
const buildBody = () => {
  if (model === 'kling-v2-standard-avatar' || model === 'kling-v2-pro-avatar') {
    if (!audio)
      throw new Error('kling avatar models are audio-driven; pass --audio <url>.')
    return { image_url: image, audio_url: audio }
  }
  // p-video-avatar (Pruna) and compatible script-or-audio models. Defaults come
  // from the canonical Sarah direction profile; --flags override per call.
  // The voice_anchor is prepended to EVERY voice_prompt (base or per-clip
  // override) so no per-clip emotional direction — however low or dry — can
  // flip her perceived gender. Her gender is locked at the profile layer.
  const anchor = direction.voice_anchor ? String(direction.voice_anchor).trim() : ''
  const rawVoicePrompt = flag(
    'voice-prompt',
    direction.voice_prompt ||
      'Warm, confident, friendly, upbeat — speaking with a genuine smile.',
  )
  const body = {
    image,
    voice: flag('voice', direction.voice || 'Zephyr (Female)'),
    voice_language: flag('voice-language', direction.voice_language || 'English (US)'),
    voice_prompt:
      anchor && !rawVoicePrompt.startsWith(anchor)
        ? `${anchor} ${rawVoicePrompt}`
        : rawVoicePrompt,
    video_prompt: flag(
      'video-prompt',
      direction.video_prompt ||
        'The person greets the camera, with natural subtle head movement and blinking.',
    ),
    negative_prompt:
      flag('negative-prompt', direction.negative_prompt) ||
      'subtitles, text, watermark, blurry, distorted face, extra fingers, ' +
        'frozen static face',
    resolution: flag('resolution', direction.resolution || '1080p'),
    seed: Number(flag('seed', '4242')),
  }
  if (audio) body.audio = audio
  else body.voice_script = script || "Hi! I'm Sarah. Shall we begin?"
  return body
}

const submit = async () => {
  const res = await fetch(`https://api.segmind.com/v2/${model}`, {
    method: 'POST',
    headers: { 'x-api-key': KEY, 'Content-Type': 'application/json' },
    body: JSON.stringify(buildBody()),
  })
  const text = await res.text()
  if (res.status >= 400)
    throw new Error(`submit HTTP ${res.status}: ${text.slice(0, 400)}`)
  const json = JSON.parse(text)
  if (!json.request_id) throw new Error(`no request_id in response: ${text}`)
  return json
}

const findVideoUrl = json => {
  const candidates = [
    json.output,
    json.video,
    json.url,
    json.data && (json.data.output || json.data.url),
    Array.isArray(json.output) ? json.output[0] : null,
  ]
  return candidates.find(u => typeof u === 'string' && /^https?:\/\//.test(u))
}

const download = async (url, res) => {
  const source = url ? await fetch(url, { headers: { 'x-api-key': KEY } }) : res
  const buf = Buffer.from(await source.arrayBuffer())
  writeFileSync(out, buf)
  return buf.length
}

const run = async () => {
  const started = Date.now()
  const submitted = await submit()
  console.log(`[submit] model=${model} request_id=${submitted.request_id}`)
  const statusUrl =
    submitted.status_url ||
    `https://api.segmind.com/v2/requests/${submitted.request_id}/status`
  const responseUrl =
    submitted.response_url ||
    `https://api.segmind.com/v2/requests/${submitted.request_id}`
  const deadline = started + timeoutMin * 60 * 1000
  while (Date.now() < deadline) {
    const res = await fetch(statusUrl, { headers: { 'x-api-key': KEY } })
    const json = await res.json().catch(async () => ({ raw: await res.text() }))
    const status = String(json.status || json.state || 'UNKNOWN')
    const elapsed = ((Date.now() - started) / 1000).toFixed(0)
    console.log(`[${elapsed}s] status=${status}`)
    if (/COMPLET|SUCCE|DONE/i.test(status)) {
      const result = await fetch(responseUrl, { headers: { 'x-api-key': KEY } })
      const ct = result.headers.get('content-type') || ''
      if (ct.includes('video') || ct.includes('octet-stream')) {
        console.log(`[done] saved ${await download(null, result)} bytes -> ${out}`)
        return
      }
      const rj = await result.json().catch(() => ({}))
      const url = findVideoUrl(rj)
      if (!url) throw new Error(`completed but no video url: ${JSON.stringify(rj).slice(0, 400)}`)
      console.log(`[done] downloaded ${await download(url)} bytes -> ${out}`)
      return
    }
    if (/FAIL|ERROR|CANCEL/i.test(status))
      throw new Error(`generation ${status}: ${JSON.stringify(json).slice(0, 400)}`)
    await new Promise(r => setTimeout(r, 10000))
  }
  throw new Error(`timed out after ${timeoutMin} min`)
}

run().catch(err => {
  console.error(`error: ${err.message}`)
  process.exit(1)
})
