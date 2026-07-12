import { createHmac } from 'node:crypto'
import { describe, expect, test } from 'vitest'

import {
  AUDIO_GRANT_DEVICE_HEADER,
  AUDIO_GRANT_ISSUE_PATH,
  AUDIO_GRANT_REQUEST_SCHEMA,
  AUDIO_GRANT_RESPONSE_SCHEMA,
  AUDIO_GRANT_TTL_MS,
  handleAudioGrantIssueRequest,
} from './audio-grant-routes'

const secret = 'audio-grant-test-secret-that-is-at-least-32-bytes'
const identity = {
  ownerRef: 'github:owner-1',
  deviceRef: 'desktop.device-1',
  threadRef: 'thread-1',
  sessionRef: 'voice-session-1',
  generation: 2,
}
const body = {
  schema: AUDIO_GRANT_REQUEST_SCHEMA,
  disclosureRef: 'disclosure.voice-retention.v1',
  identity,
}
const context = {} as ExecutionContext

const request = (
  input: unknown = body,
  options: Readonly<{
    gatewayUrl?: string | undefined
    authorization?: string | undefined
    deviceRef?: string | undefined
    method?: string | undefined
  }> = {},
): Request =>
  new Request(`https://openagents.com${AUDIO_GRANT_ISSUE_PATH}`, {
    ...((options.method ?? 'POST') === 'GET'
      ? {}
      : { body: JSON.stringify(input) }),
    headers: {
      'content-type': 'application/json',
      ...(options.authorization === undefined
        ? {}
        : { authorization: options.authorization }),
      ...(options.deviceRef === undefined
        ? {}
        : { [AUDIO_GRANT_DEVICE_HEADER]: options.deviceRef }),
    },
    method: options.method ?? 'POST',
  })

const dependencies = (
  options: Readonly<{
    now?: number | undefined
    ownerRef?: string | undefined
    secret?: string | undefined
  }> = {},
) => ({
  gatewayUrl: () =>
    options.gatewayUrl ?? 'wss://openagents-audio.example/v1/stream',
  now: () => options.now ?? 1_000_000,
  requireUserBearerSession: async (incoming: Request) =>
    incoming.headers.get('authorization') === 'Bearer desktop-session'
      ? { user: { userId: options.ownerRef ?? identity.ownerRef } }
      : undefined,
  signingSecret: () => options.secret ?? secret,
  userIdFromSession: (session: Readonly<{ user: { userId: string } }>) =>
    session.user.userId,
})

const decodeGrant = (grant: string) => {
  const [encoded, signature] = grant.split('.')
  if (encoded === undefined || signature === undefined)
    throw new Error('bad grant')
  return {
    payload: JSON.parse(Buffer.from(encoded, 'base64url').toString('utf8')),
    signature,
    expected: createHmac('sha256', secret).update(encoded).digest('base64url'),
  }
}

