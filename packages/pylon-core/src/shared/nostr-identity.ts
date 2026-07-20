import { existsSync, lstatSync, readFileSync, statSync } from "node:fs"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { homedir } from "node:os"
import { dirname, join } from "node:path"
import { secp256k1, schnorr } from "@noble/curves/secp256k1"
import { sha256 } from "@noble/hashes/sha256"
import { HDKey } from "@scure/bip32"
import { bech32 } from "@scure/base"
import { generateMnemonic, mnemonicToSeedSync, validateMnemonic } from "@scure/bip39"
import { wordlist } from "@scure/bip39/wordlists/english"
import type { BootstrapSummary } from "./bootstrap.js"

export const ENV_OPENAGENTS_IDENTITY_MNEMONIC_PATH = "OPENAGENTS_IDENTITY_MNEMONIC_PATH"
export const ENV_OPENAGENTS_PYLON_HOME = "OPENAGENTS_PYLON_HOME"
export const NIP06_DERIVATION_PATH = "m/44'/1237'/0'/0/0"
export const NIP98_KIND = 27235

export type NostrIdentityPathResolution = {
  path: string
  source:
    | "openagents_identity_mnemonic_path"
    | "historical_config_identity_path"
    | "openagents_pylon_home"
    | "explicit_pylon_home"
    | "legacy_default"
}

export type PylonNostrPrivateIdentity = {
  identityPath: string
  mnemonic: string
  publicKey: string
  npub: string
  nsec: string
  privateKeyHex: string
  privateKeyBytes: Uint8Array
}

export type Nip98Event = {
  id: string
  pubkey: string
  created_at: number
  kind: typeof NIP98_KIND
  tags: string[][]
  content: string
  sig: string
}

function bytesToHex(bytes: Uint8Array) {
  return Buffer.from(bytes).toString("hex")
}

function hexToBytes(hex: string) {
  if (!/^[0-9a-f]+$/i.test(hex) || hex.length % 2 !== 0) throw new Error("invalid hex")
  return Uint8Array.from(Buffer.from(hex, "hex"))
}

function sha256Hex(input: string | Uint8Array) {
  return bytesToHex(sha256(typeof input === "string" ? Buffer.from(input) : input))
}

function readHistoricalIdentityPathFromConfig(configPath: string) {
  if (!existsSync(configPath)) return null
  try {
    const raw = JSON.parse(readFileSync(configPath, "utf8")) as { identity_path?: unknown }
    return typeof raw.identity_path === "string" && raw.identity_path.trim() ? raw.identity_path.trim() : null
  } catch {
    return null
  }
}

export function resolveNostrIdentityPath(
  paths: BootstrapSummary["paths"],
  env: NodeJS.ProcessEnv = process.env,
): NostrIdentityPathResolution {
  const directOverride = env[ENV_OPENAGENTS_IDENTITY_MNEMONIC_PATH]?.trim()
  if (directOverride) {
    return { path: directOverride, source: "openagents_identity_mnemonic_path" }
  }

  const historicalConfigPath = env.OPENAGENTS_PYLON_CONFIG_PATH?.trim() || join(paths.home, "config.json")
  const configuredIdentityPath = readHistoricalIdentityPathFromConfig(historicalConfigPath)
  if (configuredIdentityPath) {
    return { path: configuredIdentityPath, source: "historical_config_identity_path" }
  }

  const openagentsPylonHome = env[ENV_OPENAGENTS_PYLON_HOME]?.trim()
  if (openagentsPylonHome) {
    return { path: join(openagentsPylonHome, "identity.mnemonic"), source: "openagents_pylon_home" }
  }

  if (env.PYLON_HOME?.trim() || paths.home !== join(homedir(), ".pylon")) {
    return { path: join(paths.home, "identity.mnemonic"), source: "explicit_pylon_home" }
  }

  return { path: join(homedir(), ".openagents", "pylon", "identity.mnemonic"), source: "legacy_default" }
}

function normalizeMnemonic(value: string) {
  return value.trim().split(/\s+/).join(" ")
}

