import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { Effect, Schema } from "effect";
import { describe, expect, test } from "vite-plus/test";

// `openagents_web.swap_widget.funding_gate.v1`
import {
  IMMORTAL_BROWSER_ABI_SCHEMA,
  IMMORTAL_BROWSER_ABI_VERSION,
  IMMORTAL_BROWSER_MAX_REQUEST_BYTES,
  IMMORTAL_BROWSER_MAX_RESPONSE_BYTES,
  IMMORTAL_BROWSER_OPERATIONS,
  IMMORTAL_BROWSER_SOURCE_REVISION,
  IMMORTAL_REQUESTER_API_SHA256,
  bindImmortalBrowserClient,
  ingestRequesterSession,
  loadImmortalBrowserClient,
  prepareFundingRequest,
  requesterRfq,
  verifyBeforeFund,
} from "./immortal-browser-abi.js";

const upstreamFixtureBytes = readFileSync(
  new URL("../fixtures/swp-browser-abi-v1.json", import.meta.url),
);
const UpstreamBrowserAbiFixtureSchema = Schema.Struct({
  schema: Schema.Literal(IMMORTAL_BROWSER_ABI_SCHEMA),
  abi_version: Schema.Literal(IMMORTAL_BROWSER_ABI_VERSION),
  requester_api_sha256: Schema.Literal(IMMORTAL_REQUESTER_API_SHA256),
  maximum_request_bytes: Schema.Literal(IMMORTAL_BROWSER_MAX_REQUEST_BYTES),
  maximum_response_bytes: Schema.Literal(IMMORTAL_BROWSER_MAX_RESPONSE_BYTES),
  operations: Schema.Array(Schema.String),
  host_authority: Schema.Struct({
    owned_by_engine: Schema.Array(Schema.String),
    owned_by_host: Schema.Array(Schema.String),
  }),
});
const upstreamFixture = Schema.decodeUnknownSync(UpstreamBrowserAbiFixtureSchema)(
  JSON.parse(upstreamFixtureBytes.toString("utf8")),
);

const metadata = {
  schema: IMMORTAL_BROWSER_ABI_SCHEMA,
  abi_version: IMMORTAL_BROWSER_ABI_VERSION,
  source_revision: IMMORTAL_BROWSER_SOURCE_REVISION,
  requester_api_sha256: IMMORTAL_REQUESTER_API_SHA256,
  maximum_request_bytes: IMMORTAL_BROWSER_MAX_REQUEST_BYTES,
  maximum_response_bytes: IMMORTAL_BROWSER_MAX_RESPONSE_BYTES,
  operations: IMMORTAL_BROWSER_OPERATIONS,
  custody: "host_owned",
} as const;

const requesterSessionView = {
  schema: "openagents.mkt-swp.requester-session-view.v1",
  session_id: "10".repeat(32),
  quote: {
    rfq_id: "11".repeat(32),
    quote_id: "12".repeat(32),
    provider_pubkey: "13".repeat(32),
    quote_class: "firm",
    reservation_class: "none",
    swap_type: "submarine",
    input_asset_id: "swp:1:bip122:00:btc:chain",
    output_asset_id: "swp:1:bip122:00:btc:lightning",
    input_amount: "50000",
    output_amount: "49000",
    amount_equation: "input_minus_fees",
    rounding: "floor",
    clock_skew_seconds: "30",
    expires_at: 1_800_000_300,
    effective_acceptance_deadline: 1_800_000_270,
    fees: {
      fee_bps: "25",
      provider_fee: "125",
      miner_fee_budget: "750",
      lightning_routing_fee_budget: "125",
      maximum_total_fee: "1000",
      fee_payer: "requester",
    },
    price_feed: null,
  },
  timeline: [],
  verification: {
    state: "quote_verified",
    local_verification_required: true,
    funding_authorized: false,
    status_gaps: [],
    status_forks: [],
    invalid_status_claims: [],
  },
  terminal: {
    claimed_state: "open",
    canonical_close_id: null,
    close_event_ids: [],
    principal_unresolved: null,
    loss_accounting_complete: false,
    local_effects_verified: false,
    watch_terminal: false,
  },
  deliveries: [],
} as const;

