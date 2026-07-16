import Ajv2020, { type ErrorObject, type ValidateFunction } from "ajv/dist/2020.js";

import stableSchemaDocument from "../upstream/schema-v1.19.0/schema.json" with { type: "json" };
import unstableSchemaDocument from "../upstream/schema-v1.19.0/schema.unstable.json" with { type: "json" };
import {
  STABLE_DEFINITION_NAMES,
  UNSTABLE_ONLY_DEFINITION_NAMES,
} from "./generated/definitions.ts";
import { STABLE_METHOD_MANIFEST, UNSTABLE_METHOD_MANIFEST } from "./generated/methods.ts";

export type AcpSchemaLane = "stable" | "unstable";
export type AcpMethodDirection = "client-to-agent" | "agent-to-client" | "protocol";
export type AcpPayloadPhase = "params" | "result";

export type AcpNativeEnvelope = Readonly<{
  schemaRelease: "schema-v1.19.0";
  wireVersion: 1;
  lane: AcpSchemaLane;
  direction: AcpMethodDirection | "definition";
  method: string;
  phase: AcpPayloadPhase;
  retention: "private-native";
  raw: unknown;
}>;

export type AcpDecodedPayload = Readonly<{
  _tag: "Decoded";
  native: AcpNativeEnvelope;
  value: unknown;
}>;

export type AcpDecodeFailure = Readonly<{
  _tag: "DecodeFailure";
  native: AcpNativeEnvelope;
  reason:
    | "unknown_method"
    | "unknown_definition"
    | "invalid_payload"
    | "payload_not_allowed"
    | "malformed_envelope"
    | "unsupported_protocol_version";
  detail: string;
}>;

export type AcpDecodeResult = AcpDecodedPayload | AcpDecodeFailure;

type SchemaDocument = Readonly<{ $defs: Readonly<Record<string, unknown>> }>;
type MethodMember = (typeof UNSTABLE_METHOD_MANIFEST.members)[number];
type AjvRuntime = Readonly<{
  addSchema: (schema: object, key?: string) => unknown;
  compile: (schema: object) => ValidateFunction;
}>;

type Lane = Readonly<{
  ajv: AjvRuntime;
  schemaId: string;
  definitions: ReadonlySet<string>;
  validators: Map<string, ValidateFunction>;
  members: ReadonlyArray<MethodMember>;
}>;

const Ajv2020Constructor = Ajv2020 as unknown as new (options: object) => AjvRuntime;

const makeLane = (
  name: AcpSchemaLane,
  input: SchemaDocument,
  definitions: ReadonlyArray<string>,
  members: ReadonlyArray<MethodMember>,
): Lane => {
  const schemaId = `https://openagents.local/agent-client-protocol/schema-v1.19.0/${name}`;
  const document = { ...structuredClone(input), $id: schemaId };
  const ajv = new Ajv2020Constructor({ strict: false, allErrors: true, validateFormats: false });
  ajv.addSchema(document, schemaId);
  return { ajv, schemaId, definitions: new Set(definitions), validators: new Map(), members };
};

const stableDefinitions = [...STABLE_DEFINITION_NAMES];
const lanes: Readonly<Record<AcpSchemaLane, Lane>> = {
  stable: makeLane(
    "stable",
    stableSchemaDocument,
    stableDefinitions,
    STABLE_METHOD_MANIFEST.members,
  ),
  unstable: makeLane(
    "unstable",
    unstableSchemaDocument,
    [...stableDefinitions, ...UNSTABLE_ONLY_DEFINITION_NAMES],
    UNSTABLE_METHOD_MANIFEST.members,
  ),
};

const snapshotJson = (raw: unknown): unknown => {
  try {
    return structuredClone(raw);
  } catch {
    return "[unserializable native payload]";
  }
};

const freezeJson = (value: unknown, seen = new WeakSet<object>()): unknown => {
  if (typeof value !== "object" || value === null || seen.has(value)) return value;
  seen.add(value);
  for (const child of Object.values(value)) freezeJson(child, seen);
  return Object.freeze(value);
};

const nativeEnvelope = (
  lane: AcpSchemaLane,
  direction: AcpMethodDirection | "definition",
  method: string,
  phase: AcpPayloadPhase,
  raw: unknown,
): AcpNativeEnvelope => ({
  schemaRelease: "schema-v1.19.0",
  wireVersion: 1,
  lane,
  direction,
  method,
  phase,
  retention: "private-native",
  raw: freezeJson(snapshotJson(raw)),
});

export const serializeAcpNativeEnvelope = (native: AcpNativeEnvelope): string => {
  if (native.raw === undefined) throw new TypeError("native payload is not JSON serializable");
  const encoded = JSON.stringify(native);
  if (encoded === undefined) throw new TypeError("native payload is not JSON serializable");
  return encoded;
};