async function readMnemonic(path: string) {
  const mnemonic = normalizeMnemonic(await readFile(path, "utf8"))
  if (!mnemonic) throw new Error(`identity mnemonic is empty at ${path}`)
  if (!validateMnemonic(mnemonic, wordlist)) throw new Error(`identity mnemonic is not valid BIP39 English at ${path}`)
  return mnemonic
}

async function writeMnemonic(path: string, mnemonic: string) {
  await mkdir(dirname(path), { recursive: true })
  // Exclusive create (flag "wx"): if a file already exists at the path this
  // fails with EEXIST rather than overwriting it. Create NEVER clobbers an
  // existing identity, even under a race with another process.
  await writeFile(path, `${mnemonic}\n`, { mode: 0o600, flag: "wx" })
  try {
    await import("node:fs/promises").then(({ chmod }) => chmod(path, 0o600))
  } catch {
    // chmod is best-effort on non-POSIX filesystems; writeFile mode covers the normal path.
  }
}

export function assertPrivateMnemonicPermissions(path: string) {
  if (process.platform === "win32" || !existsSync(path)) return
  const mode = statSync(path).mode & 0o777
  if ((mode & 0o077) !== 0) {
    throw new Error(`identity mnemonic permissions must not grant group/other access: ${path}`)
  }
}

/**
 * A fail-closed custody blocker for the identity mnemonic. It never carries the
 * mnemonic. `symbolic_link_refused` refuses a symbolic link by default (possible
 * substitution); `weak_permissions` refuses a file that grants group/other
 * access; `unexpected_file_type` refuses anything that is not a regular file.
 */
export type NostrIdentityCustodyBlocker =
  | "symbolic_link_refused"
  | "weak_permissions"
  | "unexpected_file_type"

/** Thrown when an OPEN finds no existing identity. Open NEVER creates one. */
export class NostrIdentityNotFoundError extends Error {
  readonly code = "nostr_identity_no_candidate" as const
  readonly identityPath: string
  constructor(identityPath: string) {
    super(`no existing Nostr identity mnemonic at ${identityPath}`)
    this.name = "NostrIdentityNotFoundError"
    this.identityPath = identityPath
  }
}

/** Thrown when a candidate exists but fails a fail-closed custody check. */
export class NostrIdentityCustodyBlockedError extends Error {
  readonly code = "nostr_identity_custody_blocked" as const
  readonly identityPath: string
  readonly blocker: NostrIdentityCustodyBlocker
  constructor(identityPath: string, blocker: NostrIdentityCustodyBlocker) {
    super(`Nostr identity custody blocked (${blocker}) at ${identityPath}`)
    this.name = "NostrIdentityCustodyBlockedError"
    this.identityPath = identityPath
    this.blocker = blocker
  }
}

/** Thrown when an explicit CREATE is asked to write over an existing candidate. */
export class NostrIdentityAlreadyExistsError extends Error {
  readonly code = "nostr_identity_already_exists" as const
  readonly identityPath: string
  constructor(identityPath: string) {
    super(`refusing to create over an existing Nostr identity at ${identityPath}`)
    this.name = "NostrIdentityAlreadyExistsError"
    this.identityPath = identityPath
  }
}

type IdentityFileInspection =
  | { readonly kind: "absent" }
  | { readonly kind: "symbolic_link" }
  | { readonly kind: "unexpected_file_type" }
  | { readonly kind: "regular_file"; readonly weakPermissions: boolean }

/**
 * Inspect the identity path by EXISTENCE and metadata only, using `lstat`. It
 * never follows a symbolic link and never reads the file content, so an open
 * path reads no secret bytes while classifying the candidate.
 */
function inspectIdentityFile(path: string): IdentityFileInspection {
  let stat: ReturnType<typeof lstatSync>
  try {
    stat = lstatSync(path)
  } catch (error) {
    const code = (error as { code?: unknown }).code
    if (code === "ENOENT" || code === "ENOTDIR") return { kind: "absent" }
    throw error
  }
  if (stat.isSymbolicLink()) return { kind: "symbolic_link" }
  if (!stat.isFile()) return { kind: "unexpected_file_type" }
  const weakPermissions = process.platform !== "win32" && ((stat.mode & 0o077) !== 0)
  return { kind: "regular_file", weakPermissions }
}

