/**
 * IDR-03 typed `decodeCandidate` seam.
 *
 * `decodeCandidate` is the single typed entry point keyed by historical format.
 * It routes an admitted candidate (that IDR-02 discovery surfaced) to the exact
 * legacy decoder and returns a `DecodedCandidate`: a bounded `RecoveredSecret`
 * plus a PUBLIC-safe result. Routing is a typed discriminated union, not string
 * matching: the `format` tag selects the decoder.
 *
 * `deriveAndAttachPublicIdentity` is the "public identifiers once derived"
 * surface. It derives the public identity through the bounded boundary (the
 * mnemonic never leaves the `use` scope) and returns the result with the public
 * identifiers attached, so a caller gets public data without touching the secret.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Effect } from "effect";
import { Npub } from "../contract/index.ts";
import { derivePublicIdentity } from "./boundary.ts";
import {
  type ComputeIdentityEncInput,
  decodeComputeIdentityEnc,
  decodeElectronSafeStorageRecord,
  decodeEncryptedPylonBackup,
  decodePlainMnemonicFile,
  decodeSovereignAgentToml,
  decodeWalletKeyringEnvelope,
  type ElectronSafeStorageInput,
  type EncryptedPylonBackupInput,
  type PlainMnemonicInput,
  type SovereignAgentTomlInput,
  type WalletKeyringInput,
} from "./formats.ts";
import type { DecodedCandidate } from "./result.ts";

/** The typed, format-keyed decode input. The `format` tag selects the decoder. */
export type DecodeCandidateInput =
  | ({ readonly format: "plain_mnemonic_file" } & PlainMnemonicInput)
  | ({ readonly format: "compute_identity_enc" } & ComputeIdentityEncInput)
  | ({ readonly format: "wallet_keyring_envelope" } & WalletKeyringInput)
  | ({ readonly format: "electron_safe_storage_record" } & ElectronSafeStorageInput)
  | ({ readonly format: "encrypted_pylon_backup" } & EncryptedPylonBackupInput)
  | ({ readonly format: "sovereign_agent_toml" } & SovereignAgentTomlInput);

/**
 * Decode one admitted candidate to a `DecodedCandidate`. Routing is typed on the
 * `format` tag. Every branch is exhaustive; an unreachable branch is a defect.
 */
export const decodeCandidate = Effect.fn("SovereignIdentity.decodeCandidate")(function* (
  input: DecodeCandidateInput,
) {
  switch (input.format) {
    case "plain_mnemonic_file":
      return yield* decodePlainMnemonicFile(input);
    case "compute_identity_enc":
      return yield* decodeComputeIdentityEnc(input);
    case "wallet_keyring_envelope":
      return yield* decodeWalletKeyringEnvelope(input);
    case "electron_safe_storage_record":
      return yield* decodeElectronSafeStorageRecord(input);
    case "encrypted_pylon_backup":
      return yield* decodeEncryptedPylonBackup(input);
    case "sovereign_agent_toml":
      return yield* decodeSovereignAgentToml(input);
    default: {
      const unreachable: never = input;
      throw new Error(
        `decodeCandidate reached an impossible format: ${JSON.stringify(unreachable)}`,
      );
    }
  }
});

/**
 * Derive the public identity through the bounded boundary and attach it to the
 * result. When the candidate carries no bounded secret (an owner-attended
 * record), it returns the candidate unchanged. The mnemonic never leaves the
 * bounded `use` scope; only the public identifiers reach the result.
 */
export const deriveAndAttachPublicIdentity = Effect.fn(
  "SovereignIdentity.deriveAndAttachPublicIdentity",
)(function* (decoded: DecodedCandidate) {
  if (decoded.secret === null) return decoded;
  const identity = yield* derivePublicIdentity(decoded.secret);
  return {
    result: {
      ...decoded.result,
      publicIdentity: {
        npub: Npub.make(identity.npub),
        nostrPublicKeyHex: identity.nostrPublicKeyHex,
        sparkPublicKeyHex: identity.sparkPublicKeyHex,
        sparkBip32FingerprintHex: identity.sparkBip32FingerprintHex,
      },
    },
    secret: decoded.secret,
  } satisfies DecodedCandidate;
});
