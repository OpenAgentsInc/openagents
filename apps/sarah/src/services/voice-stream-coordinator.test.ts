import { describe, expect, test } from "bun:test"

import {
  SarahVoiceStreamSseError,
  handleSarahChatCompletions,
  openSarahTrustedVoiceStream,
  type SarahVoiceStreamSseDependencies,
} from "../llm-openai-compat.ts"
import { SarahConversationStreamFanoutError } from "./conversation-stream-fanout.ts"
import type {
  SarahConversationStreamFanoutConfig,
  SarahConversationStreamScheduledTask,
} from "./conversation-stream-fanout.ts"
import type { GemmaStreamEvent } from "./google-inference.ts"
import type {
  SarahVoiceFragmentCoalescerConfig,
  SarahVoiceFragmentScheduledTask,
} from "./voice-fragment-coalescer.ts"
import {
  SarahVoiceStreamCoordinatorError,
  makeSarahVoiceStreamCoordinator,
  type SarahTrustedVoiceCompletion,
  type SarahTrustedVoiceInference,
  type SarahTrustedVoiceStreamScope,
} from "./voice-stream-coordinator.ts"

const scope: SarahTrustedVoiceStreamScope = {
  prospectRef: "prospect:trusted-owner",
  conversationRef: "conversation:trusted-renderer-session",
}

const inference = (fragment: string): SarahTrustedVoiceInference => ({
  system: "You are Sarah.",
  contents: [{ role: "user", parts: [{ text: fragment }] }],
})

const coalescerConfig: SarahVoiceFragmentCoalescerConfig = {
  quietWindowMs: 10,
  maxWaitMs: 40,
  maxConversationRefCharacters: 128,
  maxTextCharacters: 128,
  maxFragmentsPerGroup: 4,
  maxActiveGroups: 8,
  executionTimeoutMs: 500,
}

const fanoutConfig: SarahConversationStreamFanoutConfig = {
  maxOwnerRefCharacters: 128,
  maxConversationRefCharacters: 128,
  maxTurnRefCharacters: 128,
  maxTurns: 8,
  maxSubscribersPerTurn: 8,
  maxEventsPerTurn: 8,
  maxEventBytes: 128,
  maxBytesPerTurn: 512,
  maxSubscriberLagEvents: 8,
  maxTurnAgeMs: 1_000,
  recordTimeoutMs: 25,
  maxReplayAgeMs: 500,
}

