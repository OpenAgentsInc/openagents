/**
 * Oracles for the avatar UX behavior contracts (SQ-4 #8621).
 *
 * Coverage rule: every enforced contract in avatar-ux-contracts.ts must name
 * at least one oracle whose ref exists; the mic-wiring oracle lives here as a
 * source-level assertion on the owned surface so a refactor cannot silently
 * drop speech input, the greeting, or the typed fallbacks.
 */

import { describe, expect, test } from "bun:test"
import { existsSync, readFileSync } from "node:fs"
import { join } from "node:path"

import { sarahAvatarUxContractRegistry } from "./avatar-ux-contracts.ts"

const APP_ROOT = join(import.meta.dir, "..", "..")
const REPO_ROOT = join(APP_ROOT, "..", "..")

describe("avatar UX contract registry coverage", () => {
  test("every enforced contract has oracles with existing refs", () => {
    for (const contract of sarahAvatarUxContractRegistry.contracts) {
      expect(contract.state).toBe("enforced")
      expect(contract.oracles.length).toBeGreaterThan(0)
      expect(contract.statement.length).toBeGreaterThan(10)
      for (const oracle of contract.oracles) {
        const path = join(REPO_ROOT, oracle.ref)
        expect(existsSync(path)).toBe(true)
      }
    }
  })
})

describe("owned surface wires speech recognition with a typed fallback", () => {
  const source = readFileSync(
    join(APP_ROOT, "src", "ui", "avatar-session.ts"),
    "utf8",
  )

  test("constructs browser SpeechRecognition on the owned path", () => {
    expect(source).toContain("webkitSpeechRecognition")
    expect(source).toContain("recognition.continuous = true")
  })

  test("final utterances reach the speak bridge, serialized", () => {
    expect(source).toContain("/avatar/speak")
    expect(source).toContain("speakInFlight = speakInFlight.then")
    expect(source).toContain("result.isFinal")
  })

  test("recognition restarts when the browser ends it mid-session", () => {
    expect(source).toMatch(/onend[\s\S]{0,200}recognition\?\.start\(\)/)
  })

  test("mic denial and unsupported browsers surface typed fallback cards", () => {
    expect(source).toContain("not-allowed")
    expect(source).toContain("Microphone unavailable")
    expect(source).toContain("Voice input not supported here")
  })

  test("session teardown stops recognition", () => {
    expect(source).toMatch(/stop = async \(\) => \{[\s\S]{0,200}recognition\?\.stop\(\)/)
  })
})

describe("owned mint speaks the greeting", () => {
  test("greeting constant and mint-route wiring exist", () => {
    const ownedRenderer = readFileSync(
      join(APP_ROOT, "src", "services", "owned-renderer.ts"),
      "utf8",
    )
    expect(ownedRenderer).toContain("SARAH_OWNED_GREETING")
    expect(ownedRenderer).toContain("export async function speakOwnedGreeting")
    const server = readFileSync(join(APP_ROOT, "src", "server.ts"), "utf8")
    expect(server).toContain("speakOwnedGreeting(owned.sessionId)")
    expect(server).toContain("SARAH_AVATAR_GREETING_DELAY_MS")
  })
})
