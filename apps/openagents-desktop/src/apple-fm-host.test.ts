import { describe, expect, test } from "vite-plus/test"
import {
  createAppleFmHost,
  type AppleFmLaunchOutcome,
  type AppleFmLauncher,
  type AppleFmLauncherSession,
  type AppleFmProbe,
} from "./apple-fm-host.ts"

const readySession = (over: Partial<AppleFmLauncherSession> = {}): { session: AppleFmLauncherSession; stops: () => number } => {
  let stops = 0
  const session: AppleFmLauncherSession = {
    mode: "launched",
    probe: async (): Promise<AppleFmProbe> => ({
      status: "ready",
      ready: true,
      model: "apple-foundation-model",
      profileId: "apple-fm-local",
      usageTruth: "estimated",
    }),
    complete: async () => ({ outcome: "completed", text: "hello", usageTruth: "estimated", totalTokens: 5 }),
    stop: () => { stops += 1 },
    ...over,
  }
  return { session, stops: () => stops }
}

const launcher = (input: {
  supported?: boolean
  outcome: AppleFmLaunchOutcome
  onLaunch?: (onCrash: (failureClass: string) => void) => void
}): AppleFmLauncher => ({
  supported: () => input.supported ?? true,
  launch: async ({ onCrash }) => {
    input.onLaunch?.(onCrash)
    return input.outcome
  },
})

describe("Apple FM supervisor state machine", () => {
  test("unsupported platform stays not_supported and refuses turns without launching", async () => {
    let launched = false
    const host = createAppleFmHost(launcher({ supported: false, outcome: { kind: "helper_missing", blockerRef: "b" }, onLaunch: () => { launched = true } }))
    expect(host.status()).toMatchObject({ supported: false, state: "not_supported", ready: false, readiness: "unsupported" })
    expect(await host.ensureStarted()).toMatchObject({ state: "not_supported" })
    expect(launched).toBe(false)
    const turn = await host.runTurn("hi")
    expect(turn).toMatchObject({ ok: false, outcome: "refused_unsupported", failureClass: "unsupported_platform" })
  })

  test("missing helper reports helper_missing with a bounded blocker", async () => {
    const host = createAppleFmHost(launcher({ outcome: { kind: "helper_missing", blockerRef: "blocker.apple_fm.helper_missing" } }))
    const status = await host.ensureStarted()
    expect(status).toMatchObject({ state: "helper_missing", ready: false, unavailableReason: "helper_missing" })
    expect(status.blockerRefs).toEqual(["blocker.apple_fm.helper_missing"])
  })

  test("digest/signature failure reports failed with a typed failure class", async () => {
    const host = createAppleFmHost(launcher({ outcome: { kind: "failed", blockerRef: "blocker.apple_fm.apple_fm_helper_digest_mismatch", failureClass: "apple_fm_helper_digest_mismatch" } }))
    const status = await host.ensureStarted()
    expect(status).toMatchObject({ state: "failed", ready: false, unavailableReason: "apple_fm_helper_digest_mismatch" })
  })

  test("ready launch reaches ready and admits one bounded turn; not-ready refuses", async () => {
    const { session } = readySession()
    const host = createAppleFmHost(launcher({ outcome: { kind: "session", session } }))
    // Turn before start is refused.
    expect(await host.runTurn("hi")).toMatchObject({ outcome: "refused_not_ready" })
    const status = await host.ensureStarted()
    expect(status).toMatchObject({ state: "ready", ready: true, mode: "local_launched", model: "apple-foundation-model", profileId: "apple-fm-local", usageTruth: "estimated" })
    const turn = await host.runTurn("read the readme")
    expect(turn).toMatchObject({ ok: true, outcome: "completed", text: "hello", usageTruth: "estimated", totalTokens: 5 })
  })

  test("not-ready health projects unavailable and refuses the turn", async () => {
    const { session } = readySession({
      probe: async () => ({ status: "unsupported", ready: false, unavailableReason: "apple_intelligence_disabled" }),
    })
    const host = createAppleFmHost(launcher({ outcome: { kind: "session", session } }))
    const status = await host.ensureStarted()
    expect(status).toMatchObject({ state: "unavailable", ready: false, readiness: "unsupported", unavailableReason: "apple_intelligence_disabled" })
    expect(status.blockerRefs).toEqual(["blocker.apple_fm.apple_intelligence_disabled"])
    expect(await host.runTurn("hi")).toMatchObject({ outcome: "refused_not_ready" })
  })

  test("adopted bridge is never stopped on stop() or dispose()", async () => {
    const { session, stops } = readySession({ mode: "adopted" })
    const host = createAppleFmHost(launcher({ outcome: { kind: "session", session } }))
    const status = await host.ensureStarted()
    expect(status).toMatchObject({ mode: "local_adopted", state: "ready", ready: true })
    host.stop()
    host.dispose()
    expect(stops()).toBe(0)
  })

  test("launched bridge is stopped exactly once and stop() resets the projection", async () => {
    const { session, stops } = readySession()
    const host = createAppleFmHost(launcher({ outcome: { kind: "session", session } }))
    await host.ensureStarted()
    const stopped = host.stop()
    expect(stopped).toMatchObject({ state: "stopped", ready: false, mode: "none" })
    expect(stops()).toBe(1)
  })

  test("a crash after ready transitions to failed for that generation only", async () => {
    const holder: { crash: ((failureClass: string) => void) | null } = { crash: null }
    const { session } = readySession()
    const host = createAppleFmHost(launcher({ outcome: { kind: "session", session }, onLaunch: (onCrash) => { holder.crash = onCrash } }))
    await host.ensureStarted()
    expect(host.status().state).toBe("ready")
    holder.crash?.("helper_crashed")
    expect(host.status()).toMatchObject({ state: "failed", ready: false, unavailableReason: "helper_crashed" })
    expect(host.status().blockerRefs).toEqual(["blocker.apple_fm.helper_crashed"])
    // A stale crash from a superseded generation is ignored.
    host.stop()
    holder.crash?.("helper_crashed")
    expect(host.status().state).toBe("stopped")
  })

  test("dispose is idempotent and refuses further turns", async () => {
    const { session } = readySession()
    const host = createAppleFmHost(launcher({ outcome: { kind: "session", session } }))
    await host.ensureStarted()
    host.dispose()
    host.dispose()
    expect(await host.runTurn("hi")).toMatchObject({ outcome: "refused_unsupported" })
  })
})
