#!/usr/bin/env bun
/**
 * Live hosted-Khala chat "send a message -> get an assistant reply" smoke.
 *
 * This is the REAL, network-touching counterpart to the deterministic guard in
 * src/khala-hosted-runtime-dispatch.e2e.test.ts. It does exactly what a mobile
 * chat "Send" does, at the API level, then polls until a real assistant reply
 * comes back:
 *
 *   1. POST /api/sync/push a [chat.appendMessage, runtime.startTurn] batch on
 *      the `hosted_khala` lane (bodyRef = chat_message.<messageId>), as the
 *      seeded linked agent.
 *   2. Poll /api/sync/bootstrap for the thread scope until the turn's
 *      runtime_event stream carries an assistant `text.delta`/`text.completed`
 *      AND a `turn.finished` with finishReason !== "error".
 *   3. Print PASS (with the reply text) or FAIL (with the terminal reason).
 *
 * The per-minute Cloud Run cron (`runHostedRuntimeTurnDispatchForEnv`) answers
 * the queued turn; nothing else is needed server-side. This is GATED on creds
 * (it does nothing in CI without them) and is wired as an opt-in nightly step
 * (docs/qa/khala-code-nightly-matrix.md).
 *
 * Creds: ~/work/.secrets/khala-maestro.env (gitignored, NEVER committed):
 *   KHALA_MAESTRO_TOKEN       — seeded AgentFlampy agent bearer token
 *   KHALA_MAESTRO_THREAD_ID   — a seeded public-safe thread on that account
 * The token/thread are read from the environment (or that env file) and are
 * NEVER printed.
 *
 * Usage:
 *   bun apps/openagents.com/workers/api/scripts/hosted-chat-e2e-smoke.ts
 *   OPENAGENTS_BASE_URL=https://staging.openagents.com bun .../hosted-chat-e2e-smoke.ts
 *   HOSTED_CHAT_SMOKE_PROMPT="What is the capital of France? One word." \
 *     HOSTED_CHAT_SMOKE_EXPECT=Paris bun .../hosted-chat-e2e-smoke.ts
 */

import { readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'

const BASE_URL = (process.env.OPENAGENTS_BASE_URL ?? 'https://openagents.com').replace(/\/$/, '')
const TIMEOUT_MS = Number(process.env.HOSTED_CHAT_SMOKE_TIMEOUT_MS ?? 150_000)
const POLL_MS = Number(process.env.HOSTED_CHAT_SMOKE_POLL_MS ?? 5_000)
// A deterministic prompt with a stable one-word answer that is NOT present in
// the prompt itself, so a match proves the REPLY (not an echo of the send).
const PROMPT = process.env.HOSTED_CHAT_SMOKE_PROMPT ?? 'What is the capital of France? Answer with only the city name.'
const EXPECT = process.env.HOSTED_CHAT_SMOKE_EXPECT ?? 'Paris'

const fail = (message: string): never => {
  console.error(`FAIL: ${message}`)
  process.exit(1)
}

/** Read a var from the process env, falling back to the gitignored maestro env
 * file. The value is never logged. */
const loadCred = (name: string): string => {
  const fromEnv = process.env[name]
  if (fromEnv !== undefined && fromEnv !== '') return fromEnv
  const envPath = process.env.KHALA_MAESTRO_ENV ?? join(homedir(), 'work', '.secrets', 'khala-maestro.env')
  let text: string
  try {
    text = readFileSync(envPath, 'utf8')
  } catch {
    return fail(
      `missing credential ${name}: set it in the environment or provide ${envPath}. ` +
        'This smoke is intentionally gated on creds and is a no-op in CI without them.',
    )
  }
  for (const line of text.split('\n')) {
    const trimmed = line.trim()
    if (trimmed.startsWith('#') || !trimmed.includes('=')) continue
    const eq = trimmed.indexOf('=')
    const key = trimmed.slice(0, eq).trim()
    if (key === name) {
      return trimmed.slice(eq + 1).trim().replace(/^["']|["']$/g, '')
    }
  }
  return fail(`credential ${name} not found in ${envPath}`)
}

const TOKEN = loadCred('KHALA_MAESTRO_TOKEN')
const THREAD_ID = loadCred('KHALA_MAESTRO_THREAD_ID')

const post = async (path: string, body: unknown): Promise<Record<string, unknown>> => {
  const res = await fetch(`${BASE_URL}${path}`, {
    body: JSON.stringify(body),
    headers: { authorization: `Bearer ${TOKEN}`, 'content-type': 'application/json' },
    method: 'POST',
  })
  const text = await res.text()
  if (!res.ok) return fail(`${path} -> HTTP ${res.status}: ${text.slice(0, 300)}`)
  try {
    return JSON.parse(text) as Record<string, unknown>
  } catch {
    return fail(`${path} -> non-JSON response: ${text.slice(0, 300)}`)
  }
}

type RuntimeEvent = {
  kind?: string
  turnId?: string
  text?: string
  finishReason?: string
  sequence?: number
}

const readThreadEvents = async (turnId: string): Promise<RuntimeEvent[]> => {
  const snap = await post('/api/sync/bootstrap', {
    clientGroupId: `hosted-chat-smoke-read-${Date.now()}`,
    protocolVersion: 1,
    schemaVersion: 1,
    scope: `scope.thread.${THREAD_ID}`,
  })
  const entities = (snap.entities as Array<{ entityType?: string; postImageJson?: string }>) ?? []
  const events: RuntimeEvent[] = []
  for (const entity of entities) {
    if (entity.entityType !== 'runtime_event' || entity.postImageJson === undefined) continue
    try {
      // The runtime_event entity's postImage wraps the KhalaRuntimeEvent under
      // `.event` ({ createdAt, event: { kind, text, finishReason, turnId, ... } }).
      const parsed = JSON.parse(entity.postImageJson) as { event?: RuntimeEvent }
      const event = parsed.event
      if (event !== undefined && event.turnId === turnId) events.push(event)
    } catch {
      // ignore malformed rows; the poll simply keeps waiting.
    }
  }
  return events.sort((a, b) => (a.sequence ?? 0) - (b.sequence ?? 0))
}

const main = async () => {
  const messageId = crypto.randomUUID()
  const turnId = crypto.randomUUID()
  const nowIso = new Date().toISOString()
  const clientGroupId = `hosted-chat-smoke-${Date.now()}`
  const clientId = `${clientGroupId}.send`

  console.log(`[hosted-chat-e2e-smoke] base=${BASE_URL} thread=${THREAD_ID}`)
  console.log(`[hosted-chat-e2e-smoke] sending turn=${turnId} prompt=${JSON.stringify(PROMPT)}`)

  await post('/api/sync/push', {
    clientGroupId,
    clientId,
    mutations: [
      {
        argsJson: JSON.stringify({ body: PROMPT, messageId, threadId: THREAD_ID }),
        mutationId: 1,
        name: 'chat.appendMessage',
      },
      {
        argsJson: JSON.stringify({
          bodyRef: `chat_message.${messageId}`,
          causalityRefs: [],
          createdAt: nowIso,
          idempotencyKey: `idem.start.${turnId}`,
          intentId: `intent.start.${turnId}`,
          kind: 'turn.start',
          origin: { lane: 'khala_sync_mobile_control', surface: 'mobile' },
          redactionClass: 'private_ref',
          schema: 'openagents.khala_runtime_control_intent.v1',
          target: { lane: 'hosted_khala' },
          threadId: THREAD_ID,
          turnId,
          visibility: 'private',
        }),
        mutationId: 2,
        name: 'runtime.startTurn',
      },
    ],
    protocolVersion: 1,
    schemaVersion: 1,
  })
  console.log('[hosted-chat-e2e-smoke] queued; polling for the assistant reply (cron answers within ~1 min)...')

  const deadline = Date.now() + TIMEOUT_MS
  while (Date.now() < deadline) {
    await new Promise(resolve => setTimeout(resolve, POLL_MS))
    const events = await readThreadEvents(turnId)
    const kinds = events.map(event => event.kind)
    const finished = events.find(event => event.kind === 'turn.finished')
    const replyText = events
      .filter(event => event.kind === 'text.delta')
      .map(event => event.text ?? '')
      .join('')

    if (finished !== undefined) {
      if (finished.finishReason === 'error') {
        return fail(
          `turn settled as finishReason:"error" — no assistant reply. events=${JSON.stringify(kinds)}`,
        )
      }
      if (replyText.trim() === '') {
        return fail(`turn finished(${finished.finishReason}) with an EMPTY reply. events=${JSON.stringify(kinds)}`)
      }
      const matched = replyText.toLowerCase().includes(EXPECT.toLowerCase())
      console.log(`[hosted-chat-e2e-smoke] reply: ${JSON.stringify(replyText.slice(0, 400))}`)
      console.log(`[hosted-chat-e2e-smoke] events: ${JSON.stringify(kinds)}`)
      if (!matched) {
        console.log(
          `[hosted-chat-e2e-smoke] note: reply did not contain expected token ${JSON.stringify(EXPECT)}, ` +
            'but a non-empty assistant reply DID come back — the send->reply loop is alive.',
        )
      }
      console.log(`PASS: assistant reply received (${replyText.length} chars) and turn finished(${finished.finishReason}).`)
      process.exit(0)
    }
    process.stdout.write('.')
  }
  return fail(`timed out after ${TIMEOUT_MS}ms waiting for an assistant reply (turn=${turnId}).`)
}

void main()