export const parseAcpNativeEnvelope = (encoded: string): AcpNativeEnvelope => {
  const parsed: unknown = JSON.parse(encoded);
  if (
    typeof parsed !== "object" ||
    parsed === null ||
    (parsed as Partial<AcpNativeEnvelope>).schemaRelease !== "schema-v1.19.0" ||
    (parsed as Partial<AcpNativeEnvelope>).wireVersion !== 1 ||
    (parsed as Partial<AcpNativeEnvelope>).retention !== "private-native" ||
    !["stable", "unstable"].includes((parsed as Partial<AcpNativeEnvelope>).lane ?? "") ||
    !["client-to-agent", "agent-to-client", "protocol", "definition"].includes(
      (parsed as Partial<AcpNativeEnvelope>).direction ?? "",
    ) ||
    typeof (parsed as Partial<AcpNativeEnvelope>).method !== "string" ||
    !["params", "result"].includes((parsed as Partial<AcpNativeEnvelope>).phase ?? "") ||
    !Object.prototype.hasOwnProperty.call(parsed, "raw")
  ) {
    throw new TypeError("serialized value is not a supported ACP native envelope");
  }
  return freezeJson(parsed) as AcpNativeEnvelope;
};

const safeDetail = (errors: null | undefined | ReadonlyArray<ErrorObject>): string => {
  const rendered = (errors ?? [])
    .slice(0, 8)
    .map((error) => `${error.instancePath || "/"} ${error.message ?? "is invalid"}`);
  return (rendered.join("; ") || "generated schema rejected payload").slice(0, 1_000);
};

const validatorFor = (
  laneName: AcpSchemaLane,
  definition: string,
): ValidateFunction | undefined => {
  const lane = lanes[laneName];
  if (!lane.definitions.has(definition)) return undefined;
  const cached = lane.validators.get(definition);
  if (cached !== undefined) return cached;
  const validator = lane.ajv.compile({ $ref: `${lane.schemaId}#/$defs/${definition}` });
  lane.validators.set(definition, validator);
  return validator;
};

export const getAcpDefinitionNames = (lane: AcpSchemaLane): ReadonlyArray<string> => [
  ...lanes[lane].definitions,
];

export const compileAcpDefinitionCodecs = (lane: AcpSchemaLane): number => {
  for (const definition of lanes[lane].definitions) validatorFor(lane, definition);
  return lanes[lane].validators.size;
};

export const decodeAcpDefinition = (
  lane: AcpSchemaLane,
  definition: string,
  payload: unknown,
): AcpDecodeResult => {
  const native = nativeEnvelope(lane, "definition", definition, "params", payload);
  if (
    (definition === "InitializeRequest" || definition === "InitializeResponse") &&
    (typeof native.raw !== "object" ||
      native.raw === null ||
      (native.raw as { protocolVersion?: unknown }).protocolVersion !== 1)
  ) {
    return {
      _tag: "DecodeFailure",
      native,
      reason: "unsupported_protocol_version",
      detail: "initialize protocolVersion must equal the supported wire version 1",
    };
  }
  const validator = validatorFor(lane, definition);
  if (validator === undefined)
    return {
      _tag: "DecodeFailure",
      native,
      reason: "unknown_definition",
      detail: "definition is absent from the pinned schema lane",
    };
  if (validator(native.raw)) return { _tag: "Decoded", native, value: snapshotJson(native.raw) };
  return {
    _tag: "DecodeFailure",
    native,
    reason: "invalid_payload",
    detail: safeDetail(validator.errors),
  };
};

