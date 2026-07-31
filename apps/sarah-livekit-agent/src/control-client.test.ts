import { describe, expect, test, vi } from "vite-plus/test";
import {
  SARAH_LIVEKIT_AGENT_NAME,
  SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
} from "@openagentsinc/audio-contract";
import { deriveSarahLiveKitControlToken, makeSarahLiveKitControlClient } from "./control-client.js";

const controlRoot = "A".repeat(64);
const dispatch = {
  schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
  agentName: SARAH_LIVEKIT_AGENT_NAME,
  sessionRef: "session:one",
  generation: 1,
  roomRef: "room:one",
  roomEpoch: 1,
  participantRef: "owner:one",
  sarahParticipantRef: "principal.sarah",
  sarahPresenceLeaseRef: "presence:one",
  capabilityProfile: "omega_editor",
  roomContext: { kind: "private" },
} as const;

const responseWithResetBody = (): Response =>
  new Response(
    new ReadableStream({
      start(controller) {
        controller.error(new TypeError("response body reset"));
      },
    }),
    {
      status: 200,
      headers: { "content-type": "application/json" },
    },
  );

describe("Sarah LiveKit control client", () => {
  test("derives a claim credential outside the loggable job and sends it only as bearer auth", async () => {
    const controlToken = deriveSarahLiveKitControlToken(controlRoot, dispatch);
    const fetcher = vi.fn(async (_url: string | URL | Request, init?: RequestInit) => {
      expect(init?.headers).toMatchObject({
        authorization: `Bearer ${controlToken}`,
      });
      expect(String(init?.body)).not.toContain(controlToken);
      return Response.json({
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        admitted: true,
        sessionRef: dispatch.sessionRef,
        generation: dispatch.generation,
        sessionExpiresAtMs: 2_000_000_000_000,
        safetyIdentifier: "a".repeat(64),
        capabilityProfile: {
          kind: "private_owner_v1",
          contextRead: true,
          editorProposals: true,
          agentThreadProposals: true,
          ownerMemory: false,
          workspace: false,
          payments: false,
          release: false,
          memberAdmin: false,
          shell: false,
          git: false,
          credentials: false,
        },
      });
    });
    const client = makeSarahLiveKitControlClient(
      {
        baseUrl: "https://openagents.com",
        workerRef: "worker:one",
        controlRoot,
      },
      fetcher as typeof fetch,
    );
    const claim = await client.claim({
      dispatch,
      dispatchRef: "dispatch:one",
      jobRef: "job:one",
      roomSid: "RM_one",
    });
    expect(claim.sessionRef).toBe(dispatch.sessionRef);
  });

  test("rejects a control response that changes the generation", async () => {
    const client = makeSarahLiveKitControlClient(
      {
        baseUrl: "https://openagents.com",
        workerRef: "worker:one",
        controlRoot,
      },
      vi.fn(async () =>
        Response.json({
          schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
          admitted: true,
          sessionRef: dispatch.sessionRef,
          generation: 2,
          sessionExpiresAtMs: 2_000_000_000_000,
          safetyIdentifier: "a".repeat(64),
          capabilityProfile: {
            kind: "community_member_v1",
            contextRead: false,
            editorProposals: false,
            agentThreadProposals: false,
            ownerMemory: false,
            workspace: false,
            payments: false,
            release: false,
            memberAdmin: false,
            shell: false,
            git: false,
            credentials: false,
          },
        }),
      ) as typeof fetch,
    );
    await expect(
      client.claim({
        dispatch,
        dispatchRef: "dispatch:one",
        jobRef: "job:one",
        roomSid: "RM_one",
      }),
    ).rejects.toThrow("disagreed");
  });

  test("retries a usage event while the binding transaction becomes visible", async () => {
    const fetcher = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(Response.json({ error: "not_ready" }, { status: 409 }))
      .mockResolvedValueOnce(Response.json({ accepted: true }));
    const client = makeSarahLiveKitControlClient(
      {
        baseUrl: "https://openagents.com",
        workerRef: "worker:one",
        controlRoot,
      },
      fetcher,
    );
    await expect(
      client.event(dispatch, {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        _tag: "lease_check",
        sessionRef: dispatch.sessionRef,
        generation: dispatch.generation,
        jobRef: "job:one",
        eventRef: "lease:one",
      }),
    ).resolves.toEqual({ accepted: true });
    expect(fetcher).toHaveBeenCalledTimes(2);
  });

  test("retries the exact durable event after 200 headers followed by a body reset", async () => {
    const fetcher = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(responseWithResetBody())
      .mockResolvedValueOnce(Response.json({ accepted: true }));
    const client = makeSarahLiveKitControlClient(
      {
        baseUrl: "https://openagents.com",
        workerRef: "worker:one",
        controlRoot,
      },
      fetcher,
    );
    await expect(
      client.event(dispatch, {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        _tag: "response_usage",
        sessionRef: dispatch.sessionRef,
        generation: dispatch.generation,
        jobRef: "job:one",
        eventRef: "response:body-reset",
        providerResponseRef: "provider-response:body-reset",
        status: "completed",
        inputTokens: 2,
        outputTokens: 3,
        cachedInputTokens: 1,
        audioInputTokens: 2,
        audioOutputTokens: 3,
      }),
    ).resolves.toEqual({ accepted: true });
    expect(fetcher).toHaveBeenCalledTimes(2);
    expect(fetcher.mock.calls[0]?.[1]?.body).toBe(fetcher.mock.calls[1]?.[1]?.body);
  });

  test("accepts the authoritative session-expiry stop response", async () => {
    const client = makeSarahLiveKitControlClient(
      {
        baseUrl: "https://openagents.com",
        workerRef: "worker:one",
        controlRoot,
      },
      vi.fn(async () =>
        Response.json({ accepted: true, stopReason: "session_expired" }),
      ) as typeof fetch,
    );
    await expect(
      client.event(dispatch, {
        schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
        _tag: "lease_check",
        sessionRef: dispatch.sessionRef,
        generation: dispatch.generation,
        jobRef: "job:one",
        eventRef: "lease:expired",
      }),
    ).resolves.toEqual({ accepted: true, stopReason: "session_expired" });
  });

  test("retries an idempotent tool proposal after an unknown delivery outcome", async () => {
    const proposal = {
      proposalRef: "proposal:one",
      proposalDigest: "b".repeat(64),
      command: {
        _tag: "context_read" as const,
        target: { workspaceRef: "workspace:one", path: "src/main.ts" },
        startLine: 1,
        endLine: 20,
      },
      confirmationRequired: false,
      expiresAtMs: 2_000_000_000_000,
    };
    const fetcher = vi
      .fn<typeof fetch>()
      .mockRejectedValueOnce(new TypeError("response lost"))
      .mockResolvedValueOnce(
        Response.json({
          schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
          accepted: true,
          proposal,
        }),
      );
    const client = makeSarahLiveKitControlClient(
      {
        baseUrl: "https://openagents.com",
        workerRef: "worker:one",
        controlRoot,
      },
      fetcher,
    );
    await expect(
      client.proposeTool(dispatch, {
        sessionRef: dispatch.sessionRef,
        generation: dispatch.generation,
        jobRef: "job:one",
        eventRef: "tool:event:one",
        providerCallRef: "provider:call:one",
        command: proposal.command,
      }),
    ).resolves.toEqual(proposal);
    expect(fetcher).toHaveBeenCalledTimes(2);
    expect(fetcher.mock.calls[0]?.[1]?.body).toBe(fetcher.mock.calls[1]?.[1]?.body);
  });

  test("retries the exact tool proposal after 200 headers followed by a body reset", async () => {
    const proposal = {
      proposalRef: "proposal:body-reset",
      proposalDigest: "c".repeat(64),
      command: {
        _tag: "context_read" as const,
        target: { workspaceRef: "workspace:one", path: "src/main.ts" },
        startLine: 1,
        endLine: 20,
      },
      confirmationRequired: false,
      expiresAtMs: 2_000_000_000_000,
    };
    const fetcher = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(responseWithResetBody())
      .mockResolvedValueOnce(
        Response.json({
          schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
          accepted: true,
          proposal,
        }),
      );
    const client = makeSarahLiveKitControlClient(
      {
        baseUrl: "https://openagents.com",
        workerRef: "worker:one",
        controlRoot,
      },
      fetcher,
    );
    await expect(
      client.proposeTool(dispatch, {
        sessionRef: dispatch.sessionRef,
        generation: dispatch.generation,
        jobRef: "job:one",
        eventRef: "tool:event:body-reset",
        providerCallRef: "provider:call:body-reset",
        command: proposal.command,
      }),
    ).resolves.toEqual(proposal);
    expect(fetcher).toHaveBeenCalledTimes(2);
    expect(fetcher.mock.calls[0]?.[1]?.body).toBe(fetcher.mock.calls[1]?.[1]?.body);
  });

  test("retries a tool-state read across a transient control response", async () => {
    const fetcher = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(Response.json({ error: "unavailable" }, { status: 503 }))
      .mockResolvedValueOnce(
        Response.json({
          schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
          state: "waiting_decision",
        }),
      );
    const client = makeSarahLiveKitControlClient(
      {
        baseUrl: "https://openagents.com",
        workerRef: "worker:one",
        controlRoot,
      },
      fetcher,
    );
    await expect(
      client.readToolState(dispatch, {
        sessionRef: dispatch.sessionRef,
        generation: dispatch.generation,
        jobRef: "job:one",
        proposalRef: "proposal:one",
        proposalDigest: "b".repeat(64),
      }),
    ).resolves.toMatchObject({ state: "waiting_decision" });
    expect(fetcher).toHaveBeenCalledTimes(2);
  });

  test("retries the exact tool-state read after 200 headers followed by a body reset", async () => {
    const fetcher = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(responseWithResetBody())
      .mockResolvedValueOnce(
        Response.json({
          schema: SARAH_LIVEKIT_WORKER_PROTOCOL_VERSION,
          state: "waiting_decision",
        }),
      );
    const client = makeSarahLiveKitControlClient(
      {
        baseUrl: "https://openagents.com",
        workerRef: "worker:one",
        controlRoot,
      },
      fetcher,
    );
    await expect(
      client.readToolState(dispatch, {
        sessionRef: dispatch.sessionRef,
        generation: dispatch.generation,
        jobRef: "job:one",
        proposalRef: "proposal:body-reset",
        proposalDigest: "c".repeat(64),
      }),
    ).resolves.toMatchObject({ state: "waiting_decision" });
    expect(fetcher).toHaveBeenCalledTimes(2);
    expect(fetcher.mock.calls[0]?.[1]?.body).toBe(fetcher.mock.calls[1]?.[1]?.body);
  });

  test("derives stable credentials separated by generation and room", () => {
    const token = deriveSarahLiveKitControlToken(controlRoot, dispatch);
    expect(token).toBe("oa_sarah_lk_-zUjl0WEcpWVL65Y8bZ_CIVrygCu8tu3R0RWhrINfu0");
    expect(deriveSarahLiveKitControlToken(controlRoot, { ...dispatch })).toBe(token);
    expect(
      deriveSarahLiveKitControlToken(controlRoot, {
        ...dispatch,
        generation: 2,
      }),
    ).not.toBe(token);
    expect(
      deriveSarahLiveKitControlToken(controlRoot, {
        ...dispatch,
        roomRef: "room:two",
      }),
    ).not.toBe(token);
    expect(JSON.stringify({ job: { metadata: JSON.stringify(dispatch) } })).not.toContain(token);
  });

  test("fails closed before network I/O when the HMAC root is missing or malformed", () => {
    const fetcher = vi.fn<typeof fetch>();
    for (const root of [
      "",
      "A".repeat(63),
      "A".repeat(129),
      `${controlRoot}=`,
      ` ${controlRoot}`,
    ]) {
      expect(() =>
        makeSarahLiveKitControlClient(
          {
            baseUrl: "https://openagents.com",
            workerRef: "worker:one",
            controlRoot: root,
          },
          fetcher,
        ),
      ).toThrow("control root is invalid");
    }
    expect(fetcher).not.toHaveBeenCalled();
  });
});
