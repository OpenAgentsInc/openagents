import { Schema as S } from "effect";

import { SandboxTimestamp, Sha256Digest } from "./schemas.ts";

export const BOX_SDK_PROVENANCE = {
  package: "@asciidev/box-sdk",
  version: "0.0.24",
  license: "MIT",
  integrity:
    "sha512-w77vTWA+yrJ5O+FmchCkurjux1UZkQ5yeurnzX/FJTlQulEtj1xp0g/2cSh/GZWLXrgCV0exU99E+NyiilBeHA==",
  shasum: "eb55554ffb5b231888a70e51857f8de336735ac1",
  tarballSha256: "51ac532981c4791ab8662d800cd70b6f18d9a8a01abbd097c627bae3ae45aeb0",
  tarballBytes: 104618,
  licenseSha256: "b7d51a8c93c3b34b607bdb4e15b547e4c7618cf21321c608689b44634f3e3183",
} as const;

export const BOX_CONFORMANCE_LOCKFILE_SHA256 =
  "4a814fe782c61098657f6f4cf96f501fcf1a73c28607e3e0ff1405c66995678b" as const;

export const BOX_OPENAPI_PROVENANCE = {
  source: "https://api.ascii.com/openapi.json",
  sha256: "9ae1e0b7ded4a2d537bfa076f8e047baa2bdf7e3736de2cc397d349457c3cbac",
  capturedOn: "2026-07-19",
} as const;

export const BOX_V1_TRANSLATOR_REF = "openagents.box_v1_translator.v1" as const;

export const BOX_TRANSLATOR_PROVENANCE_SCHEMA_VERSION =
  "openagents.box_translator_provenance.v1" as const;

export const BoxTranslatorProvenanceReceiptSchema = S.Struct({
  schema: S.Literal(BOX_TRANSLATOR_PROVENANCE_SCHEMA_VERSION),
  translatorRef: S.Literal(BOX_V1_TRANSLATOR_REF),
  translatorDigest: Sha256Digest,
  sdkPackage: S.Literal(BOX_SDK_PROVENANCE.package),
  sdkVersion: S.Literal(BOX_SDK_PROVENANCE.version),
  sdkIntegrity: S.Literal(BOX_SDK_PROVENANCE.integrity),
  sdkShasum: S.Literal(BOX_SDK_PROVENANCE.shasum),
  sdkTarballSha256: S.Literal(BOX_SDK_PROVENANCE.tarballSha256),
  sdkTarballBytes: S.Literal(BOX_SDK_PROVENANCE.tarballBytes),
  sdkLicenseSha256: S.Literal(BOX_SDK_PROVENANCE.licenseSha256),
  lockfileSha256: S.Literal(BOX_CONFORMANCE_LOCKFILE_SHA256),
  openApiSha256: S.Literal(BOX_OPENAPI_PROVENANCE.sha256),
  verifiedAt: SandboxTimestamp,
});
export type BoxTranslatorProvenanceReceipt = typeof BoxTranslatorProvenanceReceiptSchema.Type;
