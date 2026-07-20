import { createHash } from "node:crypto";

import { canonicalJson } from "@openagentsinc/khala-sync";
import {
  PortableCheckpointCustodyObjectManifestSchema,
  PortableRef,
  type PortableCheckpointCustodyObjectManifest,
} from "@openagentsinc/portable-session-contract";
import { Effect, Schema } from "effect";

import type { PortableCheckpointCustodyDecryptor } from "./portable-checkpoint-artifact-resolver.js";

const BASE64 = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u;
const DEFAULT_MAX_ENCRYPTED_OBJECT_BYTES = 96 * 1024 * 1024;
const DEFAULT_MAX_CIPHERTEXT_BYTES = 64 * 1024 * 1024;
const DEFAULT_MAX_PLAINTEXT_BYTES = 64 * 1024 * 1024;
const MAX_CONFIGURED_BYTES = 256 * 1024 * 1024;
const AES_GCM_NONCE_BYTES = 12;
const AES_GCM_TAG_BYTES = 16;

const EncryptedEnvelopeSchema = Schema.Struct({
  schema: Schema.Literal("openagents.portable_checkpoint_artifact_custody_encrypted.v2"),
  algorithm: Schema.Literal("aes-256-gcm"),
  objectRef: PortableRef,
  policy: Schema.Literals(["owner_managed", "openagents_managed"]),
  keyRef: PortableRef,
  nonceBase64: Schema.String.check(Schema.isPattern(BASE64)),
  authTagBase64: Schema.String.check(Schema.isPattern(BASE64)),
  ciphertextBase64: Schema.String.check(Schema.isPattern(BASE64)),
});

const decodeManifest = Schema.decodeUnknownSync(PortableCheckpointCustodyObjectManifestSchema);
const decodeEnvelope = Schema.decodeUnknownSync(EncryptedEnvelopeSchema);
const envelopeFields = new Set([
  "schema",
  "algorithm",
  "objectRef",
  "policy",
  "keyRef",
  "nonceBase64",
  "authTagBase64",
  "ciphertextBase64",
]);

const sha256 = (value: string | Uint8Array): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;

const stableFailureRef = (code: string, objectRef: string): string =>
  `failure.portable-checkpoint-kms-custody.${createHash("sha256")
    .update(`${code}\u0000${objectRef}`)
    .digest("hex")}`;

const decodeCanonicalBase64 = (value: string): Uint8Array => {
  if (!BASE64.test(value)) throw new Error("base64_invalid");
  const decoded = Buffer.from(value, "base64");
  try {
    const bytes = Uint8Array.from(decoded);
    if (decoded.toString("base64") !== value) {
      bytes.fill(0);
      throw new Error("base64_noncanonical");
    }
    return bytes;
  } finally {
    decoded.fill(0);
  }
};

const validBound = (value: number): boolean =>
  Number.isSafeInteger(value) && value > 0 && value <= MAX_CONFIGURED_BYTES;

export class PortableCheckpointGoogleKmsCustodyError extends Schema.TaggedErrorClass<PortableCheckpointGoogleKmsCustodyError>()(
  "PortableCheckpointGoogleKmsCustodyError",
  {
    code: Schema.Literals([
      "invalid_configuration",
      "invalid_manifest",
      "owner_managed_refused",
      "encrypted_object_oversized",
      "invalid_envelope",
      "binding_mismatch",
      "ciphertext_oversized",
      "authority_unavailable",
      "plaintext_invalid",
      "plaintext_oversized",
    ]),
    failureRef: PortableRef,
  },
) {}

/**
 * A trusted Google Cloud authority performs AES-GCM authentication and
 * decryption. It resolves `keyRef` inside its KMS boundary. It must not return
 * or log key material.
 */
export type GoogleKmsDecryptAuthority = Readonly<{
  decryptAes256Gcm: (
    input: Readonly<{
      manifest: PortableCheckpointCustodyObjectManifest;
      manifestDigest: `sha256:${string}`;
      objectRef: string;
      policy: "openagents_managed";
      keyRef: string;
      nonce: Uint8Array;
      authTag: Uint8Array;
      ciphertext: Uint8Array;
      additionalAuthenticatedData: Uint8Array;
    }>,
  ) => Promise<Uint8Array>;
}>;

export type PortableCheckpointGoogleKmsCustodyConfig = Readonly<{
  authority: GoogleKmsDecryptAuthority;
  maxEncryptedObjectBytes?: number;
  maxCiphertextBytes?: number;
  maxPlaintextBytes?: number;
}>;

/**
 * Creates the OpenAgents-managed custody adapter. Owner-managed objects are
 * refused before the authority is called.
 */
