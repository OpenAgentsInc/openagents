import { Schema as S } from "effect";

import { canonicalStringify } from "../internal/canonical.js";
import { sha256Hex } from "../internal/sha256.js";
import { Sha256Hex, SignatureId } from "./refs.js";
import { SignatureContract } from "./signature.js";
import { SIGNATURE_REGISTRY, type SignatureRegistryEntry } from "./signatures.js";

/**
 * The generated signature catalog: the admitted-package / generated-catalog
 * claim.
 *
 * The catalog is DERIVED from the signature registry, never hand-written. Each
 * entry pins the signature identity, its status, and the digest of its
 * serialized contract; the catalog digest covers every entry. `SIGNATURE_CATALOG`
 * is the checked-in generated value, and `catalogClaimHolds` proves the checked-in
 * value equals a fresh derivation — a drift between a signature contract and the
 * catalog fails the claim mechanically.
 */

export const SIGNATURE_CATALOG_SCHEMA_LITERAL = "openagents.dse.signature_catalog.v1" as const;

export const SignatureCatalogEntry = S.Struct({
  signatureId: SignatureId,
  title: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
  status: S.Literals(["admitted", "draft"]),
  contractDigest: Sha256Hex,
});
export type SignatureCatalogEntry = typeof SignatureCatalogEntry.Type;

export const SignatureCatalog = S.Struct({
  schema: S.Literal(SIGNATURE_CATALOG_SCHEMA_LITERAL),
  generatedFrom: S.Literal("dse-signature-registry"),
  entries: S.Array(SignatureCatalogEntry).check(S.isMinLength(1)),
  digest: Sha256Hex,
});
export type SignatureCatalog = typeof SignatureCatalog.Type;

const decodeCatalog = S.decodeUnknownSync(SignatureCatalog);
const encodeContract = S.encodeUnknownSync(SignatureContract);

const contractDigest = (contract: typeof SignatureContract.Type): string =>
  sha256Hex(canonicalStringify(encodeContract(contract)));

/** Derive the catalog from a registry. The derivation is total and deterministic. */
export const deriveSignatureCatalog = (
  registry: ReadonlyArray<SignatureRegistryEntry> = SIGNATURE_REGISTRY,
): SignatureCatalog => {
  const entries = registry
    .map((entry) => ({
      signatureId: entry.signature.signatureId,
      title: entry.signature.contract.title,
      status: entry.status,
      contractDigest: contractDigest(entry.signature.contract),
    }))
    .sort((left, right) =>
      left.signatureId < right.signatureId ? -1 : left.signatureId > right.signatureId ? 1 : 0,
    );
  const digest = sha256Hex(canonicalStringify(entries));
  return decodeCatalog({
    schema: SIGNATURE_CATALOG_SCHEMA_LITERAL,
    generatedFrom: "dse-signature-registry",
    entries,
    digest,
  });
};

/**
 * The checked-in generated catalog. It is produced by `deriveSignatureCatalog`
 * over the current registry; the catalog claim test proves the two agree.
 */
export const SIGNATURE_CATALOG: SignatureCatalog = deriveSignatureCatalog();

/**
 * The generated-catalog claim: the checked-in catalog equals a fresh derivation
 * from the registry, and its digest covers its entries. A caller (test or CI)
 * uses this to fail closed on catalog drift.
 */
export const catalogClaimHolds = (catalog: SignatureCatalog = SIGNATURE_CATALOG): boolean => {
  const derived = deriveSignatureCatalog();
  return canonicalStringify(catalog) === canonicalStringify(derived);
};