/**
 * OPEN an EXISTING Nostr identity, or fail closed. It resolves the selected path,
 * classifies it existence-only, refuses a symbolic link or a weak-permission
 * file, reads the mnemonic only for a valid regular-file candidate, and derives
 * the identity. It NEVER creates or overwrites a file: a missing candidate throws
 * `NostrIdentityNotFoundError`. This is the recovery/open-safe entry point.
 */
export async function openNostrIdentity(
  paths: BootstrapSummary["paths"],
  env: NodeJS.ProcessEnv = process.env,
): Promise<PylonNostrPrivateIdentity> {
  const resolution = resolveNostrIdentityPath(paths, env)
  const inspection = inspectIdentityFile(resolution.path)
  switch (inspection.kind) {
    case "absent":
      throw new NostrIdentityNotFoundError(resolution.path)
    case "symbolic_link":
      throw new NostrIdentityCustodyBlockedError(resolution.path, "symbolic_link_refused")
    case "unexpected_file_type":
      throw new NostrIdentityCustodyBlockedError(resolution.path, "unexpected_file_type")
    case "regular_file":
      if (inspection.weakPermissions) {
        throw new NostrIdentityCustodyBlockedError(resolution.path, "weak_permissions")
      }
      return deriveNip06Identity(await readMnemonic(resolution.path), resolution.path)
  }
}

/**
 * CREATE a NEW Nostr identity. This is the SEPARATE explicit operation: the
 * caller invokes it on purpose to mint a mnemonic. It refuses to overwrite an
 * existing candidate (`NostrIdentityAlreadyExistsError`), so it never clobbers a
 * root. An open or recovery path must never call this.
 */
export async function createNostrIdentity(
  paths: BootstrapSummary["paths"],
  env: NodeJS.ProcessEnv = process.env,
): Promise<PylonNostrPrivateIdentity> {
  const resolution = resolveNostrIdentityPath(paths, env)
  if (inspectIdentityFile(resolution.path).kind !== "absent") {
    throw new NostrIdentityAlreadyExistsError(resolution.path)
  }
  const mnemonic = generateMnemonic(wordlist, 128)
  await writeMnemonic(resolution.path, mnemonic)
  return deriveNip06Identity(mnemonic, resolution.path)
}

export function deriveNip06Identity(mnemonic: string, identityPath: string): PylonNostrPrivateIdentity {
  const normalized = normalizeMnemonic(mnemonic)
  if (!validateMnemonic(normalized, wordlist)) throw new Error("invalid NIP-06 mnemonic")
  const seed = mnemonicToSeedSync(normalized, "")
  const node = HDKey.fromMasterSeed(seed).derive(NIP06_DERIVATION_PATH)
  if (!node.privateKey) throw new Error("failed to derive NIP-06 private key")
  const privateKeyBytes = node.privateKey
  const publicKeyBytes = secp256k1.getPublicKey(privateKeyBytes, true).slice(1)
  const publicKey = bytesToHex(publicKeyBytes)
  const privateKeyHex = bytesToHex(privateKeyBytes)

  return {
    identityPath,
    mnemonic: normalized,
    publicKey,
    npub: encodeNip19("npub", publicKeyBytes),
    nsec: encodeNip19("nsec", privateKeyBytes),
    privateKeyHex,
    privateKeyBytes,
  }
}

/**
 * The intentional REHYDRATE-OR-CREATE path. It OPENS an existing identity, and
 * ONLY when the candidate is genuinely absent does it CREATE a new one. A
 * fail-closed custody blocker (symbolic link, weak permissions, unexpected type)
 * is re-thrown, never "fixed" by creating over the suspect file. This is the
 * explicit create-on-missing path the Desktop Boot Sequence (#9103) and the
 * Pylon bootstrap rely on; an OPEN/RECOVERY path uses `openNostrIdentity`
 * instead and never auto-creates.
 */
