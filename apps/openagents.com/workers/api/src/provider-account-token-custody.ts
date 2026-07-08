import type {
  CodexOAuthAuth,
  IdFactory,
  ProviderAccountProvider,
} from './provider-account-domain'
import { CHATGPT_CODEX_PROVIDER } from './provider-account-domain'
import {
  ProviderAccountCredentialMaterial,
  ProviderAccountNotFound,
  ProviderAccountRefMismatch,
  ProviderAccountStorageFailed,
  ProviderTokenCustodyRefreshFailed,
} from './provider-account-errors'
import { refreshOpenAiCodexOAuthAuth } from './provider-account-client'
import {
  compactRandomId,
  currentDate,
  currentIsoTimestamp,
  epochMillisToIsoTimestamp,
} from './runtime-primitives'

export const PROVIDER_TOKEN_CUSTODY_KEY_ID =
  'provider-token-custody-aes-gcm.v1'
export const PROVIDER_TOKEN_CUSTODY_REFRESH_BUFFER_MS = 1000 * 60 * 5

type EncryptedText = Readonly<{
  ciphertextB64: string
  ivB64: string
  keyId: string
}>

type ProviderAccountTokenCustodyRecord = Readonly<{
  providerAccountRef: string
  ownerUserId: string
  provider: typeof CHATGPT_CODEX_PROVIDER
  secretRef: string
  refreshToken: EncryptedText
  accessToken: EncryptedText
  accessExpiresAt: string
  accountId?: string | undefined
  idToken?: EncryptedText | undefined
  createdAt: string
  updatedAt: string
  lastRefreshedAt?: string | undefined
}>

type ProviderAccountTokenCustodyAuditEvent = Readonly<{
  id: string
  providerAccountRef: string
  ownerUserId: string
  provider: typeof CHATGPT_CODEX_PROVIDER
  eventKind:
    | 'auth_stored'
    | 'access_issued'
    | 'auth_deleted'
    | 'refresh_succeeded'
    | 'refresh_failed'
  status: 'succeeded' | 'failed'
  actorRef?: string | undefined
  sourceRef?: string | undefined
  errorTag?: string | undefined
  errorMessage?: string | undefined
  metadataJson?: string | undefined
  createdAt: string
}>

type ProviderAccountTokenCustodyRow = Readonly<{
  provider_account_ref: string
  owner_user_id: string
  provider: ProviderAccountProvider
  secret_ref: string
  refresh_ciphertext_b64: string
  refresh_iv_b64: string
  refresh_key_id: string
  access_ciphertext_b64: string
  access_iv_b64: string
  access_key_id: string
  access_expires_at: string
  account_id: string | null
  id_token_ciphertext_b64: string | null
  id_token_iv_b64: string | null
  id_token_key_id: string | null
  created_at: string
  updated_at: string
  last_refreshed_at: string | null
}>

export type ProviderAccountTokenCustodyStore = Readonly<{
  findByOwnerAndRef: (
    ownerUserId: string,
    providerAccountRef: string,
  ) => Promise<ProviderAccountTokenCustodyRecord | undefined>
  findByRef: (
    providerAccountRef: string,
  ) => Promise<ProviderAccountTokenCustodyRecord | undefined>
  upsertConnectedAuth: (
    record: ProviderAccountTokenCustodyRecord,
    auditEvent: ProviderAccountTokenCustodyAuditEvent,
  ) => Promise<void>
  saveRefreshedAuth: (
    record: ProviderAccountTokenCustodyRecord,
    auditEvent: ProviderAccountTokenCustodyAuditEvent,
  ) => Promise<void>
  insertAuditEvent: (
    auditEvent: ProviderAccountTokenCustodyAuditEvent,
  ) => Promise<void>
  deleteByOwnerAndRef: (
    ownerUserId: string,
    providerAccountRef: string,
    auditEvent: ProviderAccountTokenCustodyAuditEvent,
  ) => Promise<boolean>
}>