class ManualScheduler {
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
  ): SarahConversationStreamScheduledTask & SarahVoiceFragmentScheduledTask => {
    const id = this.sequence
    this.sequence += 1
    this.tasks.set(id, { at: this.nowMs + delayMs, task })
    return { cancel: () => void this.tasks.delete(id) }
  }

  advanceBy(milliseconds: number) {
    const target = this.nowMs + milliseconds
    for (;;) {
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

class ManualRepeater implements SarahVoiceStreamSseDependencies {
  readonly keepaliveMs = 2_000
  private readonly tasks = new Set<() => void>()

  readonly repeat = (_delayMs: number, task: () => void) => {
    this.tasks.add(task)
    return { cancel: () => void this.tasks.delete(task) }
  }

  tick() {
    for (const task of [...this.tasks]) task()
  }

  get taskCount(): number {
    return this.tasks.size
  }
}

const flushMicrotasks = async () => {
  for (let index = 0; index < 12; index += 1) await Promise.resolve()
}

const readFrame = async (
  reader: ReadableStreamDefaultReader<Uint8Array>,
): Promise<string> => {
  const result = await reader.read()
  return result.value === undefined
    ? ""
    : new TextDecoder().decode(result.value)
}

const readRemaining = async (
  reader: ReadableStreamDefaultReader<Uint8Array>,
): Promise<string> => {
  const decoder = new TextDecoder()
  let text = ""
  for (;;) {
    const { done, value } = await reader.read()
    if (done) return text
    text += decoder.decode(value)
  }
}

const fixedFallback = (error: string | null): string =>
  error === "provider_busy"
    ? "Fixed busy fallback."
    : "Fixed unavailable fallback."

describe("Sarah trusted voice stream coordinator (#8600)", () => {
  test("two immediate SSE controllers coalesce cumulative input into one model stream and one canonical record", async () => {
    const scheduler = new ManualScheduler()
    const repeater = new ManualRepeater()
    const completions: SarahTrustedVoiceCompletion[] = []
    let firstRecorderCalls = 0
    let latestRecorderCalls = 0
    let modelCalls = 0
    const observedInferences: SarahTrustedVoiceInference[] = []
    let turnSequence = 0
    const coordinator = makeSarahVoiceStreamCoordinator({
      coalescerConfig,
      coalescerDependencies: scheduler,
      fanoutConfig,
      fanoutDependencies: scheduler,
      makeTurnRef: () => `turn.coalesced.${++turnSequence}`,
      fallbackReply: fixedFallback,
      streamReply: async function* (request) {
        modelCalls += 1
        observedInferences.push(request)
        yield { type: "delta", text: "Shared " }
        yield { type: "delta", text: "answer" }
      },
    })

    const first = openSarahTrustedVoiceStream({
      coordinator,
      model: "test-model",
      scope,
      fragment: "Could you",
      inference: inference("Could you"),
      publishAndRecord: () => {
        firstRecorderCalls += 1
      },
      sseDependencies: repeater,
    })
    const second = openSarahTrustedVoiceStream({
      coordinator,
      model: "test-model",
      scope,
      fragment: "Could you explain Khala?",
      inference: inference("Could you explain Khala?"),
      publishAndRecord: (completion) => {
        latestRecorderCalls += 1
        completions.push(completion)
      },
      sseDependencies: repeater,
    })

    expect(first.headers.get("x-sarah-turn-ref")).toBe("turn.coalesced.1")
    expect(second.headers.get("x-sarah-turn-ref")).toBe("turn.coalesced.1")
    const firstReader = first.body!.getReader()
    const secondReader = second.body!.getReader()
    expect(await readFrame(firstReader)).toContain('"role":"assistant"')
    expect(await readFrame(secondReader)).toContain('"role":"assistant"')
    expect(modelCalls).toBe(0)

    repeater.tick()
    expect(await readFrame(firstReader)).toContain('"delta":{}')
    expect(await readFrame(secondReader)).toContain('"delta":{}')

    scheduler.advanceBy(coalescerConfig.quietWindowMs)
    await flushMicrotasks()
    const [firstTail, secondTail] = await Promise.all([
      readRemaining(firstReader),
      readRemaining(secondReader),
    ])

    expect(modelCalls).toBe(1)
    expect(observedInferences[0]?.contents.at(-1)?.parts[0]?.text).toBe(
      "Could you explain Khala?",
    )
    expect(firstTail).toContain('"content":"Shared "')
    expect(firstTail).toContain('"content":"answer"')
    expect(secondTail).toContain('"content":"Shared "')
    expect(secondTail).toContain('"content":"answer"')
    expect(firstTail).toContain('"terminal":"complete"')
    expect(firstTail).toContain("data: [DONE]")
    expect(firstRecorderCalls).toBe(0)
    expect(latestRecorderCalls).toBe(1)
    expect(completions).toHaveLength(1)
    expect(completions[0]).toMatchObject({
      scope,
      turnRef: "turn.coalesced.1",
      userText: "Could you explain Khala?",
      assistantText: "Shared answer",
    })
    expect(repeater.taskCount).toBe(0)
  })

  test("late exact-turn replay receives ordered history and live tail without another model call", async () => {
    const scheduler = new ManualScheduler()
    const repeater = new ManualRepeater()
    let modelCalls = 0
    let release!: () => void
    const gate = new Promise<void>((resolve) => {
      release = resolve
    })
    const coordinator = makeSarahVoiceStreamCoordinator({
      coalescerConfig,
      coalescerDependencies: scheduler,
      fanoutConfig,
      fanoutDependencies: scheduler,
      makeTurnRef: () => "turn.replay",
      fallbackReply: fixedFallback,
      streamReply: async function* () {
        modelCalls += 1
        yield { type: "delta", text: "before" }
        await gate
        yield { type: "delta", text: "-after" }
      },
    })
    let records = 0
    const first = openSarahTrustedVoiceStream({
      coordinator,
      model: "test-model",
      scope,
      fragment: "one cumulative request",
      inference: inference("one cumulative request"),
      publishAndRecord: () => {
        records += 1
      },
      sseDependencies: repeater,
    })
    const firstReader = first.body!.getReader()
    await readFrame(firstReader)
    scheduler.advanceBy(coalescerConfig.quietWindowMs)
    await flushMicrotasks()
    expect(await readFrame(firstReader)).toContain('"content":"before"')

    const replay = openSarahTrustedVoiceStream({
      coordinator,
      model: "test-model",
      scope,
      fragment: "ignored during replay",
      inference: inference("ignored during replay"),
      publishAndRecord: () => {
        records += 100
      },
      replayTurnRef: "turn.replay",
      afterSequence: 0,
      sseDependencies: repeater,
    })
    const replayReader = replay.body!.getReader()
    expect(await readFrame(replayReader)).toContain('"role":"assistant"')
    expect(await readFrame(replayReader)).toContain('"content":"before"')

    release()
    await flushMicrotasks()
    const [firstTail, replayTail] = await Promise.all([
      readRemaining(firstReader),
      readRemaining(replayReader),
    ])
    expect(firstTail).toContain('"content":"-after"')
    expect(replayTail).toContain('"content":"-after"')
    expect(modelCalls).toBe(1)
    expect(records).toBe(1)
  })

  test("one controller disconnect detaches only itself while the shared producer records once", async () => {
    const scheduler = new ManualScheduler()
    const repeater = new ManualRepeater()
    let modelCalls = 0
    let records = 0
    const coordinator = makeSarahVoiceStreamCoordinator({
      coalescerConfig,
      coalescerDependencies: scheduler,
      fanoutConfig,
      fanoutDependencies: scheduler,
      makeTurnRef: () => "turn.disconnect",
      fallbackReply: fixedFallback,
      streamReply: async function* () {
        modelCalls += 1
        yield { type: "delta", text: "survives" }
      },
    })
    const open = () =>
      openSarahTrustedVoiceStream({
        coordinator,
        model: "test-model",
        scope,
        fragment: "same pending cumulative fragment",
        inference: inference("same pending cumulative fragment"),
        publishAndRecord: () => {
          records += 1
        },
        sseDependencies: repeater,
      })
    const disconnected = open()
    const survivor = open()
    const disconnectedReader = disconnected.body!.getReader()
    const survivorReader = survivor.body!.getReader()
    await readFrame(disconnectedReader)
    await readFrame(survivorReader)
    await disconnectedReader.cancel()

    scheduler.advanceBy(coalescerConfig.quietWindowMs)
    await flushMicrotasks()
    const survivorTail = await readRemaining(survivorReader)
    expect(survivorTail).toContain('"content":"survives"')
    expect(survivorTail).toContain("data: [DONE]")
    expect(modelCalls).toBe(1)
    expect(records).toBe(1)
  })

  test("late replay requires the exact prospect and conversation scope without enumeration", async () => {
    const scheduler = new ManualScheduler()
    const coordinator = makeSarahVoiceStreamCoordinator({
      coalescerConfig,
      coalescerDependencies: scheduler,
      fanoutConfig,
      fanoutDependencies: scheduler,
      makeTurnRef: () => "turn.scope",
      fallbackReply: fixedFallback,
      streamReply: async function* () {
        yield { type: "delta", text: "scoped" }
      },
    })
    const response = openSarahTrustedVoiceStream({
      coordinator,
      model: "test-model",
      scope,
      fragment: "scope test",
      inference: inference("scope test"),
      publishAndRecord: () => {},
      sseDependencies: new ManualRepeater(),
    })
    scheduler.advanceBy(coalescerConfig.quietWindowMs)
    await flushMicrotasks()
    await response.text()

    for (const foreignScope of [
      { ...scope, prospectRef: "prospect:foreign" },
      { ...scope, conversationRef: "conversation:foreign" },
    ]) {
      let failure: unknown
      try {
        coordinator.open({
          scope: foreignScope,
          fragment: "ignored",
          inference: inference("ignored"),
          publishAndRecord: () => {},
          replayTurnRef: "turn.scope",
        })
      } catch (error) {
        failure = error
      }
      expect(failure).toBeInstanceOf(SarahConversationStreamFanoutError)
      expect(failure).toMatchObject({ reason: "stream_not_available" })
      expect(JSON.stringify(failure)).not.toContain("trusted-owner")
      expect(JSON.stringify(failure)).not.toContain("foreign")
    }
  })

  test("a rejected later fragment cannot replace inference, user text, or recorder authority", async () => {
    const scheduler = new ManualScheduler()
    const completions: SarahTrustedVoiceCompletion[] = []
    let firstRecorderCalls = 0
    let rejectedRecorderCalls = 0
    let observedUser = ""
    const coordinator = makeSarahVoiceStreamCoordinator({
      coalescerConfig,
      coalescerDependencies: scheduler,
      fanoutConfig,
      fanoutDependencies: scheduler,
      makeTurnRef: () => "turn.acceptance",
      fallbackReply: fixedFallback,
      streamReply: async function* (request) {
        observedUser = request.contents.at(-1)?.parts[0]?.text ?? ""
        yield { type: "delta", text: "canonical" }
      },
    })
    const accepted = coordinator.open({
      scope,
      fragment: "accepted cumulative fragment",
      inference: inference("accepted cumulative fragment"),
      publishAndRecord: (completion) => {
        firstRecorderCalls += 1
        completions.push(completion)
      },
    })
    expect(() =>
      coordinator.open({
        scope,
        fragment: "   ",
        inference: inference("must never become inference"),
        publishAndRecord: () => {
          rejectedRecorderCalls += 1
        },
      }),
    ).toThrow(SarahVoiceStreamCoordinatorError)

    scheduler.advanceBy(coalescerConfig.quietWindowMs)
    await flushMicrotasks()
    await accepted.subscriber.next()
    await accepted.subscriber.next()
    expect(observedUser).toBe("accepted cumulative fragment")
    expect(firstRecorderCalls).toBe(1)
    expect(rejectedRecorderCalls).toBe(0)
    expect(completions[0]?.userText).toBe("accepted cumulative fragment")
  })

  test("an invalid-open flood allocates no fanout turns and cannot block the next valid turn", async () => {
    const scheduler = new ManualScheduler()
    const repeater = new ManualRepeater()
    let modelCalls = 0
    let records = 0
    const coordinator = makeSarahVoiceStreamCoordinator({
      coalescerConfig,
      coalescerDependencies: scheduler,
      fanoutConfig,
      fanoutDependencies: scheduler,
      makeTurnRef: () => "turn.invalid-first",
      fallbackReply: fixedFallback,
      streamReply: async function* () {
        modelCalls += 1
        yield { type: "delta", text: "never" }
      },
    })

    for (let index = 0; index < 32; index += 1) {
      expect(() =>
        openSarahTrustedVoiceStream({
          coordinator,
          model: "test-model",
          scope,
          fragment: "   ",
          inference: inference("must never execute"),
          publishAndRecord: () => {
            records += 1
          },
          sseDependencies: repeater,
        }),
      ).toThrow(SarahVoiceStreamCoordinatorError)
    }
    await flushMicrotasks()
    expect(modelCalls).toBe(0)
    expect(records).toBe(0)
    expect(repeater.taskCount).toBe(0)
    expect(coordinator.snapshot()).toMatchObject({
      pendingTurns: 0,
      executingTurns: 0,
      fanoutTurns: 0,
      subscribers: 0,
    })

    const valid = openSarahTrustedVoiceStream({
      coordinator,
      model: "test-model",
      scope,
      fragment: "valid after rejected flood",
      inference: inference("valid after rejected flood"),
      publishAndRecord: () => {
        records += 1
      },
      sseDependencies: repeater,
    })
    scheduler.advanceBy(coalescerConfig.quietWindowMs)
    await flushMicrotasks()
    expect(await valid.text()).toContain('"content":"never"')
    expect(modelCalls).toBe(1)
    expect(records).toBe(1)
    await coordinator.close()
    expect(scheduler.taskCount).toBe(0)
  })

  test("no-delta fallback records fixed copy once and never echoes a provider secret", async () => {
    const scheduler = new ManualScheduler()
    const repeater = new ManualRepeater()
    const secret = "SECRET-provider-no-delta-payload"
    const completions: SarahTrustedVoiceCompletion[] = []
    const coordinator = makeSarahVoiceStreamCoordinator({
      coalescerConfig,
      coalescerDependencies: scheduler,
      fanoutConfig,
      fanoutDependencies: scheduler,
      makeTurnRef: () => "turn.fallback",
      fallbackReply: fixedFallback,
      streamReply: async function* () {
        yield { type: "error", error: secret }
      },
    })
    const response = openSarahTrustedVoiceStream({
      coordinator,
      model: "test-model",
      scope,
      fragment: "fallback request",
      inference: inference("fallback request"),
      publishAndRecord: (completion) => {
        completions.push(completion)
      },
      sseDependencies: repeater,
    })
    scheduler.advanceBy(coalescerConfig.quietWindowMs)
    await flushMicrotasks()
    const text = await response.text()
    expect(text).toContain("Fixed unavailable fallback.")
    expect(text).toContain('"terminal":"complete"')
    expect(text).not.toContain(secret)
    expect(completions).toHaveLength(1)
    expect(completions[0]?.assistantText).toBe("Fixed unavailable fallback.")
  })

  test("thrown provider failure is typed, fixed, non-recording, and secret-free", async () => {
    const scheduler = new ManualScheduler()
    const secret = "SECRET-thrown-provider-failure"
    let records = 0
    const coordinator = makeSarahVoiceStreamCoordinator({
      coalescerConfig,
      coalescerDependencies: scheduler,
      fanoutConfig,
      fanoutDependencies: scheduler,
      makeTurnRef: () => "turn.provider-failure",
      fallbackReply: fixedFallback,
      streamReply: async function* (): AsyncGenerator<GemmaStreamEvent> {
        throw new Error(secret)
      },
    })
    const response = openSarahTrustedVoiceStream({
      coordinator,
      model: "test-model",
      scope,
      fragment: "provider failure request",
      inference: inference("provider failure request"),
      publishAndRecord: () => {
        records += 1
      },
      sseDependencies: new ManualRepeater(),
    })
    scheduler.advanceBy(coalescerConfig.quietWindowMs)
    await flushMicrotasks()
    const text = await response.text()
    expect(text).toContain("I'm having trouble reaching my model right now")
    expect(text).toContain('"reason":"producer_failed"')
    expect(text).not.toContain(secret)
    expect(records).toBe(0)
  })

  test("overflow aborts the provider, emits bounded typed SSE, and cannot record rejected secret bytes", async () => {
    const scheduler = new ManualScheduler()
    const secret = "SECRET-overflow-provider-chunk"
    let records = 0
    let providerSawAbort = false
    const coordinator = makeSarahVoiceStreamCoordinator({
      coalescerConfig,
      coalescerDependencies: scheduler,
      fanoutConfig: { ...fanoutConfig, maxEventBytes: 4 },
      fanoutDependencies: scheduler,
      makeTurnRef: () => "turn.overflow",
      fallbackReply: fixedFallback,
      streamReply: async function* (_request, signal) {
        try {
          yield { type: "delta", text: secret }
        } finally {
          providerSawAbort = signal.aborted
        }
      },
    })
    const response = openSarahTrustedVoiceStream({
      coordinator,
      model: "test-model",
      scope,
      fragment: "overflow request",
      inference: inference("overflow request"),
      publishAndRecord: () => {
        records += 1
      },
      sseDependencies: new ManualRepeater(),
    })
    scheduler.advanceBy(coalescerConfig.quietWindowMs)
    await flushMicrotasks()
    const text = await response.text()
    expect(text).toContain('"kind":"overflow"')
    expect(text).toContain('"limit":"event_bytes"')
    expect(text).toContain("I'm having trouble reaching my model right now")
    expect(text).not.toContain(secret)
    expect(providerSawAbort).toBe(true)
    expect(records).toBe(0)
  })

  test("record timeout aborts the recorder signal and remains a typed unknown completion", async () => {
    const scheduler = new ManualScheduler()
    let recorderSignal: AbortSignal | undefined
    let recordAttempts = 0
    const coordinator = makeSarahVoiceStreamCoordinator({
      coalescerConfig,
      coalescerDependencies: scheduler,
      fanoutConfig,
      fanoutDependencies: scheduler,
      makeTurnRef: () => "turn.record-timeout",
      fallbackReply: fixedFallback,
      streamReply: async function* () {
        yield { type: "delta", text: "delivered" }
      },
    })
    const response = openSarahTrustedVoiceStream({
      coordinator,
      model: "test-model",
      scope,
      fragment: "record timeout request",
      inference: inference("record timeout request"),
      publishAndRecord: (_completion, signal) => {
        recordAttempts += 1
        recorderSignal = signal
        return new Promise<void>(() => {})
      },
      sseDependencies: new ManualRepeater(),
    })
    scheduler.advanceBy(coalescerConfig.quietWindowMs)
    await flushMicrotasks()
    expect(recorderSignal?.aborted).toBe(false)
    scheduler.advanceBy(fanoutConfig.recordTimeoutMs)
    await flushMicrotasks()
    const text = await response.text()
    expect(text).toContain('"reason":"record_timeout"')
    expect(text).toContain('"content":"delivered"')
    expect(recorderSignal?.aborted).toBe(true)
    expect(recordAttempts).toBe(1)
  })

  test("keepalive configuration and scheduler failures are bounded before a response can escape", async () => {
    const scheduler = new ManualScheduler()
    let turn = 0
    const coordinator = makeSarahVoiceStreamCoordinator({
      coalescerConfig: { ...coalescerConfig, maxFragmentsPerGroup: 16 },
      coalescerDependencies: scheduler,
      fanoutConfig,
      fanoutDependencies: scheduler,
      makeTurnRef: () => `turn.scheduler.${++turn}`,
      fallbackReply: fixedFallback,
      streamReply: async function* () {
        yield { type: "delta", text: "safe" }
      },
    })
    let records = 0
    const base = {
      coordinator,
      model: "test-model",
      scope,
      fragment: "scheduler request",
      inference: inference("scheduler request"),
      publishAndRecord: () => {
        records += 1
      },
    } as const
    const before = coordinator.snapshot()

    for (const keepaliveMs of [0, Number.NaN, Number.POSITIVE_INFINITY, 60_001]) {
      let failure: unknown
      try {
        openSarahTrustedVoiceStream({
          ...base,
          sseDependencies: {
            keepaliveMs,
            repeat: () => ({ cancel: () => {} }),
          },
        })
      } catch (error) {
        failure = error
      }
      expect(failure).toBeInstanceOf(SarahVoiceStreamSseError)
      expect(failure).toMatchObject({ reason: "invalid_keepalive" })
    }

    let schedulerFailure: unknown
    try {
      openSarahTrustedVoiceStream({
        ...base,
        sseDependencies: {
          keepaliveMs: 2_000,
          repeat: () => {
            throw new Error("SECRET-scheduler-payload")
          },
        },
      })
    } catch (error) {
      schedulerFailure = error
    }
    expect(schedulerFailure).toBeInstanceOf(SarahVoiceStreamSseError)
    expect(schedulerFailure).toMatchObject({ reason: "scheduler_failed" })
    expect(JSON.stringify(schedulerFailure)).not.toContain("SECRET")
    expect(coordinator.snapshot()).toEqual(before)
    expect(records).toBe(0)
    expect(turn).toBe(0)
    await coordinator.close()
    expect(scheduler.taskCount).toBe(0)
  })

  test("custom and replay turn refs are validated before they can reach response headers", () => {
    const scheduler = new ManualScheduler()
    const badTurnRef = "turn.invalid\nSECRET-header"
    const coordinator = makeSarahVoiceStreamCoordinator({
      coalescerConfig,
      coalescerDependencies: scheduler,
      fanoutConfig,
      fanoutDependencies: scheduler,
      makeTurnRef: () => badTurnRef,
      fallbackReply: fixedFallback,
      streamReply: async function* () {
        yield { type: "delta", text: "never" }
      },
    })
    let generatedFailure: unknown
    try {
      openSarahTrustedVoiceStream({
        coordinator,
        model: "test-model",
        scope,
        fragment: "invalid generated turn",
        inference: inference("invalid generated turn"),
        publishAndRecord: () => {},
        sseDependencies: new ManualRepeater(),
      })
    } catch (error) {
      generatedFailure = error
    }
    expect(generatedFailure).toBeInstanceOf(SarahConversationStreamFanoutError)
    expect(generatedFailure).toMatchObject({ reason: "invalid_scope" })
    expect(JSON.stringify(generatedFailure)).not.toContain("SECRET")

    let replayFailure: unknown
    try {
      openSarahTrustedVoiceStream({
        coordinator,
        model: "test-model",
        scope,
        fragment: "ignored replay",
        inference: inference("ignored replay"),
        publishAndRecord: () => {},
        replayTurnRef: badTurnRef,
        sseDependencies: new ManualRepeater(),
      })
    } catch (error) {
      replayFailure = error
    }
    expect(replayFailure).toBeInstanceOf(SarahConversationStreamFanoutError)
    expect(replayFailure).toMatchObject({ reason: "invalid_scope" })
    expect(JSON.stringify(replayFailure)).not.toContain("SECRET")
  })

  test("the existing bearer route retains its original fixed-stream byte shape", async () => {
    const previousBearer = process.env.SARAH_AVATAR_LLM_BEARER
    process.env.SARAH_AVATAR_LLM_BEARER = "route-byte-test"
    try {
      const response = await handleSarahChatCompletions(
        new Request("http://localhost/sarah/api/llm/chat/completions", {
          method: "POST",
          headers: {
            authorization: "Bearer route-byte-test",
            "content-type": "application/json",
          },
          body: JSON.stringify({
            stream: true,
            messages: [
              { role: "system", content: "You are Sarah." },
              { role: "user", content: "Can I get a discount?" },
            ],
          }),
        }),
      )
      expect(response.headers.get("x-sarah-turn-ref")).toBeNull()
      const text = await response.text()
      expect(text).not.toContain('"openagents"')
      const frames = text
        .split("\n")
        .filter((line) => line.startsWith("data: "))
      expect(frames.at(-1)).toBe("data: [DONE]")
      const payloads = frames.slice(0, -1).map((line) =>
        JSON.parse(line.slice("data: ".length)) as Record<string, unknown>,
      )
      expect(payloads).toHaveLength(3)
      expect(payloads.map((payload) => payload.choices)).toEqual([
        [{ index: 0, delta: { role: "assistant" }, finish_reason: null }],
        [
          {
            index: 0,
            delta: {
              content:
                "I only quote public pack prices and owner-approved parameters — I won't improvise discounts. I can evaluate deal rules or open a human handoff.",
            },
            finish_reason: null,
          },
        ],
        [{ index: 0, delta: {}, finish_reason: "stop" }],
      ])
    } finally {
      if (previousBearer === undefined) {
        delete process.env.SARAH_AVATAR_LLM_BEARER
      } else {
        process.env.SARAH_AVATAR_LLM_BEARER = previousBearer
      }
    }
  })
})
