// Optional real-provider smoke for the Fireworks adapter (EPIC #5474, #5479).
//
// This is NOT part of the unit suite — it makes a real network call to
// Fireworks and is GATED on a key being present, so `bun run test` /
// `check:deploy` never require it. It exercises the same adapter the Worker
// registers, against the public `openagents/khala` model alias mapped internally
// to Fireworks DeepSeek V4 Flash, and
// prints only the receipt-first `usage` object + a short content preview. It
// NEVER prints the key.
//
// Usage:
//   bun run apps/openagents.com/workers/api/scripts/fireworks-smoke.ts
//
// Key resolution (first hit wins):
//   1. FIREWORKS_API_KEY in the environment
//   2. ~/work/.secrets/fireworks.env (gitignored; chmod 600)
//
// When no key is found the script exits 0 with a skip notice.
import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'

import {
  FIREWORKS_ADAPTER_ID,
  makeFireworksAdapter,
} from '../src/inference/fireworks-adapter'
import { KHALA_MODEL_ID } from '../src/inference/pricing'
import { type InferenceRequest } from '../src/inference/provider-adapter'

const loadKeyFromSecretsFile = (): string | undefined => {
  const path = join(homedir(), 'work', '.secrets', 'fireworks.env')
  try {
    const contents = readFileSync(path, 'utf8')
    for (const line of contents.split('\n')) {
      const match = line.match(
        /^\s*(?:export\s+)?FIREWORKS_API_KEY\s*=\s*(.+)$/,
      )
      if (match) {
        return match[1].trim().replace(/^["']|["']$/g, '')
      }
    }
  } catch {
    return undefined
  }
  return undefined
}

const resolveKey = (): string | undefined => {
  const fromEnv = process.env.FIREWORKS_API_KEY?.trim()
  if (fromEnv) {
    return fromEnv
  }
  return loadKeyFromSecretsFile()
}

const MODEL = process.env.FIREWORKS_SMOKE_MODEL?.trim() || KHALA_MODEL_ID

const main = async (): Promise<void> => {
  const apiKey = resolveKey()
  if (apiKey === undefined || apiKey === '') {
    console.log(
      '[fireworks-smoke] SKIP: no FIREWORKS_API_KEY in env or ~/work/.secrets/fireworks.env',
    )
    return
  }

  const adapter = makeFireworksAdapter({ getApiKey: () => apiKey })
  const request: InferenceRequest = {
    messages: [
      {
        content: 'Reply with exactly: OPENAGENTS FIREWORKS SMOKE OK',
        role: 'user',
      },
    ],
    model: MODEL,
    passthroughParams: { max_tokens: 64, temperature: 0 },
    stream: false,
  }

  console.log(
    `[fireworks-smoke] adapter=${FIREWORKS_ADAPTER_ID} model=${MODEL}`,
  )

  const outcome = await Effect.runPromise(
    Effect.result(adapter.complete(request)),
  )

  if (outcome._tag === 'Failure') {
    console.error(
      `[fireworks-smoke] FAIL kind=${outcome.failure.kind ?? 'unknown'} ` +
        `retryable=${outcome.failure.retryable} status=${
          outcome.failure.httpStatus ?? 'n/a'
        } reason=${outcome.failure.reason}`,
    )
    process.exitCode = 1
    return
  }

  const { content, finishReason, servedModel, usage } = outcome.success
  console.log(`[fireworks-smoke] OK servedModel=${servedModel}`)
  console.log(`[fireworks-smoke] finishReason=${finishReason}`)
  console.log(`[fireworks-smoke] usage=${JSON.stringify(usage)}`)
  console.log(
    `[fireworks-smoke] content="${content.slice(0, 120).replace(/\n/g, ' ')}"`,
  )
}

void main()
