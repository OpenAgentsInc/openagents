import { describe, expect, test } from "vite-plus/test";
import {
  SarahProviderAccounting,
  SarahProviderAttestation,
  SarahGenerationFence,
  admittedRealtimeProvider,
  closeAfterProviderAccounting,
  responseUsageEvent,
  retrySarahLiveKitWorkerClaim,
  sarahProviderConfigurationDigest,
  type SarahRealtimeProviderProfile,
  transcriptionUsageEvent,
  waitForAdmissionUntil,
} from "./generation.js";

const identity = {
  sessionRef: "session:one",
  generation: 4,
  jobRef: "job:one",
} as const;

const privateProviderProfile = {
  instructions: "Private Sarah instructions",
  tools: [
    {
      type: "function",
      name: "editor_context_read",
      description: "Read one admitted editor range.",
      parameters: {
        type: "object",
        properties: { startLine: { type: "integer", minimum: 1 } },
        required: ["startLine"],
        additionalProperties: false,
      },
    },
  ],
  toolChoice: "auto",
} as const satisfies SarahRealtimeProviderProfile;

const communityProviderProfile = {
  instructions: "Community Sarah instructions",
  tools: [],
  toolChoice: "auto",
} as const satisfies SarahRealtimeProviderProfile;

describe("Sarah LiveKit generation fence", () => {
  test("admits only the exact server-confirmed Realtime configuration", () => {
    const accounting = new SarahProviderAccounting();
    accounting.observe(
      {
        type: "session.created",
        session: { id: "sess_one", model: "gpt-realtime-2.1" },
      },
      false,
    );
    const event = providerUpdatedEvent();
    const admitted = admittedRealtimeProvider(
      event,
      accounting.providerSessionRefDigest,
      privateProviderProfile,
    );
    expect(admitted).toMatchObject({
      providerSessionRefDigest: expect.stringMatching(/^[a-f0-9]{64}$/u),
      providerConfigurationDigest: expect.stringMatching(/^[a-f0-9]{64}$/u),
    });
    expect(
      admittedRealtimeProvider(
        {
          ...event,
          session: {
            ...event.session,
            audio: {
              ...event.session.audio,
              output: { ...event.session.audio.output, voice: "alloy" },
            },
          },
        },
        accounting.providerSessionRefDigest,
        privateProviderProfile,
      ),
    ).toBe(false);
    expect(
      admittedRealtimeProvider(
        {
          ...event,
          session: {
            ...event.session,
            audio: {
              ...event.session.audio,
              input: {
                ...event.session.audio.input,
                turn_detection: {
                  ...event.session.audio.input.turn_detection,
                  interrupt_response: false,
                },
              },
            },
          },
        },
        accounting.providerSessionRefDigest,
        privateProviderProfile,
      ),
    ).toBe(false);
    expect(
      admittedRealtimeProvider(
        { type: "response.created" },
        accounting.providerSessionRefDigest,
        privateProviderProfile,
      ),
    ).toBeUndefined();
  });

  test("attests exact private and community prompts, tools, schemas, and tool choice", () => {
    const sessionDigest = createProviderSessionDigest();
    const privateEvent = providerUpdatedEvent(privateProviderProfile);
    const privateAdmission = admittedRealtimeProvider(
      privateEvent,
      sessionDigest,
      privateProviderProfile,
    );
    const communityAdmission = admittedRealtimeProvider(
      providerUpdatedEvent(communityProviderProfile),
      sessionDigest,
      communityProviderProfile,
    );
    expect(privateAdmission).toMatchObject({
      providerConfigurationDigest: sarahProviderConfigurationDigest(privateProviderProfile),
    });
    expect(communityAdmission).toMatchObject({
      providerConfigurationDigest: sarahProviderConfigurationDigest(communityProviderProfile),
    });
    expect(privateAdmission).not.toEqual(communityAdmission);
    expect(JSON.stringify(privateAdmission)).not.toContain(privateProviderProfile.instructions);

    for (const session of [
      { ...privateEvent.session, instructions: "Changed instructions" },
      {
        ...privateEvent.session,
        tools: [
          {
            ...privateEvent.session.tools[0],
            parameters: { type: "object", properties: {} },
          },
        ],
      },
      { ...privateEvent.session, tool_choice: "none" },
    ]) {
      expect(
        admittedRealtimeProvider(
          { ...privateEvent, session },
          sessionDigest,
          privateProviderProfile,
        ),
      ).toBe(false);
    }
    expect(admittedRealtimeProvider(privateEvent, sessionDigest, communityProviderProfile)).toBe(
      false,
    );
    expect(
      admittedRealtimeProvider(
        { ...privateEvent, session: { ...privateEvent.session, tools: [] } },
        sessionDigest,
        privateProviderProfile,
      ),
    ).toBeUndefined();
    expect(
      admittedRealtimeProvider(
        { ...privateEvent, session: { ...privateEvent.session, tools: [] } },
        sessionDigest,
        privateProviderProfile,
        false,
      ),
    ).toBe(false);
  });

  test("accepts only OpenAI's top-level Draft-07 marker normalization", () => {
    const sessionDigest = createProviderSessionDigest();
    const profile = {
      ...privateProviderProfile,
      tools: privateProviderProfile.tools.map((providerTool) => ({
        ...providerTool,
        parameters: {
          $schema: "http://json-schema.org/draft-07/schema#",
          ...providerTool.parameters,
        },
      })),
    };
    const event = providerUpdatedEvent(profile);
    event.session.tools = event.session.tools.map((providerTool) => {
      const parameters = providerTool.parameters as Record<string, unknown>;
      const { $schema: _schema, ...normalizedParameters } = parameters;
      return { ...providerTool, parameters: normalizedParameters };
    });

    expect(admittedRealtimeProvider(event, sessionDigest, profile)).not.toBe(false);
    expect(
      admittedRealtimeProvider(
        {
          ...event,
          session: {
            ...event.session,
            tools: event.session.tools.map((providerTool) => ({
              ...providerTool,
              parameters: {
                ...(providerTool.parameters as Record<string, unknown>),
                $schema: "https://json-schema.org/draft/2020-12/schema",
              },
            })),
          },
        },
        sessionDigest,
        profile,
      ),
    ).toBe(false);
  });

  test("rejects privacy, media, and cost policy mutations", () => {
    const sessionDigest = createProviderSessionDigest();
    const event = providerUpdatedEvent(communityProviderProfile);
    const mutations = [
      { ...event.session, type: "transcription" },
      { ...event.session, max_output_tokens: 1 },
      { ...event.session, tracing: "auto" },
      { ...event.session, prompt: { id: "pmpt_untrusted" } },
      { ...event.session, truncation: "disabled" },
      { ...event.session, reasoning: { effort: "high" } },
      { ...event.session, include: ["item.input_audio_transcription.logprobs"] },
      { ...event.session, temperature: 1.2 },
      {
        ...event.session,
        audio: {
          ...event.session.audio,
          output: { ...event.session.audio.output, speed: 1.5 },
        },
      },
      {
        ...event.session,
        audio: {
          ...event.session.audio,
          input: {
            ...event.session.audio.input,
            noise_reduction: { type: "far_field" },
          },
        },
      },
    ];
    for (const session of mutations) {
      expect(
        admittedRealtimeProvider({ ...event, session }, sessionDigest, communityProviderProfile),
      ).toBe(false);
    }
  });

  test("invalidates stale startup evidence and rejects post-admission drift", () => {
    const sessionDigest = createProviderSessionDigest();
    const attestation = new SarahProviderAttestation();
    const loadingTools = providerUpdatedEvent(privateProviderProfile);
    loadingTools.session.tools = [];
    expect(attestation.observe(loadingTools, sessionDigest, privateProviderProfile)).toEqual({
      state: "pending",
    });

    const matching = providerUpdatedEvent(privateProviderProfile);
    const candidate = attestation.observe(matching, sessionDigest, privateProviderProfile);
    expect(candidate.state).toBe("candidate");
    if (candidate.state !== "candidate") {
      throw new Error("The provider candidate was unavailable");
    }

    const changedInstructions = {
      ...matching,
      session: { ...matching.session, instructions: "Hostile replacement" },
    };
    expect(attestation.observe(changedInstructions, sessionDigest, privateProviderProfile)).toEqual(
      { state: "mismatch" },
    );
    expect(attestation.markDurable(candidate.admission)).toBe(false);

    const replacement = attestation.observe(matching, sessionDigest, privateProviderProfile);
    if (replacement.state !== "candidate") {
      throw new Error("The replacement provider candidate was unavailable");
    }
    expect(attestation.markDurable(replacement.admission)).toBe(true);
    expect(attestation.observe(changedInstructions, sessionDigest, privateProviderProfile)).toEqual(
      { state: "drift" },
    );
  });

  test("correlates the finite provider startup sequence before admitting media", () => {
    const sessionDigest = createProviderSessionDigest();
    const attestation = new SarahProviderAttestation();

    expect(attestation.observeClientEvent(startupBaseClientEvent(), privateProviderProfile)).toBe(
      true,
    );
    expect(
      attestation.observeClientEvent(
        startupInstructionsClientEvent(privateProviderProfile),
        privateProviderProfile,
      ),
    ).toBe(true);
    expect(
      attestation.observeClientEvent(
        startupToolsClientEvent(privateProviderProfile),
        privateProviderProfile,
      ),
    ).toBe(true);

    expect(
      attestation.observe(
        providerUpdatedEvent({
          instructions: "OpenAI default instructions",
          tools: [],
          toolChoice: "auto",
        }),
        sessionDigest,
        privateProviderProfile,
      ),
    ).toEqual({ state: "pending" });
    expect(
      attestation.observe(
        providerUpdatedEvent({ ...privateProviderProfile, tools: [] }),
        sessionDigest,
        privateProviderProfile,
      ),
    ).toEqual({ state: "pending" });
    const candidate = attestation.observe(
      providerUpdatedEvent(privateProviderProfile),
      sessionDigest,
      privateProviderProfile,
    );
    expect(candidate.state).toBe("candidate");
    if (candidate.state !== "candidate") {
      throw new Error("The correlated provider candidate was unavailable");
    }
    expect(attestation.markDurable(candidate.admission)).toBe(true);
  });

  test("permits only correlated tool-free follow-up and exact restoration", () => {
    const sessionDigest = createProviderSessionDigest();
    const attestation = admittedAttestation(sessionDigest, privateProviderProfile);

    expect(
      attestation.observeClientEvent(toolChoiceClientEvent("none"), privateProviderProfile),
    ).toBe(true);
    expect(
      attestation.observe(
        providerUpdatedEvent({ ...privateProviderProfile, toolChoice: "none" }),
        sessionDigest,
        privateProviderProfile,
      ),
    ).toMatchObject({ state: "confirmed" });
    expect(
      attestation.observeClientEvent(toolChoiceClientEvent("auto"), privateProviderProfile),
    ).toBe(true);
    expect(
      attestation.observe(
        providerUpdatedEvent(privateProviderProfile),
        sessionDigest,
        privateProviderProfile,
      ),
    ).toMatchObject({ state: "confirmed" });
  });

  test("rejects uncommanded provider drift and unexpected outbound updates", () => {
    const sessionDigest = createProviderSessionDigest();
    const attestation = admittedAttestation(sessionDigest, privateProviderProfile);

    expect(
      attestation.observe(
        providerUpdatedEvent({ ...privateProviderProfile, toolChoice: "none" }),
        sessionDigest,
        privateProviderProfile,
      ),
    ).toEqual({ state: "drift" });

    const fresh = admittedAttestation(sessionDigest, privateProviderProfile);
    expect(
      fresh.observeClientEvent(
        {
          type: "session.update",
          event_id: "hostile_update",
          session: { type: "realtime", instructions: "Hostile replacement" },
        },
        privateProviderProfile,
      ),
    ).toBe(false);
  });

  test("extracts exact response.done token details without transcript content", () => {
    const event = responseUsageEvent(
      {
        type: "response.done",
        response: {
          id: "resp_1",
          status: "cancelled",
          output: [{ transcript: "must not escape" }],
          usage: {
            input_tokens: 12,
            output_tokens: 7,
            input_token_details: {
              cached_tokens: 3,
              audio_tokens: 8,
            },
            output_token_details: { audio_tokens: 5 },
          },
        },
      },
      identity,
    );
    expect(event).toEqual({
      schema: "openagents.sarah.livekit-worker.v1",
      _tag: "response_usage",
      ...identity,
      eventRef: "response:resp_1",
      providerResponseRef: "resp_1",
      status: "cancelled",
      inputTokens: 12,
      outputTokens: 7,
      cachedInputTokens: 3,
      audioInputTokens: 8,
      audioOutputTokens: 5,
    });
    expect(JSON.stringify(event)).not.toContain("must not escape");
  });

  test("accounts transcription separately and rejects usage-free events", () => {
    expect(
      transcriptionUsageEvent(
        {
          type: "conversation.item.input_audio_transcription.completed",
          item_id: "item_1",
          transcript: "private words",
          usage: {
            input_tokens: 10,
            output_tokens: 4,
            input_token_details: { audio_tokens: 10 },
            output_token_details: {},
          },
        },
        identity,
      ),
    ).toEqual({
      schema: "openagents.sarah.livekit-worker.v1",
      _tag: "transcription_usage",
      ...identity,
      eventRef: "transcription:item_1",
      providerTranscriptionRef: "item_1",
      inputTokens: 10,
      outputTokens: 4,
      cachedInputTokens: 0,
      audioInputTokens: 10,
      audioOutputTokens: 0,
    });
    expect(
      transcriptionUsageEvent(
        {
          type: "conversation.item.input_audio_transcription.completed",
          item_id: "item_2",
          transcript: "not retained",
        },
        identity,
      ),
    ).toBeUndefined();
  });

  test("keeps provider and event references inside the shared 256-character boundary", () => {
    const responseAtLimit = "r".repeat(247);
    const transcriptionAtLimit = "t".repeat(242);
    const usage = {
      input_tokens: 1,
      output_tokens: 0,
    };
    expect(
      responseUsageEvent(
        {
          type: "response.done",
          response: { id: responseAtLimit, status: "completed", usage },
        },
        identity,
      )?.eventRef,
    ).toHaveLength(256);
    expect(
      responseUsageEvent(
        {
          type: "response.done",
          response: { id: `${responseAtLimit}x`, status: "completed", usage },
        },
        identity,
      ),
    ).toBeUndefined();
    expect(
      transcriptionUsageEvent(
        {
          type: "conversation.item.input_audio_transcription.completed",
          item_id: transcriptionAtLimit,
          usage,
        },
        identity,
      )?.eventRef,
    ).toHaveLength(256);
    expect(
      transcriptionUsageEvent(
        {
          type: "conversation.item.input_audio_transcription.completed",
          item_id: `${transcriptionAtLimit}x`,
          usage,
        },
        identity,
      ),
    ).toBeUndefined();
  });

  test("settles a generation once and drains tracked accounting", async () => {
    const fence = new SarahGenerationFence();
    let completedBeforeSettlement = false;
    fence.track(
      Promise.resolve().then(() => {
        completedBeforeSettlement = true;
      }),
    );
    expect(fence.settle("provider_disconnect")).toBe(true);
    expect(fence.settle("completed")).toBe(false);
    const finalUsage = responseUsageEvent(
      {
        type: "response.done",
        response: {
          id: "resp_final",
          status: "failed",
          usage: {
            input_tokens: 8,
            output_tokens: 2,
          },
        },
      },
      identity,
    );
    if (finalUsage === undefined) {
      throw new Error("The final usage fixture was not admitted");
    }
    expect(fence.accepts(finalUsage)).toBe(true);
    let completedAfterSettlement = false;
    fence.track(
      Promise.resolve().then(() => {
        completedAfterSettlement = true;
      }),
    );
    fence.seal();
    expect(fence.accepts(finalUsage)).toBe(false);
    await fence.drain();
    expect(completedBeforeSettlement).toBe(true);
    expect(completedAfterSettlement).toBe(true);
    expect(fence.closeReason).toBe("provider_disconnect");
  });

  test("waits for a late provider accounting event before the terminal event", async () => {
    const fence = new SarahGenerationFence();
    const accounting = new SarahProviderAccounting();
    const order: string[] = [];
    let idleChecks = 0;
    fence.settle("provider_disconnect");

    await closeAfterProviderAccounting(
      fence,
      accounting,
      async () => {
        order.push("provider_drain_requested");
      },
      async () => {
        order.push("provider_closed");
      },
      async (accountingStatus) => {
        order.push(`terminal_event:${accountingStatus}`);
      },
      async () => {
        idleChecks += 1;
        if (idleChecks !== 1) return;
        fence.observeProviderEvent();
        const finalUsage = Promise.resolve().then(() => {
          order.push("final_usage");
        });
        fence.track(finalUsage);
      },
    );

    expect(order).toEqual([
      "provider_drain_requested",
      "provider_closed",
      "final_usage",
      "terminal_event:exact",
    ]);
    expect(fence.accepts(closeEventForTest)).toBe(false);
  });

  test("does not finish cancellation accounting until response.done usage arrives", async () => {
    const accounting = new SarahProviderAccounting();
    accounting.observe({ type: "response.created", response: { id: "resp_cancelled" } }, false);
    let releaseTimeout: (() => void) | undefined;
    const waiting = accounting.waitForTerminalResponses(
      10_000,
      () =>
        new Promise<void>((resolve) => {
          releaseTimeout = resolve;
        }),
    );
    expect(await Promise.race([waiting, Promise.resolve("pending")])).toBe("pending");
    const done = {
      type: "response.done",
      response: {
        id: "resp_cancelled",
        status: "cancelled",
        usage: { input_tokens: 4, output_tokens: 1 },
      },
    };
    const terminalResponseRef = accounting.observe(
      done,
      responseUsageEvent(done, identity) !== undefined,
    );
    expect(await Promise.race([waiting, Promise.resolve("pending")])).toBe("pending");
    if (terminalResponseRef === undefined) {
      throw new Error("The terminal response fixture was not tracked");
    }
    accounting.acknowledgeTerminalUsage(terminalResponseRef);
    expect(await waiting).toBe(true);
    releaseTimeout?.();
  });

  test("keeps the provider open for response.done later than the old idle probe", async () => {
    const fence = new SarahGenerationFence();
    const accounting = new SarahProviderAccounting();
    const order: string[] = [];
    accounting.observe({ type: "response.created", response: { id: "resp_late" } }, false);
    await closeAfterProviderAccounting(
      fence,
      accounting,
      async () => {
        order.push("cancel_sent");
        setTimeout(() => {
          const done = {
            type: "response.done",
            response: {
              id: "resp_late",
              status: "cancelled",
              usage: { input_tokens: 9, output_tokens: 2 },
            },
          };
          const terminalResponseRef = accounting.observe(
            done,
            responseUsageEvent(done, identity) !== undefined,
          );
          if (terminalResponseRef === undefined) {
            accounting.failTerminalUsageDelivery();
          } else {
            accounting.acknowledgeTerminalUsage(terminalResponseRef);
          }
          order.push("terminal_usage");
        }, 75);
      },
      async () => {
        order.push("provider_closed");
      },
      async (accountingStatus) => {
        order.push(`generation_closed:${accountingStatus}`);
      },
      async () => undefined,
    );
    expect(order).toEqual([
      "cancel_sent",
      "terminal_usage",
      "provider_closed",
      "generation_closed:exact",
    ]);
  });

  test("marks a disconnected response without response.done as uncertain", async () => {
    const fence = new SarahGenerationFence();
    const accounting = new SarahProviderAccounting();
    accounting.observe({ type: "response.created", response: { id: "resp_lost" } }, false);
    accounting.disconnect();
    let accountingStatus: "exact" | "uncertain" | undefined;

    await closeAfterProviderAccounting(
      fence,
      accounting,
      async () => undefined,
      async () => undefined,
      async (status) => {
        accountingStatus = status;
      },
      async () => undefined,
    );

    expect(accountingStatus).toBe("uncertain");
    expect(fence.closeReason).toBe("worker_error");
  });

  test("requires durable terminal usage delivery before accounting is exact", async () => {
    const accounting = new SarahProviderAccounting();
    const done = {
      type: "response.done",
      response: {
        id: "resp_delivery",
        status: "completed",
        usage: { input_tokens: 12, output_tokens: 3 },
      },
    };
    const terminalResponseRef = accounting.observe(
      done,
      responseUsageEvent(done, identity) !== undefined,
    );
    const waiting = accounting.waitForTerminalResponses(10_000, () => new Promise<void>(() => {}));
    expect(await Promise.race([waiting, Promise.resolve("pending")])).toBe("pending");
    if (terminalResponseRef === undefined) {
      throw new Error("The terminal response fixture was not tracked");
    }
    accounting.acknowledgeTerminalUsage(terminalResponseRef);
    await expect(waiting).resolves.toBe(true);
  });

  test("waits for committed transcriptions completed out of order and durably delivered", async () => {
    const accounting = new SarahProviderAccounting();
    accounting.observe({ type: "input_audio_buffer.committed", item_id: "item_first" }, false);
    accounting.observe({ type: "input_audio_buffer.committed", item_id: "item_second" }, false);

    const secondCompleted = {
      type: "conversation.item.input_audio_transcription.completed",
      item_id: "item_second",
      usage: { input_tokens: 6, output_tokens: 2 },
    };
    const secondRef = accounting.observe(secondCompleted, "transcription_usage");
    if (secondRef === undefined) throw new Error("The second transcription was not tracked");
    accounting.acknowledgeTerminalUsage(secondRef);

    const waiting = accounting.waitForTerminalUsage(10_000, () => new Promise<void>(() => {}));
    expect(await Promise.race([waiting, Promise.resolve("pending")])).toBe("pending");

    const firstCompleted = {
      type: "conversation.item.input_audio_transcription.completed",
      item_id: "item_first",
      usage: { input_tokens: 7, output_tokens: 3 },
    };
    const firstRef = accounting.observe(firstCompleted, "transcription_usage");
    if (firstRef === undefined) throw new Error("The first transcription was not tracked");
    expect(await Promise.race([waiting, Promise.resolve("pending")])).toBe("pending");
    accounting.acknowledgeTerminalUsage(firstRef);

    await expect(waiting).resolves.toBe(true);
    expect(accounting.exact).toBe(true);
  });

  test("keeps transcription accounting pending through delivery retries", async () => {
    const accounting = new SarahProviderAccounting();
    accounting.observe({ type: "input_audio_buffer.committed", item_id: "item_retry" }, false);
    const completed = {
      type: "conversation.item.input_audio_transcription.completed",
      item_id: "item_retry",
      usage: { input_tokens: 5, output_tokens: 1 },
    };
    const firstDelivery = accounting.observe(completed, "transcription_usage");
    const retriedDelivery = accounting.observe(completed, "transcription_usage");
    if (firstDelivery === undefined || retriedDelivery === undefined) {
      throw new Error("The transcription retry was not tracked");
    }

    expect(accounting.exact).toBe(false);
    accounting.acknowledgeTerminalUsage(retriedDelivery);
    expect(accounting.exact).toBe(true);

    accounting.observe({ type: "input_audio_buffer.committed", item_id: "item_retry" }, false);
    expect(accounting.exact).toBe(true);
  });

  test("marks transcription failure, timeout, and disconnect uncertain", async () => {
    const failed = new SarahProviderAccounting();
    failed.observe({ type: "input_audio_buffer.committed", item_id: "item_failed" }, false);
    failed.observe(
      {
        type: "conversation.item.input_audio_transcription.failed",
        item_id: "item_failed",
        error: { type: "server_error" },
      },
      false,
    );
    await expect(failed.waitForTerminalUsage(10_000)).resolves.toBe(false);

    const timedOut = new SarahProviderAccounting();
    timedOut.observe({ type: "input_audio_buffer.committed", item_id: "item_timed_out" }, false);
    await expect(timedOut.waitForTerminalUsage(1, async () => undefined)).resolves.toBe(false);

    const disconnected = new SarahProviderAccounting();
    disconnected.observe(
      { type: "input_audio_buffer.committed", item_id: "item_disconnected" },
      false,
    );
    disconnected.disconnect();
    await expect(disconnected.waitForTerminalUsage(10_000)).resolves.toBe(false);
  });

  test("marks failed or disconnected control delivery as uncertain after usage", async () => {
    const failedDelivery = new SarahProviderAccounting();
    const done = {
      type: "response.done",
      response: {
        id: "resp_unknown_delivery",
        status: "completed",
        usage: { input_tokens: 12, output_tokens: 3 },
      },
    };
    failedDelivery.observe(done, responseUsageEvent(done, identity) !== undefined);
    failedDelivery.failTerminalUsageDelivery();
    await expect(failedDelivery.waitForTerminalResponses(10_000)).resolves.toBe(false);

    const disconnected = new SarahProviderAccounting();
    const responseRef = disconnected.observe(
      done,
      responseUsageEvent(done, identity) !== undefined,
    );
    if (responseRef === undefined) {
      throw new Error("The terminal response fixture was not tracked");
    }
    disconnected.acknowledgeTerminalUsage(responseRef);
    disconnected.disconnect();
    await expect(disconnected.waitForTerminalResponses(10_000)).resolves.toBe(false);
  });

  test("does not report exact when the provider disconnects during close", async () => {
    const fence = new SarahGenerationFence();
    const accounting = new SarahProviderAccounting();
    let accountingStatus: "exact" | "uncertain" | undefined;

    await closeAfterProviderAccounting(
      fence,
      accounting,
      async () => undefined,
      async () => accounting.disconnect(),
      async (status) => {
        accountingStatus = status;
      },
      async () => undefined,
    );

    expect(accountingStatus).toBe("uncertain");
  });

  test("retries a delayed worker claim beyond the old two-second window", async () => {
    let attempts = 0;
    const wait = async () => undefined;
    await expect(
      retrySarahLiveKitWorkerClaim(async () => {
        attempts += 1;
        if (attempts <= 21) throw new Error("binding not committed");
        return "claimed";
      }, wait),
    ).resolves.toBe("claimed");
    expect(attempts).toBe(22);
  });

  test("bounds participant admission by both expiry and worker shutdown", async () => {
    await expect(
      waitForAdmissionUntil(
        () => new Promise(() => {}),
        99,
        new AbortController().signal,
        () => 100,
      ),
    ).rejects.toThrow("expired");
    const controller = new AbortController();
    const waiting = waitForAdmissionUntil(
      () => new Promise(() => {}),
      Date.now() + 60_000,
      controller.signal,
    );
    controller.abort();
    await expect(waiting).rejects.toThrow("aborted");
  });
});

