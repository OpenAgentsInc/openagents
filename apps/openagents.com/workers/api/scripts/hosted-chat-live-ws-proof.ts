#!/usr/bin/env bun
/**
 * LIVE-TAIL proof for the Khala Sync capture -> LiveHub -> WebSocket pipe
 * (#8554). Unlike hosted-chat-e2e-smoke.ts (which polls /api/sync/bootstrap
 * and would pass even with a DEAD capture daemon, because bootstrap reads
 * Postgres directly), this holds an OPEN `/api/sync/connect` WebSocket and
 * proves the assistant `text.delta` arrives LIVE over the socket — i.e. the
 * capture daemon tailed the changelog write, appended it to LiveHub, and
 * LiveHub fanned the DeltaFrame out to the subscribed socket.
 *
 *   1. Open WS  /api/sync/connect?scope=scope.thread.<thread>&cursor=<head>
 *      &token=<maestro>  (cursor=head => no historical replay; only NEW
 *      appends after connect arrive).
 *   2. POST /api/sync/push [chat.appendMessage, runtime.startTurn].
 *   3. The per-minute Cloud Run cron answers the turn, writing
 *      runtime_event rows (text.delta / text.completed / turn.finished) into
 *      the thread scope. Capture pushes them to LiveHub, which fans a
 *      DeltaFrame to our socket.
 *   4. PASS when a DeltaFrame carrying a runtime_event text.delta for OUR
 *      turnId arrives over the WS.
 *
 * Creds (gitignored, never printed): ~/work/.secrets/khala-maestro.env
 *   KHALA_MAESTRO_TOKEN, KHALA_MAESTRO_THREAD_ID
 *
 * Usage:
 *   CURSOR=72 bun apps/openagents.com/workers/api/scripts/hosted-chat-live-ws-proof.ts
 */
import { readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'
import { decodeLiveFrame } from '@openagentsinc/khala-sync'

const BASE_URL = (process.env.OPENAGENTS_BASE_URL ?? 'https://openagents.com').replace(/\/$/, '')
const WS_BASE = BASE_URL.replace(/^http/, 'ws')
const TIMEOUT_MS = Number(process.env.LIVE_WS_TIMEOUT_MS ?? 180_000)
const PROMPT = process.env.HOSTED_CHAT_SMOKE_PROMPT ?? 'What is the capital of France? Answer with only the city name.'

const fail = (m: string): never => {
  console.error(`FAIL: ${m}`)
  process.exit(1)
}

const loadCred = (name: string): string => {
  const fromEnv = process.env[name]
  if (fromEnv !== undefined && fromEnv !== '') return fromEnv
  const envPath = process.env.KHALA_MAESTRO_ENV ?? join(homedir(), 'work', '.secrets', 'khala-maestro.env')
  const text = readFileSync(envPath, 'utf8')
  for (const line of text.split('\n')) {
    const t = line.trim()
    if (t.startsWith('#') || !t.includes('=')) continue
    const eq = t.indexOf('=')
    if (t.slice(0, eq).trim() === name) return t.slice(eq + 1).trim().replace(/^["']|["']$/g, '')
  }
  return fail(`credential ${name} not found in ${envPath}`)
}

const TOKEN = loadCred('KHALA_MAESTRO_TOKEN')
const THREAD_ID = loadCred('KHALA_MAESTRO_THREAD_ID')
const SCOPE = `scope.thread.${THREAD_ID}`
const CURSOR = Number(process.env.CURSOR ?? '0')

const turnId = crypto.randomUUID()
const messageId = crypto.randomUUID()
const nowIso = new Date().toISOString()

const runtimeEventText = (postImageJson: string, wantTurn: string): { kind: string; text: string } | undefined => {
  try {
    const parsed = JSON.parse(postImageJson) as { event?: { kind?: string; text?: string; turnId?: string } }
    const e = parsed.event
    if (e?.turnId === wantTurn && typeof e.kind === 'string') {
      return { kind: e.kind, text: e.text ?? '' }
    }
  } catch { /* ignore */ }
  return undefined
}

const main = async () => {
  console.log(`[live-ws-proof] base=${BASE_URL} scope=${SCOPE} cursor=${CURSOR} turn=${turnId}`)

  const url = `${WS_BASE}/api/sync/connect?scope=${encodeURIComponent(SCOPE)}&cursor=${CURSOR}&token=${encodeURIComponent(TOKEN)}`
  const ws = new WebSocket(url)

  let liveDeltaText = ''
  let sawLiveDelta = false
  let sawTurnFinished = false
  let pushed = false
  const liveKinds: Array<string> = []

  const doPush = async (): Promise<void> => {
    if (pushed) return
    pushed = true
    const res = await fetch(`${BASE_URL}/api/sync/push`, {
      method: 'POST',
      headers: { authorization: `Bearer ${TOKEN}`, 'content-type': 'application/json' },
      body: JSON.stringify({
        clientGroupId: `live-ws-proof-${Date.now()}`,
        clientId: `live-ws-proof-${Date.now()}.send`,
        protocolVersion: 1,
        schemaVersion: 1,
        mutations: [
          { name: 'chat.appendMessage', mutationId: 1, argsJson: JSON.stringify({ body: PROMPT, messageId, threadId: THREAD_ID }) },
          {
            name: 'runtime.startTurn', mutationId: 2,
            argsJson: JSON.stringify({
              bodyRef: `chat_message.${messageId}`, causalityRefs: [], createdAt: nowIso,
              idempotencyKey: `idem.start.${turnId}`, intentId: `intent.start.${turnId}`, kind: 'turn.start',
              origin: { lane: 'khala_sync_mobile_control', surface: 'mobile' }, redactionClass: 'private_ref',
              schema: 'openagents.khala_runtime_control_intent.v1', target: { lane: 'hosted_khala' },
              threadId: THREAD_ID, turnId, visibility: 'private',
            }),
          },
        ],
      }),
    })
    const body = await res.text()
    if (!res.ok) return fail(`/api/sync/push -> HTTP ${res.status}: ${body.slice(0, 300)}`)
    console.log('[live-ws-proof] pushed turn; WS open, waiting for LIVE assistant text.delta (cron answers within ~1 min)...')
  }

  ws.addEventListener('open', () => {
    console.log('[live-ws-proof] WS open; pushing the chat turn...')
    void doPush()
  })

  ws.addEventListener('message', (ev) => {
    let frame
    try {
      frame = decodeLiveFrame(JSON.parse(String(ev.data)))
    } catch {
      return
    }
    if (frame._tag !== 'DeltaFrame') return
    for (const entry of frame.entries) {
      if (entry.entityType !== 'runtime_event' || entry.postImageJson === undefined) continue
      const hit = runtimeEventText(entry.postImageJson, turnId)
      if (hit === undefined) continue
      liveKinds.push(hit.kind)
      if (hit.kind === 'text.delta') {
        sawLiveDelta = true
        liveDeltaText += hit.text
      }
      if (hit.kind === 'turn.finished') sawTurnFinished = true
    }
    if (sawLiveDelta && sawTurnFinished) {
      console.log(`[live-ws-proof] LIVE frames for turn: ${JSON.stringify(liveKinds)}`)
      console.log(`[live-ws-proof] LIVE assistant reply over WS: ${JSON.stringify(liveDeltaText.slice(0, 400))}`)
      console.log('PASS: assistant text.delta arrived LIVE over the /api/sync/connect WebSocket (capture -> LiveHub -> socket).')
      try { ws.close() } catch { /* noop */ }
      process.exit(0)
    }
  })

  ws.addEventListener('close', (ev) => {
    if (!(sawLiveDelta && sawTurnFinished)) {
      console.log(`[live-ws-proof] WS closed (code ${ev.code} ${ev.reason}); kinds seen live: ${JSON.stringify(liveKinds)}`)
    }
  })
  ws.addEventListener('error', () => { /* surfaced via close */ })

  setTimeout(() => {
    fail(`timed out after ${TIMEOUT_MS}ms; live kinds seen for turn=${turnId}: ${JSON.stringify(liveKinds)} (sawDelta=${sawLiveDelta}, finished=${sawTurnFinished})`)
  }, TIMEOUT_MS)
}

void main()
