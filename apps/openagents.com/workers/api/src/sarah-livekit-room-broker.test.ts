import { JobRestartPolicy } from "@livekit/protocol";
import {
  SARAH_LIVEKIT_AGENT_NAME,
  SARAH_LIVEKIT_CONTROL_TOPIC,
  decodeSarahLiveKitInterruptControl,
  decodeSarahLiveKitDispatchMetadata,
} from "@openagentsinc/audio-contract";
import { describe, expect, test, vi } from "vitest";

import {
  SARAH_LIVEKIT_PROVISIONING_DEADLINE_MS,
  type SarahLiveKitRoomBrokerClients,
  deriveSarahLiveKitControlToken,
  makeSarahLiveKitRoomBroker,
  parseSarahLiveKitRoomBrokerConfig,
} from "./sarah-livekit-room-broker";
import { ServerError } from "livekit-server-sdk";

const controlRoot = "A".repeat(64);
const sessionTicketSecret = "S".repeat(64);
const privateProvisionInput = {
  idempotencyKey: "sarah-livekit:session:one:1",
  ownerUserId: "owner:one",
  deviceRef: "device:one",
  threadRef: "thread:one",
  sessionRef: "session:one",
  generation: 1,
  capabilityProfile: "omega_editor",
  admissionRef: "admission:one",
  admissionDigest: "d".repeat(64),
  roomContext: { kind: "private" },
  publishAllowed: true,
  subscribeAllowed: true,
  expiresAtMs: 2_000_000_600_000,
} as const;