const createProviderSessionDigest = (): string => {
  const accounting = new SarahProviderAccounting();
  accounting.observe(
    {
      type: "session.created",
      session: { id: "sess_one", model: "gpt-realtime-2.1" },
    },
    false,
  );
  const providerSessionRefDigest = accounting.providerSessionRefDigest;
  if (providerSessionRefDigest === undefined) {
    throw new Error("The test provider session digest was unavailable");
  }
  return providerSessionRefDigest;
};

const providerUpdatedEvent = (profile: SarahRealtimeProviderProfile = privateProviderProfile) => ({
  type: "session.updated",
  session: {
    id: "sess_one",
    type: "realtime",
    model: "gpt-realtime-2.1",
    output_modalities: ["audio"],
    include: [],
    max_output_tokens: "inf",
    prompt: null,
    tracing: null,
    truncation: "auto",
    reasoning: null,
    audio: {
      input: {
        format: { type: "audio/pcm", rate: 24_000 },
        noise_reduction: null,
        transcription: {
          model: "gpt-4o-mini-transcribe",
          language: null,
          prompt: null,
        },
        turn_detection: {
          type: "semantic_vad",
          eagerness: "high",
          create_response: true,
          interrupt_response: true,
        },
      },
      output: {
        format: { type: "audio/pcm", rate: 24_000 },
        speed: 1,
        voice: "marin",
      },
    },
    instructions: profile.instructions,
    tools: profile.tools,
    tool_choice: profile.toolChoice,
  },
});

