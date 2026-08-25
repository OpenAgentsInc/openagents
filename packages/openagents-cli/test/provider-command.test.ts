import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import * as NodeServices from "@effect/platform-node/NodeServices";
import { Effect, Layer } from "effect";
import { describe, expect, it } from "vitest";

import { runCliWith } from "../src/cli.js";
import { credentialStoreUnavailableLayer } from "../src/credential-store.js";
import { environmentLayerFromValues } from "../src/environment.js";
import { outputTestLayer, type OutputDocument, type OutputMode } from "../src/output.js";
import { persistedConfigurationTestLayer } from "../src/persisted-configuration.js";
import { terminalSessionTestLayer } from "../src/terminal-session.js";

interface Written {
  readonly document: OutputDocument;
  readonly mode: OutputMode;
}

const harness = () => {
  const written: Array<Written> = [];
  const layer = Layer.mergeAll(
    NodeServices.layer,
    environmentLayerFromValues({}),
    persistedConfigurationTestLayer({}),
    terminalSessionTestLayer(false),
    credentialStoreUnavailableLayer,
    outputTestLayer((document, mode) =>
      Effect.sync(() => {
        written.push({ document, mode });
      }),
    ),
  );
  const run = (argv: ReadonlyArray<string>) =>
    Effect.runPromise(
      runCliWith([...argv]).pipe(Effect.provide(layer)) as Effect.Effect<void, unknown>,
    );
  const fail = (argv: ReadonlyArray<string>) =>
    Effect.runPromise(
      runCliWith([...argv]).pipe(Effect.provide(layer), Effect.flip) as Effect.Effect<
        unknown,
        unknown
      >,
    );
  return { run, fail, written };
};

const DIGEST = "b".repeat(64);
const PROVIDER = "npub-provider-0000000000000000000000000000";

const workspace = () => {
  const root = mkdtempSync(join(tmpdir(), "openagents-provider-"));
  const write = (name: string, value: unknown) => {
    const path = join(root, name);
    writeFileSync(path, JSON.stringify(value, undefined, 2), "utf8");
    return path;
  };
  const leasePath = write("lease.json", {
    job_id: "job-9f2c",
    lane: "validator_replay",
    provider: PROVIDER,
    price_msats: 1_000,
    expires_at: "2026-08-25T18:00:00.000Z",
  });
  const closeoutPath = write("closeout.json", {
    receiptRef: `lbr-closeout:job-9f2c:${DIGEST}`,
    requestId: "job-9f2c",
    requesterPubkey: "npub-requester-1111111111111111111111111111",
    providerPubkey: PROVIDER,
    quotedAmountMsats: 1_000,
    verificationCommandRef: "verify:pnpm-run-check-fast",
    testRef: "test:run-4471-passed",
    platformCloseoutRef: "platform-closeout:2026-08-25/job-9f2c",
    digest: DIGEST,
    settled_at: "2026-08-25T17:30:00.000Z",
  });
  const unverifiedPath = write("unverified.json", {
    receiptRef: `lbr-closeout:job-9f2c:${DIGEST}`,
    requestId: "job-9f2c",
    requesterPubkey: "npub-requester-1111111111111111111111111111",
    providerPubkey: PROVIDER,
    quotedAmountMsats: 1_000,
    verificationCommandRef: "",
    testRef: "",
    platformCloseoutRef: "platform-closeout:2026-08-25/job-9f2c",
    digest: DIGEST,
    settled_at: "2026-08-25T17:30:00.000Z",
  });
  return { root, leasePath, closeoutPath, unverifiedPath, write };
};

describe("openagents provider settle", () => {
  it("settles a job whose closeout receipt proves the work", async () => {
    const { closeoutPath, leasePath } = workspace();
    const { run, written } = harness();

    await run(["--json", "provider", "settle", "--lease", leasePath, "--closeout", closeoutPath]);

    const value = written[0]?.document.value as Record<string, unknown>;
    expect(value["schema"]).toBe("openagents.provider_settlement.v1");
    expect(value["state"]).toBe("settled");
    expect(value["earned_msats"]).toBe(1_000);
  });

  it("earns nothing when the run offers no closeout receipt", async () => {
    const { leasePath } = workspace();
    const { run, written } = harness();

    await run(["--json", "provider", "settle", "--lease", leasePath]);

    const value = written[0]?.document.value as Record<string, unknown>;
    expect(value["state"]).toBe("unsettled");
    expect(value["earned_msats"]).toBe(0);
    expect(value["refusal"]).toBe("no_closeout");
  });

  it("earns nothing when the receipt shows nothing verified the work", async () => {
    const { leasePath, unverifiedPath } = workspace();
    const { run, written } = harness();

    await run(["--json", "provider", "settle", "--lease", leasePath, "--closeout", unverifiedPath]);

    const value = written[0]?.document.value as Record<string, unknown>;
    expect(value["earned_msats"]).toBe(0);
    expect(value["refusal"]).toBe("work_not_verified");
  });

  it("never claims a payout rail or custody", async () => {
    const { closeoutPath, leasePath } = workspace();
    const { run, written } = harness();

    await run(["--json", "provider", "settle", "--lease", leasePath, "--closeout", closeoutPath]);

    const value = written[0]?.document.value as Record<string, unknown>;
    expect(value["payout_rail"]).toBe("not_connected");
    expect(value["custody"]).toBe("none");
  });

  it("refuses a lease file that is missing the fields the gate needs", async () => {
    const { write } = workspace();
    const broken = write("broken.json", { job_id: "job-9f2c", price_msats: 1_000 });
    const { fail } = harness();

    const error = await fail(["provider", "settle", "--lease", broken]);

    expect(String((error as { message?: string }).message)).toContain("missing");
  });

  it("refuses a lease path that does not parse as JSON", async () => {
    const { fail } = harness();

    const error = await fail([
      "provider",
      "settle",
      "--lease",
      join(tmpdir(), "openagents-provider-absent.json"),
    ]);

    expect(String((error as { message?: string }).message)).toContain("could not be read as JSON");
  });
});
