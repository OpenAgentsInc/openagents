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
describe("buildOpenAgentsAppleFmPrompt — agent-awareness + delegation", () => {
  const codexReady: AppleFmAvailableAgent = {
    candidate: "codex",
    label: "Codex",
    ready: true,
    canDelegate: true,
  }
  const claudeReady: AppleFmAvailableAgent = {
    candidate: "claude",
    label: "Claude Code",
    ready: true,
    canDelegate: false,
  }
  const grokReady: AppleFmAvailableAgent = {
    candidate: "grok_acp",
    label: "Grok",
    ready: true,
    canDelegate: false,
  }
  const grokNotReady: AppleFmAvailableAgent = { ...grokReady, ready: false }

  /** Pull the delegation JSON template literally out of the assembled prompt. */
  const templateInPrompt = (prompt: string): string | null => {
    const match = prompt.match(/\{"candidate":.*?\}/u)
    return match === null ? null : match[0]
  }

  test("names the ready connected agents so 'what agents do you have' can be answered", () => {
    const prompt = buildOpenAgentsAppleFmPrompt(
      [{ role: "user", text: "who are you, what agents do you have available" }],
      [codexReady, claudeReady, grokReady],
    )
    expect(prompt).toContain("Connected agents on this device")
    expect(prompt).toContain("Codex")
    expect(prompt).toContain("Claude Code")
    expect(prompt).toContain("Grok")
    // The model still knows it itself runs on Apple FM (nameable when asked).
    expect(prompt).toContain("Apple FM")
    // Honest scoping: do not claim agents that were not advertised.
    expect(prompt).toContain("Only the agents listed above are connected")
  })

  test("does not name an agent the host reports NOT ready", () => {
    const prompt = buildOpenAgentsAppleFmPrompt(
      [{ role: "user", text: "what can you do" }],
      [codexReady, grokNotReady],
    )
    expect(prompt).toContain("Codex")
    // Grok is not ready → it must not be listed as connected.
    const listSection = prompt.slice(prompt.indexOf("Connected agents"), prompt.indexOf("If the user asks"))
    expect(listSection).not.toContain("Grok")
  })

  test("carries the delegation instruction + JSON example for a ready delegate agent", () => {
    const prompt = buildOpenAgentsAppleFmPrompt(
      [{ role: "user", text: "task some example thing to codex now" }],
      [codexReady, claudeReady],
    )
    // The refusal-triggering wording is gone; routing is framed as possible.
    expect(prompt).toContain("hand one off to a connected coding agent")
    expect(prompt).toContain("never say you can't code and never refuse this")
    expect(prompt).toContain('reply with ONLY')
    // The exact JSON template the decoder accepts is present, naming codex.
    const template = templateInPrompt(prompt)
    expect(template).not.toBeNull()
    expect(template).toContain('"candidate":"codex"')
    // Only the delegate-capable agent (codex) is offered as a hand-off target;
    // claude is named as connected but not templated (its lane is not wired).
    expect(prompt).toContain('"codex" for Codex')
    expect(prompt).not.toContain('"claude" for Claude Code')
  })

  test("offers Claude Code and Grok as hand-off targets when delegate-capable (#9091)", () => {
    const claudeDelegate: AppleFmAvailableAgent = { ...claudeReady, canDelegate: true }
    const grokDelegate: AppleFmAvailableAgent = { ...grokReady, canDelegate: true }
    const prompt = buildOpenAgentsAppleFmPrompt(
      [{ role: "user", text: "task the fixture to claude" }],
      [codexReady, claudeDelegate, grokDelegate],
    )
    // The delegation instruction is present and each ready delegate is mapped to
    // its exact route-recommendation candidate string.
    expect(prompt).toContain('reply with ONLY')
    expect(prompt).toContain('"codex" for Codex')
    expect(prompt).toContain('"claude" for Claude Code')
    expect(prompt).toContain('"grok_acp" for Grok')
  })

  test("ROUND TRIP: a claude template in the prompt decodes to a claude delegation (#9091)", () => {
    const claudeDelegate: AppleFmAvailableAgent = { ...claudeReady, canDelegate: true }
    // The exact per-agent template the prompt tells the model to emit for claude.
    const claudeTemplate = JSON.stringify({
      candidate: "claude",
      taskClass: "delegate",
      reasonCode: "explicit_provider_request",
      confidence: 0.9,
    })
    const prompt = buildOpenAgentsAppleFmPrompt(
      [{ role: "user", text: "task this to claude" }],
      [codexReady, claudeDelegate],
    )
    expect(prompt).toContain('"claude" for Claude Code')
    const decoded = decodeAppleFmRouteOutput({ raw: claudeTemplate, admittedCandidates: ["codex", "claude", "grok_acp"] })
    expect(decoded._tag).toBe("Recommendation")
    const decision = decideDelegation({
      answerText: claudeTemplate,
      objective: "task this to claude",
      readiness: { claude: { ready: true, accountRef: "acct.a" } },
    })
    expect(decision.kind).toBe("delegate")
    if (decision.kind === "delegate") expect(decision.provider).toBe("claude")
  })

  test("offers NO delegation JSON when no ready agent is delegate-capable", () => {
    const prompt = buildOpenAgentsAppleFmPrompt(
      [{ role: "user", text: "hi" }],
      [claudeReady, grokReady],
    )
    // Agents are named for awareness, but no hand-off JSON is fabricated.
    expect(prompt).toContain("Claude Code")
    expect(prompt).toContain("Grok")
    expect(templateInPrompt(prompt)).toBeNull()
    expect(prompt).not.toContain("reply with ONLY")
  })

  test("ROUND TRIP: the codex template in the prompt decodes to a codex delegation", () => {
    const prompt = buildOpenAgentsAppleFmPrompt(
      [{ role: "user", text: "hand this off to codex" }],
      [codexReady],
    )
    const template = templateInPrompt(prompt)
    expect(template).not.toBeNull()
    // The decoder (AFS-02) accepts the exact template as a codex recommendation.
    const decoded = decodeAppleFmRouteOutput({ raw: template!, admittedCandidates: ["codex"] })
    expect(decoded._tag).toBe("Recommendation")
    // The router (AFS-04) turns it into exactly one codex delegation.
    const decision = decideDelegation({
      answerText: template!,
      objective: "hand this off to codex",
      readiness: { codex: { ready: true, accountRef: "acct.a" } },
    })
    expect(decision.kind).toBe("delegate")
    if (decision.kind === "delegate") {
      expect(decision.provider).toBe("codex")
      expect(decision.recommendation.candidate).toBe("codex")
    }
  })

  test("a plain-words answer (what agents are available) never dispatches", () => {
    // The model answering "who are you" in words carries no JSON → answer, no dispatch.
    const decision = decideDelegation({
      answerText: "I'm OpenAgents, running on Apple's on-device model. I can hand coding tasks to Codex.",
      objective: "who are you, what agents do you have",
      readiness: { codex: { ready: true, accountRef: "acct.a" } },
    })
    expect(decision.kind).toBe("answer")
  })

  test("malformed / action-claim / unavailable-agent output never dispatches (fail-closed)", () => {
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

    // A recommended-but-unavailable codex lane refuses without a start.
    const template = templateInPrompt(
      buildOpenAgentsAppleFmPrompt([{ role: "user", text: "task codex" }], [codexReady]),
    )
    const unavailable = decideDelegation({
      answerText: template!,
      objective: "x",
      readiness: { codex: { ready: false, unavailableReason: "no_codex_account" } },
    })
    expect(unavailable.kind).toBe("refuse_delegation")
  })
})
