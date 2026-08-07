import { readFile } from "node:fs/promises";
import { Effect, Schema } from "effect";
import { expect, test } from "vite-plus/test";

// `openagents_web.swap_widget.funding_gate.v1`
import {
  IMMORTAL_BROWSER_SOURCE_REVISION,
  createRequesterSession,
  ingestRequesterSession,
  loadImmortalBrowserClient,
  prepareFundingRequest,
  requesterOrder,
  restoreRequesterSession,
  verifyBeforeFund,
} from "./immortal-browser-abi.js";

const wasmPath = process.env.IMMORTAL_BROWSER_WASM_PATH;
const requesterFixturePath = process.env.IMMORTAL_REQUESTER_FIXTURE_PATH;
const sessionFixturePath = process.env.IMMORTAL_SESSION_FIXTURE_PATH;
const engineFixturePath = process.env.IMMORTAL_ENGINE_FIXTURE_PATH;
const hasUpstreamArtifacts =
  wasmPath !== undefined &&
  requesterFixturePath !== undefined &&
  sessionFixturePath !== undefined &&
  engineFixturePath !== undefined;

test.skipIf(!hasUpstreamArtifacts)(
  "executes order and session operations against the pinned compiled Immortal WASM",
  async () => {
    if (
      wasmPath === undefined ||
      requesterFixturePath === undefined ||
      sessionFixturePath === undefined ||
      engineFixturePath === undefined
    ) {
      throw new Error("upstream Immortal integration paths are incomplete");
    }

    const [wasm, requesterText, sessionText, engineText] = await Promise.all([
      readFile(wasmPath),
      readFile(requesterFixturePath, "utf8"),
      readFile(sessionFixturePath, "utf8"),
      readFile(engineFixturePath, "utf8"),
    ]);
    const requesterFixture = JSON.parse(requesterText) as unknown;
    const sessionFixture = JSON.parse(sessionText) as unknown;
    const engineFixture = JSON.parse(engineText) as unknown;
    const client = await Effect.runPromise(loadImmortalBrowserClient(wasm));

    expect(client.metadata.source_revision).toBe(IMMORTAL_BROWSER_SOURCE_REVISION);

    const orderInput = fixtureVector(requesterFixture, sessionFixture, "requester_order_valid");
    const order = await Effect.runPromise(requesterOrder(client, orderInput));
    expect(order.kind).toBe(39606);
    expect(order.expected_event_id).toHaveLength(64);

    const sessionInput = objectValue(
      fixtureVector(requesterFixture, sessionFixture, "requester_session_valid"),
      "requester session input",
    );
    const sourceSnapshot = objectValue(
      pointer(sessionFixture, "#/flows/submarine/snapshot"),
      "submarine snapshot",
    );
    const exitPackages = arrayValue(sourceSnapshot.exit_packages, "submarine exit packages").map(
      (entry) => objectValue(entry, "exit package").document,
    );
    const completeSessionInput = Schema.decodeUnknownSync(Schema.Json)({
      ...sessionInput,
      exit_packages: exitPackages,
    });
    const created = await Effect.runPromise(createRequesterSession(client, completeSessionInput));
    expect(created.view.schema).toBe("openagents.mkt-swp.requester-session-view.v1");
    expect(created.view.verification.state).toBe("contract_terms_verified");

    const deliveries = Schema.decodeUnknownSync(Schema.Json)(sessionInput.deliveries);
    const restored = await Effect.runPromise(
      restoreRequesterSession(client, {
        snapshot_json_hex: created.snapshot_json_hex,
        deliveries,
      }),
    );
    expect(restored.view).toEqual(created.view);

    const ingested = await Effect.runPromise(
      ingestRequesterSession(client, {
        snapshot_json_hex: created.snapshot_json_hex,
        records: [],
        deliveries,
      }),
    );
    expect(ingested.ingested_records).toBe(0);
    expect(ingested.view).toEqual(created.view);

    const verification = fundingVerification(sessionFixture, engineFixture);
    const prepared = await Effect.runPromise(
      prepareFundingRequest(client, {
        snapshot_json_hex: created.snapshot_json_hex,
        verification,
        lightning_readiness: null,
      }),
    );
    expect(prepared.action.action).toBe("broadcast_bitcoin");
    const authorized = await Effect.runPromise(
      verifyBeforeFund(client, {
        snapshot_json_hex: created.snapshot_json_hex,
        verification,
        lightning_readiness: null,
        expected_funding_request: prepared,
      }),
    );
    expect(authorized.funding_request).toEqual(prepared);
    expect(authorized.snapshot_json_hex).not.toBe(created.snapshot_json_hex);

    await expect(
      Effect.runPromise(
        verifyBeforeFund(client, {
          snapshot_json_hex: created.snapshot_json_hex,
          verification,
          lightning_readiness: null,
          expected_funding_request: {
            ...prepared,
            order_id: "00".repeat(32),
          },
        }),
      ),
    ).rejects.toMatchObject({ code: "swp_funding_not_authorized" });
  },
);

