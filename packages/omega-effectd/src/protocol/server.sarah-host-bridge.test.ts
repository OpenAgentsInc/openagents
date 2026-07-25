import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { describe, expect, test } from "vite-plus/test";

import { createOmegaEffectdService } from "../service.ts";
import {
  OMEGA_EFFECTD_MAX_FRAME_BYTES,
  OMEGA_EFFECTD_PROTOCOL_SCHEMA,
  type OmegaEffectdHostRequest,
  type OmegaEffectdHostResponse,
} from "./framed.ts";
import { createOmegaEffectdFramedServer } from "./server.ts";

const withRoot = async (run: (root: string) => Promise<void>): Promise<void> => {
  const root = mkdtempSync(path.join(tmpdir(), "oa-effectd-sarah-host-"));
  try {
    await run(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
};

const request = (id: string, generation: number, method: string, params?: unknown) =>
  JSON.stringify({
    schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
    kind: "request",
    id,
    generation,
    method,
    ...(params === undefined ? {} : { params }),
  });

describe("omega-effectd Sarah host bridge", () => {
  test("advertises and forwards every Sarah method with exact typed params", async () => {
    await withRoot(async (root) => {
      const forwarded: OmegaEffectdHostRequest[] = [];
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        {
          hostRequestHandler: async (frame) => {
            forwarded.push(frame);
            return { forwardedMethod: frame.method, params: frame.params };
          },
        },
      );
      const initialized = await server.handleLine(
        request("init", 0, "initialize", { generation: 7 }),
      );
      if (initialized?.ok !== true || initialized.result === undefined) {
        throw new Error("The omega-effectd server did not initialize.");
      }
      const capabilities = (initialized.result as { capabilities: ReadonlyArray<string> })
        .capabilities;
      expect(capabilities).toEqual(
        expect.arrayContaining([
          "sarah_session_status",
          "sarah_bootstrap",
          "sarah_room_snapshot",
          "sarah_send_message",
          "sarah_interrupt_turn",
          "sarah_renew_device_grant",
          "sarah_revoke_device_grant",
        ]),
      );

      const snapshotParams = {
        cursor: "cursor.room.10",
        limit: 12,
        transcriptCursor: "cursor.transcript.8",
        activityCursor: "cursor.activity.5",
        transcriptLimit: 24,
        activityLimit: 16,
      };
      const sendParams = {
        text: "Continue the current Sarah turn.",
        idempotencyRef: "intent.mobile.send:1",
        expectedGeneration: 7,
      };
      const interruptParams = {
        turnRef: "turn.sarah.1",
        idempotencyRef: "intent.mobile.interrupt.1",
        expectedGeneration: 7,
      };
      const renewParams = {
        grantRef: "grant.mobile.1",
        scopes: ["observe_issue31", "send_message"],
        expiresAt: 1_800_000_000,
        idempotencyRef: "intent.mobile.renew.1",
        expectedGeneration: 7,
      };
      const revokeParams = {
        grantRef: "grant.mobile.1",
        idempotencyRef: "intent.mobile.revoke.1",
        expectedGeneration: 7,
      };
      const responses = await Promise.all([
        server.handleLine(request("status", 7, "sarah_session_status")),
        server.handleLine(request("bootstrap", 7, "sarah_bootstrap", {})),
        server.handleLine(request("snapshot", 7, "sarah_room_snapshot", snapshotParams)),
        server.handleLine(request("send", 7, "sarah_send_message", sendParams)),
        server.handleLine(request("interrupt", 7, "sarah_interrupt_turn", interruptParams)),
        server.handleLine(request("renew", 7, "sarah_renew_device_grant", renewParams)),
        server.handleLine(request("revoke", 7, "sarah_revoke_device_grant", revokeParams)),
      ]);

      expect(responses.every((response) => response?.ok === true)).toBe(true);
      expect(responses.every((response) => response?.error?.code !== "unknown_method")).toBe(true);
      expect(forwarded).toEqual([
        expect.objectContaining({ method: "sarah_session_status", params: {} }),
        expect.objectContaining({ method: "sarah_bootstrap", params: {} }),
        expect.objectContaining({ method: "sarah_room_snapshot", params: snapshotParams }),
        expect.objectContaining({ method: "sarah_send_message", params: sendParams }),
        expect.objectContaining({ method: "sarah_interrupt_turn", params: interruptParams }),
        expect.objectContaining({ method: "sarah_renew_device_grant", params: renewParams }),
        expect.objectContaining({ method: "sarah_revoke_device_grant", params: revokeParams }),
      ]);
      expect(responses[3]?.result).toEqual({
        forwardedMethod: "sarah_send_message",
        params: sendParams,
      });
    });
  });

  test("rejects excess, oversized, unsafe, and stale inputs before host dispatch", async () => {
    await withRoot(async (root) => {
      const forwarded: OmegaEffectdHostRequest[] = [];
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        {
          hostRequestHandler: async (frame) => {
            forwarded.push(frame);
            return { accepted: true };
          },
        },
      );
      await server.handleLine(request("init", 0, "initialize", { generation: 3 }));

      const invalidRequests = [
        request("status-excess", 3, "sarah_session_status", { excess: true }),
        request("bootstrap-null", 3, "sarah_bootstrap", null),
        request("snapshot-cursor", 3, "sarah_room_snapshot", { cursor: "c".repeat(257) }),
        request("snapshot-limit", 3, "sarah_room_snapshot", { activityLimit: 65 }),
        request("snapshot-excess", 3, "sarah_room_snapshot", { limit: 8, excess: true }),
        request("send-empty", 3, "sarah_send_message", {
          text: "   ",
          idempotencyRef: "intent.mobile.send.empty",
          expectedGeneration: 3,
        }),
        request("send-long", 3, "sarah_send_message", {
          text: "x".repeat(8_001),
          idempotencyRef: "intent.mobile.send.long",
          expectedGeneration: 3,
        }),
        request("send-utf8", 3, "sarah_send_message", {
          text: "🙂".repeat(3_000),
          idempotencyRef: "intent.mobile.send.utf8",
          expectedGeneration: 3,
        }),
        request("send-secret-ref", 3, "sarah_send_message", {
          text: "hello",
          idempotencyRef: `nsec1${"a".repeat(58)}`,
          expectedGeneration: 3,
        }),
        request("send-short-ref", 3, "sarah_send_message", {
          text: "hello",
          idempotencyRef: "x",
          expectedGeneration: 3,
        }),
        request("send-single-segment-ref", 3, "sarah_send_message", {
          text: "hello",
          idempotencyRef: "intent",
          expectedGeneration: 3,
        }),
        request("send-uppercase-leading-ref", 3, "sarah_send_message", {
          text: "hello",
          idempotencyRef: "Intent.mobile.send",
          expectedGeneration: 3,
        }),
        request("send-old-generation", 3, "sarah_send_message", {
          text: "hello",
          idempotencyRef: "intent.mobile.send.stale",
          expectedGeneration: 2,
        }),
        request("send-excess", 3, "sarah_send_message", {
          text: "hello",
          idempotencyRef: "intent.mobile.send.excess",
          expectedGeneration: 3,
          token: "must-not-cross",
        }),
        request("interrupt-path", 3, "sarah_interrupt_turn", {
          turnRef: "/Users/owner/private-turn",
          idempotencyRef: "intent.mobile.interrupt.path",
          expectedGeneration: 3,
        }),
        request("interrupt-stale", 3, "sarah_interrupt_turn", {
          turnRef: "turn.sarah.1",
          idempotencyRef: "intent.mobile.interrupt.stale",
          expectedGeneration: 2,
        }),
        request("renew-repeated-scope", 3, "sarah_renew_device_grant", {
          grantRef: "grant.mobile.1",
          scopes: ["observe_issue31", "observe_issue31"],
          expiresAt: 1_800_000_000,
          idempotencyRef: "intent.mobile.renew.repeated",
          expectedGeneration: 3,
        }),
        request("renew-empty-scopes", 3, "sarah_renew_device_grant", {
          grantRef: "grant.mobile.1",
          scopes: [],
          expiresAt: 1_800_000_000,
          idempotencyRef: "intent.mobile.renew.empty",
          expectedGeneration: 3,
        }),
        request("renew-unknown-scope", 3, "sarah_renew_device_grant", {
          grantRef: "grant.mobile.1",
          scopes: ["cloud_fallback"],
          expiresAt: 1_800_000_000,
          idempotencyRef: "intent.mobile.renew.unknown",
          expectedGeneration: 3,
        }),
        request("renew-unsafe-expiry", 3, "sarah_renew_device_grant", {
          grantRef: "grant.mobile.1",
          scopes: ["observe_issue31"],
          expiresAt: Number.MAX_SAFE_INTEGER + 1,
          idempotencyRef: "intent.mobile.renew.unsafe-expiry",
          expectedGeneration: 3,
        }),
        request("renew-event-id-as-ref", 3, "sarah_renew_device_grant", {
          grantRef: "a".repeat(64),
          scopes: ["observe_issue31"],
          expiresAt: 1_800_000_000,
          idempotencyRef: "intent.mobile.renew.event-id",
          expectedGeneration: 3,
        }),
        request("renew-stale", 3, "sarah_renew_device_grant", {
          grantRef: "grant.mobile.1",
          scopes: ["observe_issue31"],
          expiresAt: 1_800_000_000,
          idempotencyRef: "intent.mobile.renew.stale",
          expectedGeneration: 2,
        }),
        request("revoke-secret", 3, "sarah_revoke_device_grant", {
          grantRef: "grant.mobile.1",
          reasonRef: `nsec1${"a".repeat(58)}`,
          idempotencyRef: "intent.mobile.revoke.secret",
          expectedGeneration: 3,
        }),
        request("revoke-excess", 3, "sarah_revoke_device_grant", {
          grantRef: "grant.mobile.1",
          reasonRef: "reason.owner.revoked",
          idempotencyRef: "intent.mobile.revoke.excess",
          expectedGeneration: 3,
          nsec: "must-not-cross",
        }),
      ];

      const invalidResponses = await Promise.all(
        invalidRequests.map(async (invalidRequest) => ({
          invalidRequest,
          response: await server.handleLine(invalidRequest),
        })),
      );
      for (const { invalidRequest, response } of invalidResponses) {
        const requestId = (JSON.parse(invalidRequest) as { id: string }).id;
        expect(response?.ok, requestId).toBe(false);
        expect(response?.error?.code, requestId).toBe("invalid_request");
      }
      const staleFrame = await server.handleLine(
        request("outer-stale", 2, "sarah_room_snapshot", {}),
      );
      expect(staleFrame?.error?.code).toBe("stale_generation");
      const oversizedFrame = await server.handleLine("x".repeat(OMEGA_EFFECTD_MAX_FRAME_BYTES + 1));
      expect(oversizedFrame?.error?.code).toBe("frame_too_large");
      expect(forwarded).toHaveLength(0);
    });
  });

  test("rejects non-object and oversized host results", async () => {
    await withRoot(async (root) => {
      let oversized = false;
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        {
          hostRequestHandler: async () =>
            oversized ? { payload: "x".repeat(OMEGA_EFFECTD_MAX_FRAME_BYTES) } : "invalid",
        },
      );
      await server.handleLine(request("init", 0, "initialize", { generation: 1 }));

      const nonObject = await server.handleLine(request("non-object", 1, "sarah_bootstrap"));
      expect(nonObject?.error?.code).toBe("internal");
      oversized = true;
      const tooLarge = await server.handleLine(request("too-large", 1, "sarah_session_status"));
      expect(tooLarge?.error?.code).toBe("frame_too_large");
    });
  });

  test("maps typed private host failures without losing generation semantics", async () => {
    await withRoot(async (root) => {
      const emitted: OmegaEffectdHostRequest[] = [];
      const service = createOmegaEffectdService({ paths: { dataRoot: root } });
      const server = createOmegaEffectdFramedServer(
        service,
        { dataRoot: root },
        {
          emitHostRequest: (frame) => {
            emitted.push(frame);
          },
        },
      );
      await server.handleLine(request("init", 0, "initialize", { generation: 9 }));

      const assertHostError = async (
        hostCode: NonNullable<OmegaEffectdHostResponse["error"]>["code"],
        protocolCode: string,
      ) => {
        const responsePromise = server.handleLine(
          request(`request-${hostCode}`, 9, "sarah_session_status"),
        );
        await Promise.resolve();
        const frame = emitted.shift();
        expect(frame).toBeDefined();
        if (frame === undefined) {
          throw new Error(`The host request for ${hostCode} was not emitted.`);
        }
        const hostResponse: OmegaEffectdHostResponse = {
          schema: OMEGA_EFFECTD_PROTOCOL_SCHEMA,
          kind: "host_response",
          id: frame.id,
          generation: frame.generation,
          ok: false,
          error: {
            code: hostCode,
            message: `Typed ${hostCode} host failure.`,
          },
        };
        await server.handleLine(JSON.stringify(hostResponse));
        const response = await responsePromise;
        expect(response?.ok).toBe(false);
        expect(response?.error?.code).toBe(protocolCode);
        expect(response?.error?.message).toBe(`Typed ${hostCode} host failure.`);
      };
      await assertHostError("stale_generation", "stale_generation");
      await assertHostError("invalid_request", "invalid_request");
      await assertHostError("unsupported", "host_unavailable");
      await assertHostError("unavailable", "host_unavailable");
      await assertHostError("internal", "internal");
    });
  });
});