export type ProviderTokenCustodyKeyEnv = Readonly<{
  PROVIDER_TOKEN_CUSTODY_AES_KEY_B64?: string | undefined
  PROVIDER_TOKEN_CUSTODY_AES_KEY_ID?: string | undefined
}>

export type ProviderAccountTokenCustodyCipher = Readonly<{
  keyId: string
  encryptText: (plaintext: string) => Promise<EncryptedText>
  decryptText: (encrypted: EncryptedText) => Promise<string>
}>

export type ProviderAccountShortLivedCodexAccess = Readonly<{
  providerAccountRef: string
  access: string
  expires: number
  accountId?: string | undefined
  idToken?: string | undefined
}>

export type ProviderAccountCodexAuthMaterial = Readonly<{
  authContentEnv: 'OPENCODE_AUTH_CONTENT'
  authContentJson: string
}>

export type RefreshCodexOAuthAuth = (
  auth: CodexOAuthAuth,
) => ReturnType<typeof refreshOpenAiCodexOAuthAuth>

const textEncoder = new TextEncoder()
const textDecoder = new TextDecoder()

const bytesToBase64 = (bytes: Uint8Array): string =>
  btoa(Array.from(bytes, byte => String.fromCharCode(byte)).join(''))

const base64ToBytes = (value: string): Uint8Array<ArrayBuffer> | undefined => {
  const normalized = value
    .trim()
    .replaceAll('-', '+')
    .replaceAll('_', '/')
    .padEnd(Math.ceil(value.trim().length / 4) * 4, '=')

  try {
    const binary = atob(normalized)
    const bytes = new Uint8Array(binary.length)
    Array.from(binary).forEach((character, index) => {
      bytes[index] = character.charCodeAt(0)
    })

    return bytes
  } catch {
    return undefined
  }
}

const arrayBufferFromBytes = (bytes: Uint8Array): ArrayBuffer => {
  const copy = new Uint8Array(bytes.byteLength)
  copy.set(bytes)

  return copy.buffer
}

const requiredTokenText = (value: string, fieldName: string): string => {
  if (value.trim() === '') {
    throw new ProviderAccountCredentialMaterial({
      fieldName,
      message: 'Codex token custody material is missing a required field.',
    })
  }

  return value
}

const providerSecretRef = (providerAccountRef: string): string =>
  `codex-auth://${providerAccountRef}`

const toRecord = (
  row: ProviderAccountTokenCustodyRow,
): ProviderAccountTokenCustodyRecord => {
  if (row.provider !== CHATGPT_CODEX_PROVIDER) {
    throw new ProviderAccountStorageFailed({
      operation: 'provider_token_custody_reload',
      message: 'Provider token custody row has an unsupported provider.',
    })
  }

  return {
    providerAccountRef: row.provider_account_ref,
    ownerUserId: row.owner_user_id,
    provider: row.provider,
    secretRef: row.secret_ref,
    refreshToken: {
      ciphertextB64: row.refresh_ciphertext_b64,
      ivB64: row.refresh_iv_b64,
      keyId: row.refresh_key_id,
    },
    accessToken: {
      ciphertextB64: row.access_ciphertext_b64,
      ivB64: row.access_iv_b64,
      keyId: row.access_key_id,
    },
    accessExpiresAt: row.access_expires_at,
    ...(row.account_id === null ? {} : { accountId: row.account_id }),
    ...(row.id_token_ciphertext_b64 === null ||
    row.id_token_iv_b64 === null ||
    row.id_token_key_id === null
      ? {}
      : {
          idToken: {
            ciphertextB64: row.id_token_ciphertext_b64,
            ivB64: row.id_token_iv_b64,
            keyId: row.id_token_key_id,
          },
        }),
    createdAt: row.created_at,
    updatedAt: row.updated_at,
    ...(row.last_refreshed_at === null
      ? {}
      : { lastRefreshedAt: row.last_refreshed_at }),
  }
}

const bindAuditEvent = (
  statement: D1PreparedStatement,
  event: ProviderAccountTokenCustodyAuditEvent,
): D1PreparedStatement =>
  statement.bind(
    event.id,
    event.providerAccountRef,
    event.ownerUserId,
    event.provider,
    event.eventKind,
    event.status,
    event.actorRef ?? null,
    event.sourceRef ?? null,
    event.errorTag ?? null,
    event.errorMessage ?? null,
    event.metadataJson ?? null,
    event.createdAt,
  )