const preparedLightningFundingRequest = {
  session_id: "10".repeat(32),
  order_id: "14".repeat(32),
  quote_id: "12".repeat(32),
  swap_type: "submarine",
  action: {
    action: "pay_lightning_invoice",
    effect_id: "15".repeat(32),
    leg_id: "lightning-payment",
    invoice: "lnbcrt1fixture",
    maximum_routing_fee: "125",
    invoice_expires_at: 1_800_000_300,
    minimum_final_cltv_delta: 40,
    hold_invoice_required: true,
    hold_expiry_height: 500,
  },
} as const;

interface AbiRequest {
  readonly abi_version: number;
  readonly operation: string;
  readonly input: unknown;
}

const fakeExports = (responseFor: (request: AbiRequest) => unknown): WebAssembly.Exports => {
  const request: number[] = [];
  let response = new Uint8Array();
  return {
    immortal_mkt_swp_browser_abi_version: () => IMMORTAL_BROWSER_ABI_VERSION,
    immortal_mkt_swp_browser_max_request_bytes: () => IMMORTAL_BROWSER_MAX_REQUEST_BYTES,
    immortal_mkt_swp_browser_max_response_bytes: () => IMMORTAL_BROWSER_MAX_RESPONSE_BYTES,
    immortal_mkt_swp_browser_request_reset: () => {
      request.length = 0;
      response = new Uint8Array();
      return 0;
    },
    immortal_mkt_swp_browser_request_push: (byte: number) => {
      request.push(byte);
      return 0;
    },
    immortal_mkt_swp_browser_invoke: () => {
      const parsed: unknown = JSON.parse(new TextDecoder().decode(Uint8Array.from(request)));
      if (
        typeof parsed !== "object" ||
        parsed === null ||
        !("abi_version" in parsed) ||
        !("operation" in parsed) ||
        !("input" in parsed) ||
        typeof parsed.abi_version !== "number" ||
        typeof parsed.operation !== "string"
      ) {
        return 1;
      }
      const result = responseFor({
        abi_version: parsed.abi_version,
        operation: parsed.operation,
        input: parsed.input,
      });
      response = new TextEncoder().encode(JSON.stringify(result));
      return 0;
    },
    immortal_mkt_swp_browser_response_len: () => response.byteLength,
    immortal_mkt_swp_browser_response_byte: (index: number) => response.at(index) ?? 256,
  };
};

const success = (result: unknown) => ({
  schema: IMMORTAL_BROWSER_ABI_SCHEMA,
  abi_version: IMMORTAL_BROWSER_ABI_VERSION,
  source_revision: IMMORTAL_BROWSER_SOURCE_REVISION,
  requester_api_sha256: IMMORTAL_REQUESTER_API_SHA256,
  result,
});

const metadataThen = (operationResult: unknown) =>
  fakeExports((request) =>
    request.operation === "metadata" ? success(metadata) : success(operationResult),
  );