const startupBaseClientEvent = () => ({
  type: "session.update",
  session: {
    type: "realtime",
    model: "gpt-realtime-2.1",
    output_modalities: ["audio"],
    audio: {
      input: {
        format: { type: "audio/pcm", rate: 24_000 },
        noise_reduction: undefined,
        transcription: { model: "gpt-4o-mini-transcribe" },
        turn_detection: {
          type: "semantic_vad",
          eagerness: "high",
          create_response: true,
          interrupt_response: true,
        },
      },
      output: {
        format: { type: "audio/pcm", rate: 24_000 },
        speed: 1,
        voice: "marin",
      },
    },
    max_output_tokens: "inf",
    tool_choice: "auto",
    tracing: null,
    instructions: undefined,
  },
});

const startupInstructionsClientEvent = (profile: SarahRealtimeProviderProfile) => ({
  type: "session.update",
  event_id: "instructions_update_one",
  session: {
    type: "realtime",
    instructions: profile.instructions,
  },
});

const startupToolsClientEvent = (profile: SarahRealtimeProviderProfile) => ({
  type: "session.update",
  event_id: "tools_update_one",
  session: {
    type: "realtime",
    model: "gpt-realtime-2.1",
    tools: profile.tools,
  },
});

const toolChoiceClientEvent = (toolChoice: "auto" | "none") => ({
  type: "session.update",
  event_id: `options_update_${toolChoice}`,
  session: {
    type: "realtime",
    tool_choice: toolChoice,
  },
});

