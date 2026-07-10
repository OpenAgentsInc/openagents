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
  const surfaceSource = readFileSync(
    join(APP_ROOT, "src", "ui", "main.ts"),
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
    expect(source).toMatch(
      /const teardownLocalMedia = \([\s\S]{0,500}recognitionActive = false[\s\S]{0,100}recognition\?\.stop\(\)/,
    )
    expect(source).toMatch(
      /const stop = async \(\) => \{[\s\S]{0,200}stopLocalMedia\(\{ status: "ended" \}\)/,
    )
  })

  test("local teardown never substitutes for authoritative server-slot release", () => {
    expect(source).toContain("const makeEnsureServerStop")
    expect(source).toMatch(
      /const stop = async \(\) => \{[\s\S]{0,250}await (?:Promise\.all\(\[ensureSdkStop\(\), )?ensureServerStop\(\)/,
    )
    expect(source).toContain('void ensureServerStop("beacon")')
  })

  test("post-handle cleanup observations drive the shared replacement gate", () => {
    expect(source).toContain('callbacks.onCleanup("pending")')
    expect(source).toContain('callbacks.onCleanup("confirmed")')
    expect(source).toContain('callbacks.onCleanup("unconfirmed")')
    expect(surfaceSource).toContain("onCleanup: (cleanup) =>")
    expect(surfaceSource).toContain(
      "applyAvatarCleanupObservation(runtime.avatarGate, cleanup)",
    )
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

/**
 * Source oracles for sarah.avatar_opens_with_shippable_opener_clip.v1
 * (epic #8610): a refactor cannot silently drop the clip tier, its crossfade,
 * its barge-in, or its honest degradation back to the TTS greeting.
 */
describe("owned surface wires the pre-rendered clip tier (epic #8610)", () => {
  const source = readFileSync(
    join(APP_ROOT, "src", "ui", "avatar-session.ts"),
    "utf8",
  )

  test("mint requests the client clip greeting so the server suppresses TTS", () => {
    expect(source).toContain('greeting: "client_clip"')
    expect(source).toContain("openerClip")
  })

  test("the opener plays through the clip layer and crossfades on live media", () => {
    expect(source).toContain("makeAvatarClipLayer")
    expect(source).toContain(
      'clipLayer.play({ url: mint.openerClip.url, kind: "opener" })',
    )
    expect(source).toContain("clipLayer.notifyLiveMedia()")
  })

  test("clip failure restores the server TTS greeting (never dead air)", () => {
    expect(source).toContain("/avatar/greet")
    expect(source).toContain("requestServerGreeting")
    const server = readFileSync(join(APP_ROOT, "src", "server.ts"), "utf8")
    expect(server).toContain('"/api/avatar/greet"')
    // No-clip mints still fall through to the spoken TTS greeting.
    expect(server).toMatch(/if \(openerClip\) \{[\s\S]{0,400}speakOwnedGreeting\(owned\.sessionId\)/)
  })

  test("canned SSE clip events play over the live stream; barge-in drops the clip", () => {
    expect(source).toContain('event.type === "clip"')
    expect(source).toContain('kind: "canned"')
    expect(source).toContain("clipLayer.interrupt()")
  })

  test("owned session teardown destroys the clip layer", () => {
    expect(source).toMatch(
      /const teardownLocalMedia = \(observation: MediaObservation\) => \{\s*clipLayer\.destroy\(\)/,
    )
  })

  test("the license law is encoded in the catalog module", () => {
    const catalog = readFileSync(
      join(APP_ROOT, "src", "services", "opener-clips.ts"),
      "utf8",
    )
    expect(catalog).toContain("hallo2_512_mit")
    expect(catalog).toContain("NON-commercial")
    // Statically: no SR filename can appear in the shippable catalog source.
    expect(catalog.includes("-sr.mp4\"")).toBe(false)
  })
})
