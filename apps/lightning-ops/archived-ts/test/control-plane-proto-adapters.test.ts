import { Effect } from "effect";
import { describe, expect, it } from "vitest";

import {
  decodeControlPlaneSecurityStateResponseFromAny,
  decodeControlPlaneSnapshotResponseFromAny,
  decodeSettlementWriteResponseFromAny,
} from "../src/controlPlane/protoAdapters.js";

import { makePaywall } from "./fixtures.js";

describe("lightning-ops control-plane proto adapters", () => {
  it("decodes proto-style snapshot payloads with snake_case fields and enum names", async () => {
    const raw = {
      ok: true,
      paywalls: [
        {
          paywall_id: "pw_proto",
          owner_id: "owner_1",
          name: "Paywall pw_proto",
          status: "PAYWALL_STATUS_ACTIVE",
          created_at_ms: "1730000000000",
          updated_at_ms: "1730000000500",
          policy: {
            paywall_id: "pw_proto",
            owner_id: "owner_1",
            pricing_mode: "PRICING_MODE_FIXED",
            fixed_amount_msats: "2000",
            max_per_request_msats: "5000",
            allowed_hosts: ["openagents.com"],
            blocked_hosts: [],
            quota_per_minute: "120",
            quota_per_day: "10000",
            kill_switch: false,
            created_at_ms: "1730000000000",
            updated_at_ms: "1730000000500",
          },
          routes: [
            {
              route_id: "route_pw_proto",
              paywall_id: "pw_proto",
              owner_id: "owner_1",
              host_pattern: "openagents.com",
              path_pattern: "/api/pw_proto",
              upstream_url: "https://upstream.example.com/pw_proto",
              protocol: "ROUTE_PROTOCOL_HTTPS",
              timeout_ms: "6000",
              priority: "10",
              created_at_ms: "1730000000000",
              updated_at_ms: "1730000000500",
            },
          ],
        },
      ],
    };

    const decoded = await Effect.runPromise(decodeControlPlaneSnapshotResponseFromAny(raw));
    expect(decoded).toEqual({
      ok: true,
      paywalls: [makePaywall("pw_proto")],
    });
  });

  it("decodes proto enum values and int64 strings for security state", async () => {
    const raw = {
      ok: true,
      global: {
        state_id: "global",
        global_pause: true,
        deny_reason_code: "SECURITY_DENY_REASON_CODE_GLOBAL_PAUSE_ACTIVE",
        deny_reason: "ops freeze",
        updated_by: "ops-bot",
        updated_at_ms: "1730000000900",
      },
      owner_controls: [
        {
          owner_id: "owner_1",
          kill_switch: true,
          deny_reason_code: "SECURITY_DENY_REASON_CODE_OWNER_KILL_SWITCH_ACTIVE",
          deny_reason: "chargeback",
          updated_by: "ops-bot",
          updated_at_ms: "1730000000901",
        },
      ],
      credential_roles: [
        {
          role: "CREDENTIAL_ROLE_GATEWAY_INVOICE",
          status: "CREDENTIAL_ROLE_STATUS_ROTATING",
          version: "4",
          fingerprint: "fp_123",
          note: "rotation",
          updated_at_ms: "1730000000902",
          last_rotated_at_ms: "1730000000902",
        },
      ],
    };

    const decoded = await Effect.runPromise(decodeControlPlaneSecurityStateResponseFromAny(raw));

    expect(decoded.global.globalPause).toBe(true);
    expect(decoded.global.denyReasonCode).toBe("global_pause_active");
    expect(decoded.global.updatedAtMs).toBe(1_730_000_000_900);

    expect(decoded.ownerControls[0]).toMatchObject({
      ownerId: "owner_1",
      killSwitch: true,
      denyReasonCode: "owner_kill_switch_active",
      updatedAtMs: 1_730_000_000_901,
    });

    expect(decoded.credentialRoles[0]).toMatchObject({
      role: "gateway_invoice",
      status: "rotating",
      version: 4,
      updatedAtMs: 1_730_000_000_902,
      lastRotatedAtMs: 1_730_000_000_902,
    });
  });

  it("decodes proto settlement payloads and keeps legacy payload compatibility", async () => {
    const protoRaw = {
      ok: true,
      existed: false,
      settlement: {
        settlement_id: "set_1",
        paywall_id: "pw_1",
        owner_id: "owner_1",
        invoice_id: "inv_1",
        amount_msats: "2000",
        payment_proof_ref: "proof_ref",
        request_id: "req_1",
        metadata: { source: "proto" },
        created_at_ms: "1730000000999",
      },
      invoice: {
        invoice_id: "inv_1",
        paywall_id: "pw_1",
        owner_id: "owner_1",
        amount_msats: "2000",
        status: "INVOICE_LIFECYCLE_STATUS_SETTLED",
        payment_hash: "hash_1",
        payment_request: "lnbc1...",
        payment_proof_ref: "proof_ref",
        request_id: "req_1",
        created_at_ms: "1730000000000",
        updated_at_ms: "1730000000999",
        settled_at_ms: "1730000000999",
      },
    };

    const protoDecoded = await Effect.runPromise(decodeSettlementWriteResponseFromAny(protoRaw));
    expect(protoDecoded.settlement.amountMsats).toBe(2_000);
    expect(protoDecoded.invoice?.status).toBe("settled");
    expect(protoDecoded.invoice?.updatedAtMs).toBe(1_730_000_000_999);

    const legacyRaw = {
      ok: true,
      existed: false,
      settlement: {
        settlementId: "set_legacy",
        paywallId: "pw_1",
        ownerId: "owner_1",
        amountMsats: 2000,
        paymentProofRef: "proof_ref",
        createdAtMs: 1_730_000_000_999,
      },
    };
    const legacyDecoded = await Effect.runPromise(decodeSettlementWriteResponseFromAny(legacyRaw));
    expect(legacyDecoded).toEqual(legacyRaw);
  });
});
