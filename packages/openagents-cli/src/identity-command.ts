/**
 * The `openagents identity` command family.
 *
 * One seed phrase is the whole of a user's local sovereignty here: it produces
 * the Nostr identity the CLI signs with and the wallet branch that receives, and
 * it is the only thing worth backing up. These commands create it, import one,
 * show what it derives, hand it back for backup, and forget it.
 *
 * The split between the public and the secret path is the point of the family.
 * `show` prints identifiers that are safe anywhere — an `npub`, a receive
 * address, the derivation paths — and is the command every other surface and
 * script should call. `backup` is the single command that prints the phrase, it
 * says so in its own name, and it refuses `--json` so the phrase cannot be
 * captured by a caller that was collecting machine output. `create` and
 * `import` never echo the phrase at all.
 *
 * Spending is absent on purpose. The wallet's rail — self-custodial MDK/LDK, or
 * the deterministic Spark rail Pylon v1.0 used — is an owner decision that is
 * not recorded yet (monorepo issue #29), and a receive address is the same under
 * either. `show` names the gap rather than implying a spend path exists.
 */

import { Effect } from "effect";
import { Command, Flag } from "effect/unstable/cli";

import { InputError } from "./errors.js";
import { Output, type OutputMode } from "./output.js";
import { SecretInput } from "./secret-input.js";
import {
  describeSeedProtection,
  deriveSeedIdentity,
  forgetSeedPhrase,
  generateSeedPhrase,
  isValidSeedPhrase,
  loadSeed,
  protectSeed,
  seedEncryptedAtRest,
  seedPath,
  seedPresent,
  seedProtectionAvailable,
  storeSeedPhrase,
  type SeedIdentity,
  type SeedProtection,
} from "./seed-identity.js";

/** The shared flags a handler reads back off the root command. */
interface SharedFlags {
  readonly json: boolean;
}

const outputMode = (json: boolean): OutputMode => (json ? "json" : "human");

/** The one sentence that says where the rail decision stands. */
const RAIL_NOTE =
  "Spending rail: not selected. The wallet receives under either candidate rail; " +
  "which one it spends over is an owner decision (monorepo issue #29).";

const NO_IDENTITY =
  "No seed is stored. Run openagents identity create to make one, or " +
  "openagents identity import to restore an existing seed phrase.";

const identityValue = (identity: SeedIdentity, protection: SeedProtection) => ({
  schema: "openagents.cli_identity.v1",
  profile: identity.profile,
  npub: identity.npub,
  nostr_public_key: identity.nostrPublicKeyHex,
  nostr_derivation_path: identity.nostrDerivationPath,
  wallet_address: identity.walletAddress,
  wallet_public_key: identity.walletPublicKeyHex,
  wallet_fingerprint: identity.walletFingerprintHex,
  wallet_derivation_path: identity.walletDerivationPath,
  spending_rail: null,
  seed_path: seedPath(),
  seed_protection: protection,
  seed_encrypted_at_rest: seedEncryptedAtRest(protection),
});

const identityHuman = (
  identity: SeedIdentity,
  protection: SeedProtection,
): ReadonlyArray<string> => [
  `Identity: ${identity.npub}`,
  `  public key   ${identity.nostrPublicKeyHex}`,
  `  path         ${identity.nostrDerivationPath}`,
  `Wallet:   ${identity.walletAddress}`,
  `  public key   ${identity.walletPublicKeyHex}`,
  `  fingerprint  ${identity.walletFingerprintHex}`,
  `  path         ${identity.walletDerivationPath}`,
  `Profile:  ${identity.profile}`,
  RAIL_NOTE,
  // Whether the seed on this machine is encrypted or is readable text is not
  // something a person can infer from the path, and the plaintext fallback is
  // only honest if the surface that shows an identity says so every time.
  describeSeedProtection(protection, seedPath()),
];

/**
 * Move a plaintext seed under the OS keychain, and report what protects it.
 *
 * Every identity command starts here. A seed written before the CLI could
 * encrypt one stays plaintext until something moves it, and the move is the same
 * atomic rename either way, so the first `show` after an upgrade protects it
 * rather than waiting for the next `import`.
 */
