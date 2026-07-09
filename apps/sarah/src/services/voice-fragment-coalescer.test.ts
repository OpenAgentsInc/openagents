import { describe, expect, test } from "bun:test"

import {
  makeSarahVoiceFragmentCoalescer,
  SarahVoiceFragmentCoalescerError,
  type SarahVoiceFragmentBatch,
  type SarahVoiceFragmentCoalescerConfig,
  type SarahVoiceFragmentCoalescerDependencies,
  type SarahVoiceFragmentScheduledTask,
} from "./voice-fragment-coalescer.ts"

const config: SarahVoiceFragmentCoalescerConfig = {
  quietWindowMs: 100,
  maxWaitMs: 250,
  maxConversationRefCharacters: 64,
  maxTextCharacters: 24,
  maxFragmentsPerGroup: 4,
  maxActiveGroups: 4,
  executionTimeoutMs: 500,
}

class ManualScheduler implements SarahVoiceFragmentCoalescerDependencies {
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
  ): SarahVoiceFragmentScheduledTask => {
    const id = this.sequence
    this.sequence += 1
    this.tasks.set(id, { at: this.nowMs + delayMs, task })
    return { cancel: () => void this.tasks.delete(id) }
  }

  advanceBy(milliseconds: number) {
    const target = this.nowMs + milliseconds
    while (true) {
      const due = [...this.tasks.entries()]
        .filter(([, task]) => task.at <= target)
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

  get scheduledTaskCount(): number {
    return this.tasks.size
  }
}

const drainPromises = async () => {
  await Promise.resolve()
  await Promise.resolve()
}

describe("Sarah VAD fragment coalescer (FC-BRAIN #8600)", () => {
  test("same-ref fragments share one quiet-window execution and response", async () => {
    const scheduler = new ManualScheduler()
    const batches: SarahVoiceFragmentBatch[] = []
    const coalescer = makeSarahVoiceFragmentCoalescer({
      config,
      dependencies: scheduler,
      execute: async (batch) => {
        batches.push(batch)
        return `reply:${batch.text}`
      },
    })

    const first = coalescer.join({
      conversationRef: "prospect:same",
      fragment: "Could you",
    })
    scheduler.advanceBy(60)
    const second = coalescer.join({
      conversationRef: "prospect:same",
      fragment: "Could you explain Khala?",
    })

    expect(first).toBe(second)
    scheduler.advanceBy(99)
    expect(batches).toHaveLength(0)
    scheduler.advanceBy(1)
    await expect(Promise.all([first, second])).resolves.toEqual([
      "reply:Could you explain Khala?",
      "reply:Could you explain Khala?",
    ])
    expect(batches).toEqual([
      {
        conversationRef: "prospect:same",
        text: "Could you explain Khala?",
        fragmentCount: 2,
        flushReason: "quiet_window",
        openedAtMs: 0,
        flushedAtMs: 160,
      },
    ])
  })

  test("acceptance receipt keeps rejected adjacent request state out of a pending group", async () => {
    const scheduler = new ManualScheduler()
    const batches: SarahVoiceFragmentBatch[] = []
    const coalescer = makeSarahVoiceFragmentCoalescer({
      config,
      dependencies: scheduler,
      execute: async (batch) => {
        batches.push(batch)
        return batch.text
      },
    })
    expect(
      coalescer.preflight({
        conversationRef: "prospect:acceptance",
        fragment: "   ",
      }),
    ).toEqual({ accepted: false, reason: "empty_fragment" })
    expect(coalescer.snapshot().activeGroups).toBe(0)
    expect(scheduler.scheduledTaskCount).toBe(0)
    const accepted = coalescer.joinWithAcceptance({
      conversationRef: "prospect:acceptance",
      fragment: "canonical fragment",
    })
    const rejected = coalescer.joinWithAcceptance({
      conversationRef: "prospect:acceptance",
      fragment: "   ",
    })

    expect(accepted.accepted).toBe(true)
    expect(rejected.accepted).toBe(false)
    await expect(rejected.result).rejects.toMatchObject({
      reason: "empty_fragment",
    })
    scheduler.advanceBy(config.quietWindowMs)
    await expect(accepted.result).resolves.toBe("canonical fragment")
    expect(batches).toHaveLength(1)
    expect(batches[0]).toMatchObject({
      text: "canonical fragment",
      fragmentCount: 1,
    })
  })

  test("separate conversation refs never share groups, text, or responses", async () => {
    const scheduler = new ManualScheduler()
    const batches: SarahVoiceFragmentBatch[] = []
    const coalescer = makeSarahVoiceFragmentCoalescer({
      config,
      dependencies: scheduler,
      execute: (batch) => {
        batches.push(batch)
        return `${batch.conversationRef}:${batch.text}`
      },
    })

    const prospectA = coalescer.join({
      conversationRef: "prospect:a",
      fragment: "private A fragment",
    })
    const prospectB = coalescer.join({
      conversationRef: "prospect:b",
      fragment: "private B fragment",
    })
    expect(prospectA).not.toBe(prospectB)

    scheduler.advanceBy(config.quietWindowMs)
    await expect(prospectA).resolves.toBe("prospect:a:private A fragment")
    await expect(prospectB).resolves.toBe("prospect:b:private B fragment")
    expect(batches.map((batch) => batch.conversationRef).sort()).toEqual([
      "prospect:a",
      "prospect:b",
    ])
    expect(batches.find((batch) => batch.conversationRef === "prospect:a")?.text).toBe(
      "private A fragment",
    )
    expect(batches.find((batch) => batch.conversationRef === "prospect:b")?.text).toBe(
      "private B fragment",
    )
    await expect(
      coalescer.join({
        conversationRef: " prospect:a",
        fragment: "must not normalize into prospect A",
      }),
    ).rejects.toMatchObject({ reason: "invalid_conversation_ref" })
  })

  test("uses the latest bounded cumulative fragment without concatenating partials", async () => {
    const scheduler = new ManualScheduler()
    const executed: SarahVoiceFragmentBatch[] = []
    const coalescer = makeSarahVoiceFragmentCoalescer({
      config,
      dependencies: scheduler,
      execute: (batch) => {
        executed.push(batch)
        return batch.text
      },
    })

    const result = coalescer.join({
      conversationRef: "prospect:latest",
      fragment: "Need",
    })
    coalescer.join({
      conversationRef: "prospect:latest",
      fragment: "Need help",
    })
    coalescer.join({
      conversationRef: "prospect:latest",
      fragment: "Need help with agents",
    })
    scheduler.advanceBy(config.quietWindowMs)

    await expect(result).resolves.toBe("Need help with agents")
    expect(executed[0]).toMatchObject({
      text: "Need help with agents",
      fragmentCount: 3,
    })
    expect(executed[0]?.text).not.toContain("NeedNeed")
  })

  test("continuous fragments flush at max wait instead of extending forever", async () => {
    const scheduler = new ManualScheduler()
    let executed: SarahVoiceFragmentBatch | null = null
    const coalescer = makeSarahVoiceFragmentCoalescer({
      config,
      dependencies: scheduler,
      execute: (batch) => {
        executed = batch
        return batch.text
      },
    })

    const result = coalescer.join({
      conversationRef: "prospect:max-wait",
      fragment: "fragment one",
    })
    scheduler.advanceBy(80)
    coalescer.join({
      conversationRef: "prospect:max-wait",
      fragment: "fragment two",
    })
    scheduler.advanceBy(80)
    coalescer.join({
      conversationRef: "prospect:max-wait",
      fragment: "fragment three",
    })
    scheduler.advanceBy(80)
    coalescer.join({
      conversationRef: "prospect:max-wait",
      fragment: "fragment four",
    })
    scheduler.advanceBy(9)
    expect(executed).toBeNull()
    scheduler.advanceBy(1)

    await expect(result).resolves.toBe("fragment four")
    expect(executed).toMatchObject({
      fragmentCount: 4,
      flushReason: "max_wait",
      openedAtMs: 0,
      flushedAtMs: 250,
    })
  })

  test("cleans timers and groups after success, then accepts a fresh same-ref group", async () => {
    const scheduler = new ManualScheduler()
    let executions = 0
    const coalescer = makeSarahVoiceFragmentCoalescer({
      config,
      dependencies: scheduler,
      execute: (batch) => {
        executions += 1
        return batch.text
      },
    })

    const first = coalescer.join({
      conversationRef: "prospect:cleanup",
      fragment: "first turn",
    })
    scheduler.advanceBy(config.quietWindowMs)
    await first
    await drainPromises()
    expect(coalescer.snapshot()).toEqual({
      activeGroups: 0,
      pendingGroups: 0,
      executingGroups: 0,
    })
    expect(scheduler.scheduledTaskCount).toBe(0)

    const second = coalescer.join({
      conversationRef: "prospect:cleanup",
      fragment: "second turn",
    })
    expect(second).not.toBe(first)
    scheduler.advanceBy(config.quietWindowMs)
    await expect(second).resolves.toBe("second turn")
    expect(executions).toBe(2)
  })

  test("fans one typed executor failure out to every joined caller", async () => {
    const scheduler = new ManualScheduler()
    const coalescer = makeSarahVoiceFragmentCoalescer({
      config,
      dependencies: scheduler,
      execute: () => {
        throw new Error("provider failure carrying private detail")
      },
    })

    const first = coalescer.join({
      conversationRef: "prospect:failure",
      fragment: "first",
    })
    const second = coalescer.join({
      conversationRef: "prospect:failure",
      fragment: "latest",
    })
    const firstFailure = first.catch((error) => error)
    const secondFailure = second.catch((error) => error)
    scheduler.advanceBy(config.quietWindowMs)
    const [left, right] = await Promise.all([firstFailure, secondFailure])

    expect(first).toBe(second)
    expect(left).toBe(right)
    expect(left).toBeInstanceOf(SarahVoiceFragmentCoalescerError)
    expect(left).toMatchObject({
      reason: "execution_failed",
      message: "Voice fragment execution failed.",
    })
    expect(JSON.stringify(left)).not.toContain("private detail")
    expect(coalescer.snapshot().activeGroups).toBe(0)
  })

  test("refuses same-ref text while execution runs without consuming another group", async () => {
    const scheduler = new ManualScheduler()
    const batches: SarahVoiceFragmentBatch[] = []
    let finishLongRunning!: (reply: string) => void
    const longRunning = new Promise<string>((resolve) => {
      finishLongRunning = resolve
    })
    const coalescer = makeSarahVoiceFragmentCoalescer({
      config: { ...config, maxActiveGroups: 2 },
      dependencies: scheduler,
      execute: (batch) => {
        batches.push(batch)
        return batch.conversationRef === "prospect:long-running"
          ? longRunning
          : `reply:${batch.text}`
      },
    })

    const first = coalescer.join({
      conversationRef: "prospect:long-running",
      fragment: "first accepted text",
    })
    scheduler.advanceBy(config.quietWindowMs)
    expect(batches).toHaveLength(1)
    expect(coalescer.snapshot()).toEqual({
      activeGroups: 1,
      pendingGroups: 0,
      executingGroups: 1,
    })

    const busyFailures = Array.from({ length: 6 }, (_, index) =>
      coalescer
        .join({
          conversationRef: "prospect:long-running",
          fragment: `new text ${index}`,
        })
        .catch((error) => error),
    )
    const failures = await Promise.all(busyFailures)
    expect(
      failures.every(
        (failure) => failure.reason === "conversation_busy",
      ),
    ).toBe(true)
    expect(batches).toHaveLength(1)
    expect(coalescer.snapshot()).toEqual({
      activeGroups: 1,
      pendingGroups: 0,
      executingGroups: 1,
    })

    const unrelated = coalescer.join({
      conversationRef: "prospect:unrelated",
      fragment: "independent text",
    })
    expect(coalescer.snapshot()).toEqual({
      activeGroups: 2,
      pendingGroups: 1,
      executingGroups: 1,
    })
    scheduler.advanceBy(config.quietWindowMs)
    await expect(unrelated).resolves.toBe("reply:independent text")
    expect(
      batches.filter(
        (batch) => batch.conversationRef === "prospect:long-running",
      ),
    ).toHaveLength(1)

    finishLongRunning("reply:first accepted text")
    await expect(first).resolves.toBe("reply:first accepted text")
    await drainPromises()
    expect(coalescer.snapshot().activeGroups).toBe(0)
  })

  test("fails text, fragment-count, and active-group overflow with bounded typed errors", async () => {
    const scheduler = new ManualScheduler()
    const bounded = makeSarahVoiceFragmentCoalescer({
      config: {
        ...config,
        maxTextCharacters: 5,
        maxFragmentsPerGroup: 2,
        maxActiveGroups: 1,
      },
      dependencies: scheduler,
      execute: (batch) => batch.text,
    })

    await expect(
      bounded.join({
        conversationRef: "prospect:text-overflow",
        fragment: "sixsix",
      }),
    ).rejects.toMatchObject({ reason: "fragment_too_large" })
    await expect(
      bounded.join({
        conversationRef: "x".repeat(config.maxConversationRefCharacters + 1),
        fragment: "one",
      }),
    ).rejects.toMatchObject({ reason: "conversation_ref_too_large" })

    const first = bounded.join({
      conversationRef: "prospect:group-overflow",
      fragment: "one",
    })
    const second = bounded.join({
      conversationRef: "prospect:group-overflow",
      fragment: "two",
    })
    const firstFailure = first.catch((error) => error)
    const secondFailure = second.catch((error) => error)
    const third = bounded.join({
      conversationRef: "prospect:group-overflow",
      fragment: "three",
    })
    const [left, middle, right] = await Promise.all([
      firstFailure,
      secondFailure,
      third.catch((error) => error),
    ])
    expect(left).toBe(middle)
    expect(middle).toBe(right)
    expect(right).toMatchObject({ reason: "too_many_fragments" })

    const active = bounded.join({
      conversationRef: "prospect:active-a",
      fragment: "one",
    })
    await expect(
      bounded.join({
        conversationRef: "prospect:active-b",
        fragment: "two",
      }),
    ).rejects.toMatchObject({ reason: "too_many_active_groups" })
    scheduler.advanceBy(config.quietWindowMs)
    await active
  })

  test("times out execution with an abort signal and removes the group", async () => {
    const scheduler = new ManualScheduler()
    const signals: AbortSignal[] = []
    const coalescer = makeSarahVoiceFragmentCoalescer({
      config,
      dependencies: scheduler,
      execute: (_batch, receivedSignal) => {
        signals.push(receivedSignal)
        return new Promise<string>(() => {})
      },
    })
    const result = coalescer.join({
      conversationRef: "prospect:timeout",
      fragment: "please answer",
    })
    const failure = result.catch((error) => error)

    scheduler.advanceBy(config.quietWindowMs)
    expect(coalescer.snapshot()).toEqual({
      activeGroups: 1,
      pendingGroups: 0,
      executingGroups: 1,
    })
    scheduler.advanceBy(config.executionTimeoutMs)
    expect(await failure).toMatchObject({ reason: "execution_timeout" })
    expect(signals[0]?.aborted).toBe(true)
    expect(coalescer.snapshot().activeGroups).toBe(0)
    expect(scheduler.scheduledTaskCount).toBe(0)
  })

  test("close rejects pending work, cancels timers, and refuses later joins", async () => {
    const scheduler = new ManualScheduler()
    const coalescer = makeSarahVoiceFragmentCoalescer({
      config,
      dependencies: scheduler,
      execute: (batch) => batch.text,
    })
    const pending = coalescer.join({
      conversationRef: "prospect:close",
      fragment: "pending",
    })
    const failure = pending.catch((error) => error)

    coalescer.close()
    expect(await failure).toMatchObject({ reason: "service_closed" })
    expect(coalescer.snapshot().activeGroups).toBe(0)
    expect(scheduler.scheduledTaskCount).toBe(0)
    await expect(
      coalescer.join({
        conversationRef: "prospect:after-close",
        fragment: "later",
      }),
    ).rejects.toMatchObject({ reason: "service_closed" })
  })
})