const auditInsertStatement = (db: D1Database): D1PreparedStatement =>
  db.prepare(
    `INSERT INTO provider_account_token_custody_audit
      (id, provider_account_ref, owner_user_id, provider, event_kind, status,
       actor_ref, source_ref, error_tag, error_message, metadata_json, created_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
  )

export const makeD1ProviderAccountTokenCustodyStore = (
  db: D1Database,
): ProviderAccountTokenCustodyStore => ({
  findByOwnerAndRef: async (ownerUserId, providerAccountRef) => {
    const row = await db
      .prepare(
        `SELECT *
         FROM provider_account_token_custody
         WHERE owner_user_id = ?
           AND provider_account_ref = ?`,
      )
      .bind(ownerUserId, providerAccountRef)
      .first<ProviderAccountTokenCustodyRow>()

    return row === null ? undefined : toRecord(row)
  },

  findByRef: async providerAccountRef => {
    const row = await db
      .prepare(
        `SELECT *
         FROM provider_account_token_custody
         WHERE provider_account_ref = ?`,
      )
      .bind(providerAccountRef)
      .first<ProviderAccountTokenCustodyRow>()

    return row === null ? undefined : toRecord(row)
  },

  upsertConnectedAuth: async (record, auditEvent) => {
    const existing = await db
      .prepare(
        `SELECT owner_user_id
         FROM provider_account_token_custody
         WHERE provider_account_ref = ?`,
      )
      .bind(record.providerAccountRef)
      .first<{ owner_user_id: string }>()

    if (
      existing !== null &&
      existing.owner_user_id !== record.ownerUserId
    ) {
      throw new ProviderAccountRefMismatch({
        message: 'Provider account custody row belongs to a different owner.',
      })
    }

    await db.batch([
      db
        .prepare(
          `INSERT INTO provider_account_token_custody
            (provider_account_ref, owner_user_id, provider, secret_ref,
             refresh_ciphertext_b64, refresh_iv_b64, refresh_key_id,
             access_ciphertext_b64, access_iv_b64, access_key_id,
             access_expires_at, account_id, id_token_ciphertext_b64,
             id_token_iv_b64, id_token_key_id, created_at, updated_at,
             last_refreshed_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(provider_account_ref) DO UPDATE SET
             secret_ref = excluded.secret_ref,
             refresh_ciphertext_b64 = excluded.refresh_ciphertext_b64,
             refresh_iv_b64 = excluded.refresh_iv_b64,
             refresh_key_id = excluded.refresh_key_id,
             access_ciphertext_b64 = excluded.access_ciphertext_b64,
             access_iv_b64 = excluded.access_iv_b64,
             access_key_id = excluded.access_key_id,
             access_expires_at = excluded.access_expires_at,
             account_id = excluded.account_id,
             id_token_ciphertext_b64 = excluded.id_token_ciphertext_b64,
             id_token_iv_b64 = excluded.id_token_iv_b64,
             id_token_key_id = excluded.id_token_key_id,
             updated_at = excluded.updated_at,
             last_refreshed_at = excluded.last_refreshed_at
           WHERE provider_account_token_custody.owner_user_id = excluded.owner_user_id`,
        )
        .bind(
          record.providerAccountRef,
          record.ownerUserId,
          record.provider,
          record.secretRef,
          record.refreshToken.ciphertextB64,
          record.refreshToken.ivB64,
          record.refreshToken.keyId,
          record.accessToken.ciphertextB64,
          record.accessToken.ivB64,
          record.accessToken.keyId,
          record.accessExpiresAt,
          record.accountId ?? null,
          record.idToken?.ciphertextB64 ?? null,
          record.idToken?.ivB64 ?? null,
          record.idToken?.keyId ?? null,
          record.createdAt,
          record.updatedAt,
          record.lastRefreshedAt ?? null,
        ),
      bindAuditEvent(auditInsertStatement(db), auditEvent),
    ])
  },

  saveRefreshedAuth: async (record, auditEvent) => {
    await db.batch([
      db
        .prepare(
          `UPDATE provider_account_token_custody
           SET refresh_ciphertext_b64 = ?,
               refresh_iv_b64 = ?,
               refresh_key_id = ?,
               access_ciphertext_b64 = ?,
               access_iv_b64 = ?,
               access_key_id = ?,
               access_expires_at = ?,
               account_id = ?,
               id_token_ciphertext_b64 = ?,
               id_token_iv_b64 = ?,
               id_token_key_id = ?,
               updated_at = ?,
               last_refreshed_at = ?
           WHERE provider_account_ref = ?
             AND owner_user_id = ?`,
        )
        .bind(
          record.refreshToken.ciphertextB64,
          record.refreshToken.ivB64,
          record.refreshToken.keyId,
          record.accessToken.ciphertextB64,
          record.accessToken.ivB64,
          record.accessToken.keyId,
          record.accessExpiresAt,
          record.accountId ?? null,
          record.idToken?.ciphertextB64 ?? null,
          record.idToken?.ivB64 ?? null,
          record.idToken?.keyId ?? null,
          record.updatedAt,
          record.lastRefreshedAt ?? null,
          record.providerAccountRef,
          record.ownerUserId,
        ),
      bindAuditEvent(auditInsertStatement(db), auditEvent),
    ])
  },

  insertAuditEvent: async auditEvent => {
    await bindAuditEvent(auditInsertStatement(db), auditEvent).run()
  },

  deleteByOwnerAndRef: async (ownerUserId, providerAccountRef, auditEvent) => {
    const existing = await db
      .prepare(
        `SELECT owner_user_id
         FROM provider_account_token_custody
         WHERE provider_account_ref = ?`,
      )
      .bind(providerAccountRef)
      .first<{ owner_user_id: string }>()

    if (existing === null) {
      await bindAuditEvent(auditInsertStatement(db), auditEvent).run()
      return false
    }

    if (existing.owner_user_id !== ownerUserId) {
      throw new ProviderAccountRefMismatch({
        message: 'Provider account custody row belongs to a different owner.',
      })
    }

    await db.batch([
      db
        .prepare(
          `DELETE FROM provider_account_token_custody
           WHERE owner_user_id = ?
             AND provider_account_ref = ?`,
        )
        .bind(ownerUserId, providerAccountRef),
      bindAuditEvent(auditInsertStatement(db), auditEvent),
    ])

    return true
  },
})

export const makeProviderAccountTokenCustodyCipher = async (
  input: Readonly<{
    keyId: string
    keyBytes: Uint8Array
  }>,
): Promise<ProviderAccountTokenCustodyCipher> => {
  if (input.keyBytes.byteLength !== 32) {
    throw new ProviderAccountCredentialMaterial({
      fieldName: 'PROVIDER_TOKEN_CUSTODY_AES_KEY_B64',
      message: 'Provider token custody AES-GCM key must decode to 32 bytes.',
    })
  }

  const key = await crypto.subtle.importKey(
    'raw',
    arrayBufferFromBytes(input.keyBytes),
    { name: 'AES-GCM' },
    false,
    ['encrypt', 'decrypt'],
  )

  return {
    keyId: input.keyId,
    encryptText: async plaintext => {
      const iv = crypto.getRandomValues(new Uint8Array(12))
      const ciphertext = await crypto.subtle.encrypt(
        { name: 'AES-GCM', iv },
        key,
        textEncoder.encode(plaintext),
      )

      return {
        ciphertextB64: bytesToBase64(new Uint8Array(ciphertext)),
        ivB64: bytesToBase64(iv),
        keyId: input.keyId,
      }
    },
    decryptText: async encrypted => {
      if (encrypted.keyId !== input.keyId) {
        throw new ProviderAccountCredentialMaterial({
          fieldName: 'provider_account_token_custody.key_id',
          message: 'Provider token custody key id is not available.',
        })
      }

      const iv = base64ToBytes(encrypted.ivB64)
      const ciphertext = base64ToBytes(encrypted.ciphertextB64)

      if (iv === undefined || ciphertext === undefined) {
        throw new ProviderAccountCredentialMaterial({
          fieldName: 'provider_account_token_custody.ciphertext',
          message: 'Provider token custody ciphertext is malformed.',
        })
      }

      try {
        const plaintext = await crypto.subtle.decrypt(
          { name: 'AES-GCM', iv },
          key,
          arrayBufferFromBytes(ciphertext),
        )

        return textDecoder.decode(plaintext)
      } catch (error) {
        throw new ProviderAccountStorageFailed({
          operation: 'provider_token_custody_decrypt',
          message: error instanceof Error ? error.message : String(error),
        })
      }
    },
  }
}

export const providerAccountTokenCustodyCipherFromEnv = async (
  env: ProviderTokenCustodyKeyEnv,
): Promise<ProviderAccountTokenCustodyCipher> => {
  const encodedKey = env.PROVIDER_TOKEN_CUSTODY_AES_KEY_B64?.trim()

  if (encodedKey === undefined || encodedKey === '') {
    throw new ProviderAccountCredentialMaterial({
      fieldName: 'PROVIDER_TOKEN_CUSTODY_AES_KEY_B64',
      message: 'Provider token custody AES-GCM key is not configured.',
    })
  }

  const keyBytes = base64ToBytes(encodedKey)

  if (keyBytes === undefined) {
    throw new ProviderAccountCredentialMaterial({
      fieldName: 'PROVIDER_TOKEN_CUSTODY_AES_KEY_B64',
      message: 'Provider token custody AES-GCM key is not valid base64.',
    })
  }

  return makeProviderAccountTokenCustodyCipher({
    keyBytes,
    keyId:
      env.PROVIDER_TOKEN_CUSTODY_AES_KEY_ID?.trim() ||
      PROVIDER_TOKEN_CUSTODY_KEY_ID,
  })
}

const encryptedAuthRecord = async (
  cipher: ProviderAccountTokenCustodyCipher,
  input: Readonly<{
    ownerUserId: string
    providerAccountRef: string
    auth: CodexOAuthAuth
    createdAt: string
    updatedAt: string
    lastRefreshedAt?: string | undefined
  }>,
): Promise<ProviderAccountTokenCustodyRecord> => {
  const expiresAt = epochMillisToIsoTimestamp(input.auth.expires)

  return {
    providerAccountRef: input.providerAccountRef,
    ownerUserId: input.ownerUserId,
    provider: CHATGPT_CODEX_PROVIDER,
    secretRef: providerSecretRef(input.providerAccountRef),
    refreshToken: await cipher.encryptText(
      requiredTokenText(input.auth.refresh, 'auth.refresh'),
    ),
    accessToken: await cipher.encryptText(
      requiredTokenText(input.auth.access, 'auth.access'),
    ),
    accessExpiresAt: expiresAt,
    ...(input.auth.accountId === undefined
      ? {}
      : { accountId: requiredTokenText(input.auth.accountId, 'auth.accountId') }),
    ...(input.auth.idToken === undefined
      ? {}
      : { idToken: await cipher.encryptText(input.auth.idToken) }),
    createdAt: input.createdAt,
    updatedAt: input.updatedAt,
    ...(input.lastRefreshedAt === undefined
      ? {}
      : { lastRefreshedAt: input.lastRefreshedAt }),
  }
}

const auditEvent = (
  input: Readonly<{
    ownerUserId: string
    providerAccountRef: string
    eventKind: ProviderAccountTokenCustodyAuditEvent['eventKind']
    status: ProviderAccountTokenCustodyAuditEvent['status']
    nowIso: string
    makeId: IdFactory
    actorRef?: string | undefined
    sourceRef?: string | undefined
    errorTag?: string | undefined
    errorMessage?: string | undefined
    metadataJson?: string | undefined
  }>,
): ProviderAccountTokenCustodyAuditEvent => ({
  id: input.makeId('provider_token_custody_audit'),
  providerAccountRef: input.providerAccountRef,
  ownerUserId: input.ownerUserId,
  provider: CHATGPT_CODEX_PROVIDER,
  eventKind: input.eventKind,
  status: input.status,
  ...(input.actorRef === undefined ? {} : { actorRef: input.actorRef }),
  ...(input.sourceRef === undefined ? {} : { sourceRef: input.sourceRef }),
  ...(input.errorTag === undefined ? {} : { errorTag: input.errorTag }),
  ...(input.errorMessage === undefined
    ? {}
    : { errorMessage: input.errorMessage }),
  ...(input.metadataJson === undefined
    ? {}
    : { metadataJson: input.metadataJson }),
  createdAt: input.nowIso,
})

export const storeConnectedCodexAuthInCustody = async (
  store: ProviderAccountTokenCustodyStore,
  cipher: ProviderAccountTokenCustodyCipher,
  input: Readonly<{
    ownerUserId: string
    providerAccountRef: string
    auth: CodexOAuthAuth
    nowIso?: string | undefined
    makeId?: IdFactory | undefined
  }>,
): Promise<string> => {
  const nowIso = input.nowIso ?? currentIsoTimestamp()
  const makeId = input.makeId ?? compactRandomId
  const existing = await store.findByRef(input.providerAccountRef)

  if (existing !== undefined && existing.ownerUserId !== input.ownerUserId) {
    throw new ProviderAccountRefMismatch({
      message: 'Provider account custody row belongs to a different owner.',
    })
  }

  const record = await encryptedAuthRecord(cipher, {
    ownerUserId: input.ownerUserId,
    providerAccountRef: input.providerAccountRef,
    auth: input.auth,
    createdAt: existing?.createdAt ?? nowIso,
    updatedAt: nowIso,
    lastRefreshedAt: nowIso,
  })

  await store.upsertConnectedAuth(
    record,
    auditEvent({
      eventKind: 'auth_stored',
      makeId,
      nowIso,
      ownerUserId: input.ownerUserId,
      providerAccountRef: input.providerAccountRef,
      sourceRef: `providerAccount:${input.providerAccountRef}`,
      status: 'succeeded',
    }),
  )

  return record.secretRef
}

export const deleteConnectedCodexAuthFromCustody = async (
  store: ProviderAccountTokenCustodyStore,
  input: Readonly<{
    ownerUserId: string
    providerAccountRef: string
    actorRef?: string | undefined
    nowIso?: string | undefined
    makeId?: IdFactory | undefined
  }>,
): Promise<boolean> => {
  const nowIso = input.nowIso ?? currentIsoTimestamp()
  const makeId = input.makeId ?? compactRandomId

  return store.deleteByOwnerAndRef(
    input.ownerUserId,
    input.providerAccountRef,
    auditEvent({
      actorRef: input.actorRef,
      eventKind: 'auth_deleted',
      makeId,
      nowIso,
      ownerUserId: input.ownerUserId,
      providerAccountRef: input.providerAccountRef,
      sourceRef: `providerAccount:${input.providerAccountRef}`,
      status: 'succeeded',
    }),
  )
}

const shortLivedAccessFromRecord = async (
  record: ProviderAccountTokenCustodyRecord,
  cipher: ProviderAccountTokenCustodyCipher,
): Promise<ProviderAccountShortLivedCodexAccess> => ({
  providerAccountRef: record.providerAccountRef,
  access: await cipher.decryptText(record.accessToken),
  expires: Date.parse(record.accessExpiresAt),
  ...(record.accountId === undefined ? {} : { accountId: record.accountId }),
  ...(record.idToken === undefined
    ? {}
    : { idToken: await cipher.decryptText(record.idToken) }),
})

export const issueShortLivedCodexAccessFromCustody = async (
  store: ProviderAccountTokenCustodyStore,
  cipher: ProviderAccountTokenCustodyCipher,
  input: Readonly<{
    ownerUserId: string
    providerAccountRef: string
    now?: Date | undefined
    makeId?: IdFactory | undefined
    refreshCodexOAuthAuth?: RefreshCodexOAuthAuth | undefined
    actorRef?: string | undefined
  }>,
): Promise<ProviderAccountShortLivedCodexAccess> => {
  const now = input.now ?? currentDate()
  const nowIso = now.toISOString()
  const makeId = input.makeId ?? compactRandomId
  const record = await store.findByOwnerAndRef(
    input.ownerUserId,
    input.providerAccountRef,
  )

  if (record === undefined) {
    throw new ProviderAccountNotFound({
      message: 'Provider token custody row was not found for this owner.',
    })
  }

  if (
    Date.parse(record.accessExpiresAt) - now.getTime() >
    PROVIDER_TOKEN_CUSTODY_REFRESH_BUFFER_MS
  ) {
    const access = await shortLivedAccessFromRecord(record, cipher)

    await store.insertAuditEvent(
      auditEvent({
        actorRef: input.actorRef,
        eventKind: 'access_issued',
        makeId,
        nowIso,
        ownerUserId: input.ownerUserId,
        providerAccountRef: input.providerAccountRef,
        sourceRef: `providerAccount:${input.providerAccountRef}`,
        status: 'succeeded',
      }),
    )

    return access
  }

  const refreshToken = await cipher.decryptText(record.refreshToken)
  const accessToken = await cipher.decryptText(record.accessToken)
  const idToken =
    record.idToken === undefined
      ? undefined
      : await cipher.decryptText(record.idToken)
  const refreshAuth = input.refreshCodexOAuthAuth ?? refreshOpenAiCodexOAuthAuth
  const refreshed = await refreshAuth({
    type: 'oauth',
    refresh: refreshToken,
    access: accessToken,
    expires: Date.parse(record.accessExpiresAt),
    ...(record.accountId === undefined ? {} : { accountId: record.accountId }),
    ...(idToken === undefined ? {} : { idToken }),
  })

  if (refreshed.status === 'failed') {
    await store.insertAuditEvent(
      auditEvent({
        actorRef: input.actorRef,
        errorMessage: refreshed.code,
        errorTag: refreshed.failureClass,
        eventKind: 'refresh_failed',
        makeId,
        metadataJson: JSON.stringify({
          providerStatus: refreshed.providerStatus,
        }),
        nowIso,
        ownerUserId: input.ownerUserId,
        providerAccountRef: input.providerAccountRef,
        sourceRef: `providerAccount:${input.providerAccountRef}`,
        status: 'failed',
      }),
    )

    throw new ProviderTokenCustodyRefreshFailed({
      providerAccountRef: input.providerAccountRef,
      failureClass: refreshed.failureClass,
      providerStatus: refreshed.providerStatus,
      message: `Provider token custody refresh failed: ${refreshed.failureClass}.`,
    })
  }

  const refreshedRecord = await encryptedAuthRecord(cipher, {
    ownerUserId: record.ownerUserId,
    providerAccountRef: record.providerAccountRef,
    auth: refreshed.auth,
    createdAt: record.createdAt,
    updatedAt: nowIso,
    lastRefreshedAt: nowIso,
  })

  await store.saveRefreshedAuth(
    refreshedRecord,
    auditEvent({
      actorRef: input.actorRef,
      eventKind: 'refresh_succeeded',
      makeId,
      nowIso,
      ownerUserId: input.ownerUserId,
      providerAccountRef: input.providerAccountRef,
      sourceRef: `providerAccount:${input.providerAccountRef}`,
      status: 'succeeded',
    }),
  )

  return shortLivedAccessFromRecord(refreshedRecord, cipher)
}

export const codexAccessToAuthMaterial = (
  access: ProviderAccountShortLivedCodexAccess,
): ProviderAccountCodexAuthMaterial => ({
  authContentEnv: 'OPENCODE_AUTH_CONTENT',
  authContentJson: JSON.stringify({
    openai: {
      type: 'oauth',
      access: access.access,
      expires: access.expires,
      ...(access.accountId === undefined ? {} : { accountId: access.accountId }),
      ...(access.idToken === undefined ? {} : { idToken: access.idToken }),
    },
  }),
})
