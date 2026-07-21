import { describe, expect, test } from "vite-plus/test"

import { decodeAppleFmRouteOutput } from "@openagentsinc/apple-fm-runtime"

import {
  APPLE_FM_PROMPT_MAX_CHARS,
  buildOpenAgentsAppleFmPrompt,
  renderAppleFmEnvironmentContext,
  type AppleFmAvailableAgent,
  type AppleFmEnvironmentContext,
} from "./apple-fm-prompt.ts"
import { buildAppleFmEnvironmentContext } from "./apple-fm-environment.ts"
import { decideDelegation } from "./desktop-delegation.ts"

/**
 * AFS-03 (#9081): these are the honesty + history-window behavior contracts for
 * the Apple FM prompt, moved out of the renderer and behind the shared turn
 * kernel's host-owned Apple FM provider. The renderer no longer builds this
 * prompt; the guarantees are unchanged, now asserted on the host module.
 */
describe("buildOpenAgentsAppleFmPrompt (host-owned)", () => {
  test("the frozen renderer-prepared bound is preserved on the host module", () => {
    expect(APPLE_FM_PROMPT_MAX_CHARS).toBe(3900)
  })

  test("keeps the newest turns within the window, always the last message, and cues the assistant", () => {
    const turns = Array.from({ length: 40 }, (_, i) => ({
      role: (i % 2 === 0 ? "user" : "assistant") as "user" | "assistant",
      text: `line ${i} ${"x".repeat(200)}`,
    }))
    const prompt = buildOpenAgentsAppleFmPrompt(turns, [], undefined, 1000)
    expect(prompt.length).toBeLessThanOrEqual(1000)
    expect(prompt).toContain("line 39")
    expect(prompt).not.toContain("line 0 ")
    expect(prompt.endsWith("Assistant:")).toBe(true)
  })

  test("flattens history into User/Assistant lines including the newest turn", () => {
    const prompt = buildOpenAgentsAppleFmPrompt([
      { role: "user", text: "Hi" },
      { role: "assistant", text: "Hello there." },
      { role: "user", text: "Are you there?" },
    ])
    expect(prompt).toContain("Assistant: Hello there.")
    expect(prompt).toContain("User: Are you there?")
  })

  test("stays helpful while forbidding claimed actions or invented facts (owner directive 2026-07-20)", () => {
    const prompt = buildOpenAgentsAppleFmPrompt([
      { role: "user", text: "dispatch a subagent to set a reminder" },
    ])
    // Honesty limit is preserved: no tools, never claim to have acted, no made-up facts.
    expect(prompt).toContain("no tools")
    expect(prompt).toContain("you cannot run commands")
    expect(prompt).toContain("never claim you did, are doing, or will do any such action")
    expect(prompt).toContain("Do not make up facts")
    // Positive-first framing so the small model does not fall into a refusal
    // spiral on benign questions ("what can you do").
    expect(prompt).toContain("helpful, friendly assistant")
    expect(prompt).toContain("always try to be helpful and give a real answer")
    expect(prompt).not.toContain("CANNOT take any action")
  })
})

/**
 * AFS ambient context: the host now seeds the prompt with the environment facts
 * it already holds (working directory, OS, date, PUBLIC identity npub) so the
 * local assistant answers "what do you know about me" truthfully instead of
 * "I don't have any information about you" — while staying honest that it has no
 * durable per-user memory and only PUBLIC identity ever reaches the prompt.
 */