const protectionInForce = Effect.fn("Identity.protectionInForce")(function* () {
  return yield* Effect.try({
    try: () => protectSeed() ?? seedProtectionAvailable(),
    catch: (cause) => new InputError({ message: String(cause) }),
  });
});

/** Read the stored seed, or fail with the sentence that says what to do. */
const storedSeed = Effect.fn("Identity.storedSeed")(function* () {
  const stored = yield* Effect.try({
    try: () => loadSeed(),
    catch: (cause) => new InputError({ message: String(cause) }),
  });
  if (stored === undefined) return yield* new InputError({ message: NO_IDENTITY });
  return stored;
});

/** Derive from the stored seed, or fail with the sentence that says what to do. */
const storedIdentity = Effect.fn("Identity.storedIdentity")(function* () {
  const stored = yield* storedSeed();
  return yield* Effect.try({
    try: () => deriveSeedIdentity(stored.phrase),
    catch: () =>
      new InputError({
        message: `The seed stored at ${seedPath()} is not a valid English BIP-39 mnemonic. Re-import the correct phrase with openagents identity import.`,
      }),
  });
});

const wordsFlag = Flag.integer("words").pipe(
  Flag.withDefault(12),
  Flag.withDescription("Words in the new seed phrase: 12 for 128 bits, 24 for 256"),
);

const forceFlag = Flag.boolean("force").pipe(
  Flag.withDescription("Replace the stored seed. The identity and wallet it derives are lost"),
);