const fundingVerification = (sessionFixture: unknown, engineFixture: unknown): Schema.Json => {
  const snapshot = objectValue(
    pointer(sessionFixture, "#/flows/submarine/snapshot"),
    "submarine snapshot",
  );
  const exit = objectValue(
    objectValue(
      arrayValue(snapshot.exit_packages, "submarine exit packages")[0],
      "submarine exit package",
    ).document,
    "submarine exit document",
  );
  const funding = objectValue(exit.funding, "submarine exit funding");
  const verification = objectValue(exit.verification, "submarine exit verification");
  const commitments = objectValue(exit.secret_commitments, "submarine public secret commitments");
  const deterministic = objectValue(
    objectValue(engineFixture, "engine fixture").deterministic_session,
    "deterministic engine session",
  );

  return Schema.decodeUnknownSync(Schema.Json)({
    observed_at: 500,
    payment_hash: commitments.payment_hash,
    funding: {
      raw_transaction: funding.transaction_template,
      output_index: funding.output_index,
      expected_amount: funding.amount,
      expected_script_pubkey: funding.script_pubkey,
      taproot_output_key: String(funding.script_pubkey).slice(4),
      taproot_script: verification.taproot_script,
      taproot_control_block: verification.taproot_control_block,
    },
    invoice: {
      invoice: deterministic.invoice,
      expected_network: "bitcoin",
      expected_amount_msat: deterministic.invoice_amount_msat,
      observed_at: deterministic.invoice_observed_at,
      required_minimum_final_cltv_delta: deterministic.invoice_minimum_final_cltv_delta,
    },
    timeout_ladder: {
      swap_type: "submarine",
      current_height: 100,
      fund_last: 110,
      claim_last: 120,
      refund_first: 140,
      chain_finality_blocks: 1,
      broadcast_safety_blocks: 2,
      reorg_safety_blocks: 6,
      invoice_expiration_time: 2_000,
      claim_expected_time: 1_000,
    },
    minimum_confirmations: 1,
    replacement_policy: "reject",
  });
};

const fixtureVector = (
  requesterFixture: unknown,
  sessionFixture: unknown,
  name: string,
): Schema.Json => {
  const root = objectValue(requesterFixture, "requester fixture");
  const vectors = objectValue(root.vectors, "requester fixture vectors");
  if (!(name in vectors)) {
    throw new Error(`requester fixture omits vector ${name}`);
  }
  return Schema.decodeUnknownSync(Schema.Json)(
    resolveFixture(vectors[name], requesterFixture, sessionFixture, 0),
  );
};

const resolveFixture = (
  value: unknown,
  requesterFixture: unknown,
  sessionFixture: unknown,
  depth: number,
): unknown => {
  if (depth > 64) {
    throw new Error("requester fixture reference depth exceeds its bound");
  }
  if (Array.isArray(value)) {
    return value.map((item) => resolveFixture(item, requesterFixture, sessionFixture, depth + 1));
  }
  if (value === null || typeof value !== "object") return value;

  const object = value as Record<string, unknown>;
  const keys = Object.keys(object);
  if (keys.length === 1) {
    const key = keys[0];
    if (key === undefined) return {};
    const reference = object[key];
    if (typeof reference === "string") {
      if (key === "$artifact_ref") {
        return resolveFixture(
          pointer(requesterFixture, reference),
          requesterFixture,
          sessionFixture,
          depth + 1,
        );
      }
      if (key === "$fixture_ref") {
        return pointer(sessionFixture, reference);
      }
      if (key === "$artifact_json_hex_ref") {
        const resolved = resolveFixture(
          pointer(requesterFixture, reference),
          requesterFixture,
          sessionFixture,
          depth + 1,
        );
        return Buffer.from(JSON.stringify(resolved), "utf8").toString("hex");
      }
      if (key === "$fixture_json_hex_ref") {
        return Buffer.from(JSON.stringify(pointer(sessionFixture, reference)), "utf8").toString(
          "hex",
        );
      }
    }
  }

  return Object.fromEntries(
    Object.entries(object).map(([key, item]) => [
      key,
      resolveFixture(item, requesterFixture, sessionFixture, depth + 1),
    ]),
  );
};

const pointer = (root: unknown, reference: string): unknown => {
  if (!reference.startsWith("#/")) {
    throw new Error(`fixture reference is not local: ${reference}`);
  }
  return reference
    .slice(2)
    .split("/")
    .map((part) => part.replaceAll("~1", "/").replaceAll("~0", "~"))
    .reduce<unknown>((current, part) => {
      if (Array.isArray(current)) {
        if (!/^(0|[1-9][0-9]*)$/.test(part)) {
          throw new Error(`fixture pointer has invalid array index: ${reference}`);
        }
        const index = Number(part);
        if (!Number.isSafeInteger(index) || index >= current.length) {
          throw new Error(`fixture pointer does not resolve: ${reference}`);
        }
        return current[index];
      }
      const object = objectValue(current, `fixture pointer ${reference}`);
      if (!(part in object)) {
        throw new Error(`fixture pointer does not resolve: ${reference}`);
      }
      return object[part];
    }, root);
};

const objectValue = (value: unknown, label: string): Record<string, unknown> => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  return value as Record<string, unknown>;
};

const arrayValue = (value: unknown, label: string): readonly unknown[] => {
  if (!Array.isArray(value)) {
    throw new Error(`${label} must be an array`);
  }
  return value;
};
