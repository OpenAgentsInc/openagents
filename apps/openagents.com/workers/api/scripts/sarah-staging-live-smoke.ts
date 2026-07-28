import {
  AUDIO_MEDIA_MAGIC,
  AUDIO_PROTOCOL_VERSION,
  MAX_AUDIO_PAYLOAD_BYTES,
  SARAH_VOICE_NOSTR_AUTH_METHOD,
  SARAH_VOICE_NOSTR_CHALLENGE_PATH,
  SARAH_VOICE_NOSTR_CHALLENGE_PROTOCOL_VERSION,
  SARAH_VOICE_PROTOCOL_VERSION,
  SARAH_VOICE_SESSION_PATH,
  SarahVoiceNostrChallengeResponseSchema,
  SarahVoiceServerControlSchema,
  SarahVoiceSessionResponseSchema,
  type VoiceIdentity,
} from '@openagentsinc/audio-contract'
import { Schema as S } from 'effect'
import { createHash, randomUUID } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { hashPayloadBytes } from 'nostr-effect/nip98'
import {
  finalizeEvent,
  generateSecretKey,
  getPublicKey,
} from 'nostr-effect/pure'
import postgres from 'postgres'
import WebSocket, { type RawData } from 'ws'

const EXPECTED_RESERVATION_MSAT = 256_000
const EXPECTED_CREDIT_RATE_MSAT_PER_MILLION_TOKENS = 64_000_000
const EXPECTED_MAX_DURATION_SECONDS = 300
const CLIENT_DEADLINE_MS = 45_000
const SETTLEMENT_DEADLINE_MS = 20_000

const OmegaSessionResponse = S.Struct({
  accessToken: S.String,
  expiresIn: S.Number,
  user: S.Struct({
    userId: S.String,
    provider: S.String,
  }),
})

const AccountRow = S.Struct({
  balance_msat: S.Union([S.Number, S.String]),
  held_msat: S.Union([S.Number, S.String]),
})

const SessionEvidenceRow = S.Struct({
  state: S.String,
  reserved_msat: S.Union([S.Number, S.String]),
  charged_msat: S.Union([S.Number, S.String]),
  input_tokens: S.Union([S.Number, S.String]),
  output_tokens: S.Union([S.Number, S.String]),
  cached_input_tokens: S.Union([S.Number, S.String]),
  audio_input_tokens: S.Union([S.Number, S.String]),
  audio_output_tokens: S.Union([S.Number, S.String]),
  settlement_receipt_ref: S.NullOr(S.String),
  close_reason: S.NullOr(S.String),
})

const UsageEvidenceRow = S.Struct({
  usage_rows: S.Union([S.Number, S.String]),
  usage_charge_msat: S.Union([S.Number, S.String]),
})

const requiredEnv = (name: string): string => {
  const value = process.env[name]?.trim()
  if (value === undefined || value === '') {
    throw new Error(`${name} is required`)
  }
  return value
}

const exactIntegerEnv = (name: string, expected: number): number => {
  const parsed = Number(requiredEnv(name))
  if (!Number.isSafeInteger(parsed) || parsed !== expected) {
    throw new Error(`${name} must equal ${expected}`)
  }
  return parsed
}

const sha256Hex = (value: Uint8Array | string): string =>
  createHash('sha256').update(value).digest('hex')

const nip98Authorization = (
  secret: Uint8Array,
  url: string,
  payload: Uint8Array,
): string => {
  const event = finalizeEvent(
    {
      content: '',
      created_at: Math.floor(Date.now() / 1_000),
      kind: 27_235,
      tags: [
        ['u', url],
        ['method', 'POST'],
        ['payload', hashPayloadBytes(payload)],
      ],
    },
    secret,
  )
  return `Nostr ${Buffer.from(JSON.stringify(event), 'utf8').toString('base64')}`
}

const decodeJsonResponse = async <A, I>(
  response: Response,
  schema: S.Schema<A, I>,
  operation: string,
): Promise<A> => {
  const body: unknown = await response.json()
  if (!response.ok) {
    const safeError =
      typeof body === 'object' &&
      body !== null &&
      'error' in body &&
      typeof body.error === 'string'
        ? body.error
        : 'request_rejected'
    throw new Error(`${operation} failed (${response.status} ${safeError})`)
  }
  return S.decodeUnknownSync(schema)(body, { onExcessProperty: 'preserve' })
}

