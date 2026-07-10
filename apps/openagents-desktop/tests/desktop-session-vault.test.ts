import { describe, expect, test } from "bun:test"
import {
  chmodSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  DesktopSessionVaultError,
  openDesktopSessionVault,
  type SafeStorageLike,
} from "../src/desktop-session-vault.ts"

const root = (): string =>
  mkdtempSync(path.join(tmpdir(), "openagents-desktop-session-"))

const safeStorage = (backend = "keychain"): SafeStorageLike => ({
  isEncryptionAvailable: () => true,
  getSelectedStorageBackend: () => backend,
  encryptString: plaintext => Buffer.from(`encrypted:${plaintext}`, "utf8"),
  decryptString: encrypted => {
    const value = encrypted.toString("utf8")
    if (!value.startsWith("encrypted:")) throw new Error("undecryptable")
    return value.slice("encrypted:".length)
  },
})

const credential = {
  ownerUserId: "owner.fixture",
  accessToken: "access-fixture",
  refreshToken: "refresh-fixture",
}

describe("contract openagents_desktop.session.os_encrypted_custody.v1", () => {
  test("round-trips one encrypted private record without plaintext credential fields", () => {
    const directory = root()
    const filePath = path.join(directory, "session", "native-session.enc")
    try {
      const vault = openDesktopSessionVault({ filePath, safeStorage: safeStorage() })
      vault.save(credential)
      expect(vault.load()).toEqual(credential)
      expect(vault.recover()).toEqual({ state: "credential_present_unverified" })

      const disk = readFileSync(filePath, "utf8")
      expect(disk).toContain("electron-safe-storage")
      expect(disk).not.toContain("owner.fixture")
      expect(disk).not.toContain("access-fixture")
      expect(disk).not.toContain("refresh-fixture")
      if (process.platform !== "win32") {
        expect(statSync(path.dirname(filePath)).mode & 0o777).toBe(0o700)
        expect(statSync(filePath).mode & 0o777).toBe(0o600)
      }
    } finally {
      rmSync(directory, { recursive: true, force: true })
    }
  })

  test("refuses unavailable encryption and Linux basic_text custody", () => {
    const directory = root()
    try {
      const filePath = path.join(directory, "native-session.enc")
      expect(() => openDesktopSessionVault({
        filePath,
        safeStorage: {
          ...safeStorage(),
          isEncryptionAvailable: () => false,
        },
      })).toThrow(DesktopSessionVaultError)
      expect(() => openDesktopSessionVault({
        filePath,
        safeStorage: safeStorage("basic_text"),
      })).toThrow("OS-encrypted desktop session custody is unavailable")
      expect(existsSync(filePath)).toBe(false)
    } finally {
      rmSync(directory, { recursive: true, force: true })
    }
  })

  test("purges malformed, undecryptable, incomplete, and retired records", () => {
    const directory = root()
    const filePath = path.join(directory, "native-session.enc")
    try {
      const storage = safeStorage()
      const vault = openDesktopSessionVault({ filePath, safeStorage: storage })
      const malformed = [
        "not-json",
        JSON.stringify({ schemaVersion: 1, encryption: "electron-safe-storage", payload: "YmFk" }),
        JSON.stringify({
          schemaVersion: 1,
          encryption: "electron-safe-storage",
          payload: storage.encryptString(JSON.stringify({
            schemaVersion: 1,
            credentialEpoch: "retired",
            ...credential,
          })).toString("base64"),
        }),
        JSON.stringify({
          schemaVersion: 1,
          encryption: "electron-safe-storage",
          payload: storage.encryptString(JSON.stringify({
            schemaVersion: 1,
            credentialEpoch: "2026-07-10-openauth-native-session-v1",
            ownerUserId: "owner.fixture",
            accessToken: "",
            refreshToken: "refresh-fixture",
          })).toString("base64"),
        }),
      ]
      for (const value of malformed) {
        writeFileSync(filePath, value)
        expect(vault.load()).toBeNull()
        expect(existsSync(filePath)).toBe(false)
      }
    } finally {
      rmSync(directory, { recursive: true, force: true })
    }
  })

  test("clears idempotently and returns only bounded recovery state", () => {
    const directory = root()
    const filePath = path.join(directory, "native-session.enc")
    try {
      const vault = openDesktopSessionVault({ filePath, safeStorage: safeStorage() })
      expect(vault.recover()).toEqual({ state: "signed_out" })
      vault.save(credential)
      const recovery = vault.recover()
      expect(JSON.stringify(recovery)).not.toContain("fixture")
      vault.clear()
      vault.clear()
      expect(vault.recover()).toEqual({ state: "signed_out" })
    } finally {
      rmSync(directory, { recursive: true, force: true })
    }
  })

  test("rejects incomplete writes and wraps storage failures without secrets", () => {
    const directory = root()
    try {
      const vault = openDesktopSessionVault({
        filePath: path.join(directory, "native-session.enc"),
        safeStorage: safeStorage(),
      })
      expect(() => vault.save({ ...credential, accessToken: "" })).toThrow(
        "desktop session credential is incomplete",
      )

      chmodSync(directory, 0o500)
      const failing = openDesktopSessionVault({
        filePath: path.join(directory, "blocked", "native-session.enc"),
        safeStorage: {
          ...safeStorage(),
          encryptString: () => { throw new Error("failed with access-fixture") },
        },
      })
      let error: unknown
      try { failing.save(credential) } catch (caught) { error = caught }
      expect(error).toBeInstanceOf(DesktopSessionVaultError)
      expect(JSON.stringify(error)).not.toContain("access-fixture")
      expect((error as DesktopSessionVaultError).reason).toBe("storage_unavailable")
    } finally {
      chmodSync(directory, 0o700)
      rmSync(directory, { recursive: true, force: true })
    }
  })
})
