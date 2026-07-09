import { describe, expect, test } from "bun:test"

import {
  SarahConversationStreamFanoutError,
  makeSarahConversationStreamFanout,
  type SarahConversationStreamCanonicalRecord,
  type SarahConversationStreamFanoutConfig,
  type SarahConversationStreamFanoutDependencies,
  type SarahConversationStreamScheduledTask,
  type SarahConversationStreamScope,
} from "./conversation-stream-fanout.ts"

const scope: SarahConversationStreamScope = {
  ownerRef: "owner.sarah.1",
  conversationRef: "conversation.sarah.1",
  turnRef: "turn.sarah.1",
}

const config: SarahConversationStreamFanoutConfig = {
  maxOwnerRefCharacters: 64,
  maxConversationRefCharacters: 64,
  maxTurnRefCharacters: 64,
  maxTurns: 8,
  maxSubscribersPerTurn: 4,
  maxEventsPerTurn: 8,
  maxEventBytes: 32,
  maxBytesPerTurn: 128,
  maxSubscriberLagEvents: 4,
  maxTurnAgeMs: 1_000,
  recordTimeoutMs: 100,
  maxReplayAgeMs: 500,
}

class ManualScheduler implements SarahConversationStreamFanoutDependencies {
  private nowMs = 0
  private sequence = 0
  private readonly tasks = new Map<
    number,
    { readonly at: number; readonly task: () => void }
  >()

  readonly now = () => this.nowMs

  readonly schedule = (
    delayMs: number,
    task: () => void,
  ): SarahConversationStreamScheduledTask => {
    const id = this.sequence
    this.sequence += 1
    this.tasks.set(id, { at: this.nowMs + delayMs, task })
    return { cancel: () => void this.tasks.delete(id) }
  }

  advanceBy(milliseconds: number) {
    const target = this.nowMs + milliseconds
    while (true) {
      const due = [...this.tasks.entries()]
        .filter(([, scheduled]) => scheduled.at <= target)
        .sort(
          ([leftId, left], [rightId, right]) =>
            left.at - right.at || leftId - rightId,
        )[0]
      if (due === undefined) break
      const [id, scheduled] = due
      this.tasks.delete(id)
      this.nowMs = scheduled.at
      scheduled.task()
    }
    this.nowMs = target
  }

  get taskCount(): number {
    return this.tasks.size
  }
}

const makeHarness = (
  overrides: Partial<SarahConversationStreamFanoutConfig> = {},
) => {
  const scheduler = new ManualScheduler()
  const records: SarahConversationStreamCanonicalRecord[] = []
  const fanout = makeSarahConversationStreamFanout({
    config: { ...config, ...overrides },
    dependencies: scheduler,
  })
  const publisher = fanout.startTurn({
    scope,
    publishAndRecord: (record) => {
      records.push(record)
    },
  })
  return { scheduler, records, fanout, publisher }
}

