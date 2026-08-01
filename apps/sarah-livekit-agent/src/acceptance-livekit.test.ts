import { createHash } from "node:crypto";
import {
  SARAH_VOICE_MODEL,
  SARAH_VOICE_PROTOCOL_VERSION,
  SARAH_VOICE_SETTLEMENT_PROTOCOL_VERSION,
  type VoiceIdentity,
} from "@openagentsinc/audio-contract";
import { describe, expect, test, vi } from "vite-plus/test";
import type { ClientOptions, RawData } from "ws";
import {
  acquireSarahLiveKitCommunityFloor,
  classifySarahLiveKitScenarioIcePaths,
  measureSarahInterruptAudioTail,
  openSarahLiveKitAcceptanceControlChannel,
  pollSarahLiveKitSettlement,
  type SarahLiveKitAcceptanceControlSocket,
  type SarahLiveKitAcceptanceControlSocketFactory,
} from "./acceptance-livekit.js";
import type { RtcStatsEntry } from "./ice-classification.js";

describe("Sarah LiveKit community floor acceptance", () => {
  test("joins the admitted room and acquires the local participant floor", async () => {
    const requests: Array<Readonly<{ url: string; body: Readonly<Record<string, unknown>> }>> = [];
    const fetch = vi.fn(async (input: string | URL, init?: RequestInit) => {
      requests.push({
        url: String(input),
        body: JSON.parse(String(init?.body)) as Readonly<Record<string, unknown>>,
      });
      return requests.length === 1
        ? Response.json({
            roomRef: "room-community",
            participantRef: "member-local",
            presenceLeaseRef: "presence-community",
            authority: { revision: 4 },
          })
        : Response.json({ revision: 5 });
    });

    await expect(
      acquireSarahLiveKitCommunityFloor({
        fetch,
        bearer: "acceptance-bearer",
        deviceRef: "device-community",
        communityRef: "openagents-public",
        channelRef: "agent-chat",
        roomRef: "room-community",
        participantRef: "member-local",
      }),
    ).resolves.toBeUndefined();

    expect(requests).toHaveLength(2);
    expect(requests[0]).toEqual({
      url: "https://openagents.com/api/sarah/livekit/room/join",
      body: {
        communityRef: "openagents-public",
        channelRef: "agent-chat",
      },
    });
    expect(requests[1]?.url).toBe("https://openagents.com/api/sarah/livekit/room/floor/member");
    expect(requests[1]?.body).toMatchObject({
      action: "acquire",
      presenceLeaseRef: "presence-community",
      expectedRevision: 4,
      requestedLeaseMs: 30_000,
    });
    expect(requests[1]?.body.nonce).toMatch(/^[A-Za-z0-9_-]{32,256}$/u);
  });
});

const icePeerStats = (
  peer: string,
  candidate: Readonly<{
    localType: number;
    remoteType: number;
    protocol: string;
    relayProtocol?: number;
  }>,
  overrides: Readonly<{ nominated?: boolean }> = {},
): readonly RtcStatsEntry[] => [
  {
    stats: {
      case: "transport",
      value: {
        rtc: { id: `${peer}-transport` },
        transport: { selectedCandidatePairId: `${peer}-pair` },
      },
    },
  },
  {
    stats: {
      case: "candidatePair",
      value: {
        rtc: { id: `${peer}-pair` },
        candidatePair: {
          localCandidateId: `${peer}-local`,
          remoteCandidateId: `${peer}-remote`,
          nominated: overrides.nominated ?? true,
          packetsSent: 128n,
          packetsReceived: 96n,
        },
      },
    },
  },
  {
    stats: {
      case: "localCandidate",
      value: {
        rtc: { id: `${peer}-local` },
        candidate: {
          candidateType: candidate.localType,
          protocol: candidate.protocol,
          ...(candidate.relayProtocol === undefined
            ? {}
            : { relayProtocol: candidate.relayProtocol }),
        },
      },
    },
  },
  {
    stats: {
      case: "remoteCandidate",
      value: {
        rtc: { id: `${peer}-remote` },
        candidate: { candidateType: candidate.remoteType, protocol: candidate.protocol },
      },
    },
  },
];

const directUdp = { localType: 1, remoteType: 0, protocol: "udp" } as const;
const turnTls = { localType: 3, remoteType: 0, protocol: "tcp", relayProtocol: 2 } as const;