describe("buildOpenAgentsAppleFmPrompt — ambient environment context", () => {
  const fixtureEnvironment: AppleFmEnvironmentContext = {
    nowIso: "2026-07-20T00:00:00.000Z",
    humanDate: "Monday, July 20, 2026",
    platform: "macOS",
    appName: "OpenAgents Dev",
    workingDirectory: "/Users/owner/work/openagents",
    identityNpub: "npub1exampleownerpublickey00000000000000000000000000000000000",
    isOwnerDevice: true,
  }

  test("REGRESSION GUARD: empty environment + no agents equals the plain honesty base", () => {
    const turns = [{ role: "user" as const, text: "hi" }]
    const withNothing = buildOpenAgentsAppleFmPrompt(turns, [])
    const withEmptyEnv = buildOpenAgentsAppleFmPrompt(turns, [], {})
    const withUndefinedEnv = buildOpenAgentsAppleFmPrompt(turns, [], undefined)
    // An entirely empty (or absent) context adds NOTHING — byte-for-byte the
    // pre-existing plain preamble, so no live behavior regresses.
    expect(withEmptyEnv).toBe(withNothing)
    expect(withUndefinedEnv).toBe(withNothing)
    expect(withNothing).not.toContain("Here is the context you have about the user")
  })

  test("includes each present fact as a stated-truth context block (deterministic)", () => {
    const prompt = buildOpenAgentsAppleFmPrompt(
      [{ role: "user", text: "what do you know about me" }],
      [],
      fixtureEnvironment,
    )
    expect(prompt).toContain("Here is the context you have about the user and this session")
    expect(prompt).toContain("Current date: Monday, July 20, 2026")
    expect(prompt).toContain("Operating system: macOS")
    expect(prompt).toContain("Application: OpenAgents Dev")
    expect(prompt).toContain("Working directory: /Users/owner/work/openagents")
    expect(prompt).toContain("This is the owner's own device.")
    expect(prompt).toContain(
      "The user's public identity (npub): npub1exampleownerpublickey00000000000000000000000000000000000",
    )
    // Active framing (proven against the live model): the model must ANSWER from
    // the context and never claim it has no information; honesty about no durable
    // cross-session memory and no invented facts is retained.
    expect(prompt).toContain("never reply that you have no information or cannot access it")
    expect(prompt).toContain("never invent facts that are not listed above")
    expect(prompt).toContain("You do not remember personal facts across past sessions")
    // Fully deterministic (no wall-clock): the same inputs render the same prompt.
    expect(buildOpenAgentsAppleFmPrompt(
      [{ role: "user", text: "what do you know about me" }],
      [],
      fixtureEnvironment,
    )).toBe(prompt)
  })

  test("fail-soft: a missing fact simply omits its line", () => {
    const block = renderAppleFmEnvironmentContext({ platform: "macOS", workingDirectory: "/w" })
    expect(block).toContain("Operating system: macOS")
    expect(block).toContain("Working directory: /w")
    expect(block).not.toContain("Current date:")
    expect(block).not.toContain("public npub")
    expect(block).not.toContain("owner's own device")
    // A context with no usable fact renders nothing at all.
    expect(renderAppleFmEnvironmentContext({})).toBe("")
    expect(renderAppleFmEnvironmentContext({ appName: "   " })).toBe("")
    expect(renderAppleFmEnvironmentContext(undefined)).toBe("")
  })

  test("TRIPWIRE: only a public npub1 identity is ever printed — never nsec/mnemonic/seed", () => {
    const npub = "npub1exampleownerpublickey00000000000000000000000000000000000"
    const good = renderAppleFmEnvironmentContext({ identityNpub: npub })
    expect(good).toContain(npub)

    // Anything that is not a well-formed public npub is refused outright, so a
    // mis-wired secret can never leak into the prompt.
    const secretShapes = [
      "nsec1qqqqqqowner00000000000000000000000000000000000000000000000",
      "abandon abandon abandon abandon abandon abandon abandon abandon about",
      "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcd",
      "npub", // truncated / not bech32 body
    ]
    for (const secret of secretShapes) {
      const block = renderAppleFmEnvironmentContext({ identityNpub: secret })
      expect(block).not.toContain(secret)
      expect(block).not.toContain("public npub")
    }
    // Belt-and-suspenders on the full prompt: no private-key marker ever appears.
    const prompt = buildOpenAgentsAppleFmPrompt(
      [{ role: "user", text: "who am i" }],
      [],
      { ...fixtureEnvironment, identityNpub: npub },
    )
    expect(prompt).toContain(npub)
    expect(prompt).not.toContain("nsec1")
    expect(prompt.toLowerCase()).not.toContain("mnemonic")
    expect(prompt.toLowerCase()).not.toContain("seed phrase")
  })

  test("buildAppleFmEnvironmentContext maps host inputs via an INJECTED clock", () => {
    const context = buildAppleFmEnvironmentContext({
      now: new Date("2026-07-20T12:34:56.000Z"),
      platform: "darwin",
      appName: "OpenAgents Dev",
      workingDirectory: "/Users/owner/work/openagents",
      identityNpub: "npub1exampleownerpublickey00000000000000000000000000000000000",
      isOwnerDevice: true,
    })
    // darwin → macOS, and the date is derived purely from the injected clock.
    expect(context.platform).toBe("macOS")
    expect(context.humanDate).toBe("Monday, July 20, 2026")
    expect(context.nowIso).toBe("2026-07-20T12:34:56.000Z")
    expect(context.workingDirectory).toBe("/Users/owner/work/openagents")
    expect(context.isOwnerDevice).toBe(true)
    // A blank/absent host fact is dropped, never rendered as an empty line.
    const sparse = buildAppleFmEnvironmentContext({
      now: new Date("2026-01-01T00:00:00.000Z"),
      platform: "linux",
      appName: "   ",
      workingDirectory: null,
      identityNpub: null,
    })
    expect(sparse.platform).toBe("Linux")
    expect(sparse.appName).toBeUndefined()
    expect(sparse.workingDirectory).toBeUndefined()
    expect(sparse.identityNpub).toBeUndefined()
    expect(sparse.isOwnerDevice).toBeUndefined()
  })
})

/**
 * Follow-up to AFS-04: the host prompt must make the on-device model
 * agent-aware (so "who are you / what agents do you have" is answered honestly)
 * and elicit the exact route-recommendation JSON the AFS-02 decoder + AFS-04
 * router already accept (so "task X to codex" actually delegates instead of the
 * model refusing "I'm just an AI, I can't code").
 */
