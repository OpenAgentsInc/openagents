import { describe, expect, test } from "vite-plus/test";
import {
  SarahProviderAccounting,
  SarahGenerationFence,
  admittedRealtimeProvider,
  closeAfterProviderAccounting,
  responseUsageEvent,
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
      { ...privateEvent.session, tools: [] },
      { ...privateEvent.session, tool_choice: "none" },
    ]) {
      expect(
        admittedRealtimeProvider(
          { ...privateEvent, session },
          sessionDigest,
          privateProviderProfile,
        ),
      ).toBeUndefined();
    }
    expect(
      admittedRealtimeProvider(privateEvent, sessionDigest, communityProviderProfile),
    ).toBeUndefined();
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
    accounting.observe(done, responseUsageEvent(done, identity) !== undefined);
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
          accounting.observe(done, responseUsageEvent(done, identity) !== undefined);
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
    model: "gpt-realtime-2.1",
    output_modalities: ["audio"],
    audio: {
      input: {
        format: { type: "audio/pcm", rate: 24_000 },
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
        voice: "marin",
      },
    },
    instructions: profile.instructions,
    tools: profile.tools,
    tool_choice: profile.toolChoice,
  },
});

const closeEventForTest = {
  schema: "openagents.sarah.livekit-worker.v1",
  _tag: "close",
  ...identity,
  eventRef: "close:job:one",
  reason: "provider_disconnect",
  accountingStatus: "uncertain",
} as const;