export async function loadOrCreateNostrIdentity(
  paths: BootstrapSummary["paths"],
  env: NodeJS.ProcessEnv = process.env,
): Promise<PylonNostrPrivateIdentity> {
  try {
    return await openNostrIdentity(paths, env)
  } catch (error) {
    if (error instanceof NostrIdentityNotFoundError) {
      return await createNostrIdentity(paths, env)
    }
    throw error
  }
}

export function encodeNip19(prefix: "npub" | "nsec", bytes: Uint8Array) {
  return bech32.encode(prefix, bech32.toWords(bytes))
}

export function decodeNip19(prefix: "npub" | "nsec", value: string) {
  const decoded = bech32.decode(value as `${string}1${string}`, 2048)
  if (decoded.prefix !== prefix) throw new Error(`expected ${prefix}`)
  return Uint8Array.from(bech32.fromWords(decoded.words))
}

export function sha256HexBody(input: string) {
  return sha256Hex(input)
}

function serializeNostrEvent(event: Omit<Nip98Event, "id" | "sig">) {
  return JSON.stringify([0, event.pubkey, event.created_at, event.kind, event.tags, event.content])
}

export function createNip98Event(input: {
  method: string
  url: string
  body: string
  identity: PylonNostrPrivateIdentity
  now?: Date
}): Nip98Event {
  const unsigned = {
    pubkey: input.identity.publicKey,
    created_at: Math.floor((input.now ?? new Date()).getTime() / 1000),
    kind: NIP98_KIND,
    tags: [
      ["u", input.url],
      ["method", input.method.toUpperCase()],
      ["payload", sha256HexBody(input.body)],
    ],
    content: "",
  } satisfies Omit<Nip98Event, "id" | "sig">
  const id = sha256Hex(serializeNostrEvent(unsigned))
  const sig = bytesToHex(schnorr.sign(hexToBytes(id), input.identity.privateKeyBytes))
  return { ...unsigned, id, sig }
}

export function encodeNip98Authorization(event: Nip98Event) {
  return `Nostr ${Buffer.from(JSON.stringify(event), "utf8").toString("base64")}`
}

export function decodeNip98Authorization(value: string | null): Nip98Event {
  if (!value?.startsWith("Nostr ")) throw new Error("missing Nostr authorization")
  const event = JSON.parse(Buffer.from(value.slice("Nostr ".length), "base64").toString("utf8")) as Nip98Event
  return event
}

export function verifyNip98Authorization(
  authorization: string | null,
  input: { method: string; url: string; body: string; now?: Date; maxSkewSeconds?: number },
) {
  const event = decodeNip98Authorization(authorization)
  if (event.kind !== NIP98_KIND) throw new Error("invalid NIP-98 kind")
  if (event.content !== "") throw new Error("invalid NIP-98 content")
  const expectedId = sha256Hex(serializeNostrEvent(event))
  if (event.id !== expectedId) throw new Error("invalid NIP-98 event id")

  const tagValue = (name: string) => event.tags.find((tag) => tag[0] === name)?.[1]
  if (tagValue("u") !== input.url) throw new Error("invalid NIP-98 URL")
  if (tagValue("method") !== input.method.toUpperCase()) throw new Error("invalid NIP-98 method")
  if (tagValue("payload") !== sha256HexBody(input.body)) throw new Error("invalid NIP-98 payload hash")

  const nowSeconds = Math.floor((input.now ?? new Date()).getTime() / 1000)
  const maxSkewSeconds = input.maxSkewSeconds ?? 60
  if (Math.abs(nowSeconds - event.created_at) > maxSkewSeconds) throw new Error("stale NIP-98 event")

  if (!schnorr.verify(hexToBytes(event.sig), hexToBytes(event.id), hexToBytes(event.pubkey))) {
    throw new Error("invalid NIP-98 signature")
  }
  return event
}