describe("Sarah LiveKit scenario ICE classification", () => {
  test("classifies both peer connections from the live stats shape", () => {
    expect(
      classifySarahLiveKitScenarioIcePaths("private", {
        publisherStats: icePeerStats("publisher", directUdp),
        subscriberStats: icePeerStats("subscriber", turnTls),
      }),
    ).toEqual({
      publisher: {
        localCandidateType: "srflx",
        remoteCandidateType: "host",
        protocol: "udp",
        relayed: false,
        packetsObserved: true,
      },
      subscriber: {
        localCandidateType: "relay",
        remoteCandidateType: "host",
        protocol: "tls",
        relayed: true,
        packetsObserved: true,
      },
    });
  });

  test("fails closed and names the typed reason instead of recording an unclassified pass", () => {
    expect(() =>
      classifySarahLiveKitScenarioIcePaths("community", {
        publisherStats: icePeerStats("publisher", directUdp),
        subscriberStats: icePeerStats("subscriber", directUdp, { nominated: false }),
      }),
    ).toThrow(
      "community subscriber selected ICE path was not classifiable (selected_pair_not_nominated)",
    );

    expect(() =>
      classifySarahLiveKitScenarioIcePaths("private", {
        publisherStats: [],
        subscriberStats: icePeerStats("subscriber", directUdp),
      }),
    ).toThrow(
      "private publisher selected ICE path was not classifiable (no_selected_candidate_pair)",
    );
  });
});

const interruptTailClock = (tailMs: number) => {
  let now = 0;
  return {
    clock: {
      now: () => now,
      sleep: async (durationMs: number) => {
        now += durationMs;
      },
    },
    lastAudibleAt: () => (now < tailMs ? now : tailMs),
  };
};

describe("Sarah LiveKit interrupt audio tail", () => {
  test("accepts the 750ms boundary after observing the full quiet window", async () => {
    const fixture = interruptTailClock(750);
    await expect(
      measureSarahInterruptAudioTail(0, fixture.lastAudibleAt, fixture.clock),
    ).resolves.toBe(750);
  });

  test("rejects audio beyond the 750ms boundary", async () => {
    const fixture = interruptTailClock(751);
    await expect(
      measureSarahInterruptAudioTail(0, fixture.lastAudibleAt, fixture.clock),
    ).rejects.toThrow("did not become quiet");
  });
});

const identity: VoiceIdentity = {
  ownerRef: "owner-private",
  deviceRef: "device-private",
  threadRef: "thread-private",
  sessionRef: "session-private",
  generation: 3,
};

class FakeControlSocket implements SarahLiveKitAcceptanceControlSocket {
  readonly sent: string[] = [];
  readonly terminate = vi.fn();
  private openListener: (() => void) | undefined;
  private messageListener: ((data: RawData, isBinary: boolean) => void) | undefined;
  private errorListener: (() => void) | undefined;
  private closeListener: (() => void) | undefined;

  onOpen(listener: () => void): void {
    this.openListener = listener;
  }

  onMessage(listener: (data: RawData, isBinary: boolean) => void): void {
    this.messageListener = listener;
  }

  onError(listener: () => void): void {
    this.errorListener = listener;
  }

  onClose(listener: () => void): void {
    this.closeListener = listener;
  }

  send(message: string): void {
    this.sent.push(message);
  }

  open(): void {
    this.openListener?.();
  }

  message(value: unknown, isBinary = false): void {
    this.messageListener?.(Buffer.from(JSON.stringify(value)), isBinary);
  }

  error(): void {
    this.errorListener?.();
  }

  closed(): void {
    this.closeListener?.();
  }
}

const serverControl = (
  sequence: number,
  control: Readonly<Record<string, unknown>>,
): Readonly<Record<string, unknown>> => ({
  schema: SARAH_VOICE_PROTOCOL_VERSION,
  identity,
  sequence,
  ...control,
});

