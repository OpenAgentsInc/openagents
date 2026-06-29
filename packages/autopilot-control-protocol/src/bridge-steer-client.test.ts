import { describe, expect, test } from "bun:test"

import {
  buildCoordinatorPauseEnvelope,
  buildCoordinatorResumeEnvelope,
  buildDeployCloudEnvelope,
  buildIntentSubmitEnvelope,
  buildSpawnEnvelope,
  buildTurnSteerEnvelope,
  canDeployCloud,
  canPauseResumeCoordinator,
  canSpawnSession,
  canSubmitIntent,
  canSteerTurn,
} from "./bridge-steer-client.js"
import { verbAllowedByCapabilities, type Capability } from "./bridge.js"

const base = {
  pairingRef: "pairing.fixture.0001",
  capabilityRef: "spawn_session",
  clientRequestId: "client.request.fixture.0001",
} as const

describe("bridge steer client — #5494 capability-scoped steer envelopes", () => {
  test("buildSpawnEnvelope carries adapter/objective/verify/lane with defaults", () => {
    expect(buildSpawnEnvelope({ ...base, adapter: "codex", objective: "ship it" })).toEqual({
      verb: "session.spawn",
      clientRequestId: "client.request.fixture.0001",
      idempotencyKey: "client.request.fixture.0001",
      pairingRef: "pairing.fixture.0001",
      capabilityRef: "spawn_session",
      adapter: "codex",
      objective: "ship it",
      verify: [],
      lane: "auto",
    })
  })

  test("buildSpawnEnvelope preserves explicit verify + lane", () => {
    const e = buildSpawnEnvelope({
      ...base,
      adapter: "claude_agent",
      objective: "fix bug",
      verify: ["bun test"],
      lane: "cloud-gcp",
    })
    expect(e.verify).toEqual(["bun test"])
    expect(e.lane).toBe("cloud-gcp")
  })

  test("buildIntentSubmitEnvelope carries title/body and optional fields", () => {
    expect(
      buildIntentSubmitEnvelope({
        ...base,
        capabilityRef: "send_instruction",
        title: "Ask",
        body: "do the thing",
        submittedByClientRef: "mobile",
      }),
    ).toEqual({
      verb: "intent.submit",
      clientRequestId: "client.request.fixture.0001",
      idempotencyKey: "client.request.fixture.0001",
      pairingRef: "pairing.fixture.0001",
      capabilityRef: "send_instruction",
      title: "Ask",
      body: "do the thing",
      submittedByClientRef: "mobile",
    })
  })

  test("buildIntentSubmitEnvelope omits undefined optional fields", () => {
    const e = buildIntentSubmitEnvelope({ ...base, capabilityRef: "send_instruction", title: "t", body: "b" })
    expect("scopeHint" in e).toBe(false)
    expect("submittedByClientRef" in e).toBe(false)
  })

  test("buildTurnSteerEnvelope carries sessionRef and instruction", () => {
    expect(
      buildTurnSteerEnvelope({
        ...base,
        capabilityRef: "send_instruction",
        sessionRef: "session.parent.1",
        instruction: "continue with the regression test",
        timeoutSeconds: 120,
      }),
    ).toEqual({
      verb: "turn.steer",
      clientRequestId: "client.request.fixture.0001",
      idempotencyKey: "client.request.fixture.0001",
      pairingRef: "pairing.fixture.0001",
      capabilityRef: "send_instruction",
      sessionRef: "session.parent.1",
      instruction: "continue with the regression test",
      timeoutSeconds: 120,
    })
  })

  test("buildTurnSteerEnvelope omits undefined timeoutSeconds", () => {
    const e = buildTurnSteerEnvelope({
      ...base,
      capabilityRef: "send_instruction",
      sessionRef: "session.parent.1",
      instruction: "next",
    })
    expect("timeoutSeconds" in e).toBe(false)
  })

  test("buildCoordinatorPause/Resume build the matching verbs", () => {
    expect(buildCoordinatorPauseEnvelope({ ...base, capabilityRef: "pause_resume" }).verb).toBe("coordinator.pause")
    expect(buildCoordinatorResumeEnvelope({ ...base, capabilityRef: "pause_resume" }).verb).toBe("coordinator.resume")
  })

  test("buildDeployCloudEnvelope carries target/ref and optional env", () => {
    expect(
      buildDeployCloudEnvelope({ ...base, capabilityRef: "deploy_cloud", target: "cloudrun", ref: "main", env: "production" }),
    ).toEqual({
      verb: "deploy.cloud",
      clientRequestId: "client.request.fixture.0001",
      idempotencyKey: "client.request.fixture.0001",
      pairingRef: "pairing.fixture.0001",
      capabilityRef: "deploy_cloud",
      target: "cloudrun",
      ref: "main",
      env: "production",
    })
  })

  test("idempotencyKey defaults to clientRequestId and is overridable", () => {
    expect(buildSpawnEnvelope({ ...base, adapter: "codex", objective: "x" }).idempotencyKey).toBe(
      "client.request.fixture.0001",
    )
    expect(
      buildSpawnEnvelope({ ...base, adapter: "codex", objective: "x", idempotencyKey: "idem.1" }).idempotencyKey,
    ).toBe("idem.1")
  })

  // Capability gating: each steer verb requires its own class, and a read-only
  // viewer can never reach any of them.
  test("each steer verb is gated on the correct capability class", () => {
    expect(canSpawnSession(["spawn_session"])).toBe(true)
    expect(canSpawnSession(["observe_public"])).toBe(false)

    expect(canSubmitIntent(["send_instruction"])).toBe(true)
    expect(canSubmitIntent(["observe_public"])).toBe(false)

    expect(canSteerTurn(["send_instruction"])).toBe(true)
    expect(canSteerTurn(["observe_public"])).toBe(false)

    expect(canPauseResumeCoordinator(["pause_resume"])).toBe(true)
    expect(canPauseResumeCoordinator(["observe_public"])).toBe(false)

    expect(canDeployCloud(["deploy_cloud"])).toBe(true)
    expect(canDeployCloud(["observe_public"])).toBe(false)
  })

  test("verbAllowedByCapabilities maps the promoted verbs to their classes", () => {
    const caps: Capability[] = [
      "observe_public",
      "spawn_session",
      "send_instruction",
      "pause_resume",
      "deploy_cloud",
    ]
    expect(verbAllowedByCapabilities("session.spawn", caps)).toBe(true)
    expect(verbAllowedByCapabilities("intent.submit", caps)).toBe(true)
    expect(verbAllowedByCapabilities("turn.steer", caps)).toBe(true)
    expect(verbAllowedByCapabilities("coordinator.pause", caps)).toBe(true)
    expect(verbAllowedByCapabilities("coordinator.resume", caps)).toBe(true)
    expect(verbAllowedByCapabilities("deploy.cloud", caps)).toBe(true)
  })

  test("a read-only credential is denied every promoted steer verb", () => {
    const readOnly: Capability[] = ["observe_public", "read_artifact"]
    for (const verb of ["session.spawn", "intent.submit", "turn.steer", "coordinator.pause", "coordinator.resume", "deploy.cloud"] as const) {
      expect(verbAllowedByCapabilities(verb, readOnly)).toBe(false)
    }
  })
})