const admittedAttestation = (
  sessionDigest: string,
  profile: SarahRealtimeProviderProfile,
): SarahProviderAttestation => {
  const attestation = new SarahProviderAttestation();
  if (
    !attestation.observeClientEvent(startupBaseClientEvent(), profile) ||
    !attestation.observeClientEvent(startupInstructionsClientEvent(profile), profile) ||
    !attestation.observeClientEvent(startupToolsClientEvent(profile), profile)
  ) {
    throw new Error("The test startup client sequence was rejected");
  }
  attestation.observe(
    providerUpdatedEvent({
      instructions: "OpenAI default instructions",
      tools: [],
      toolChoice: "auto",
    }),
    sessionDigest,
    profile,
  );
  attestation.observe(providerUpdatedEvent({ ...profile, tools: [] }), sessionDigest, profile);
  const candidate = attestation.observe(providerUpdatedEvent(profile), sessionDigest, profile);
  if (candidate.state !== "candidate" || !attestation.markDurable(candidate.admission)) {
    throw new Error("The test provider admission was not durable");
  }
  return attestation;
};

const closeEventForTest = {
  schema: "openagents.sarah.livekit-worker.v1",
  _tag: "close",
  ...identity,
  eventRef: "close:job:one",
  reason: "provider_disconnect",
  accountingStatus: "uncertain",
} as const;