describe('AUDIO-2 host grant issuer', () => {
  test('mints the exact AUDIO-2 HMAC wire for a verified owner and device', async () => {
    const response = await handleAudioGrantIssueRequest(
      dependencies(),
      request(body, {
        authorization: 'Bearer desktop-session',
        deviceRef: identity.deviceRef,
      }),
      {},
      context,
    )
    expect(response.status).toBe(201)
    expect(response.headers.get('cache-control')).toBe('no-store')
    const json = (await response.json()) as Record<string, unknown>
    expect(json.schema).toBe(AUDIO_GRANT_RESPONSE_SCHEMA)
    expect(json.disclosureRef).toBe(body.disclosureRef)
    expect(json.expiresAtMs).toBe(1_000_000 + AUDIO_GRANT_TTL_MS)
    expect(json.gatewayUrl).toBe('wss://openagents-audio.example/v1/stream')
    const decoded = decodeGrant(String(json.grant))
    expect(decoded.signature).toBe(decoded.expected)
    expect(decoded.payload).toEqual({
      expiresAtMs: 1_000_000 + AUDIO_GRANT_TTL_MS,
      identity,
    })
    expect(JSON.stringify(json)).not.toContain(secret)
  })

  test('refuses missing/revoked bearer before minting', async () => {
    const response = await handleAudioGrantIssueRequest(
      dependencies(),
      request(body, { deviceRef: identity.deviceRef }),
      {},
      context,
    )
    expect(response.status).toBe(401)
    expect(await response.json()).toEqual({ error: 'unauthorized' })
  })

  test('refuses owner and device substitution', async () => {
    for (const candidate of [
      request(
        { ...body, identity: { ...identity, ownerRef: 'github:other' } },
        {
          authorization: 'Bearer desktop-session',
          deviceRef: identity.deviceRef,
        },
      ),
      request(body, {
        authorization: 'Bearer desktop-session',
        deviceRef: 'desktop.other',
      }),
      request(body, { authorization: 'Bearer desktop-session' }),
    ]) {
      const response = await handleAudioGrantIssueRequest(
        dependencies(),
        candidate,
        {},
        context,
      )
      expect(response.status).toBe(403)
      expect(await response.json()).toEqual({
        error: 'audio_identity_mismatch',
      })
    }
  })

  test('rejects excess, malformed, and out-of-bound identity fields', async () => {
    const invalid = [
      { ...body, extra: true },
      { ...body, identity: { ...identity, generation: 0 } },
      { ...body, identity: { ...identity, threadRef: 'x'.repeat(257) } },
      { ...body, disclosureRef: '' },
    ]
    for (const input of invalid) {
      const response = await handleAudioGrantIssueRequest(
        dependencies(),
        request(input, {
          authorization: 'Bearer desktop-session',
          deviceRef: identity.deviceRef,
        }),
        {},
        context,
      )
      expect(response.status).toBe(400)
    }
  })

  test('rejects a body above the bounded host request size', async () => {
    const oversized = new Request(
      `https://openagents.com${AUDIO_GRANT_ISSUE_PATH}`,
      {
        body: JSON.stringify({ ...body, padding: 'x'.repeat(9_000) }),
        headers: {
          authorization: 'Bearer desktop-session',
          [AUDIO_GRANT_DEVICE_HEADER]: identity.deviceRef,
        },
        method: 'POST',
      },
    )
    const response = await handleAudioGrantIssueRequest(
      dependencies(),
      oversized,
      {},
      context,
    )
    expect(response.status).toBe(400)
  })

  test('fails closed when the shared signing secret is missing or weak', async () => {
    for (const missingSecret of ['', 'too-short']) {
      const response = await handleAudioGrantIssueRequest(
        dependencies({ secret: missingSecret }),
        request(body, {
          authorization: 'Bearer desktop-session',
          deviceRef: identity.deviceRef,
        }),
        {},
        context,
      )
      expect(response.status).toBe(503)
      expect(await response.json()).toEqual({
        error: 'audio_grant_issuer_unavailable',
      })
    }
  })

  test('fails closed when the server-derived gateway URL is absent or unsafe', async () => {
    for (const gatewayUrl of [
      '',
      'https://openagents-audio.example/v1/stream',
      'wss://user:pass@openagents-audio.example/v1/stream',
      'wss://openagents-audio.example/v1/stream?token=caller',
      'wss://openagents-audio.example/other',
    ]) {
      const response = await handleAudioGrantIssueRequest(
        dependencies({ gatewayUrl }),
        request(body, {
          authorization: 'Bearer desktop-session',
          deviceRef: identity.deviceRef,
        }),
        {},
        context,
      )
      expect(response.status).toBe(503)
      expect(await response.json()).toEqual({
        error: 'audio_grant_issuer_unavailable',
      })
    }
  })

  test('allows POST only', async () => {
    const response = await handleAudioGrantIssueRequest(
      dependencies(),
      request(undefined, { method: 'GET' }),
      {},
      context,
    )
    expect(response.status).toBe(405)
  })
})