const setup = () => {
  const socket = new FakeControlSocket();
  let observedUrl: string | undefined;
  let observedOptions: ClientOptions | undefined;
  const factory: SarahLiveKitAcceptanceControlSocketFactory = (url, options) => {
    observedUrl = url;
    observedOptions = options;
    return socket;
  };
  let now = 100;
  const channel = openSarahLiveKitAcceptanceControlChannel(
    {
      gatewayUrl: "wss://openagents.com/api/omega/sarah/voice/connect",
      ticket: "acceptance-ticket-must-stay-private",
      identity,
      disclosureRef: "disclosure.sarah_livekit_acceptance.v1",
    },
    () => now,
    factory,
  );
  return {
    channel,
    socket,
    observed: () => ({ url: observedUrl, options: observedOptions }),
    setNow: (value: number) => {
      now = value;
    },
  };
};

describe("Sarah LiveKit acceptance control channel", () => {
  test("rejects a non-production gateway or malformed ticket before opening a socket", () => {
    const factory = vi.fn<SarahLiveKitAcceptanceControlSocketFactory>();
    expect(() =>
      openSarahLiveKitAcceptanceControlChannel(
        {
          gatewayUrl: "wss://openagents.com/api/omega/sarah/voice/connect?ticket=private",
          ticket: "acceptance-ticket-must-stay-private",
          identity,
          disclosureRef: "disclosure.sarah_livekit_acceptance.v1",
        },
        () => 100,
        factory,
      ),
    ).toThrow("control grant is invalid");
    expect(() =>
      openSarahLiveKitAcceptanceControlChannel(
        {
          gatewayUrl: "wss://openagents.com/api/omega/sarah/voice/connect",
          ticket: "short",
          identity,
          disclosureRef: "disclosure.sarah_livekit_acceptance.v1",
        },
        () => 100,
        factory,
      ),
    ).toThrow("control grant is invalid");
    expect(factory).not.toHaveBeenCalled();
  });

  test("authenticates, sequences hello-interrupt-close, and requires both interrupt signals", async () => {
    const { channel, socket, observed, setNow } = setup();
    socket.open();

    expect(observed().url).toBe("wss://openagents.com/api/omega/sarah/voice/connect");
    expect(observed().options?.headers).toMatchObject({
      "x-openagents-sarah-voice-session": identity.sessionRef,
      "x-openagents-sarah-voice-ticket": "acceptance-ticket-must-stay-private",
    });
    expect(JSON.parse(socket.sent[0] ?? "{}")).toEqual({
      schema: SARAH_VOICE_PROTOCOL_VERSION,
      identity,
      sequence: 0,
      _tag: "session_hello",
      disclosureRef: "disclosure.sarah_livekit_acceptance.v1",
    });
    expect(socket.sent[0]).not.toContain("acceptance-ticket-must-stay-private");

    socket.message(serverControl(0, { _tag: "lifecycle", state: "connecting" }));
    socket.message(
      serverControl(1, {
        _tag: "session_ready",
        model: "gpt-realtime-2.1",
        providerGenerationRef: "provider_generation:fixture-one",
        expiresAtMs: 10_000,
        reservedCreditMsat: 1_000,
      }),
    );
    await expect(channel.ready).resolves.toBe(100);

    const interruption = channel.interrupt();
    expect(JSON.parse(socket.sent[1] ?? "{}")).toMatchObject({
      sequence: 1,
      _tag: "interrupt",
    });
    setNow(120);
    socket.message(serverControl(2, { _tag: "interrupt_ack" }));
    expect(await Promise.race([interruption, Promise.resolve("pending")])).toBe("pending");
    setNow(125);
    socket.message(serverControl(3, { _tag: "lifecycle", state: "interrupted" }));
    await expect(interruption).resolves.toEqual({
      acknowledgedAtMs: 120,
      interruptedAtMs: 125,
    });

    const closed = channel.close();
    expect(JSON.parse(socket.sent[2] ?? "{}")).toMatchObject({
      sequence: 2,
      _tag: "close",
      reason: "user_stop",
    });
    socket.closed();
    await expect(closed).resolves.toBeUndefined();
    expect(socket.terminate).not.toHaveBeenCalled();
  });

  test("fails closed on binary media without exposing the ticket", async () => {
    const { channel, socket } = setup();
    const ready = channel.ready;
    socket.open();
    socket.message({ ignored: true }, true);

    const error = await ready.catch((cause: unknown) => cause);
    expect(error).toBeInstanceOf(Error);
    expect((error as Error).message).toBe(
      "Sarah LiveKit acceptance control channel received binary media",
    );
    expect((error as Error).message).not.toContain("acceptance-ticket-must-stay-private");
    expect(socket.terminate).toHaveBeenCalledOnce();
  });

  test("rejects server identity or sequence drift and early transport failure", async () => {
    const first = setup();
    const firstReady = first.channel.ready;
    first.socket.open();
    first.socket.message(
      serverControl(1, {
        _tag: "session_ready",
        model: "gpt-realtime-2.1",
        providerGenerationRef: "provider_generation:fixture-two",
        expiresAtMs: 10_000,
        reservedCreditMsat: 1_000,
      }),
    );
    await expect(firstReady).rejects.toThrow("authority or sequence disagreed");

    const second = setup();
    const secondReady = second.channel.ready;
    second.socket.open();
    second.socket.error();
    await expect(secondReady).rejects.toThrow("control transport failed");
    expect(second.socket.terminate).toHaveBeenCalledOnce();
  });
});

