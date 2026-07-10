import {
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs"
import path from "node:path"

import { Schema } from "effect"

export const OPENAGENTS_DESKTOP_SESSION_EPOCH =
  "2026-07-10-openauth-native-session-v1"

const DesktopSessionRecordSchema = Schema.Struct({
  schemaVersion: Schema.Literal(1),
  credentialEpoch: Schema.Literal(OPENAGENTS_DESKTOP_SESSION_EPOCH),
  ownerUserId: Schema.String,
  accessToken: Schema.String,
  refreshToken: Schema.String,
})

const EncryptedDesktopSessionEnvelopeSchema = Schema.Struct({
  schemaVersion: Schema.Literal(1),
  encryption: Schema.Literal("electron-safe-storage"),
  payload: Schema.String,
})

type DesktopSessionRecord = typeof DesktopSessionRecordSchema.Type

export type DesktopSessionCredential = Readonly<{
  ownerUserId: string
  accessToken: string
  refreshToken: string
}>

export type DesktopSessionRecovery = Readonly<{
  state: "signed_out" | "credential_present_unverified"
}>

export type SafeStorageLike = Readonly<{
  isEncryptionAvailable: () => boolean
  encryptString: (plainText: string) => Buffer
  decryptString: (encrypted: Buffer) => string
  getSelectedStorageBackend?: () => string
}>

export type DesktopSessionVaultErrorReason =
  | "encryption_unavailable"
  | "invalid_credential"
  | "storage_unavailable"

export class DesktopSessionVaultError extends Error {
  readonly _tag = "DesktopSessionVaultError"
  override readonly name = "DesktopSessionVaultError"

  constructor(
    readonly reason: DesktopSessionVaultErrorReason,
    message: string,
  ) {
    super(message)
  }
}

export type DesktopSessionVault = Readonly<{
  save: (credential: DesktopSessionCredential) => void
  load: () => DesktopSessionCredential | null
  clear: () => void
  recover: () => DesktopSessionRecovery
}>

const publicFailure = (error: unknown): DesktopSessionVaultError =>
  error instanceof DesktopSessionVaultError
    ? error
    : new DesktopSessionVaultError(
        "storage_unavailable",
        "desktop secure session storage is unavailable",
      )

const normalizedCredential = (
  credential: DesktopSessionCredential,
): DesktopSessionCredential => {
  const normalized = {
    ownerUserId: credential.ownerUserId.trim(),
    accessToken: credential.accessToken.trim(),
    refreshToken: credential.refreshToken.trim(),
  }
  if (
    normalized.ownerUserId === "" ||
    normalized.accessToken === "" ||
    normalized.refreshToken === ""
  ) {
    throw new DesktopSessionVaultError(
      "invalid_credential",
      "desktop session credential is incomplete",
    )
  }
  return normalized
}

const requireOsEncryption = (safeStorage: SafeStorageLike): void => {
  let available = false
  let backend: string | undefined
  try {
    available = safeStorage.isEncryptionAvailable()
    backend = safeStorage.getSelectedStorageBackend?.()
  } catch {
    available = false
  }
  if (!available || backend === "basic_text") {
    throw new DesktopSessionVaultError(
      "encryption_unavailable",
      "OS-encrypted desktop session custody is unavailable",
    )
  }
}

const ensurePrivateParent = (filePath: string): void => {
  const parent = path.dirname(filePath)
  mkdirSync(parent, { recursive: true, mode: 0o700 })
  if (process.platform !== "win32") chmodSync(parent, 0o700)
}

const writePrivateAtomic = (filePath: string, content: string): void => {
  ensurePrivateParent(filePath)
  const pendingPath = `${filePath}.pending`
  try {
    rmSync(pendingPath, { force: true })
    writeFileSync(pendingPath, content, { encoding: "utf8", mode: 0o600 })
    if (process.platform !== "win32") chmodSync(pendingPath, 0o600)
    renameSync(pendingPath, filePath)
    if (process.platform !== "win32") chmodSync(filePath, 0o600)
  } catch (error) {
    rmSync(pendingPath, { force: true })
    throw error
  }
}

const decodeCredential = (
  raw: string,
  safeStorage: SafeStorageLike,
): DesktopSessionCredential => {
  const envelope = Schema.decodeUnknownSync(
    EncryptedDesktopSessionEnvelopeSchema,
  )(JSON.parse(raw))
  const plaintext = safeStorage.decryptString(
    Buffer.from(envelope.payload, "base64"),
  )
  const record = Schema.decodeUnknownSync(DesktopSessionRecordSchema)(
    JSON.parse(plaintext),
  )
  return normalizedCredential(record)
}

export const openDesktopSessionVault = (input: Readonly<{
  filePath: string
  safeStorage: SafeStorageLike
}>): DesktopSessionVault => {
  const filePath = path.resolve(input.filePath)
  requireOsEncryption(input.safeStorage)

  const clear = (): void => {
    try {
      rmSync(filePath, { force: true })
      rmSync(`${filePath}.pending`, { force: true })
    } catch (error) {
      throw publicFailure(error)
    }
  }

  const load = (): DesktopSessionCredential | null => {
    let raw: string
    try {
      if (!existsSync(filePath)) return null
      raw = readFileSync(filePath, "utf8")
    } catch (error) {
      throw publicFailure(error)
    }
    try {
      return decodeCredential(raw, input.safeStorage)
    } catch {
      try {
        clear()
        return null
      } catch (error) {
        throw publicFailure(error)
      }
    }
  }

  return {
    save: credential => {
      const normalized = normalizedCredential(credential)
      const record: DesktopSessionRecord = {
        schemaVersion: 1,
        credentialEpoch: OPENAGENTS_DESKTOP_SESSION_EPOCH,
        ...normalized,
      }
      try {
        const encrypted = input.safeStorage.encryptString(JSON.stringify(record))
        writePrivateAtomic(filePath, JSON.stringify({
          schemaVersion: 1,
          encryption: "electron-safe-storage",
          payload: encrypted.toString("base64"),
        }))
      } catch (error) {
        throw publicFailure(error)
      }
    },
    load,
    clear,
    recover: () => ({
      state: load() === null
        ? "signed_out"
        : "credential_present_unverified",
    }),
  }
}