describe("Sarah bounded conversation streaming fanout (#8600)", () => {
  test("two scoped subscribers share one canonical publish/record and ordered sequence", async () => {
    const { records, fanout, publisher } = makeHarness()
    const left = fanout.subscribe(scope)
    const right = fanout.subscribe(scope)
    const leftNext = left.next()
    const rightNext = right.next()

    const chunk = publisher.publish("hello")
    expect(await leftNext).toEqual(chunk)
    expect(await rightNext).toEqual(chunk)
    const terminal = await publisher.complete()
    expect(await left.next()).toEqual(terminal)
    expect(await right.next()).toEqual(terminal)

    expect(records).toHaveLength(1)
    expect(records[0]).toMatchObject({
      outcome: { kind: "terminal", terminal: "complete" },
      finalSequence: 2,
      eventCount: 1,
      byteCount: 5,
    })
    expect(records[0]?.chunks.map((frame) => frame.sequence)).toEqual([1])
    expect(chunk.sequence).toBe(1)
    expect(terminal.sequence).toBe(2)

    // Repeated settlement cannot duplicate the canonical record.
    expect(await publisher.complete()).toBe(terminal)
    expect(await publisher.fail()).toBe(terminal)
    expect(records).toHaveLength(1)
  })

  test("hostile injected clocks fail with one fixed error before any unsafe timestamp exists", () => {
    const privateClockFailure = "private-clock-driver-payload"
    const hostileReads: ReadonlyArray<() => number> = [
      () => Number.NaN,
      () => Number.POSITIVE_INFINITY,
      () => -1,
      () => Number.MAX_SAFE_INTEGER + 1,
      () => {
        throw new Error(privateClockFailure)
      },
    ]

    for (const now of hostileReads) {
      let recordAttempts = 0
      const fanout = makeSarahConversationStreamFanout({
        config,
        dependencies: {
          now,
          schedule: () => ({ cancel: () => {} }),
        },
      })
      let failure: unknown
      try {
        fanout.startTurn({
          scope,
          publishAndRecord: () => {
            recordAttempts += 1
          },
        })
      } catch (error) {
        failure = error
      }

      expect(failure).toBeInstanceOf(SarahConversationStreamFanoutError)
      expect(failure).toMatchObject({ reason: "invalid_clock" })
      expect((failure as Error).message).toBe(
        "Conversation stream fanout clock is invalid.",
      )
      expect(JSON.stringify(failure)).not.toContain(privateClockFailure)
      expect(recordAttempts).toBe(0)
      expect(fanout.snapshot().turns).toBe(0)
    }
  })

  test("hostile mid-turn clocks cannot enter chunks or canonical records", async () => {
    const scheduler = new ManualScheduler()
    const records: SarahConversationStreamCanonicalRecord[] = []
    let clock: number | "throw" = 10
    const fanout = makeSarahConversationStreamFanout({
      config,
      dependencies: {
        now: () => {
          if (clock === "throw") throw new Error("private-mid-turn-clock")
          return clock
        },
        schedule: scheduler.schedule,
      },
    })
    const publisher = fanout.startTurn({
      scope,
      publishAndRecord: (record) => {
        records.push(record)
      },
    })

    clock = Number.POSITIVE_INFINITY
    expect(() => publisher.publish("never-recorded")).toThrow(
      "Conversation stream fanout clock is invalid.",
    )
    clock = "throw"
    await expect(publisher.complete()).rejects.toMatchObject({
      reason: "invalid_clock",
    })
    expect(publisher.snapshot()).toMatchObject({
      state: "active",
      eventCount: 0,
      byteCount: 0,
    })
    expect(records).toHaveLength(0)

    clock = 11
    expect(publisher.publish("safe").observedAtMs).toBe(11)
    const terminal = await publisher.complete()
    expect(terminal.observedAtMs).toBe(11)
    expect(records).toHaveLength(1)
    expect(records[0]).toMatchObject({ openedAtMs: 10, settledAtMs: 11 })
    expect(JSON.stringify(records)).not.toContain("never-recorded")
  })

  test("clock failure at max age still aborts and settles with a safe explicit outcome", async () => {
    const scheduler = new ManualScheduler()
    const records: SarahConversationStreamCanonicalRecord[] = []
    let clockFails = false
    const fanout = makeSarahConversationStreamFanout({
      config,
      dependencies: {
        now: () => {
          if (clockFails) throw new Error("private-expiry-clock")
          return scheduler.now()
        },
        schedule: scheduler.schedule,
      },
    })
    const publisher = fanout.startTurn({
      scope,
      publishAndRecord: (record) => {
        records.push(record)
      },
    })
    const subscriber = fanout.subscribe(scope)
    const terminalRead = subscriber.next()

    clockFails = true
    scheduler.advanceBy(config.maxTurnAgeMs)
    const terminal = await publisher.settled

    expect(terminal).toMatchObject({
      kind: "error",
      reason: "invalid_clock",
      observedAtMs: config.maxTurnAgeMs,
    })
    expect(await terminalRead).toBe(terminal)
    expect(publisher.signal.aborted).toBe(true)
    expect(publisher.snapshot().state).toBe("error")
    expect(records).toHaveLength(1)
    expect(records[0]).toMatchObject({
      settledAtMs: config.maxTurnAgeMs,
      outcome: { kind: "error", reason: "invalid_clock" },
    })
  })

  test("close under a hostile clock settles active turns and clears every retained resource", async () => {
    const scheduler = new ManualScheduler()
    const records: SarahConversationStreamCanonicalRecord[] = []
    let clock: number | "throw" = 7
    const fanout = makeSarahConversationStreamFanout({
      config,
      dependencies: {
        now: () => {
          if (clock === "throw") throw new Error("private-close-clock")
          return clock
        },
        schedule: scheduler.schedule,
      },
    })
    const publisher = fanout.startTurn({
      scope,
      publishAndRecord: (record) => {
        records.push(record)
      },
    })
    clock = 8
    publisher.publish("safe-before-close")
    clock = "throw"

    await expect(fanout.close()).resolves.toBeUndefined()
    const terminal = await publisher.settled
    expect(terminal).toMatchObject({
      kind: "error",
      reason: "invalid_clock",
      observedAtMs: 8,
    })
    expect(publisher.signal.aborted).toBe(true)
    expect(records).toHaveLength(1)
    expect(records[0]).toMatchObject({
      openedAtMs: 7,
      settledAtMs: 8,
      outcome: { kind: "error", reason: "invalid_clock" },
    })
    expect(fanout.snapshot()).toEqual({
      closed: true,
      turns: 0,
      activeTurns: 0,
      retainedTurns: 0,
      subscribers: 0,
      events: 0,
      bytes: 0,
    })
    expect(scheduler.taskCount).toBe(0)
  })

  test("a late subscriber receives bounded replay and then immediate live chunks", async () => {
    const { fanout, publisher } = makeHarness()
    const first = publisher.publish("first")
    const late = fanout.subscribe(scope)

    expect(late.replayFromSequence).toBe(1)
    expect(await late.next()).toEqual(first)
    const waiting = late.next()
    const second = publisher.publish("second")
    expect(await waiting).toEqual(second)
    const terminal = await publisher.complete()
    expect(await late.next()).toEqual(terminal)
    expect([first.sequence, second.sequence, terminal.sequence]).toEqual([
      1, 2, 3,
    ])
  })

  test("subscriber detach is isolated and never aborts the shared turn", async () => {
    const { fanout, publisher } = makeHarness()
    const detached = fanout.subscribe(scope)
    const survivor = fanout.subscribe(scope)
    const detachedRead = detached.next().catch((error) => error)
    const survivorRead = survivor.next()

    detached.detach()
    expect(await detachedRead).toMatchObject({
      reason: "subscriber_detached",
    })
    expect(publisher.signal.aborted).toBe(false)
    const frame = publisher.publish("shared")
    expect(await survivorRead).toEqual(frame)
    expect(publisher.snapshot()).toMatchObject({
      state: "active",
      subscribers: 1,
    })
  })

  test("owner and conversation identity are exact and do not reveal foreign turns", () => {
    const { fanout } = makeHarness()
    for (const foreign of [
      { ...scope, ownerRef: "owner.sarah.2" },
      { ...scope, conversationRef: "conversation.sarah.2" },
      { ...scope, turnRef: "turn.sarah.2" },
    ]) {
      expect(() => fanout.subscribe(foreign)).toThrow(
        "Conversation stream is not available for this scope.",
      )
    }
    expect(() =>
      fanout.subscribe({ ...scope, ownerRef: ` ${scope.ownerRef}` }),
    ).toThrow("Conversation stream scope is invalid.")
  })

  test("slow consumers fail independently and stream overflow is explicit and bounded", async () => {
    const scheduler = new ManualScheduler()
    const records: SarahConversationStreamCanonicalRecord[] = []
    const fanout = makeSarahConversationStreamFanout({
      config: {
        ...config,
        maxEventsPerTurn: 3,
        maxSubscriberLagEvents: 1,
      },
      dependencies: scheduler,
    })
    const publisher = fanout.startTurn({
      scope,
      publishAndRecord: (record) => {
        records.push(record)
      },
    })
    const slow = fanout.subscribe(scope)
    publisher.publish("one")
    publisher.publish("two")
    await expect(slow.next()).rejects.toMatchObject({
      reason: "slow_consumer",
    })
    expect(publisher.signal.aborted).toBe(false)

    publisher.publish("three")
    expect(() => publisher.publish("four")).toThrow(
      "Conversation stream exceeded a configured bound.",
    )
    const overflow = await publisher.settled
    expect(overflow).toMatchObject({
      sequence: 4,
      kind: "overflow",
      limit: "event_count",
    })
    expect(records).toHaveLength(1)
    expect(records[0]?.chunks.map((frame) => frame.chunk)).toEqual([
      "one",
      "two",
      "three",
    ])
    expect(records[0]?.byteCount).toBe(11)
  })

  test("subscriber and total-byte limits remain hard bounds", async () => {
    const scheduler = new ManualScheduler()
    const records: SarahConversationStreamCanonicalRecord[] = []
    const fanout = makeSarahConversationStreamFanout({
      config: {
        ...config,
        maxSubscribersPerTurn: 1,
        maxEventBytes: 5,
        maxBytesPerTurn: 5,
      },
      dependencies: scheduler,
    })
    const publisher = fanout.startTurn({
      scope,
      publishAndRecord: (record) => {
        records.push(record)
      },
    })
    const first = fanout.subscribe(scope)
    expect(() => fanout.subscribe(scope)).toThrow(
      "Conversation stream is at its subscriber limit.",
    )
    first.detach()
    expect(() => fanout.subscribe(scope)).not.toThrow()

    publisher.publish("abc")
    expect(() => publisher.publish("def")).toThrow(
      "Conversation stream exceeded a configured bound.",
    )
    expect(await publisher.settled).toMatchObject({
      kind: "overflow",
      limit: "stream_bytes",
    })
    expect(records[0]).toMatchObject({
      outcome: { kind: "overflow", limit: "stream_bytes" },
      eventCount: 1,
      byteCount: 3,
    })
  })

  test("late subscribers replay both error and terminal outcomes", async () => {
    const scheduler = new ManualScheduler()
    const fanout = makeSarahConversationStreamFanout({
      config,
      dependencies: scheduler,
    })
    const records: SarahConversationStreamCanonicalRecord[] = []
    const errorPublisher = fanout.startTurn({
      scope: { ...scope, turnRef: "turn.error" },
      publishAndRecord: (record) => {
        records.push(record)
      },
    })
    errorPublisher.publish("partial")
    const errorFrame = await errorPublisher.fail()
    const errorReplay = fanout.subscribe({ ...scope, turnRef: "turn.error" })
    expect((await errorReplay.next()).kind).toBe("chunk")
    expect(await errorReplay.next()).toEqual(errorFrame)

    const terminalPublisher = fanout.startTurn({
      scope: { ...scope, turnRef: "turn.complete" },
      publishAndRecord: (record) => {
        records.push(record)
      },
    })
    terminalPublisher.publish("done")
    const terminalFrame = await terminalPublisher.complete()
    const terminalReplay = fanout.subscribe({
      ...scope,
      turnRef: "turn.complete",
    })
    expect((await terminalReplay.next()).kind).toBe("chunk")
    expect(await terminalReplay.next()).toEqual(terminalFrame)
    expect(records.map((record) => record.outcome.kind)).toEqual([
      "error",
      "terminal",
    ])
  })

  test("abort and age overflow clean producers without subscriber cancellation authority", async () => {
    const { scheduler, records, fanout, publisher } = makeHarness()
    const subscriber = fanout.subscribe(scope)
    const aborted = await publisher.abort()
    expect(publisher.signal.aborted).toBe(true)
    expect(await subscriber.next()).toEqual(aborted)
    expect(records[0]?.outcome).toEqual({ kind: "aborted" })

    const aged = fanout.startTurn({
      scope: { ...scope, turnRef: "turn.aged" },
      publishAndRecord: (record) => {
        records.push(record)
      },
    })
    scheduler.advanceBy(config.maxTurnAgeMs)
    expect(await aged.settled).toMatchObject({
      kind: "overflow",
      limit: "turn_age",
    })
    expect(aged.signal.aborted).toBe(true)
  })

  test("close publishes one explicit closed state, cancels timers, and clears memory", async () => {
    const { scheduler, records, fanout, publisher } = makeHarness()
    const subscriber = fanout.subscribe(scope)
    const pending = subscriber.next().catch((error) => error)

    await fanout.close()
    expect(await pending).toMatchObject({ kind: "closed", sequence: 1 })
    expect(publisher.signal.aborted).toBe(true)
    expect(records).toHaveLength(1)
    expect(records[0]?.outcome).toEqual({ kind: "closed" })
    expect(fanout.snapshot()).toEqual({
      closed: true,
      turns: 0,
      activeTurns: 0,
      retainedTurns: 0,
      subscribers: 0,
      events: 0,
      bytes: 0,
    })
    expect(scheduler.taskCount).toBe(0)
    expect(() => fanout.startTurn({
      scope: { ...scope, turnRef: "turn.after-close" },
      publishAndRecord: () => {},
    })).toThrow("Conversation stream fanout is closed.")
  })

  test("overflow and record failures never echo secret chunk or provider payloads", async () => {
    const secret = "SECRET-provider-payload-that-must-never-echo"
    const scheduler = new ManualScheduler()
    const fanout = makeSarahConversationStreamFanout({
      config,
      dependencies: scheduler,
    })
    const publisher = fanout.startTurn({
      scope,
      publishAndRecord: () => {
        throw new Error(secret)
      },
    })
    let failure: unknown
    try {
      publisher.publish(secret)
    } catch (error) {
      failure = error
    }
    expect(failure).toBeInstanceOf(SarahConversationStreamFanoutError)
    expect(JSON.stringify(failure)).not.toContain(secret)
    expect((failure as Error).message).not.toContain(secret)
    const terminal = await publisher.settled
    expect(terminal).toMatchObject({
      kind: "error",
      reason: "record_failed",
    })
    expect(JSON.stringify(terminal)).not.toContain(secret)
  })

  test("duplicate exact-turn start is refused before a second canonical recorder exists", async () => {
    const { records, fanout, publisher } = makeHarness()
    let duplicateCalls = 0
    expect(() =>
      fanout.startTurn({
        scope,
        publishAndRecord: () => {
          duplicateCalls += 1
        },
      }),
    ).toThrow("Conversation stream turn already exists.")

    publisher.publish("canonical")
    await publisher.complete()
    expect(records).toHaveLength(1)
    expect(duplicateCalls).toBe(0)
  })

  test("a publishAndRecord rejection settles once as replayable record_failed without retry", async () => {
    const scheduler = new ManualScheduler()
    const fanout = makeSarahConversationStreamFanout({
      config,
      dependencies: scheduler,
    })
    let recordAttempts = 0
    const publisher = fanout.startTurn({
      scope,
      publishAndRecord: () => {
        recordAttempts += 1
        throw new Error("private durable store failure")
      },
    })
    publisher.publish("bounded partial")
    const failed = await publisher.complete()
    expect(failed).toMatchObject({
      kind: "error",
      reason: "record_failed",
      sequence: 2,
    })

    const replay = fanout.subscribe(scope)
    expect(await replay.next()).toMatchObject({
      kind: "chunk",
      chunk: "bounded partial",
      sequence: 1,
    })
    expect(await replay.next()).toEqual(failed)
    expect(await publisher.complete()).toBe(failed)
    expect(await publisher.fail()).toBe(failed)
    expect(recordAttempts).toBe(1)
  })

  test("a stuck canonical recorder is aborted and settles as unknown record_timeout exactly once", async () => {
    const scheduler = new ManualScheduler()
    const fanout = makeSarahConversationStreamFanout({
      config,
      dependencies: scheduler,
    })
    let recordAttempts = 0
    let recordSignal: AbortSignal | undefined
    let resolveLateRecord!: () => void
    const publisher = fanout.startTurn({
      scope,
      publishAndRecord: (_record, signal) => {
        recordAttempts += 1
        recordSignal = signal
        return new Promise<void>((resolve) => {
          resolveLateRecord = resolve
        })
      },
    })
    publisher.publish("bounded")
    const settlement = publisher.complete()
    await Promise.resolve()
    expect(recordSignal?.aborted).toBe(false)
    scheduler.advanceBy(config.recordTimeoutMs)

    const timedOut = await settlement
    expect(timedOut).toMatchObject({
      kind: "error",
      reason: "record_timeout",
    })
    expect(recordAttempts).toBe(1)
    expect(recordSignal?.aborted).toBe(true)
    expect(publisher.signal.aborted).toBe(true)
    expect(publisher.snapshot().state).toBe("error")

    // A non-cooperative recorder may commit after its deadline. That is an
    // unknown completion, never permission to rewrite the settled outcome.
    resolveLateRecord()
    await Promise.resolve()
    await Promise.resolve()
    expect(await publisher.complete()).toBe(timedOut)
    expect(await publisher.settled).toBe(timedOut)
    expect(timedOut).toMatchObject({ reason: "record_timeout" })
    expect(recordAttempts).toBe(1)
  })

  test("overflow records a failure outcome, never a successful truncated transcript", async () => {
    const scheduler = new ManualScheduler()
    const records: SarahConversationStreamCanonicalRecord[] = []
    const fanout = makeSarahConversationStreamFanout({
      config: { ...config, maxEventsPerTurn: 1, maxSubscriberLagEvents: 1 },
      dependencies: scheduler,
    })
    const publisher = fanout.startTurn({
      scope,
      publishAndRecord: (record) => {
        records.push(record)
      },
    })
    publisher.publish("kept")
    expect(() => publisher.publish("rejected")).toThrow(
      "Conversation stream exceeded a configured bound.",
    )
    const overflow = await publisher.settled

    expect(overflow).toMatchObject({
      kind: "overflow",
      limit: "event_count",
    })
    expect(publisher.signal.aborted).toBe(true)
    expect(records).toHaveLength(1)
    expect(records[0]?.outcome).toEqual({
      kind: "overflow",
      limit: "event_count",
    })
    expect(records[0]?.chunks.map((frame) => frame.chunk)).toEqual(["kept"])
    expect(records[0]?.outcome.kind).not.toBe("terminal")
  })

  test("settled replay is bounded by count and expires by age", async () => {
    const scheduler = new ManualScheduler()
    const fanout = makeSarahConversationStreamFanout({
      config: { ...config, maxTurns: 1 },
      dependencies: scheduler,
    })
    const first = fanout.startTurn({
      scope,
      publishAndRecord: () => {},
    })
    first.publish("retained")
    await first.complete()
    expect(fanout.snapshot()).toMatchObject({
      turns: 1,
      activeTurns: 0,
      retainedTurns: 1,
    })
    expect(() =>
      fanout.startTurn({
        scope: { ...scope, turnRef: "turn.blocked.by-retention" },
        publishAndRecord: () => {},
      }),
    ).toThrow("Conversation stream fanout is at its turn limit.")

    scheduler.advanceBy(config.maxReplayAgeMs)
    expect(fanout.snapshot()).toMatchObject({ turns: 0, retainedTurns: 0 })
    expect(() => fanout.subscribe(scope)).toThrow(
      "Conversation stream is not available for this scope.",
    )
    const next = fanout.startTurn({
      scope: { ...scope, turnRef: "turn.after-replay-expiry" },
      publishAndRecord: () => {},
    })
    expect(next.snapshot().state).toBe("active")
  })
})
