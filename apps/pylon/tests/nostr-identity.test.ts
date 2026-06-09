import { chmod, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { describe, expect, test } from "bun:test"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import {
  NIP06_DERIVATION_PATH,
  createNip98Event,
  deriveNip06Identity,
  encodeNip98Authorization,
  loadOrCreateNostrIdentity,
  resolveNostrIdentityPath,
  verifyNip98Authorization,
} from "../src/nostr-identity"
import { resolveStatePaths } from "../src/state"

const vectorMnemonic = "leader monkey parrot ring guide accident before fence cannon height naive bean"

async function withTempHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-nostr-identity-test-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

describe("Pylon NIP-06 identity", () => {
  test("derives the deprecated Rust Pylon NIP-06 account-zero vector", () => {
    const identity = deriveNip06Identity(vectorMnemonic, "/tmp/identity.mnemonic")

    expect(NIP06_DERIVATION_PATH).toBe("m/44'/1237'/0'/0/0")
    expect(identity.privateKeyHex).toBe("7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a")
    expect(identity.publicKey).toBe("17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917")
    expect(identity.nsec).toBe("nsec10allq0gjx7fddtzef0ax00mdps9t2kmtrldkyjfs8l5xruwvh2dq0lhhkp")
    expect(identity.npub).toBe("npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu")
  })

  test("reuses an existing compatibility mnemonic without overwriting it", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs([]), { PYLON_HOME: home }, "darwin")
      const paths = resolveStatePaths(summary.paths)
      await writeFile(paths.identityMnemonic, `${vectorMnemonic}\n`, { mode: 0o600 })

      const identity = await loadOrCreateNostrIdentity(paths, {})
      const persisted = await readFile(paths.identityMnemonic, "utf8")

      expect(identity.identityPath).toBe(paths.identityMnemonic)
      expect(identity.npub).toBe("npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu")
      expect(persisted).toBe(`${vectorMnemonic}\n`)
    })
  })

  test("creates a missing mnemonic at the selected compatibility path with private permissions", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs([]), { PYLON_HOME: home }, "linux")
      const paths = resolveStatePaths(summary.paths)

      const identity = await loadOrCreateNostrIdentity(paths, {})
      const persisted = (await readFile(paths.identityMnemonic, "utf8")).trim()

      expect(identity.identityPath).toBe(paths.identityMnemonic)
      expect(persisted.split(/\s+/).length).toBe(12)
      expect(identity.npub.startsWith("npub1")).toBe(true)
      if (process.platform !== "win32") {
        expect((await stat(paths.identityMnemonic)).mode & 0o077).toBe(0)
      }
    })
  })

  test("fails closed for invalid or overexposed mnemonic files", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs([]), { PYLON_HOME: home }, "darwin")
      const paths = resolveStatePaths(summary.paths)

      await writeFile(paths.identityMnemonic, "not a valid mnemonic\n", { mode: 0o600 })
      await expect(loadOrCreateNostrIdentity(paths, {})).rejects.toThrow("not valid BIP39")

      await writeFile(paths.identityMnemonic, `${vectorMnemonic}\n`, { mode: 0o644 })
      if (process.platform !== "win32") {
        await chmod(paths.identityMnemonic, 0o644)
        await expect(loadOrCreateNostrIdentity(paths, {})).rejects.toThrow("permissions")
      }
    })
  })

  test("uses compatibility environment override order", async () => {
    await withTempHome(async (home) => {
      const summary = createBootstrapSummary(parseBootstrapArgs([]), { PYLON_HOME: home }, "linux")
      const direct = join(home, "direct.mnemonic")
      const openagentsHome = join(home, "openagents-home")

      expect(
        resolveNostrIdentityPath(summary.paths, {
          OPENAGENTS_IDENTITY_MNEMONIC_PATH: direct,
          OPENAGENTS_PYLON_HOME: openagentsHome,
        }).path,
      ).toBe(direct)
      expect(resolveNostrIdentityPath(summary.paths, { OPENAGENTS_PYLON_HOME: openagentsHome }).path).toBe(
        join(openagentsHome, "identity.mnemonic"),
      )
    })
  })

  test("creates and verifies a strict NIP-98 event", () => {
    const identity = deriveNip06Identity(vectorMnemonic, "/tmp/identity.mnemonic")
    const event = createNip98Event({
      method: "POST",
      url: "https://openagents.com/api/pylons/pylon.test/heartbeat",
      body: "{\"pylonRef\":\"pylon.test\"}",
      identity,
      now: new Date("2026-06-09T00:00:00.000Z"),
    })
    const verified = verifyNip98Authorization(encodeNip98Authorization(event), {
      method: "POST",
      url: "https://openagents.com/api/pylons/pylon.test/heartbeat",
      body: "{\"pylonRef\":\"pylon.test\"}",
      now: new Date("2026-06-09T00:00:30.000Z"),
    })

    expect(verified.pubkey).toBe(identity.publicKey)
    expect(verified.kind).toBe(27235)
    expect(verified.tags.find((tag) => tag[0] === "method")?.[1]).toBe("POST")
  })
})