describe("Sarah LiveKit control channel terminal observation", () => {
  test("resolves the server's own closing reason without weakening the failure path", async () => {
    const { channel, socket, setNow } = setup();
    socket.open();
    socket.message(
      serverControl(0, {
        _tag: "session_ready",
        model: SARAH_VOICE_MODEL,
        providerGenerationRef: "provider_generation:fixture-three",
        expiresAtMs: 9_000,
        reservedCreditMsat: 5_000,
      }),
    );
    await expect(channel.ready).resolves.toBe(100);

    setNow(450);
    socket.message(serverControl(1, { _tag: "closing", reason: "transport_error" }));

    await expect(channel.terminal).resolves.toEqual({
      atMs: 450,
      kind: "closing",
      reason: "transport_error",
    });
  });

  test("preserves an unrequested transport close as terminal evidence", async () => {
    const { channel, socket, setNow } = setup();
    socket.open();
    setNow(700);
    socket.closed();

    await expect(channel.terminal).resolves.toEqual({
      atMs: 700,
      kind: "transport_closed",
      reason: "closed_without_terminal_frame",
    });
    // The acceptance failure semantics are untouched: an unrequested close is
    // still a rejected readiness, not a quiet pass.
    await expect(channel.ready).rejects.toThrow("closed early");
  });

  test("records only the first terminal signal", async () => {
    const { channel, socket, setNow } = setup();
    socket.open();
    setNow(300);
    socket.message(serverControl(0, { _tag: "closing", reason: "session_expired" }));
    setNow(900);
    socket.closed();

    await expect(channel.terminal).resolves.toEqual({
      atMs: 300,
      kind: "closing",
      reason: "session_expired",
    });
    await expect(channel.ready).rejects.toThrow("closed early");
  });
});

