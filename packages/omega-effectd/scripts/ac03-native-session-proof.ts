/**
 * Runs OMEGA-AC-03 with the signed-in OpenAgents Desktop session on macOS.
 * The Keychain password and decrypted session remain process-local and are
 * never printed, persisted, or passed as command-line arguments.
 */
import { createDecipheriv, pbkdf2Sync } from "node:crypto"
import { execFileSync } from "node:child_process"
import { readFileSync } from "node:fs"
import { homedir } from "node:os"
import path from "node:path"

import { runAc03LiveProof } from "./ac03-live-proof.ts"

type NativeSessionRecord = Readonly<{
  ownerUserId: string
  accessToken: string
  refreshToken: string
}>

const decryptMacSafeStorage = (payload: Buffer, password: string): string => {
  if (payload.subarray(0, 3).toString("utf8") !== "v10") {
    throw new Error("native session uses an unsupported safe-storage envelope")
  }
  const key = pbkdf2Sync(password, "saltysalt", 1003, 16, "sha1")
  const decipher = createDecipheriv("aes-128-cbc", key, Buffer.alloc(16, " "))
  return Buffer.concat([decipher.update(payload.subarray(3)), decipher.final()]).toString("utf8")
}

const loadNativeSession = (): NativeSessionRecord => {
  if (process.platform !== "darwin") {
    throw new Error("native OpenAgents session proof currently requires macOS")
  }
  const filePath =
    process.env.OPENAGENTS_DESKTOP_SESSION_FILE?.trim() ||
    path.join(
      homedir(),
      "Library",
      "Application Support",
      process.env.OPENAGENTS_DESKTOP_SESSION_APP_NAME?.trim() || "OpenAgents Dev",
      "session",
      "native-session.enc",
    )
  const service =
    process.env.OPENAGENTS_DESKTOP_KEYCHAIN_SERVICE?.trim() ||
    "OpenAgents Dev Safe Storage"
  const password = execFileSync(
    "/usr/bin/security",
    ["find-generic-password", "-s", service, "-w"],
    { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] },
  ).trim()
  const envelope = JSON.parse(readFileSync(filePath, "utf8")) as {
    payload?: unknown
  }
  if (typeof envelope.payload !== "string" || password.length === 0) {
    throw new Error("native OpenAgents session is unavailable")
  }
  const record = JSON.parse(
    decryptMacSafeStorage(Buffer.from(envelope.payload, "base64"), password),
  ) as Partial<NativeSessionRecord>
  if (
    typeof record.ownerUserId !== "string" ||
    typeof record.accessToken !== "string" ||
    typeof record.refreshToken !== "string" ||
    record.ownerUserId.trim() === "" ||
    record.accessToken.trim() === "" ||
    record.refreshToken.trim() === ""
  ) {
    throw new Error("native OpenAgents session is incomplete")
  }
  return {
    ownerUserId: record.ownerUserId.trim(),
    accessToken: record.accessToken.trim(),
    refreshToken: record.refreshToken.trim(),
  }
}

const verifiedAccessToken = async (credential: NativeSessionRecord): Promise<string> => {
  const response = await fetch("https://openagents.com/api/mobile/auth/session", {
    headers: {
      authorization: `Bearer ${credential.accessToken}`,
      "x-openagents-refresh-token": credential.refreshToken,
    },
    signal: AbortSignal.timeout(15_000),
  })
  if (!response.ok) {
    throw new Error(`native OpenAgents session verification failed (${response.status})`)
  }
  const body = (await response.json()) as {
    authenticated?: unknown
    user?: { userId?: unknown }
    tokens?: { access?: unknown }
  }
  if (
    body.authenticated !== true ||
    body.user?.userId !== credential.ownerUserId
  ) {
    throw new Error("native OpenAgents session owner verification failed")
  }
  return typeof body.tokens?.access === "string" && body.tokens.access.trim() !== ""
    ? body.tokens.access.trim()
    : credential.accessToken
}

const main = async (): Promise<number> => {
  console.error(JSON.stringify({ stage: "native_session_vault" }))
  const credential = loadNativeSession()
  const accessToken = await verifiedAccessToken(credential)
  console.error(JSON.stringify({ stage: "omega_effectd_live_turn" }))
  return runAc03LiveProof(accessToken)
}

const exitCode = await main().catch(error => {
  console.error(
    JSON.stringify({
      ok: false,
      error: error instanceof Error ? error.message : "native-session proof failed",
    }),
  )
  return 1
})
process.exit(exitCode)
