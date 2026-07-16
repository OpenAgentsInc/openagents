export type * from "./generated/stable-types.ts";
export type * from "./generated/unstable-types.ts";
export {
  STABLE_DEFINITION_NAMES,
  UNSTABLE_ONLY_DEFINITION_NAMES,
} from "./generated/definitions.ts";
export { UNSTABLE_METHOD_MANIFEST } from "./generated/methods.ts";

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

export const compileUnstableAcpDefinitionCodecs = () => compileAcpDefinitionCodecs("unstable");
export const getUnstableAcpDefinitionNames = () => getAcpDefinitionNames("unstable");
export const decodeUnstableAcpDefinition = (definition: string, payload: unknown) =>
  decodeAcpDefinition("unstable", definition, payload);
export const decodeUnstableAcpMethodPayload = (
  input: Omit<Parameters<typeof decodeAcpMethodPayload>[0], "lane">,
) => decodeAcpMethodPayload({ lane: "unstable", ...input });
export const decodeUnstableAcpJsonRpcEnvelope = (
  input: Omit<Parameters<typeof decodeAcpJsonRpcEnvelope>[0], "lane">,
) => decodeAcpJsonRpcEnvelope({ lane: "unstable", ...input });
export const serializeUnstableAcpNativeEnvelope = (native: AcpNativeEnvelope) => {
  if (native.lane !== "unstable") throw new TypeError("unstable API cannot serialize another lane");
  return serializeAcpNativeEnvelope(native);
};
export const parseUnstableAcpNativeEnvelope = (encoded: string) => {
  const native = parseAcpNativeEnvelope(encoded);
  if (native.lane !== "unstable") throw new TypeError("unstable API cannot parse another lane");
  return native;
};
