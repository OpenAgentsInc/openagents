export type * from "./generated/stable-types.ts";
export { STABLE_DEFINITION_NAMES } from "./generated/definitions.ts";
export { STABLE_METHOD_MANIFEST } from "./generated/methods.ts";
export type { AcpDecodeFailure, AcpDecodeResult, AcpDecodedPayload } from "./runtime.ts";

import {
  compileAcpDefinitionCodecs,
  decodeAcpDefinition,
  decodeAcpJsonRpcEnvelope,
  decodeAcpMethodPayload,
  getAcpDefinitionNames,
  parseAcpNativeEnvelope,
  serializeAcpNativeEnvelope,
  type AcpNativeEnvelope,
} from "./runtime.ts";

export const compileStableAcpDefinitionCodecs = () => compileAcpDefinitionCodecs("stable");
export const getStableAcpDefinitionNames = () => getAcpDefinitionNames("stable");
export const decodeStableAcpDefinition = (definition: string, payload: unknown) =>
  decodeAcpDefinition("stable", definition, payload);
export const decodeStableAcpMethodPayload = (
  input: Omit<Parameters<typeof decodeAcpMethodPayload>[0], "lane">,
) => decodeAcpMethodPayload({ lane: "stable", ...input });
export const decodeStableAcpJsonRpcEnvelope = (
  input: Omit<Parameters<typeof decodeAcpJsonRpcEnvelope>[0], "lane">,
) => decodeAcpJsonRpcEnvelope({ lane: "stable", ...input });
export const serializeStableAcpNativeEnvelope = (native: AcpNativeEnvelope) => {
  if (native.lane !== "stable") throw new TypeError("stable API cannot serialize another lane");
  return serializeAcpNativeEnvelope(native);
};
export const parseStableAcpNativeEnvelope = (encoded: string) => {
  const native = parseAcpNativeEnvelope(encoded);
  if (native.lane !== "stable") throw new TypeError("stable API cannot parse another lane");
  return native;
};