export const decodeAcpMethodPayload = (
  input: Readonly<{
    lane: AcpSchemaLane;
    direction: AcpMethodDirection;
    method: string;
    phase: AcpPayloadPhase;
    payload: unknown;
  }>,
): AcpDecodeResult => {
  const native = nativeEnvelope(
    input.lane,
    input.direction,
    input.method,
    input.phase,
    input.payload,
  );
  const member = lanes[input.lane].members.find(
    (candidate) => candidate.direction === input.direction && candidate.method === input.method,
  );
  if (member === undefined)
    return {
      _tag: "DecodeFailure",
      native,
      reason: "unknown_method",
      detail: "method is absent from the pinned method manifest",
    };
  const definition = input.phase === "params" ? member.paramsSchema : member.responseSchema;
  if (
    (definition === "InitializeRequest" || definition === "InitializeResponse") &&
    (typeof native.raw !== "object" ||
      native.raw === null ||
      (native.raw as { protocolVersion?: unknown }).protocolVersion !== 1)
  ) {
    return {
      _tag: "DecodeFailure",
      native,
      reason: "unsupported_protocol_version",
      detail: "initialize protocolVersion must equal the supported wire version 1",
    };
  }
  if (definition === null) {
    if (input.phase === "result" && member.kind !== "request") {
      return {
        _tag: "DecodeFailure",
        native,
        reason: "payload_not_allowed",
        detail: "notifications do not accept response payloads",
      };
    }
    if (
      native.raw === undefined ||
      native.raw === null ||
      (typeof native.raw === "object" &&
        native.raw !== null &&
        !Array.isArray(native.raw) &&
        Object.keys(native.raw).length === 0)
    ) {
      return { _tag: "Decoded", native, value: undefined };
    }
    return {
      _tag: "DecodeFailure",
      native,
      reason: "payload_not_allowed",
      detail: "this method phase does not accept a payload",
    };
  }
  const validator = validatorFor(input.lane, definition);
  if (validator === undefined) {
    return {
      _tag: "DecodeFailure",
      native,
      reason: "unknown_definition",
      detail: `manifest codec ${definition} is unavailable in the pinned schema lane`,
    };
  }
  if (validator(native.raw)) return { _tag: "Decoded", native, value: snapshotJson(native.raw) };
  return {
    _tag: "DecodeFailure",
    native,
    reason: "invalid_payload",
    detail: safeDetail(validator.errors),
  };
};

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const hasOwn = (value: Record<string, unknown>, key: string): boolean =>
  Object.prototype.hasOwnProperty.call(value, key);
const validId = (value: unknown): boolean =>
  value === null ||
  typeof value === "string" ||
  (typeof value === "number" && Number.isSafeInteger(value));

export const decodeAcpJsonRpcEnvelope = (
  input: Readonly<{
    lane: AcpSchemaLane;
    direction: AcpMethodDirection;
    message: unknown;
    expectedMethod?: string;
  }>,
): AcpDecodeResult => {
  const candidate = snapshotJson(input.message);
  const method =
    isObject(candidate) && typeof candidate.method === "string"
      ? candidate.method
      : (input.expectedMethod ?? "<unknown>");
  const phase =
    isObject(candidate) && Object.prototype.hasOwnProperty.call(candidate, "method")
      ? "params"
      : "result";
  const native = nativeEnvelope(input.lane, input.direction, method, phase, candidate);
  const malformed = (detail: string): AcpDecodeFailure => ({
    _tag: "DecodeFailure",
    native,
    reason: "malformed_envelope",
    detail,
  });

  if (!isObject(candidate)) return malformed("JSON-RPC envelope must be an object");
  if (candidate.jsonrpc !== "2.0") return malformed("jsonrpc must equal 2.0");

  if (hasOwn(candidate, "method")) {
    if (typeof candidate.method !== "string" || candidate.method.length === 0) {
      return malformed("request or notification method must be a non-empty string");
    }
    const member = lanes[input.lane].members.find(
      (entry) => entry.direction === input.direction && entry.method === candidate.method,
    );
    if (member === undefined) {
      return {
        _tag: "DecodeFailure",
        native,
        reason: "unknown_method",
        detail: "method is absent from the pinned method manifest",
      };
    }
    const carriesId = hasOwn(candidate, "id");
    if (carriesId && !validId(candidate.id))
      return malformed("request id must be a string or integer");
    if (member.kind === "request" && !carriesId) return malformed("request method requires an id");
    if (member.kind === "notification" && carriesId)
      return malformed("notification method must not carry an id");
    if (hasOwn(candidate, "result") || hasOwn(candidate, "error")) {
      return malformed("request or notification must not carry result or error");
    }
    const decoded = decodeAcpMethodPayload({
      lane: input.lane,
      direction: input.direction,
      method: candidate.method,
      phase: "params",
      payload: candidate.params,
    });
    if (decoded["_tag"] === "DecodeFailure") return { ...decoded, native };
    return { _tag: "Decoded", native, value: candidate };
  }

  if (!hasOwn(candidate, "id") || !validId(candidate.id)) {
    return malformed("response id must be a string or integer");
  }
  const hasResult = hasOwn(candidate, "result");
  const hasError = hasOwn(candidate, "error");
  if (hasResult === hasError)
    return malformed("response must contain exactly one of result or error");
  if (hasError) {
    if (
      !isObject(candidate.error) ||
      typeof candidate.error.code !== "number" ||
      !Number.isInteger(candidate.error.code) ||
      typeof candidate.error.message !== "string"
    ) {
      return malformed("error response requires integer code and string message");
    }
    return { _tag: "Decoded", native, value: candidate };
  }
  if (input.expectedMethod === undefined)
    return malformed("successful response requires expectedMethod correlation");
  const decoded = decodeAcpMethodPayload({
    lane: input.lane,
    direction: input.direction,
    method: input.expectedMethod,
    phase: "result",
    payload: candidate.result,
  });
  if (decoded["_tag"] === "DecodeFailure") return { ...decoded, native };
  return { _tag: "Decoded", native, value: candidate };
};
