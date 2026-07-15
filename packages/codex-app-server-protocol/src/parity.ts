import { Schema } from "effect";
import bundledManifest from "../manifests/bundled-0.144.1.json" with { type: "json" };
import currentSourceManifest from "../manifests/current-source.json" with { type: "json" };

export const ProtocolDirection = Schema.Literals([
  "client-request",
  "client-notification",
  "server-request",
  "server-notification",
]);

export const ProtocolMember = Schema.Struct({
  method: Schema.String,
  direction: ProtocolDirection,
  stability: Schema.Literals(["experimental-gated", "stable-or-runtime-declared"]),
  generation: Schema.Literals([
    "upstream-generated",
    "deprecated-compatibility",
    "runtime-compatibility",
  ]),
  paramsSchema: Schema.String,
  resultSchema: Schema.NullOr(Schema.String),
  errorSchema: Schema.NullOr(Schema.String),
  decodeState: Schema.Literal("generated"),
  handlerState: Schema.String,
  nativeProjection: Schema.String,
  productSurface: Schema.String,
  authorityClass: Schema.String,
  fixture: Schema.String,
  realBinaryProof: Schema.String,
});

export const ProtocolManifest = Schema.Struct({
  schemaVersion: Schema.Literal(1),
  identity: Schema.Record(Schema.String, Schema.Json),
  experimentalApiDefault: Schema.Literal(false),
  counts: Schema.Record(Schema.String, Schema.Number),
  stableCounts: Schema.Record(Schema.String, Schema.Number),
  requestPartition: Schema.Struct({
    generatedStable: Schema.Number,
    deprecatedCompatibility: Schema.Number,
    experimentalGated: Schema.Number,
  }),
  generatedSchemaSha256: Schema.String,
  members: Schema.Array(ProtocolMember),
});

export type ProtocolManifest = typeof ProtocolManifest.Type;
export type ProtocolMember = typeof ProtocolMember.Type;

const decodeManifest = Schema.decodeUnknownSync(ProtocolManifest);

export const currentSourceProtocolManifest = decodeManifest(currentSourceManifest);
export const bundledCodex01441ProtocolManifest = decodeManifest(bundledManifest);
