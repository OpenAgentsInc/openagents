import { inspect } from "node:util";
import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";
import { PUBLIC_TEST_MNEMONIC } from "../contract/index.ts";
import { REDACTED_SECRET_MARKER } from "./boundary.ts";
import {
  type DecodeCandidateInput,
  decodeCandidate,
  deriveAndAttachPublicIdentity,
} from "./decode-candidate.ts";
import {
  computeIdentityEncEnvelopeFixture,
  DECODE_FIXTURE_INPUTS,
  electronOpaqueFixtureInput,
  encryptedPylonBackupManifestOnlyInput,
  FIXTURE_EXPECTED_IDENTITY,
  plainMnemonicFixtureInput,
} from "./fixtures.ts";
import { DecodeCandidateError } from "./result.ts";

/** The forbidden secret needles. None may appear in any public-safe output. */
const SECRET_NEEDLES = [PUBLIC_TEST_MNEMONIC, "abandon", "about"];

/** Assert a serialized string carries no secret text. */
const expectNoSecretText = (serialized: string): void => {
  for (const needle of SECRET_NEEDLES) {
    expect(serialized.includes(needle)).toBe(false);
  }
};

const run = <A, E>(effect: Effect.Effect<A, E, never>): Promise<A> => Effect.runPromise(effect);
const runError = <A, E>(effect: Effect.Effect<A, E, never>): Promise<E> =>
  Effect.runPromise(Effect.flip(effect));

describe("decodeCandidate decodes every historical format to the frozen public identity", () => {
  test("there is one decodable fixture per format", () => {
    expect(DECODE_FIXTURE_INPUTS.map((input) => input.format)).toEqual([
      "plain_mnemonic_file",
      "compute_identity_enc",
      "wallet_keyring_envelope",
      "electron_safe_storage_record",
      "encrypted_pylon_backup",
      "sovereign_agent_toml",
    ]);
  });

  for (const input of DECODE_FIXTURE_INPUTS) {
    test(`${input.format} decodes to the expected public identity`, async () => {
      const decoded = await run(decodeCandidate(input));
      expect(decoded.result.status).toBe("decoded");
      expect(decoded.result.decoded).toBe(true);
      expect(decoded.secret).not.toBeNull();
      // Before derivation the public identity is not yet attached.
      expect(decoded.result.publicIdentity).toBeNull();

      const withIdentity = await run(deriveAndAttachPublicIdentity(decoded));
      expect(withIdentity.result.publicIdentity).toEqual(FIXTURE_EXPECTED_IDENTITY);
    });
  }
});

describe("secret-logging tripwire: no phrase in any public-safe output", () => {
  for (const input of DECODE_FIXTURE_INPUTS) {
    test(`${input.format} keeps the phrase out of the result, secret, and derived output`, async () => {
      const decoded = await run(decodeCandidate(input));
      const withIdentity = await run(deriveAndAttachPublicIdentity(decoded));
      const secret = decoded.secret;
      if (secret === null) throw new Error("expected a bounded secret");

      // The public result and derived result carry no phrase.
      expectNoSecretText(JSON.stringify(decoded.result));
      expectNoSecretText(JSON.stringify(withIdentity.result));

      // The bounded secret redacts every serialization surface.
      expectNoSecretText(JSON.stringify(secret));
      expectNoSecretText(JSON.stringify(decoded));
      expectNoSecretText(String(secret));
      expectNoSecretText(inspect(secret));
      expect(String(secret)).toContain(REDACTED_SECRET_MARKER);
      expect(secret.toJSON().secret).toBe(REDACTED_SECRET_MARKER);

      // The phrase is reachable ONLY inside the bounded use scope.
      const revealedLength = await run(secret.use((mnemonic) => Effect.succeed(mnemonic.length)));
      expect(revealedLength).toBe(PUBLIC_TEST_MNEMONIC.length);
    });
  }
});

describe("owner-attended and manifest-only candidates do not decode offline", () => {
  test("a normal Electron OS safeStorage record requires an owner-attended run", async () => {
    const decoded = await run(decodeCandidate(electronOpaqueFixtureInput));
    expect(decoded.result.status).toBe("owner_attended_required");
    expect(decoded.secret).toBeNull();
    expect(decoded.result.decoded).toBe(false);
    expect(typeof decoded.result.note).toBe("string");
  });

  test("a Pylon backup manifest with no payload only reports the public manifest", async () => {
    const decoded = await run(decodeCandidate(encryptedPylonBackupManifestOnlyInput));
    expect(decoded.result.status).toBe("owner_attended_required");
    expect(decoded.secret).toBeNull();
  });
});

describe("decode fails closed without leaking the secret", () => {
  test("a wrong password on the Compute envelope fails with decrypt_failed", async () => {
    const wrongPassword: DecodeCandidateInput = {
      format: "compute_identity_enc",
      password: "a-different-password",
      envelope: computeIdentityEncEnvelopeFixture,
    };
    const error = await runError(decodeCandidate(wrongPassword));
    expect(error).toBeInstanceOf(DecodeCandidateError);
    expect(error.reason).toBe("decrypt_failed");
    expect(error.format).toBe("compute_identity_enc");
    // The typed error carries no secret.
    expectNoSecretText(JSON.stringify(error));
  });

  test("an empty password on the Compute envelope fails with missing_password", async () => {
    const noPassword: DecodeCandidateInput = {
      format: "compute_identity_enc",
      password: "",
      envelope: computeIdentityEncEnvelopeFixture,
    };
    const error = await runError(decodeCandidate(noPassword));
    expect(error.reason).toBe("missing_password");
  });

  test("a plain file with a non-BIP-39 phrase fails with invalid_mnemonic", async () => {
    const error = await runError(
      decodeCandidate({
        format: "plain_mnemonic_file",
        contentUtf8: "not a real mnemonic phrase at all",
      }),
    );
    expect(error.reason).toBe("invalid_mnemonic");
    // The rejected phrase never appears in the error.
    expect(JSON.stringify(error).includes("not a real mnemonic")).toBe(false);
  });

  test("a malformed Compute envelope fails with malformed_envelope", async () => {
    const error = await runError(
      decodeCandidate({
        format: "compute_identity_enc",
        password: "x",
        envelope: { version: 1, cipher: "aes-256-gcm" },
      }),
    );
    expect(error.reason).toBe("malformed_envelope");
  });

  test("a sovereign TOML without the phrase field fails with malformed_envelope", async () => {
    const error = await runError(
      decodeCandidate({ format: "sovereign_agent_toml", tomlText: 'npub = "npub1abc"\n' }),
    );
    expect(error.reason).toBe("malformed_envelope");
  });
});

describe("the plain fixture proves the bounded-boundary contract directly", () => {
  test("the mnemonic is reachable only inside use and derives the frozen npub", async () => {
    const decoded = await run(decodeCandidate(plainMnemonicFixtureInput));
    const secret = decoded.secret;
    if (secret === null) throw new Error("expected a bounded secret");
    const npub = await run(
      secret.use((mnemonic) =>
        Effect.sync(() => (mnemonic === PUBLIC_TEST_MNEMONIC ? "matched" : "mismatch")),
      ),
    );
    expect(npub).toBe("matched");
    const withIdentity = await run(deriveAndAttachPublicIdentity(decoded));
    expect(withIdentity.result.publicIdentity?.npub).toBe(FIXTURE_EXPECTED_IDENTITY.npub);
  });
});