export const createPortableCheckpointGoogleKmsCustodyDecryptor = (
  config: PortableCheckpointGoogleKmsCustodyConfig,
): PortableCheckpointCustodyDecryptor => {
  const maxEncryptedObjectBytes =
    config.maxEncryptedObjectBytes ?? DEFAULT_MAX_ENCRYPTED_OBJECT_BYTES;
  const maxCiphertextBytes = config.maxCiphertextBytes ?? DEFAULT_MAX_CIPHERTEXT_BYTES;
  const maxPlaintextBytes = config.maxPlaintextBytes ?? DEFAULT_MAX_PLAINTEXT_BYTES;
  if (
    typeof config.authority?.decryptAes256Gcm !== "function" ||
    !validBound(maxEncryptedObjectBytes) ||
    !validBound(maxCiphertextBytes) ||
    !validBound(maxPlaintextBytes)
  ) {
    throw new PortableCheckpointGoogleKmsCustodyError({
      code: "invalid_configuration",
      failureRef: stableFailureRef("invalid_configuration", "configuration"),
    });
  }

  const failure = (code: PortableCheckpointGoogleKmsCustodyError["code"], objectRef: string) =>
    new PortableCheckpointGoogleKmsCustodyError({
      code,
      failureRef: stableFailureRef(code, objectRef),
    });

  const decryptEffect = Effect.fn("PortableCheckpointGoogleKmsCustody.decrypt")(
    (input: Parameters<PortableCheckpointCustodyDecryptor["decrypt"]>[0]) =>
      Effect.tryPromise({
        try: async () => {
          let manifest: PortableCheckpointCustodyObjectManifest;
          try {
            manifest = decodeManifest(input.manifest);
          } catch {
            throw failure("invalid_manifest", "manifest");
          }
          if (manifest.custodyPolicy === "owner_managed") {
            throw failure("owner_managed_refused", manifest.objectRef);
          }
          if (
            input.encryptedObjectBytes.byteLength === 0 ||
            input.encryptedObjectBytes.byteLength > maxEncryptedObjectBytes ||
            input.encryptedObjectBytes.byteLength > manifest.byteLimit
          ) {
            throw failure("encrypted_object_oversized", manifest.objectRef);
          }
          if (sha256(input.encryptedObjectBytes) !== manifest.objectDigest) {
            throw failure("binding_mismatch", manifest.objectRef);
          }

          let envelopeUnknown: unknown;
          try {
            envelopeUnknown = JSON.parse(
              new TextDecoder("utf-8", { fatal: true }).decode(input.encryptedObjectBytes),
            ) as unknown;
          } catch {
            throw failure("invalid_envelope", manifest.objectRef);
          }
          if (
            typeof envelopeUnknown !== "object" ||
            envelopeUnknown === null ||
            Array.isArray(envelopeUnknown) ||
            Object.keys(envelopeUnknown).some((field) => !envelopeFields.has(field)) ||
            Object.keys(envelopeUnknown).length !== envelopeFields.size
          ) {
            throw failure("invalid_envelope", manifest.objectRef);
          }

          let envelope: typeof EncryptedEnvelopeSchema.Type;
          try {
            envelope = decodeEnvelope(envelopeUnknown);
          } catch {
            throw failure("invalid_envelope", manifest.objectRef);
          }
          if (
            envelope.objectRef !== manifest.objectRef ||
            envelope.policy !== "openagents_managed" ||
            envelope.policy !== manifest.custodyPolicy ||
            envelope.keyRef !== manifest.keyRef
          ) {
            throw failure("binding_mismatch", manifest.objectRef);
          }

          let nonce: Uint8Array | undefined;
          let authTag: Uint8Array | undefined;
          let ciphertext: Uint8Array | undefined;
          let aad: Uint8Array | undefined;
          let authorityPlaintext: Uint8Array | undefined;
          try {
            try {
              nonce = decodeCanonicalBase64(envelope.nonceBase64);
              authTag = decodeCanonicalBase64(envelope.authTagBase64);
              ciphertext = decodeCanonicalBase64(envelope.ciphertextBase64);
            } catch {
              throw failure("invalid_envelope", manifest.objectRef);
            }
            if (
              nonce.byteLength !== AES_GCM_NONCE_BYTES ||
              authTag.byteLength !== AES_GCM_TAG_BYTES ||
              ciphertext.byteLength === 0
            ) {
              throw failure("invalid_envelope", manifest.objectRef);
            }
            if (
              ciphertext.byteLength > maxCiphertextBytes ||
              sha256(ciphertext) !== manifest.ciphertextDigest
            ) {
              throw failure(
                ciphertext.byteLength > maxCiphertextBytes
                  ? "ciphertext_oversized"
                  : "binding_mismatch",
                manifest.objectRef,
              );
            }
            aad = new TextEncoder().encode(
              canonicalJson({
                schema: envelope.schema,
                algorithm: envelope.algorithm,
                objectRef: envelope.objectRef,
                policy: envelope.policy,
                keyRef: envelope.keyRef,
              }),
            );
            try {
              authorityPlaintext = await config.authority.decryptAes256Gcm({
                manifest,
                manifestDigest: sha256(canonicalJson(manifest)),
                objectRef: manifest.objectRef,
                policy: "openagents_managed",
                keyRef: manifest.keyRef,
                nonce,
                authTag,
                ciphertext,
                additionalAuthenticatedData: aad,
              });
            } catch {
              throw failure("authority_unavailable", manifest.objectRef);
            }
            if (
              !(authorityPlaintext instanceof Uint8Array) ||
              authorityPlaintext.byteLength === 0
            ) {
              throw failure("plaintext_invalid", manifest.objectRef);
            }
            if (authorityPlaintext.byteLength > maxPlaintextBytes) {
              throw failure("plaintext_oversized", manifest.objectRef);
            }
            return Uint8Array.from(authorityPlaintext);
          } finally {
            nonce?.fill(0);
            authTag?.fill(0);
            ciphertext?.fill(0);
            aad?.fill(0);
            authorityPlaintext?.fill(0);
          }
        },
        catch: (cause) =>
          cause instanceof PortableCheckpointGoogleKmsCustodyError
            ? cause
            : failure("authority_unavailable", input.manifest.objectRef),
      }),
  );

  return {
    decrypt: (input) => Effect.runPromise(decryptEffect(input)),
  };
};