const fundDedicatedAccount = async (
  sql: postgres.Sql,
  input: Readonly<{
    actorRef: string
    fundingMsat: number
    runRef: string
    userId: string
  }>,
): Promise<string> => {
  const nowIso = new Date().toISOString()
  const payInId = `sarah:staging:fund:${input.runRef}`
  const legId = `${payInId}:balance`
  const receiptRef = `receipt.staging.sarah_voice_test_funding.${input.runRef}`
  await sql.begin(async tx => {
    const users: unknown = await tx`
      SELECT id
      FROM users
      WHERE id = ${input.userId}
        AND status = 'active'
        AND deleted_at IS NULL
      FOR UPDATE
    `
    if (!Array.isArray(users) || users.length !== 1) {
      throw new Error('The dedicated staging user is not active')
    }
    await tx`
      INSERT INTO agent_balances (
        actor_ref, balance_msat, held_msat, created_at, updated_at
      ) VALUES (
        ${input.actorRef}, 0, 0, ${nowIso}, ${nowIso}
      )
      ON CONFLICT (actor_ref) DO NOTHING
    `
    const balances = S.decodeUnknownSync(S.Array(AccountRow))(
      await tx`
        SELECT balance_msat, held_msat
        FROM agent_balances
        WHERE actor_ref = ${input.actorRef}
        FOR UPDATE
      `,
    )
    const balance = balances[0]
    if (
      balance === undefined ||
      Number(balance.balance_msat) !== 0 ||
      Number(balance.held_msat) !== 0
    ) {
      throw new Error('The dedicated staging account is not empty')
    }
    await tx`
      INSERT INTO pay_ins (
        id, pay_in_type, payer_ref, cost_msat, state, rung, context_ref,
        idempotency_key, public_receipt_ref, genesis_id, created_at,
        state_changed_at
      ) VALUES (
        ${payInId}, 'adjustment', 'operator:staging_sarah_voice_smoke',
        ${input.fundingMsat}, 'paid', NULL, ${input.runRef},
        ${`sarah:staging:fund:${input.runRef}`}, ${receiptRef}, NULL,
        ${nowIso}, ${nowIso}
      )
    `
    await tx`
      UPDATE agent_balances
      SET balance_msat = balance_msat + ${input.fundingMsat},
          updated_at = ${nowIso}
      WHERE actor_ref = ${input.actorRef}
    `
    await tx`
      INSERT INTO pay_in_legs (
        id, pay_in_id, direction, kind, party_ref, amount_msat,
        resulting_balance_msat, external_ref, refund_of_leg_id, created_at
      )
      SELECT ${legId}, ${payInId}, 'out', 'balance', ${input.actorRef},
        ${input.fundingMsat}, balance_msat, 'staging_sarah_voice_smoke',
        NULL, ${nowIso}
      FROM agent_balances
      WHERE actor_ref = ${input.actorRef}
    `
  })
  return receiptRef
}

const clawBackUnusedFunding = async (
  sql: postgres.Sql,
  input: Readonly<{
    actorRef: string
    runRef: string
  }>,
): Promise<Readonly<{ clawedBackMsat: number; receiptRef: string | null }>> => {
  const nowIso = new Date().toISOString()
  return sql.begin(async tx => {
    const balances = S.decodeUnknownSync(S.Array(AccountRow))(
      await tx`
        SELECT balance_msat, held_msat
        FROM agent_balances
        WHERE actor_ref = ${input.actorRef}
        FOR UPDATE
      `,
    )
    const balance = balances[0]
    if (balance === undefined) throw new Error('The test balance is missing')
    const balanceMsat = Number(balance.balance_msat)
    const heldMsat = Number(balance.held_msat)
    if (
      !Number.isSafeInteger(balanceMsat) ||
      balanceMsat < 0 ||
      heldMsat !== 0
    ) {
      throw new Error('The test balance is not ready for cleanup')
    }
    if (balanceMsat === 0) {
      return { clawedBackMsat: 0, receiptRef: null }
    }
    const payInId = `sarah:staging:cleanup:${input.runRef}`
    const receiptRef = `receipt.staging.sarah_voice_test_cleanup.${input.runRef}`
    await tx`
      INSERT INTO pay_ins (
        id, pay_in_type, payer_ref, cost_msat, state, rung, context_ref,
        idempotency_key, public_receipt_ref, genesis_id, created_at,
        state_changed_at
      ) VALUES (
        ${payInId}, 'adjustment', ${input.actorRef}, ${balanceMsat}, 'paid',
        NULL, ${input.runRef}, ${`sarah:staging:cleanup:${input.runRef}`},
        ${receiptRef}, NULL, ${nowIso}, ${nowIso}
      )
    `
    await tx`
      UPDATE agent_balances
      SET balance_msat = balance_msat - ${balanceMsat},
          updated_at = ${nowIso}
      WHERE actor_ref = ${input.actorRef}
        AND held_msat = 0
        AND balance_msat = ${balanceMsat}
    `
    await tx`
      INSERT INTO pay_in_legs (
        id, pay_in_id, direction, kind, party_ref, amount_msat,
        resulting_balance_msat, external_ref, refund_of_leg_id, created_at
      )
      SELECT ${`${payInId}:balance`}, ${payInId}, 'in', 'balance',
        ${input.actorRef}, ${balanceMsat}, balance_msat,
        'staging_sarah_voice_smoke_cleanup', NULL, ${nowIso}
      FROM agent_balances
      WHERE actor_ref = ${input.actorRef}
    `
    return { clawedBackMsat: balanceMsat, receiptRef }
  })
}