describe("buildOpenAgentsAppleFmPrompt — router mode + guided-route decoding", () => {
  const codex: AppleFmAvailableAgent = { candidate: "codex", label: "Codex", ready: true, canDelegate: true }
  const claude: AppleFmAvailableAgent = { candidate: "claude", label: "Claude Code", ready: true, canDelegate: true }
  const grok: AppleFmAvailableAgent = { candidate: "grok_acp", label: "Grok", ready: true, canDelegate: true }
  const grokNotReady: AppleFmAvailableAgent = { ...grok, ready: false }

  /** The exact well-formed route shape the bridge's guided generation returns. */
  const routeJson = (candidate: string): string =>
    JSON.stringify({ candidate, taskClass: "delegate", reasonCode: "needs_delegation", confidence: 1 })

  /** Any literal route JSON in the prompt (there must be none — guided gen owns the shape). */
  const templateInPrompt = (prompt: string): string | null => {
    const match = prompt.match(/\{"candidate":.*?\}/u)
    return match === null ? null : match[0]
  }

  test("router preamble names each connected delegate by candidate + role, with the policy", () => {
    const prompt = buildOpenAgentsAppleFmPrompt(
      [{ role: "user", text: "fix the login bug" }],
      [codex, claude, grok],
    )
    expect(prompt).toContain("local router")
    expect(prompt).toContain("You do NOT answer the user yourself")
    expect(prompt).toContain("codex (Codex)")
    expect(prompt).toContain("claude (Claude Code)")
    expect(prompt).toContain("grok_acp (Grok)")
    // The routing policy that guides the choice (owner directive 2026-07-20).
    expect(prompt).toContain("use codex for coding tasks")
    expect(prompt).toContain("use grok_acp for simple mechanical string/rename tasks")
    expect(prompt).toContain("use claude for planning, strategy, analysis")
    // It routes — it is not the chat assistant, so the direct-answer base is gone.
    expect(prompt).not.toContain("helpful, friendly assistant")
  })

  test("router preamble names no not-ready delegate", () => {
    const prompt = buildOpenAgentsAppleFmPrompt([{ role: "user", text: "x" }], [codex, grokNotReady])
    expect(prompt).toContain("codex (Codex)")
    expect(prompt).not.toContain("grok_acp")
  })

  test("guided generation owns the shape: the router prompt carries NO JSON template", () => {
    const prompt = buildOpenAgentsAppleFmPrompt([{ role: "user", text: "x" }], [codex, claude, grok])
    expect(templateInPrompt(prompt)).toBeNull()
  })

  test("no connected delegate → OpenAgents answers directly (not a router)", () => {
    const prompt = buildOpenAgentsAppleFmPrompt(
      [{ role: "user", text: "hi" }],
      [{ ...claude, canDelegate: false }, { ...grok, ready: false }],
    )
    expect(prompt).toContain("helpful, friendly assistant")
    expect(prompt).not.toContain("local router")
  })

  test("decode + decide: a codex route JSON dispatches codex", () => {
    const raw = routeJson("codex")
    expect(decodeAppleFmRouteOutput({ raw, admittedCandidates: ["codex"] })._tag).toBe("Recommendation")
    const decision = decideDelegation({
      answerText: raw,
      objective: "fix the login bug",
      readiness: { codex: { ready: true, accountRef: "acct.a" } },
    })
    expect(decision.kind).toBe("delegate")
    if (decision.kind === "delegate") {
      expect(decision.provider).toBe("codex")
      expect(decision.recommendation.candidate).toBe("codex")
    }
  })

  test("decode + decide: a claude route JSON dispatches claude (#9091)", () => {
    const decision = decideDelegation({
      answerText: routeJson("claude"),
      objective: "plan the architecture",
      readiness: { claude: { ready: true, accountRef: "acct.a" } },
    })
    expect(decision.kind).toBe("delegate")
    if (decision.kind === "delegate") expect(decision.provider).toBe("claude")
  })

  test("a plain-words answer never dispatches (fail-closed to a local answer)", () => {
    const decision = decideDelegation({
      answerText: "I'm OpenAgents, running on Apple's on-device model.",
      objective: "who are you",
      readiness: { codex: { ready: true, accountRef: "acct.a" } },
    })
    expect(decision.kind).toBe("answer")
  })

  test("malformed / action-claim / unavailable route never dispatches (fail-closed)", () => {
    const malformed = decideDelegation({
      answerText: '{"candidate":"codex","confidence":"high"}',
      objective: "x",
      readiness: { codex: { ready: true, accountRef: "acct.a" } },
    })
    expect(malformed.kind).not.toBe("delegate")

    const actionClaim = decideDelegation({
      answerText: '{"candidate":"codex","command":"rm -rf /","confidence":0.9}',
      objective: "x",
      readiness: { codex: { ready: true, accountRef: "acct.a" } },
    })
    expect(actionClaim.kind).not.toBe("delegate")

    // A well-formed route for a lane the host reports unavailable refuses, no start.
    const unavailable = decideDelegation({
      answerText: routeJson("codex"),
      objective: "x",
      readiness: { codex: { ready: false, unavailableReason: "no_codex_account" } },
    })
    expect(unavailable.kind).toBe("refuse_delegation")
  })
})