export const makeIdentityCommand = <R>(root: Effect.Effect<SharedFlags, never, R>) => {
  const identityShowCommand = Command.make("show", {}, () =>
    Effect.gen(function* () {
      const flags = yield* root;
      const output = yield* Output;
      const protection = yield* protectionInForce();
      const identity = yield* storedIdentity();
      yield* output.write(
        {
          value: identityValue(identity, protection),
          human: identityHuman(identity, protection),
        },
        outputMode(flags.json),
      );
    }),
  ).pipe(
    Command.withDescription(
      "Show the identity and wallet this machine's seed derives, and what is protecting the seed at rest. Public identifiers only: the seed phrase, the nsec, and the private keys are never printed.",
    ),
  );

  const identityCreateCommand = Command.make(
    "create",
    { words: wordsFlag, force: forceFlag },
    ({ force, words }) =>
      Effect.gen(function* () {
        const flags = yield* root;
        const output = yield* Output;
        if (words !== 12 && words !== 24) {
          return yield* new InputError({ message: "--words must be 12 or 24." });
        }
        if (seedPresent() && !force) {
          return yield* new InputError({
            message: `A seed is already stored at ${seedPath()}. Back it up with openagents identity backup first, then pass --force to replace it.`,
          });
        }
        const created = yield* Effect.try({
          try: () => {
            const phrase = generateSeedPhrase(words);
            const derived = deriveSeedIdentity(phrase);
            const { protection } = storeSeedPhrase(phrase);
            return { identity: derived, protection };
          },
          catch: (cause) =>
            new InputError({
              message: `The new seed could not be stored at ${seedPath()}: ${String(cause)}`,
            }),
        });
        yield* output.write(
          {
            value: { ...identityValue(created.identity, created.protection), created: true },
            human: [
              `Wrote a new ${words}-word seed to ${seedPath()} (mode 0600).`,
              "Back it up now with openagents identity backup. Nothing else on this machine can recover it.",
              ...identityHuman(created.identity, created.protection),
            ],
          },
          outputMode(flags.json),
        );
      }),
  ).pipe(
    Command.withDescription(
      "Generate a seed phrase and store it encrypted under the OS keychain, or 0600 plaintext where there is no keychain. The phrase itself is not printed; run openagents identity backup to see it.",
    ),
  );

  const identityImportCommand = Command.make("import", { force: forceFlag }, ({ force }) =>
    Effect.gen(function* () {
      const flags = yield* root;
      const output = yield* Output;
      const input = yield* SecretInput;
      if (seedPresent() && !force) {
        return yield* new InputError({
          message: `A seed is already stored at ${seedPath()}. Back it up with openagents identity backup first, then pass --force to replace it.`,
        });
      }
      const phrase = yield* input.readToken();
      if (!isValidSeedPhrase(phrase)) {
        // The phrase is never echoed back, not even the part that parsed.
        return yield* new InputError({
          message:
            "That is not a valid English BIP-39 seed phrase. Check the word count (12, 15, 18, 21, or 24) and the spelling of each word.",
        });
      }
      const imported = yield* Effect.try({
        try: () => {
          const derived = deriveSeedIdentity(phrase);
          const { protection } = storeSeedPhrase(phrase);
          return { identity: derived, protection };
        },
        catch: (cause) =>
          new InputError({
            message: `The seed could not be stored at ${seedPath()}: ${String(cause)}`,
          }),
      });
      yield* output.write(
        {
          value: { ...identityValue(imported.identity, imported.protection), imported: true },
          human: [
            `Stored the seed at ${seedPath()} (mode 0600).`,
            ...identityHuman(imported.identity, imported.protection),
          ],
        },
        outputMode(flags.json),
      );
    }),
  ).pipe(
    Command.withDescription(
      "Read a seed phrase from standard input and store it encrypted under the OS keychain, or 0600 plaintext where there is no keychain. The phrase is never echoed, and an invalid phrase is rejected before anything is written.",
    ),
  );

  const identityBackupCommand = Command.make("backup", {}, () =>
    Effect.gen(function* () {
      const flags = yield* root;
      const output = yield* Output;
      if (flags.json) {
        return yield* new InputError({
          message:
            "openagents identity backup does not support --json. The seed phrase must not land in machine-collected output; run it without --json and copy the phrase yourself.",
        });
      }
      const protection = yield* protectionInForce();
      const stored = yield* storedSeed();
      yield* output.write(
        {
          value: { schema: "openagents.cli_identity_backup.v1" },
          human: [
            "This is the only secret on this machine. Anyone holding it holds the identity and the wallet.",
            // The person about to write the phrase down is the one who most
            // needs to know whether the copy left behind on disk is encrypted
            // or is the phrase itself.
            describeSeedProtection(protection, seedPath()),
            stored.phrase,
          ],
        },
        "human",
      );
    }),
  ).pipe(
    Command.withDescription(
      "Print the stored seed phrase so you can write it down. This is the one command that shows the secret; it refuses --json.",
    ),
  );

  const identityForgetCommand = Command.make("forget", { force: forceFlag }, ({ force }) =>
    Effect.gen(function* () {
      const flags = yield* root;
      const output = yield* Output;
      if (!force) {
        return yield* new InputError({
          message: `Deleting ${seedPath()} destroys the identity and the wallet it derives. Back the phrase up with openagents identity backup, then pass --force.`,
        });
      }
      const removed = yield* Effect.try({
        try: forgetSeedPhrase,
        catch: (cause) =>
          new InputError({
            message: `The seed at ${seedPath()} could not be removed: ${String(cause)}`,
          }),
      });
      yield* output.write(
        {
          value: { schema: "openagents.cli_identity_forget.v1", removed, seed_path: seedPath() },
          human: [removed ? `Removed ${seedPath()}.` : `No seed was stored at ${seedPath()}.`],
        },
        outputMode(flags.json),
      );
    }),
  ).pipe(
    Command.withDescription(
      "Delete the stored seed. Requires --force, because the identity and wallet are unrecoverable without the phrase.",
    ),
  );

  return Command.make("identity").pipe(
    Command.withDescription(
      "One seed phrase, one Nostr identity, one wallet. The seed is stored 0600 on this machine and everything else is derived from it.",
    ),
    Command.withSubcommands([
      identityShowCommand,
      identityCreateCommand,
      identityImportCommand,
      identityBackupCommand,
      identityForgetCommand,
    ]),
  );
};