const encodeClientAudio = (
  identity: VoiceIdentity,
  sequence: number,
  payload: Uint8Array,
): Uint8Array => {
  const header = Buffer.from(
    JSON.stringify({
      schema: AUDIO_PROTOCOL_VERSION,
      kind: 'client_audio',
      identity,
      sequence,
      codec: 'pcm_s16le',
      sampleRateHz: 24_000,
      channels: 1,
      payloadLength: payload.byteLength,
      sha256: sha256Hex(payload),
    }),
    'utf8',
  )
  const frame = Buffer.allocUnsafe(8 + header.byteLength + payload.byteLength)
  frame.write(AUDIO_MEDIA_MAGIC, 0, 'ascii')
  frame.writeUInt32BE(header.byteLength, 4)
  header.copy(frame, 8)
  Buffer.from(payload).copy(frame, 8 + header.byteLength)
  return frame
}

const serverAudioPayloadBytes = (data: RawData): number => {
  const bytes = Buffer.isBuffer(data)
    ? data
    : Array.isArray(data)
      ? Buffer.concat(data)
      : Buffer.from(data)
  if (
    bytes.byteLength < 8 ||
    bytes.subarray(0, 4).toString('ascii') !== AUDIO_MEDIA_MAGIC
  ) {
    throw new Error('The server audio frame has an invalid envelope')
  }
  const headerLength = bytes.readUInt32BE(4)
  const header = JSON.parse(
    bytes.subarray(8, 8 + headerLength).toString('utf8'),
  ) as unknown
  const decoded = S.decodeUnknownSync(
    S.Struct({
      kind: S.Literal('server_tts'),
      payloadLength: S.Number,
    }),
  )(header, { onExcessProperty: 'preserve' })
  const payloadLength = bytes.byteLength - 8 - headerLength
  if (payloadLength !== decoded.payloadLength || payloadLength <= 0) {
    throw new Error('The server audio payload length is invalid')
  }
  return payloadLength
}

const runVoiceSocket = async (
  gatewayUrl: string,
  input: Readonly<{
    audio: Uint8Array
    disclosureRef: string
    identity: VoiceIdentity
    ticket: string
  }>,
): Promise<
  Readonly<{
    assistantTranscriptFinal: boolean
    audioBytes: number
    closeCode: number
    interruptAck: boolean
    sessionReady: boolean
    userTranscriptFinal: boolean
  }>