describe("Sarah LiveKit settlement poll", () => {
  const settlement = {
    schema: SARAH_VOICE_SETTLEMENT_PROTOCOL_VERSION,
    sessionRef: "session-poll",
    state: "settled" as const,
    creditMode: "metered" as const,
    finalChargeMsat: 1_234,
    spendableRemainingCreditMsat: 99,
    receiptRef: "receipt.sarah_voice_settlement.poll",
  };

  const pollClock = () => {
    let now = 0;
    return {
      now: () => now,
      sleep: (durationMs: number) => {
        now += durationMs;
        return Promise.resolve();
      },
    };
  };

  test("returns the terminal reading with the last non-terminal instant", async () => {
    const clock = pollClock();
    let attempts = 0;
    const urls: string[] = [];
    const fetch = vi.fn(async (input: string | URL) => {
      urls.push(String(input));
      attempts += 1;
      return attempts < 3
        ? new Response(JSON.stringify({ error: "not_terminal" }), { status: 404 })
        : Response.json(settlement);
    });

    const reading = await pollSarahLiveKitSettlement(
      fetch,
      clock,
      { bearer: "poll-bearer", sessionRef: "session-poll" },
      { windowMs: 10_000, intervalMs: 500 },
    );

    expect(reading?.settlement.state).toBe("settled");
    expect(reading?.terminalObservedAtMs).toBe(1_000);
    // The authority turned terminal somewhere in the last interval, so the
    // uncertainty is published rather than hidden.
    expect(reading?.lastNonTerminalObservedAtMs).toBe(500);
    expect(urls[0]).toBe("https://openagents.com/api/omega/sarah/voice/settlement");
  });

  test("returns null when the window closes with no terminal settlement", async () => {
    const clock = pollClock();
    const fetch = vi.fn(
      async () => new Response(JSON.stringify({ error: "not_terminal" }), { status: 404 }),
    );

    await expect(
      pollSarahLiveKitSettlement(
        fetch,
        clock,
        { bearer: "poll-bearer", sessionRef: "session-poll" },
        { windowMs: 2_000, intervalMs: 500 },
      ),
    ).resolves.toBeNull();
    expect(fetch).toHaveBeenCalledTimes(4);
  });

  test("raises a non-404 settlement failure instead of polling through it", async () => {
    const clock = pollClock();
    const fetch = vi.fn(async () => Response.json({ error: "forbidden" }, { status: 403 }));

    await expect(
      pollSarahLiveKitSettlement(
        fetch,
        clock,
        { bearer: "poll-bearer", sessionRef: "session-poll" },
        { windowMs: 2_000, intervalMs: 500 },
      ),
    ).rejects.toThrow("failed with HTTP 403 (forbidden)");
  });

  test("accepts durable zero-credit uncertainty only through the explicit drill opt-in", async () => {
    const clock = pollClock();
    const captureDigest = "b".repeat(64);
    const ledgerDigest = "c".repeat(64);
    const balanceDigest = "d".repeat(64);
    const uncertain = {
      error: "sarah_voice_accounting_uncertain",
      sessionRef: "session-poll",
      state: "accounting_uncertain",
      creditMode: "owner_waived_unmetered",
      recordedChargeMsat: 0,
      reservedCreditMsat: 0,
      holdPreserved: false,
      noHoldCreated: true,
      reason: "livekit_worker_heartbeat_expired",
      unmeteredAuthorityCapture: {
        schema: "openagents.sarah.unmetered-authority-capture.v1",
        authority: "owner_waived_unmetered_v1",
        generation: 7,
        sessionRefDigest: createHash("sha256").update("session-poll").digest("hex"),
        startLedgerStateDigest: ledgerDigest,
        endLedgerStateDigest: ledgerDigest,
        startBalanceStateDigest: balanceDigest,
        endBalanceStateDigest: balanceDigest,
        ledgerMutationCount: 0,
        captureReceiptRef: `sarah_voice_unmetered_authority:${captureDigest}`,
        captureDigest,
      },
    };
    const fetch = vi.fn(async () => Response.json(uncertain, { status: 409 }));

    await expect(
      pollSarahLiveKitSettlement(
        fetch,
        clock,
        { bearer: "poll-bearer", sessionRef: "session-poll", generation: 7 },
        { windowMs: 2_000, intervalMs: 500 },
        true,
      ),
    ).resolves.toMatchObject({
      settlement: {
        state: "accounting_uncertain",
        creditMode: "owner_waived_unmetered",
        recordedChargeMsat: 0,
        reservedCreditMsat: 0,
        noHoldCreated: true,
        authorityCaptureDigest: `sha256:${captureDigest}`,
        exactAccounting: false,
      },
    });

    await expect(
      pollSarahLiveKitSettlement(
        fetch,
        pollClock(),
        { bearer: "poll-bearer", sessionRef: "session-poll", generation: 7 },
        { windowMs: 2_000, intervalMs: 500 },
      ),
    ).rejects.toThrow("failed with HTTP 409");
  });

  test("rejects uncertain accounting without a durable zero-credit capture", async () => {
    const fetch = vi.fn(async () =>
      Response.json(
        {
          error: "sarah_voice_accounting_uncertain",
          sessionRef: "session-poll",
          state: "accounting_uncertain",
          creditMode: "owner_waived_unmetered",
          recordedChargeMsat: 0,
          reservedCreditMsat: 1,
          holdPreserved: true,
          noHoldCreated: false,
          reason: "livekit_worker_heartbeat_expired",
        },
        { status: 409 },
      ),
    );

    await expect(
      pollSarahLiveKitSettlement(
        fetch,
        pollClock(),
        { bearer: "poll-bearer", sessionRef: "session-poll", generation: 7 },
        { windowMs: 2_000, intervalMs: 500 },
        true,
      ),
    ).rejects.toThrow("omitted durable unmetered authority");
  });
});
