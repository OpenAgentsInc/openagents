/**
 * What `openagents identity` shows, and what it must never show.
 *
 * The derivation itself is pinned in `seed-identity.test.ts`. This file covers
 * the surface a person actually touches: that `show` and `create` print the
 * public identity and not the phrase, that `backup` is the single command that
 * does print it and refuses `--json`, and that a destructive command asks first.
 */

import { mkdtempSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import * as NodeServices from "@effect/platform-node/NodeServices";
import { Effect, Layer } from "effect";
import { afterEach, describe, expect, it } from "vitest";

import { runCliWith } from "../src/cli.js";
import { credentialStoreUnavailableLayer } from "../src/credential-store.js";
import { environmentLayerFromValues } from "../src/environment.js";
import { outputTestLayer, type OutputDocument, type OutputMode } from "../src/output.js";
import { persistedConfigurationTestLayer } from "../src/persisted-configuration.js";
import { secretInputTestLayer } from "../src/secret-input.js";
import { seedPath } from "../src/seed-identity.js";
import { terminalSessionTestLayer } from "../src/terminal-session.js";

const TEST_PHRASE =
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const TEST_NPUB = "npub1az708q3kd9zy6z6f44zav5ygvdwelkzspf6mtusttx47lft2z38sghk0w7";
const TEST_ADDRESS = "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA";

interface Written {
  readonly document: OutputDocument;
  readonly mode: OutputMode;
}

const harness = (stdin = TEST_PHRASE) => {
  process.env["OPENAGENTS_IDENTITY_DIR"] = mkdtempSync(join(tmpdir(), "identity-command-"));
  const written: Array<Written> = [];
  const layer = Layer.mergeAll(
    NodeServices.layer,
    environmentLayerFromValues({}),
    persistedConfigurationTestLayer({}),
    terminalSessionTestLayer(false),
    credentialStoreUnavailableLayer,
    secretInputTestLayer(stdin),
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
  const last = () => written[written.length - 1];
  return { run, fail, written, last };
};

const messageOf = (error: unknown): string =>
  typeof error === "object" && error !== null && "message" in error
    ? String((error as { message: unknown }).message)
    : String(error);

afterEach(() => {
  delete process.env["OPENAGENTS_IDENTITY_DIR"];
});

describe("openagents identity", () => {
  it("says what to do when no seed is stored", async () => {
    const cli = harness();
    expect(messageOf(await cli.fail(["identity", "show"]))).toMatch(
      /No seed is stored.*identity create/s,
    );
  });

  it("imports a phrase from standard input and shows what it derives", async () => {
    const cli = harness();
    await cli.run(["identity", "import"]);
    await cli.run(["identity", "show", "--json"]);
    const document = cli.last()?.document;
    expect(document?.value).toMatchObject({
      npub: TEST_NPUB,
      wallet_address: TEST_ADDRESS,
      nostr_derivation_path: "m/44'/1237'/0'/0/0",
      wallet_derivation_path: "m/44'/0'/0'/0/0",
      spending_rail: null,
    });
    expect(statSync(seedPath()).mode & 0o777).toBe(0o600);
  });

  it("never echoes the phrase through import or show", async () => {
    const cli = harness();
    await cli.run(["identity", "import"]);
    await cli.run(["identity", "show"]);
    const printed = JSON.stringify(cli.written);
    expect(printed).toContain(TEST_NPUB);
    expect(printed).not.toContain("abandon");
  });

  it("rejects an invalid phrase without writing or quoting it", async () => {
    const cli = harness("clearly not a bip39 phrase");
    const error = await cli.fail(["identity", "import"]);
    expect(messageOf(error)).toMatch(/not a valid English BIP-39 seed phrase/);
    expect(messageOf(error)).not.toContain("clearly");
    expect(messageOf(await cli.fail(["identity", "show"]))).toMatch(/No seed is stored/);
  });

  it("creates a seed without printing it, and refuses to overwrite one", async () => {
    const cli = harness();
    await cli.run(["identity", "create"]);
    const created = cli.last();
    expect(created?.document.human.join("\n")).toMatch(/identity backup/);
    expect(JSON.stringify(created?.document)).not.toContain(TEST_PHRASE);
    expect(statSync(seedPath()).mode & 0o777).toBe(0o600);

    expect(messageOf(await cli.fail(["identity", "create"]))).toMatch(/already stored.*--force/s);
    expect(messageOf(await cli.fail(["identity", "import"]))).toMatch(/already stored.*--force/s);
  });

  it("rejects a word count that is not 12 or 24", async () => {
    const cli = harness();
    expect(messageOf(await cli.fail(["identity", "create", "--words", "13"]))).toMatch(
      /--words must be 12 or 24/,
    );
  });

  it("prints the phrase only through backup, and never as JSON", async () => {
    const cli = harness();
    await cli.run(["identity", "import"]);
    expect(messageOf(await cli.fail(["--json", "identity", "backup"]))).toMatch(
      /does not support --json/,
    );
    await cli.run(["identity", "backup"]);
    const backup = cli.last();
    expect(backup?.mode).toBe("human");
    expect(backup?.document.human).toContain(TEST_PHRASE);
    expect(JSON.stringify(backup?.document.value)).not.toContain("abandon");
  });

  it("forgets the seed only when forced", async () => {
    const cli = harness();
    await cli.run(["identity", "import"]);
    expect(messageOf(await cli.fail(["identity", "forget"]))).toMatch(/--force/);
    await cli.run(["identity", "forget", "--force"]);
    expect(cli.last()?.document.value).toMatchObject({ removed: true });
    expect(messageOf(await cli.fail(["identity", "show"]))).toMatch(/No seed is stored/);
  });

  it("names the rail decision rather than implying a spend path exists", async () => {
    const cli = harness();
    await cli.run(["identity", "import"]);
    await cli.run(["identity", "show"]);
    expect(cli.last()?.document.human.join("\n")).toMatch(/Spending rail: not selected/);
  });
});