> =>
  new Promise((resolve, reject) => {
    const socket = new WebSocket(gatewayUrl, {
      headers: {
        'x-openagents-sarah-voice-session': input.identity.sessionRef,
        'x-openagents-sarah-voice-ticket': input.ticket,
      },
      handshakeTimeout: 10_000,
      maxPayload: 1_048_576,
    })
    let assistantTranscriptFinal = false
    let audioAcknowledgements = 0
    let audioBytes = 0
    let closeSent = false
    let controlSequence = 0
    let interruptAck = false
    let interruptSent = false
    let sessionReady = false
    let userTranscriptFinal = false
    const timer = setTimeout(() => {
      socket.close(1000, 'transport_error')
      reject(
        new Error(
          `The Sarah voice client deadline expired (${JSON.stringify({
            assistantTranscriptFinal,
            audioAcknowledgements,
            audioBytes,
            interruptAck,
            sessionReady,
            userTranscriptFinal,
          })})`,
        ),
      )
    }, CLIENT_DEADLINE_MS)

    const sendControl = (
      control:
        | Readonly<{ _tag: 'session_hello'; disclosureRef: string }>
        | Readonly<{ _tag: 'interrupt' }>
        | Readonly<{ _tag: 'close'; reason: 'user_stop' }>,
    ): void => {
      socket.send(
        JSON.stringify({
          schema: SARAH_VOICE_PROTOCOL_VERSION,
          identity: input.identity,
          sequence: controlSequence,
          ...control,
        }),
      )
      controlSequence += 1
    }

    const maybeInterrupt = (): void => {
      if (
        !interruptSent &&
        userTranscriptFinal &&
        assistantTranscriptFinal &&
        audioBytes > 0
      ) {
        interruptSent = true
        sendControl({ _tag: 'interrupt' })
      }
    }

    const sendAudio = (): void => {
      const silence = new Uint8Array(24_000 * 2 * 3)
      const source = Buffer.concat([
        Buffer.from(input.audio),
        Buffer.from(silence),
      ])
      let audioSequence = 0
      for (
        let offset = 0;
        offset < source.byteLength;
        offset += MAX_AUDIO_PAYLOAD_BYTES
      ) {
        const payload = source.subarray(
          offset,
          Math.min(offset + MAX_AUDIO_PAYLOAD_BYTES, source.byteLength),
        )
        socket.send(encodeClientAudio(input.identity, audioSequence, payload))
        audioSequence += 1
      }
    }

    socket.on('open', () => {
      sendControl({
        _tag: 'session_hello',
        disclosureRef: input.disclosureRef,
      })
    })
    socket.on('message', (data, isBinary) => {
      try {
        if (isBinary) {
          audioBytes += serverAudioPayloadBytes(data)
          maybeInterrupt()
          return
        }
        const control = S.decodeUnknownSync(SarahVoiceServerControlSchema)(
          JSON.parse(data.toString()) as unknown,
          { onExcessProperty: 'error' },
        )
        if (control._tag === 'session_ready' && !sessionReady) {
          sessionReady = true
          sendAudio()
        } else if (control._tag === 'audio_ack') {
          audioAcknowledgements += 1
        } else if (control._tag === 'transcript_final') {
          if (control.source === 'user') userTranscriptFinal = true
          if (control.source === 'assistant') {
            assistantTranscriptFinal = true
          }
          maybeInterrupt()
        } else if (control._tag === 'interrupt_ack') {
          interruptAck = true
          if (!closeSent) {
            closeSent = true
            sendControl({ _tag: 'close', reason: 'user_stop' })
          }
        } else if (control._tag === 'error') {
          throw new Error(`The gateway returned ${control.code}`)
        }
      } catch (error) {
        socket.close(1000, 'transport_error')
        reject(error)
      }
    })
    socket.on('error', error => {
      clearTimeout(timer)
      reject(error)
    })
    socket.on('close', code => {
      clearTimeout(timer)
      if (
        !sessionReady ||
        !userTranscriptFinal ||
        !assistantTranscriptFinal ||
        audioBytes === 0 ||
        !interruptAck ||
        !closeSent
      ) {
        reject(new Error('The Sarah voice lifecycle was incomplete'))
        return
      }
      resolve({
        assistantTranscriptFinal,
        audioBytes,
        closeCode: code,
        interruptAck,
        sessionReady,
        userTranscriptFinal,
      })
    })
  })

const waitForSettlement = async (
  sql: postgres.Sql,
  sessionRef: string,
): Promise<
  Readonly<{
    session: typeof SessionEvidenceRow.Type
    usage: typeof UsageEvidenceRow.Type
  }>
> => {
  const deadline = Date.now() + SETTLEMENT_DEADLINE_MS
  while (Date.now() < deadline) {
    const sessions = S.decodeUnknownSync(S.Array(SessionEvidenceRow))(
      await sql`
        SELECT state, reserved_msat, charged_msat, input_tokens,
          output_tokens, cached_input_tokens, audio_input_tokens,
          audio_output_tokens, settlement_receipt_ref, close_reason
        FROM sarah_realtime_voice_sessions
        WHERE session_ref = ${sessionRef}
      `,
    )
    const session = sessions[0]
    if (session !== undefined && session.state === 'settled') {
      const usages = S.decodeUnknownSync(S.Array(UsageEvidenceRow))(
        await sql`
          SELECT COUNT(*) AS usage_rows,
            COALESCE(SUM(charge_msat), 0) AS usage_charge_msat
          FROM sarah_realtime_voice_usage
          WHERE session_ref = ${sessionRef}
        `,
      )
      const usage = usages[0]
      if (usage === undefined) throw new Error('Usage evidence is missing')
      return { session, usage }
    }
    await new Promise<void>(resolve => {
      setTimeout(resolve, 500)
    })
  }
  throw new Error('The Sarah voice session did not settle before the deadline')
}

