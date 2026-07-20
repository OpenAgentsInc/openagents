import { chmodSync, existsSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { afterEach, beforeEach, describe, expect, test } from "vite-plus/test"

import {
  NostrIdentityAlreadyExistsError,
  NostrIdentityCustodyBlockedError,
  NostrIdentityNotFoundError,
  createNostrIdentity,
  deriveNip06Identity,
  loadOrCreateNostrIdentity,
  openNostrIdentity,
} from "./nostr-identity.js"

// The canonical published BIP-39 TEST phrase. It is NOT a real secret.
const PUBLIC_TEST_MNEMONIC =
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"

let dir = ""
let identityFile = ""

const paths = () => ({
  home: dir,
  config: path.join(dir, "config.json"),
  cache: path.join(dir, "cache"),
  releases: path.join(dir, "releases"),
})

// Point the resolver straight at `identityFile` via the direct override, so the
// selected path is deterministic and no config.json is read.
const env = (): NodeJS.ProcessEnv => ({ OPENAGENTS_IDENTITY_MNEMONIC_PATH: identityFile })

beforeEach(() => {
  dir = mkdtempSync(path.join(tmpdir(), "idr02-loader-"))
  identityFile = path.join(dir, "identity.mnemonic")
})

afterEach(() => {
  rmSync(dir, { recursive: true, force: true })
})

describe("fail-closed openNostrIdentity", () => {
  test("a missing candidate STOPS with NoCandidateFound and creates NO file", async () => {
    await expect(openNostrIdentity(paths(), env())).rejects.toBeInstanceOf(NostrIdentityNotFoundError)
    // The whole point: an open never mints a mnemonic on a missing file.
    expect(existsSync(identityFile)).toBe(false)
  })

  test("an existing 0600 file opens and derives the expected NIP-06 identity", async () => {
    writeFileSync(identityFile, `${PUBLIC_TEST_MNEMONIC}\n`, { mode: 0o600 })
    chmodSync(identityFile, 0o600)
    const identity = await openNostrIdentity(paths(), env())
    expect(identity.npub).toBe(deriveNip06Identity(PUBLIC_TEST_MNEMONIC, identityFile).npub)
    expect(identity.npub.startsWith("npub1")).toBe(true)
  })

  test("a symbolic-link candidate is refused by default", async () => {
    const target = path.join(dir, "real.mnemonic")
    writeFileSync(target, `${PUBLIC_TEST_MNEMONIC}\n`, { mode: 0o600 })
    chmodSync(target, 0o600)
    symlinkSync(target, identityFile)
    const error = await openNostrIdentity(paths(), env()).catch((cause: unknown) => cause)
    expect(error).toBeInstanceOf(NostrIdentityCustodyBlockedError)
    expect((error as NostrIdentityCustodyBlockedError).blocker).toBe("symbolic_link_refused")
  })

  test("weak file permissions produce a typed custody blocker", async () => {
    writeFileSync(identityFile, `${PUBLIC_TEST_MNEMONIC}\n`, { mode: 0o644 })
    chmodSync(identityFile, 0o644)
    const error = await openNostrIdentity(paths(), env()).catch((cause: unknown) => cause)
    expect(error).toBeInstanceOf(NostrIdentityCustodyBlockedError)
    expect((error as NostrIdentityCustodyBlockedError).blocker).toBe("weak_permissions")
  })
})

describe("explicit createNostrIdentity", () => {
  test("creates a new 0600 mnemonic when the path is absent", async () => {
    const identity = await createNostrIdentity(paths(), env())
    expect(existsSync(identityFile)).toBe(true)
    if (process.platform !== "win32") {
      const { statSync } = await import("node:fs")
      expect(statSync(identityFile).mode & 0o777).toBe(0o600)
    }
    expect(identity.npub.startsWith("npub1")).toBe(true)
  })

  test("refuses to overwrite an existing candidate", async () => {
    writeFileSync(identityFile, `${PUBLIC_TEST_MNEMONIC}\n`, { mode: 0o600 })
    chmodSync(identityFile, 0o600)
    const before = readFileSync(identityFile, "utf8")
    await expect(createNostrIdentity(paths(), env())).rejects.toBeInstanceOf(
      NostrIdentityAlreadyExistsError,
    )
    // The existing mnemonic file is untouched.
    expect(readFileSync(identityFile, "utf8")).toBe(before)
  })
})

describe("intentional rehydrate-or-create loadOrCreateNostrIdentity", () => {
  test("creates on the first call and rehydrates the SAME identity on the next", async () => {
    const created = await loadOrCreateNostrIdentity(paths(), env())
    expect(existsSync(identityFile)).toBe(true)
    const rehydrated = await loadOrCreateNostrIdentity(paths(), env())
    expect(rehydrated.npub).toBe(created.npub)
    // Rehydration opened the existing file; it never overwrote the mnemonic.
    expect(rehydrated.mnemonic).toBe(created.mnemonic)
  })

  test("a symbolic-link candidate fails closed and is NOT created over", async () => {
    const target = path.join(dir, "real.mnemonic")
    writeFileSync(target, `${PUBLIC_TEST_MNEMONIC}\n`, { mode: 0o600 })
    chmodSync(target, 0o600)
    symlinkSync(target, identityFile)
    await expect(loadOrCreateNostrIdentity(paths(), env())).rejects.toBeInstanceOf(
      NostrIdentityCustodyBlockedError,
    )
    // The link target keeps its content; recovery never clobbered it.
    expect(readFileSync(target, "utf8")).toBe(`${PUBLIC_TEST_MNEMONIC}\n`)
  })
})