describe("Sarah LiveKit room broker configuration", () => {
  test("accepts only exact WSS and server credential shapes", () => {
    expect(
      parseSarahLiveKitRoomBrokerConfig({
        SARAH_LIVEKIT_URL: "wss://livekit.openagents.com",
        SARAH_LIVEKIT_API_KEY: `API${"A".repeat(12)}`,
        SARAH_LIVEKIT_API_SECRET: "b".repeat(48),
        SARAH_LIVEKIT_CONTROL_ROOT: controlRoot,
        OPENAGENTS_AUDIO_TOKEN_SECRET: sessionTicketSecret,
      }),
    ).toEqual({
      livekitUrl: "wss://livekit.openagents.com",
      apiKey: `API${"A".repeat(12)}`,
      apiSecret: "b".repeat(48),
      controlRoot,
      sessionTicketSecret,
    });
    expect(
      parseSarahLiveKitRoomBrokerConfig({
        SARAH_LIVEKIT_URL: "https://livekit.openagents.com",
        SARAH_LIVEKIT_API_KEY: `API${"A".repeat(12)}`,
        SARAH_LIVEKIT_API_SECRET: "b".repeat(48),
        SARAH_LIVEKIT_CONTROL_ROOT: controlRoot,
        OPENAGENTS_AUDIO_TOKEN_SECRET: sessionTicketSecret,
      }),
    ).toBeUndefined();
  });

  test("does not accept client or worker credentials in the URL", () => {
    expect(
      parseSarahLiveKitRoomBrokerConfig({
        SARAH_LIVEKIT_URL: "wss://secret@livekit.openagents.com",
        SARAH_LIVEKIT_API_KEY: `API${"A".repeat(12)}`,
        SARAH_LIVEKIT_API_SECRET: "b".repeat(48),
        SARAH_LIVEKIT_CONTROL_ROOT: controlRoot,
        OPENAGENTS_AUDIO_TOKEN_SECRET: sessionTicketSecret,
      }),
    ).toBeUndefined();
  });

  test("accepts the exact whole Secret Manager server-key projection", () => {
    const apiKey = `API${"A".repeat(12)}`;
    const apiSecret = "b".repeat(48);
    expect(
      parseSarahLiveKitRoomBrokerConfig({
        SARAH_LIVEKIT_URL: "wss://livekit.openagents.com",
        SARAH_LIVEKIT_SERVER_KEYS_JSON: JSON.stringify({
          api_key: apiKey,
          api_secret: apiSecret,
          keys_yaml: `${apiKey}: ${apiSecret}\n`,
        }),
        SARAH_LIVEKIT_CONTROL_ROOT: controlRoot,
        OPENAGENTS_AUDIO_TOKEN_SECRET: sessionTicketSecret,
      }),
    ).toEqual({
      livekitUrl: "wss://livekit.openagents.com",
      apiKey,
      apiSecret,
      controlRoot,
      sessionTicketSecret,
    });
  });

  test("fails closed for malformed, conflicting, or widened server-key JSON", () => {
    const apiKey = `API${"A".repeat(12)}`;
    const apiSecret = "b".repeat(48);
    const base = {
      SARAH_LIVEKIT_URL: "wss://livekit.openagents.com",
      SARAH_LIVEKIT_CONTROL_ROOT: controlRoot,
      OPENAGENTS_AUDIO_TOKEN_SECRET: sessionTicketSecret,
    };
    for (const serverKeysJson of [
      "",
      "null",
      '{"api_key":',
      JSON.stringify({ api_key: apiKey, api_secret: apiSecret }),
      JSON.stringify({
        api_key: apiKey,
        api_secret: apiSecret,
        keys_yaml: `${apiKey}: ${apiSecret}`,
        extra: true,
      }),
      JSON.stringify({
        api_key: apiKey,
        api_secret: apiSecret,
        keys_yaml: `${apiKey}: ${"c".repeat(48)}`,
      }),
    ]) {
      expect(
        parseSarahLiveKitRoomBrokerConfig({
          ...base,
          SARAH_LIVEKIT_SERVER_KEYS_JSON: serverKeysJson,
        }),
      ).toBeUndefined();
    }
    expect(
      parseSarahLiveKitRoomBrokerConfig({
        ...base,
        SARAH_LIVEKIT_SERVER_KEYS_JSON: JSON.stringify({
          api_key: apiKey,
          api_secret: apiSecret,
          keys_yaml: `${apiKey}: ${apiSecret}`,
        }),
        SARAH_LIVEKIT_API_KEY: apiKey,
        SARAH_LIVEKIT_API_SECRET: apiSecret,
      }),
    ).toBeUndefined();
  });

  test("fails closed for missing, short, padded, or whitespace-normalized HMAC roots", () => {
    const base = {
      SARAH_LIVEKIT_URL: "wss://livekit.openagents.com",
      SARAH_LIVEKIT_API_KEY: `API${"A".repeat(12)}`,
      SARAH_LIVEKIT_API_SECRET: "b".repeat(48),
      OPENAGENTS_AUDIO_TOKEN_SECRET: sessionTicketSecret,
    };
    for (const root of [
      undefined,
      "A".repeat(63),
      "A".repeat(129),
      `${controlRoot}=`,
      ` ${controlRoot}`,
      `${controlRoot}\n`,
    ]) {
      expect(
        parseSarahLiveKitRoomBrokerConfig({
          ...base,
          SARAH_LIVEKIT_CONTROL_ROOT: root,
        }),
      ).toBeUndefined();
    }
  });

  test("fails closed for missing, weak, padded, or normalized API-only ticket secrets", () => {
    const base = {
      SARAH_LIVEKIT_URL: "wss://livekit.openagents.com",
      SARAH_LIVEKIT_API_KEY: `API${"A".repeat(12)}`,
      SARAH_LIVEKIT_API_SECRET: "b".repeat(48),
      SARAH_LIVEKIT_CONTROL_ROOT: controlRoot,
    };
    for (const secret of [
      undefined,
      "S".repeat(63),
      "S".repeat(65),
      `${sessionTicketSecret}=`,
      ` ${sessionTicketSecret}`,
      `${sessionTicketSecret}\n`,
    ]) {
      expect(
        parseSarahLiveKitRoomBrokerConfig({
          ...base,
          OPENAGENTS_AUDIO_TOKEN_SECRET: secret,
        }),
      ).toBeUndefined();
    }
  });

  test("creates a credential-free no-restart dispatch and a microphone-only client grant", async () => {
    const createRoom = vi.fn(async () => undefined);
    const deleteRoom = vi.fn(async () => undefined);
    let dispatchOptions: Parameters<SarahLiveKitRoomBrokerClients["createDispatch"]>[2] | undefined;
    const createDispatch = vi.fn(
      async (
        _roomRef: string,
        _agentName: string,
        options: Parameters<SarahLiveKitRoomBrokerClients["createDispatch"]>[2],
      ) => {
        dispatchOptions = options;
        return { id: "dispatch:one" };
      },
    );
    const listDispatch = vi.fn(async () => []);
    const deleteDispatch = vi.fn(async () => undefined);
    const sendData = vi.fn<SarahLiveKitRoomBrokerClients["sendData"]>(async () => undefined);
    const broker = makeSarahLiveKitRoomBroker(
      {
        livekitUrl: "wss://livekit.openagents.com",
        apiKey: `API${"A".repeat(12)}`,
        apiSecret: "b".repeat(48),
        controlRoot,
        sessionTicketSecret,
      },
      () => 2_000_000_000_000,
      {
        createRoom,
        deleteRoom,
        createDispatch,
        listDispatch,
        deleteDispatch,
        sendData,
      },
    );
    const digest = broker.workerControlTokenDigest(privateProvisionInput);
    const provision = await broker.provision(privateProvisionInput);

    expect(createRoom).toHaveBeenCalledWith(expect.objectContaining({ maxParticipants: 2 }));
    expect(createDispatch).toHaveBeenCalledWith(
      provision.roomRef,
      SARAH_LIVEKIT_AGENT_NAME,
      expect.objectContaining({
        restartPolicy: JobRestartPolicy.JRP_NEVER,
      }),
    );
    expect(dispatchOptions).toBeDefined();
    if (dispatchOptions === undefined) {
      throw new Error("The explicit dispatch options were not observed");
    }
    const dispatch = decodeSarahLiveKitDispatchMetadata(JSON.parse(dispatchOptions.metadata));
    expect(dispatch).toMatchObject({
      sessionRef: "session:one",
      generation: 1,
      sarahParticipantRef: "principal.sarah",
    });
    const token = deriveSarahLiveKitControlToken(controlRoot, dispatch);
    expect(digest).toMatch(/^[a-f0-9]{64}$/u);
    expect(JSON.stringify(dispatch)).not.toContain(token);
    expect(JSON.stringify({ job: { metadata: dispatchOptions.metadata } })).not.toMatch(
      /bearer|credential|controlToken/iu,
    );
    if (broker.interrupt === undefined) {
      throw new Error("The Sarah LiveKit broker did not expose interrupt delivery");
    }
    await broker.interrupt({
      sessionRef: dispatch.sessionRef,
      generation: dispatch.generation,
      roomRef: dispatch.roomRef,
      roomEpoch: dispatch.roomEpoch,
      sarahParticipantRef: dispatch.sarahParticipantRef,
      interruptSequence: 2,
    });
    expect(sendData).toHaveBeenCalledWith(
      dispatch.roomRef,
      expect.any(Uint8Array),
      SARAH_LIVEKIT_CONTROL_TOPIC,
      [dispatch.sarahParticipantRef],
    );
    const payload = decodeSarahLiveKitInterruptControl(
      JSON.parse(new TextDecoder().decode(sendData.mock.calls[0]?.[1])),
    );
    expect(payload).toMatchObject({
      sessionRef: dispatch.sessionRef,
      generation: dispatch.generation,
      roomRef: dispatch.roomRef,
      roomEpoch: dispatch.roomEpoch,
      interruptSequence: 2,
    });
    expect(payload.signature).toMatch(/^[A-Za-z0-9_-]{43}$/u);
    expect(broker.workerControlTokenDigest(privateProvisionInput)).toBe(digest);
    expect(broker.sessionTicket(privateProvisionInput)).toMatch(/^[A-Za-z0-9_-]{43}$/u);
    expect(broker.sessionTicket(privateProvisionInput)).toBe(
      broker.sessionTicket({ ...privateProvisionInput }),
    );
    const differentControlRootBroker = makeSarahLiveKitRoomBroker(
      {
        livekitUrl: "wss://livekit.openagents.com",
        apiKey: `API${"A".repeat(12)}`,
        apiSecret: "b".repeat(48),
        controlRoot: "C".repeat(64),
        sessionTicketSecret,
      },
      () => 2_000_000_000_000,
      {
        createRoom,
        deleteRoom,
        createDispatch,
        listDispatch,
        deleteDispatch,
        sendData,
      },
    );
    expect(differentControlRootBroker.sessionTicket(privateProvisionInput)).toBe(
      broker.sessionTicket(privateProvisionInput),
    );
    expect(differentControlRootBroker.workerControlTokenDigest(privateProvisionInput)).not.toBe(
      digest,
    );
    const workerControlRootOnlyBroker = makeSarahLiveKitRoomBroker(
      {
        livekitUrl: "wss://livekit.openagents.com",
        apiKey: `API${"A".repeat(12)}`,
        apiSecret: "b".repeat(48),
        controlRoot,
        sessionTicketSecret: controlRoot,
      },
      () => 2_000_000_000_000,
      {
        createRoom,
        deleteRoom,
        createDispatch,
        listDispatch,
        deleteDispatch,
        sendData,
      },
    );
    expect(workerControlRootOnlyBroker.sessionTicket(privateProvisionInput)).not.toBe(
      broker.sessionTicket(privateProvisionInput),
    );
    const differentApiSecretBroker = makeSarahLiveKitRoomBroker(
      {
        livekitUrl: "wss://livekit.openagents.com",
        apiKey: `API${"A".repeat(12)}`,
        apiSecret: "b".repeat(48),
        controlRoot,
        sessionTicketSecret: "T".repeat(64),
      },
      () => 2_000_000_000_000,
      {
        createRoom,
        deleteRoom,
        createDispatch,
        listDispatch,
        deleteDispatch,
        sendData,
      },
    );
    expect(differentApiSecretBroker.sessionTicket(privateProvisionInput)).not.toBe(
      broker.sessionTicket(privateProvisionInput),
    );
    expect(differentApiSecretBroker.workerControlTokenDigest(privateProvisionInput)).toBe(digest);
    expect(
      broker.workerControlTokenDigest({
        ...privateProvisionInput,
        generation: 2,
      }),
    ).not.toBe(digest);
    expect(
      broker.workerControlTokenDigest({
        ...privateProvisionInput,
        idempotencyKey: "sarah-livekit:session:other-room:1",
      }),
    ).not.toBe(digest);
    expect(() =>
      broker.sessionTicket({
        ...privateProvisionInput,
        capabilityProfile: "mobile_voice_only",
      }),
    ).toThrow("capability profile is unsupported");
    expect(provision.grantClaims).toEqual(
      expect.objectContaining({
        roomJoin: true,
        canPublishData: false,
        canUpdateOwnMetadata: false,
        canPublishSources: ["microphone"],
        roomAdmin: false,
        roomCreate: false,
        roomList: false,
      }),
    );
    await broker.cleanup(provision);
    expect(deleteDispatch).toHaveBeenCalledWith(provision.dispatchRef, provision.roomRef);
    expect(deleteRoom).toHaveBeenCalledWith(provision.roomRef);
  });

  test("reuses only the exact existing no-restart Sarah dispatch", async () => {
    let firstDispatchMetadata = "";
    let existing:
      | Awaited<ReturnType<SarahLiveKitRoomBrokerClients["listDispatch"]>>[number]
      | undefined;
    const createDispatch = vi.fn(
      async (
        room: string,
        agentName: string,
        options: Parameters<SarahLiveKitRoomBrokerClients["createDispatch"]>[2],
      ) => {
        firstDispatchMetadata = options.metadata;
        existing = {
          id: "dispatch:stable",
          agentName,
          room,
          metadata: options.metadata,
          restartPolicy: options.restartPolicy,
          deployment: "",
        };
        return { id: "dispatch:stable" };
      },
    );
    const clients: SarahLiveKitRoomBrokerClients = {
      createRoom: vi.fn(async () => undefined),
      deleteRoom: vi.fn(async () => undefined),
      createDispatch,
      listDispatch: vi.fn(async () => (existing === undefined ? [] : [existing])),
      deleteDispatch: vi.fn(async () => undefined),
      sendData: vi.fn(async () => undefined),
    };
    const broker = makeSarahLiveKitRoomBroker(
      {
        livekitUrl: "wss://livekit.openagents.com",
        apiKey: `API${"A".repeat(12)}`,
        apiSecret: "b".repeat(48),
        controlRoot,
        sessionTicketSecret,
      },
      () => 2_000_000_000_000,
      clients,
    );

    const [first, concurrentReplay] = await Promise.all([
      broker.provision(privateProvisionInput),
      broker.provision({ ...privateProvisionInput }),
    ]);
    expect(concurrentReplay.dispatchRef).toBe(first.dispatchRef);
    const replay = await broker.provision(privateProvisionInput);
    expect(replay.dispatchRef).toBe(first.dispatchRef);
    expect(createDispatch).toHaveBeenCalledTimes(1);
    expect(firstDispatchMetadata).not.toBe("");
  });

  test("replaces LiveKit's inert automatic room dispatch with Sarah's named dispatch", async () => {
    let roomRef = "";
    const createDispatch = vi.fn(async () => ({ id: "dispatch:sarah" }));
    const deleteDispatch = vi.fn(async () => undefined);
    const broker = makeSarahLiveKitRoomBroker(
      {
        livekitUrl: "wss://livekit.openagents.com",
        apiKey: `API${"A".repeat(12)}`,
        apiSecret: "b".repeat(48),
        controlRoot,
        sessionTicketSecret,
      },
      () => 2_000_000_000_000,
      {
        createRoom: vi.fn(async (input) => {
          roomRef = input.name;
        }),
        deleteRoom: vi.fn(async () => undefined),
        createDispatch,
        listDispatch: vi.fn(async () => [
          {
            id: "dispatch:automatic",
            agentName: "",
            room: roomRef,
            metadata: "",
            restartPolicy: JobRestartPolicy.JRP_NEVER,
            deployment: "",
          },
        ]),
        deleteDispatch,
        sendData: vi.fn(async () => undefined),
      },
    );

    const provision = await broker.provision(privateProvisionInput);

    expect(deleteDispatch).toHaveBeenCalledWith("dispatch:automatic", provision.roomRef);
    expect(createDispatch).toHaveBeenCalledWith(
      provision.roomRef,
      SARAH_LIVEKIT_AGENT_NAME,
      expect.objectContaining({
        restartPolicy: JobRestartPolicy.JRP_NEVER,
      }),
    );
    expect(provision.dispatchRef).toBe("dispatch:sarah");
  });

  test("stops external side effects when the end-to-end provisioning deadline expires", async () => {
    let nowMs = 2_000_000_000_000;
    const createRoom = vi.fn(async () => {
      nowMs += SARAH_LIVEKIT_PROVISIONING_DEADLINE_MS;
    });
    const listDispatch = vi.fn(async () => []);
    const createDispatch = vi.fn(async () => ({ id: "dispatch:late" }));
    const broker = makeSarahLiveKitRoomBroker(
      {
        livekitUrl: "wss://livekit.openagents.com",
        apiKey: `API${"A".repeat(12)}`,
        apiSecret: "b".repeat(48),
        controlRoot,
        sessionTicketSecret,
      },
      () => nowMs,
      {
        createRoom,
        deleteRoom: vi.fn(async () => undefined),
        createDispatch,
        listDispatch,
        deleteDispatch: vi.fn(async () => undefined),
        sendData: vi.fn(async () => undefined),
      },
    );

    expect(SARAH_LIVEKIT_PROVISIONING_DEADLINE_MS).toBeLessThan(30_000);
    await expect(broker.provision(privateProvisionInput)).rejects.toThrow(
      "provisioning deadline expired",
    );
    expect(createRoom).toHaveBeenCalledOnce();
    expect(listDispatch).not.toHaveBeenCalled();
    expect(createDispatch).not.toHaveBeenCalled();
  });

  test("rejects a single existing dispatch with mismatched authority", async () => {
    const mismatchKinds = ["agent", "room", "metadata", "restart", "deployment"] as const;
    for (const mismatch of mismatchKinds) {
      const createDispatch = vi.fn(async () => ({ id: "dispatch:new" }));
      let expectedRoom = "";
      let expectedMetadata = "";
      const clients: SarahLiveKitRoomBrokerClients = {
        createRoom: vi.fn(async (input) => {
          expectedRoom = input.name;
        }),
        deleteRoom: vi.fn(async () => undefined),
        createDispatch,
        listDispatch: vi.fn(async () => [
          {
            id: "dispatch:foreign",
            agentName: mismatch === "agent" ? "another-agent" : SARAH_LIVEKIT_AGENT_NAME,
            room: mismatch === "room" ? "another-room" : expectedRoom,
            metadata:
              mismatch === "metadata" ? JSON.stringify({ schema: "foreign" }) : expectedMetadata,
            restartPolicy:
              mismatch === "restart" ? JobRestartPolicy.JRP_ON_FAILURE : JobRestartPolicy.JRP_NEVER,
            deployment: mismatch === "deployment" ? "another-deployment" : "",
          },
        ]),
        deleteDispatch: vi.fn(async () => undefined),
        sendData: vi.fn(async () => undefined),
      };
      const broker = makeSarahLiveKitRoomBroker(
        {
          livekitUrl: "wss://livekit.openagents.com",
          apiKey: `API${"A".repeat(12)}`,
          apiSecret: "b".repeat(48),
          controlRoot,
          sessionTicketSecret,
        },
        () => 2_000_000_000_000,
        clients,
      );
      const derivedBroker = makeSarahLiveKitRoomBroker(
        {
          livekitUrl: "wss://livekit.openagents.com",
          apiKey: `API${"A".repeat(12)}`,
          apiSecret: "b".repeat(48),
          controlRoot,
          sessionTicketSecret,
        },
        () => 2_000_000_000_000,
        {
          ...clients,
          listDispatch: vi.fn(async () => []),
          createDispatch: vi.fn(async (_room, _agent, options) => {
            expectedMetadata = options.metadata;
            return { id: "derive-only" };
          }),
        },
      );
      await derivedBroker.provision(privateProvisionInput);
      await expect(broker.provision(privateProvisionInput)).rejects.toThrow(
        /dispatch (authority conflicts|metadata is invalid)/u,
      );
      expect(createDispatch).not.toHaveBeenCalled();
    }
  });

  test("treats already-absent dispatches and rooms as idempotent cleanup", async () => {
    const notFound = () => new ServerError("Not Found", "resource absent", 404, "not_found");
    const broker = makeSarahLiveKitRoomBroker(
      {
        livekitUrl: "wss://livekit.openagents.com",
        apiKey: `API${"A".repeat(12)}`,
        apiSecret: "b".repeat(48),
        controlRoot,
        sessionTicketSecret,
      },
      () => 2_000_000_000_000,
      {
        createRoom: vi.fn(async () => undefined),
        deleteRoom: vi.fn(async () => {
          throw notFound();
        }),
        createDispatch: vi.fn(async () => ({ id: "dispatch:unused" })),
        listDispatch: vi.fn(async () => []),
        deleteDispatch: vi.fn(async () => {
          throw notFound();
        }),
        sendData: vi.fn(async () => undefined),
      },
    );

    await expect(
      broker.cleanupRoom({
        sessionRef: "session:one",
        generation: 1,
        roomRef: "room:one",
        roomEpoch: 1,
        dispatchRef: "dispatch:one",
        sarahPresenceLeaseRef: "presence:one",
      }),
    ).resolves.toBeUndefined();
  });
});