const main = async (): Promise<void> => {
  const baseUrl = requiredEnv('SARAH_STAGING_BASE_URL').replace(/\/+$/u, '')
  const databaseHost = requiredEnv('SARAH_STAGING_DB_HOST')
  const databaseName = requiredEnv('SARAH_STAGING_DB_NAME')
  const databasePassword = requiredEnv('SARAH_STAGING_DB_PASSWORD')
  const databasePort = Number(requiredEnv('SARAH_STAGING_DB_PORT'))
  const databaseUser = requiredEnv('SARAH_STAGING_DB_USER')
  if (
    !Number.isSafeInteger(databasePort) ||
    databasePort < 1 ||
    databasePort > 65_535
  ) {
    throw new Error('SARAH_STAGING_DB_PORT must be a valid port')
  }
  const audioPath = requiredEnv('SARAH_STAGING_PCM_PATH')
  const fundingMsat = exactIntegerEnv(
    'SARAH_STAGING_FUNDING_MSAT',
    EXPECTED_RESERVATION_MSAT,
  )
  exactIntegerEnv(
    'SARAH_STAGING_CREDIT_RATE_MSAT_PER_MILLION_TOKENS',
    EXPECTED_CREDIT_RATE_MSAT_PER_MILLION_TOKENS,
  )
  const runRef = `sarah-staging-smoke-${randomUUID()}`
  const secret = generateSecretKey()
  const pubkey = getPublicKey(secret)
  const identityDigest = sha256Hex(pubkey)
  const sql = postgres({
    database: databaseName,
    host: databaseHost,
    max: 1,
    password: databasePassword,
    port: databasePort,
    username: databaseUser,
  })
  let actorRef: string | undefined
  let cleanupCompleted = false
  let fundingReceiptRef: string | undefined

  try {
    const omegaSessionUrl = `${baseUrl}/api/omega/auth/session`
    const emptyPayload = new Uint8Array()
    const omegaSession = await decodeJsonResponse(
      await fetch(omegaSessionUrl, {
        method: 'POST',
        headers: {
          authorization: nip98Authorization(
            secret,
            omegaSessionUrl,
            emptyPayload,
          ),
        },
        body: emptyPayload,
      }),
      OmegaSessionResponse,
      'Omega Nostr self-provisioning',
    )
    if (omegaSession.user.provider !== 'nostr') {
      throw new Error(
        'The smoke identity did not get a dedicated Nostr account',
      )
    }
    actorRef = `agent:${omegaSession.user.userId}`
    fundingReceiptRef = await fundDedicatedAccount(sql, {
      actorRef,
      fundingMsat,
      runRef,
      userId: omegaSession.user.userId,
    })

    const deviceRef = `omega-staging-smoke-${randomUUID()}`
    const challengeUrl = `${baseUrl}${SARAH_VOICE_NOSTR_CHALLENGE_PATH}`
    const challengeRequest = JSON.stringify({
      schema: SARAH_VOICE_NOSTR_CHALLENGE_PROTOCOL_VERSION,
      deviceRef,
      pubkey,
    })
    const challenge = await decodeJsonResponse(
      await fetch(challengeUrl, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: challengeRequest,
      }),
      SarahVoiceNostrChallengeResponseSchema,
      'Sarah Nostr challenge',
    )
    const identity: VoiceIdentity = {
      ownerRef: challenge.ownerRef,
      deviceRef,
      threadRef: `thread-staging-smoke-${randomUUID()}`,
      sessionRef: `voice-staging-smoke-${randomUUID()}`,
      generation: 1,
    }
    const disclosureRef = 'omega.voice.disclosure.v1'
    const sessionBody = JSON.stringify({
      schema: SARAH_VOICE_PROTOCOL_VERSION,
      identity,
      disclosureRef,
      auth: {
        method: SARAH_VOICE_NOSTR_AUTH_METHOD,
        challenge: challenge.challenge,
      },
    })
    const sessionPayload = new TextEncoder().encode(sessionBody)
    const sessionUrl = `${baseUrl}${SARAH_VOICE_SESSION_PATH}`
    const session = await decodeJsonResponse(
      await fetch(sessionUrl, {
        method: 'POST',
        headers: {
          authorization: nip98Authorization(secret, sessionUrl, sessionPayload),
          'content-type': 'application/json',
          'x-openagents-omega-device-ref': deviceRef,
        },
        body: sessionBody,
      }),
      SarahVoiceSessionResponseSchema,
      'Sarah voice session',
    )
    if (
      session.reservedCreditMsat !== EXPECTED_RESERVATION_MSAT ||
      session.maxDurationSeconds !== EXPECTED_MAX_DURATION_SECONDS
    ) {
      throw new Error('The live Sarah session has unexpected staging limits')
    }

    const socket = await runVoiceSocket(session.gatewayUrl, {
      audio: await readFile(audioPath),
      disclosureRef,
      identity,
      ticket: session.ticket,
    })
    const evidence = await waitForSettlement(sql, identity.sessionRef)
    const sessionChargeMsat = Number(evidence.session.charged_msat)
    const usageChargeMsat = Number(evidence.usage.usage_charge_msat)
    const usageRows = Number(evidence.usage.usage_rows)
    if (
      evidence.session.state !== 'settled' ||
      evidence.session.close_reason !== 'user_stop' ||
      sessionChargeMsat <= 0 ||
      sessionChargeMsat > EXPECTED_RESERVATION_MSAT ||
      usageChargeMsat < sessionChargeMsat ||
      usageRows < 1
    ) {
      throw new Error('The Sarah settlement evidence is invalid')
    }
    const cleanup = await clawBackUnusedFunding(sql, { actorRef, runRef })
    cleanupCompleted = true
    const finalBalances = S.decodeUnknownSync(S.Array(AccountRow))(
      await sql`
        SELECT balance_msat, held_msat
        FROM agent_balances
        WHERE actor_ref = ${actorRef}
      `,
    )
    const finalBalance = finalBalances[0]
    if (
      finalBalance === undefined ||
      Number(finalBalance.balance_msat) !== 0 ||
      Number(finalBalance.held_msat) !== 0
    ) {
      throw new Error('The dedicated test balance did not return to zero')
    }

    console.log(
      JSON.stringify({
        schema: 'openagents.sarah.voice.staging-smoke.v1',
        ok: true,
        runRef,
        identityDigest,
        configuration: {
          reservationMsat: EXPECTED_RESERVATION_MSAT,
          creditMsatPerMillionTokens:
            EXPECTED_CREDIT_RATE_MSAT_PER_MILLION_TOKENS,
          maxDurationSeconds: EXPECTED_MAX_DURATION_SECONDS,
          clientDeadlineSeconds: CLIENT_DEADLINE_MS / 1_000,
        },
        authentication: {
          challenge: 'accepted',
          nip98: 'accepted',
          shortLivedSession: 'issued',
          deviceBinding: 'accepted',
        },
        gateway: socket,
        provider: {
          model: session.model,
          inputTokens: Number(evidence.session.input_tokens),
          outputTokens: Number(evidence.session.output_tokens),
          cachedInputTokens: Number(evidence.session.cached_input_tokens),
          audioInputTokens: Number(evidence.session.audio_input_tokens),
          audioOutputTokens: Number(evidence.session.audio_output_tokens),
          usageRows,
        },
        credit: {
          fundedMsat: fundingMsat,
          chargedMsat: sessionChargeMsat,
          usageChargeMsat,
          clawedBackMsat: cleanup.clawedBackMsat,
          finalBalanceMsat: 0,
          finalHeldMsat: 0,
          fundingReceiptRef,
          settlementReceiptRef: evidence.session.settlement_receipt_ref,
          cleanupReceiptRef: cleanup.receiptRef,
        },
        lifecycle: {
          sessionState: evidence.session.state,
          closeReason: evidence.session.close_reason,
        },
      }),
    )
  } finally {
    secret.fill(0)
    if (
      actorRef !== undefined &&
      fundingReceiptRef !== undefined &&
      !cleanupCompleted
    ) {
      try {
        await clawBackUnusedFunding(sql, { actorRef, runRef })
      } catch {
        process.stderr.write(
          'The staging smoke cleanup needs operator reconciliation.\n',
        )
      }
    }
    await sql.end()
  }
}

await main()
