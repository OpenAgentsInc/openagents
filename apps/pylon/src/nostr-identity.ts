import { existsSync, readFileSync, statSync } from "node:fs"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { createHash } from "node:crypto"
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
  await writeFile(path, `${mnemonic}\n`, { mode: 0o600 })
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

export async function loadOrCreateNostrIdentity(
  paths: BootstrapSummary["paths"],
  env: NodeJS.ProcessEnv = process.env,
): Promise<PylonNostrPrivateIdentity> {
  const resolution = resolveNostrIdentityPath(paths, env)
  if (existsSync(resolution.path)) {
    assertPrivateMnemonicPermissions(resolution.path)
    return deriveNip06Identity(await readMnemonic(resolution.path), resolution.path)
  }

  const mnemonic = generateMnemonic(wordlist, 128)
  await writeMnemonic(resolution.path, mnemonic)
  return deriveNip06Identity(mnemonic, resolution.path)
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