describe("Immortal browser ABI binding", () => {
  test("pins the exact upstream ABI fixture and host-authority boundary", () => {
    expect(createHash("sha256").update(upstreamFixtureBytes).digest("hex")).toBe(
      "2a25819d6277f7e182ff5a10c80f00c403fe61ccbc98f20e7971e527b8ac4400",
    );
    expect(upstreamFixture.operations).toEqual(IMMORTAL_BROWSER_OPERATIONS);
    expect(upstreamFixture.host_authority.owned_by_engine).toEqual([]);
    expect(upstreamFixture.host_authority.owned_by_host).toEqual([
      "entropy",
      "nostr_signing",
      "gift_wrap",
      "relay_transport",
      "snapshot_storage",
      "wallet_actions",
      "rail_observation",
    ]);
  });

  test("pins metadata before exposing a client", async () => {
    const client = await Effect.runPromise(bindImmortalBrowserClient(metadataThen({})));
    expect(client.metadata).toEqual(metadata);
  });

  test("rejects a missing required export", async () => {
    const exports = metadataThen({});
    delete exports.immortal_mkt_swp_browser_invoke;
    await expect(Effect.runPromise(bindImmortalBrowserClient(exports))).rejects.toMatchObject({
      code: "browser_wasm_export_missing",
      stage: "compatibility",
    });
  });

  test("rejects ABI version and transfer-bound mismatches", async () => {
    const wrongVersion = metadataThen({});
    wrongVersion.immortal_mkt_swp_browser_abi_version = () => 2;
    await expect(Effect.runPromise(bindImmortalBrowserClient(wrongVersion))).rejects.toMatchObject({
      code: "browser_abi_version_mismatch",
      stage: "compatibility",
    });

    const wrongBounds = metadataThen({});
    wrongBounds.immortal_mkt_swp_browser_max_response_bytes = () =>
      IMMORTAL_BROWSER_MAX_RESPONSE_BYTES + 1;
    await expect(Effect.runPromise(bindImmortalBrowserClient(wrongBounds))).rejects.toMatchObject({
      code: "browser_abi_bounds_mismatch",
      stage: "compatibility",
    });
  });

  test("rejects metadata provenance, digest, and operation drift", async () => {
    for (const drifted of [
      { ...metadata, source_revision: "00".repeat(20) },
      { ...metadata, requester_api_sha256: "00".repeat(32) },
    ]) {
      await expect(
        Effect.runPromise(
          bindImmortalBrowserClient(
            fakeExports((request) =>
              request.operation === "metadata" ? success(drifted) : success({}),
            ),
          ),
        ),
      ).rejects.toMatchObject({
        code: "browser_metadata_invalid",
        stage: "compatibility",
      });
    }

    await expect(
      Effect.runPromise(
        bindImmortalBrowserClient(
          fakeExports((request) =>
            request.operation === "metadata"
              ? success({
                  ...metadata,
                  operations: IMMORTAL_BROWSER_OPERATIONS.slice(0, -1),
                })
              : success({}),
          ),
        ),
      ),
    ).rejects.toMatchObject({
      code: "browser_operations_mismatch",
      stage: "compatibility",
    });
  });

  test("rejects every imported WASM authority before instantiation", async () => {
    const importedFunctionModule = Uint8Array.from([
      0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x04, 0x01, 0x60, 0x00, 0x00, 0x02,
      0x09, 0x01, 0x03, 0x65, 0x6e, 0x76, 0x01, 0x66, 0x00, 0x00,
    ]);
    await expect(
      Effect.runPromise(loadImmortalBrowserClient(importedFunctionModule)),
    ).rejects.toMatchObject({
      code: "browser_wasm_imports_forbidden",
      stage: "compatibility",
    });
  });

  test("preserves a typed engine refusal", async () => {
    const exports = fakeExports((request) =>
      request.operation === "metadata"
        ? success(metadata)
        : {
            schema: IMMORTAL_BROWSER_ABI_SCHEMA,
            abi_version: IMMORTAL_BROWSER_ABI_VERSION,
            source_revision: IMMORTAL_BROWSER_SOURCE_REVISION,
            requester_api_sha256: IMMORTAL_REQUESTER_API_SHA256,
            error: {
              code: "swp_quote_expired",
              detail: "the selected Quote has expired",
            },
          },
    );
    const client = await Effect.runPromise(bindImmortalBrowserClient(exports));
    await expect(Effect.runPromise(requesterRfq(client, {}))).rejects.toMatchObject({
      code: "swp_quote_expired",
      detail: "the selected Quote has expired",
      stage: "invoke",
    });
  });

  test("schema-decodes an operation result", async () => {
    const signingRequest = {
      pubkey: "11".repeat(32),
      created_at: 1_800_000_000,
      kind: 39604,
      tags: [["d", "22".repeat(32)]],
      content: "{}",
      expected_event_id: "33".repeat(32),
    };
    const client = await Effect.runPromise(bindImmortalBrowserClient(metadataThen(signingRequest)));
    await expect(Effect.runPromise(requesterRfq(client, {}))).resolves.toEqual(signingRequest);
  });

  test("fails closed on malformed or widened operation output", async () => {
    const client = await Effect.runPromise(
      bindImmortalBrowserClient(
        metadataThen({
          pubkey: "11".repeat(32),
          created_at: 1_800_000_000,
          kind: 39604,
          tags: [["d", "22".repeat(32)]],
          content: "{}",
          expected_event_id: "33".repeat(32),
          unreviewed: true,
        }),
      ),
    );
    await expect(Effect.runPromise(requesterRfq(client, {}))).rejects.toMatchObject({
      code: "browser_requester_rfq_response_invalid",
      stage: "response",
    });
  });

  test("fails closed when a response carries both result and error", async () => {
    const exports = fakeExports((request) =>
      request.operation === "metadata"
        ? success(metadata)
        : {
            ...success({}),
            error: { code: "swp_terms_mismatch", detail: "conflict" },
          },
    );
    const client = await Effect.runPromise(bindImmortalBrowserClient(exports));
    await expect(Effect.runPromise(requesterRfq(client, {}))).rejects.toMatchObject({
      code: "browser_response_invalid",
      stage: "response",
    });
  });

  test("accepts the upstream session_ingest result extension", async () => {
    const client = await Effect.runPromise(
      bindImmortalBrowserClient(
        metadataThen({
          snapshot_json_hex: "00",
          view: requesterSessionView,
          ingested_records: 0,
        }),
      ),
    );
    await expect(Effect.runPromise(ingestRequesterSession(client, {}))).resolves.toMatchObject({
      ingested_records: 0,
    });
  });

  test("decodes the exact prepared external-effect action", async () => {
    const client = await Effect.runPromise(
      bindImmortalBrowserClient(metadataThen(preparedLightningFundingRequest)),
    );
    await expect(Effect.runPromise(prepareFundingRequest(client, {}))).resolves.toEqual(
      preparedLightningFundingRequest,
    );
  });

  test("rejects a widened wallet-effect action", async () => {
    const client = await Effect.runPromise(
      bindImmortalBrowserClient(
        metadataThen({
          ...preparedLightningFundingRequest,
          action: {
            ...preparedLightningFundingRequest.action,
            wallet_seed: "must never cross the ABI",
          },
        }),
      ),
    );
    await expect(Effect.runPromise(prepareFundingRequest(client, {}))).rejects.toMatchObject({
      code: "browser_prepare_funding_request_response_invalid",
      stage: "response",
    });
  });

  test("preserves the engine's fail-closed funding refusal", async () => {
    const client = await Effect.runPromise(
      bindImmortalBrowserClient(
        fakeExports((request) =>
          request.operation === "metadata"
            ? success(metadata)
            : {
                schema: IMMORTAL_BROWSER_ABI_SCHEMA,
                abi_version: IMMORTAL_BROWSER_ABI_VERSION,
                source_revision: IMMORTAL_BROWSER_SOURCE_REVISION,
                requester_api_sha256: IMMORTAL_REQUESTER_API_SHA256,
                error: {
                  code: "swp_funding_not_authorized",
                  detail: "the expected funding request does not match local verification",
                },
              },
        ),
      ),
    );
    await expect(Effect.runPromise(verifyBeforeFund(client, {}))).rejects.toMatchObject({
      code: "swp_funding_not_authorized",
      stage: "invoke",
    });
  });

  test("maps a trapped WASM export to a typed boundary error", async () => {
    const client = await Effect.runPromise(
      bindImmortalBrowserClient(
        fakeExports((request) => {
          if (request.operation === "metadata") return success(metadata);
          throw new Error("fixture trap");
        }),
      ),
    );
    await expect(Effect.runPromise(requesterRfq(client, {}))).rejects.toMatchObject({
      code: "browser_wasm_trapped",
      stage: "invoke",
    });
  });
});
