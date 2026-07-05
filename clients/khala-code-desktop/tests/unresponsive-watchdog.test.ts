import { describe, expect, test } from "bun:test"

import { createKhalaCodeDesktopUnresponsiveWatchdog } from "../src/shared/unresponsive-watchdog"

describe("createKhalaCodeDesktopUnresponsiveWatchdog", () => {
  test("starts responsive and stays responsive under the timeout", () => {
    const watchdog = createKhalaCodeDesktopUnresponsiveWatchdog({ now: () => 0, timeoutMs: 5000 })
    expect(watchdog.state()).toBe("responsive")
    expect(watchdog.checkNow(4000)).toBe("responsive")
  })

  test("flags unresponsive once a heartbeat is overdue", () => {
    const watchdog = createKhalaCodeDesktopUnresponsiveWatchdog({ now: () => 0, timeoutMs: 5000 })
    expect(watchdog.checkNow(5001)).toBe("unresponsive")
    expect(watchdog.state()).toBe("unresponsive")
  })

  test("recovers to responsive once a fresh heartbeat arrives", () => {
    const watchdog = createKhalaCodeDesktopUnresponsiveWatchdog({ now: () => 0, timeoutMs: 1000 })
    watchdog.checkNow(2000)
    expect(watchdog.state()).toBe("unresponsive")
    watchdog.recordHeartbeat(2100)
    expect(watchdog.state()).toBe("responsive")
    expect(watchdog.checkNow(2500)).toBe("responsive")
  })

  test("fires onStateChange only on actual transitions, not on repeated checks", () => {
    const transitions: string[] = []
    const watchdog = createKhalaCodeDesktopUnresponsiveWatchdog({
      now: () => 0,
      onStateChange: state => transitions.push(state),
      timeoutMs: 1000,
    })
    watchdog.checkNow(500) // still responsive, no transition
    watchdog.checkNow(1500) // -> unresponsive
    watchdog.checkNow(1600) // still unresponsive, no transition
    watchdog.recordHeartbeat(1700) // -> responsive
    expect(transitions).toEqual(["unresponsive", "responsive"])
  })

  test("msSinceLastHeartbeat reports elapsed time and never goes negative", () => {
    const watchdog = createKhalaCodeDesktopUnresponsiveWatchdog({ now: () => 0, timeoutMs: 1000 })
    expect(watchdog.msSinceLastHeartbeat(300)).toBe(300)
    watchdog.recordHeartbeat(300)
    expect(watchdog.msSinceLastHeartbeat(200)).toBe(0)
  })

  test("rejects a non-positive timeoutMs", () => {
    expect(() => createKhalaCodeDesktopUnresponsiveWatchdog({ timeoutMs: 0 })).toThrow()
    expect(() => createKhalaCodeDesktopUnresponsiveWatchdog({ timeoutMs: -1 })).toThrow()
  })
})
