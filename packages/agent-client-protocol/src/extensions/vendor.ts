export type AcpVendorExtensionMethod = Readonly<{
  method: string;
  direction: "client-to-agent" | "agent-to-client";
  kind: "request" | "notification";
  payloadCodec: "opaque-native";
}>;

export type AcpVendorExtensionProfile<Peer extends string> = Readonly<{
  protocol: "Agent Client Protocol";
  schemaRelease: "schema-v1.19.0";
  wireVersion: 1;
  profileVersion: 1;
  peer: Peer;
  gate: "explicit-peer-profile";
  methods: ReadonlyArray<AcpVendorExtensionMethod>;
}>;

export type AcpVendorExtensionDecodeResult =
  | Readonly<{ _tag: "DecodedVendorExtension"; raw: unknown }>
  | Readonly<{
      _tag: "VendorExtensionFailure";
      reason: "peer_profile_required" | "malformed_envelope" | "unknown_vendor_method";
      detail: string;
      raw: unknown;
    }>;

const snapshot = (value: unknown): unknown => {
  try {
    return structuredClone(value);
  } catch {
    return "[unserializable vendor payload]";
  }
};

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

export const decodeAcpVendorExtensionEnvelope = <Peer extends string>(input: {
  profile: AcpVendorExtensionProfile<Peer>;
  enabledPeer: Peer | undefined;
  direction: "client-to-agent" | "agent-to-client";
  message: unknown;
}): AcpVendorExtensionDecodeResult => {
  const raw = snapshot(input.message);
  if (input.enabledPeer !== input.profile.peer) {
    return {
      _tag: "VendorExtensionFailure",
      reason: "peer_profile_required",
      detail: `extension requires explicit ${input.profile.peer} peer profile`,
      raw,
    };
  }
  if (!isObject(raw) || raw.jsonrpc !== "2.0" || typeof raw.method !== "string") {
    return {
      _tag: "VendorExtensionFailure",
      reason: "malformed_envelope",
      detail: "vendor extension must be a JSON-RPC 2.0 request or notification",
      raw,
    };
  }
  const member = input.profile.methods.find(
    (candidate) => candidate.direction === input.direction && candidate.method === raw.method,
  );
  if (member === undefined) {
    return {
      _tag: "VendorExtensionFailure",
      reason: "unknown_vendor_method",
      detail: "method is absent from the enabled vendor profile",
      raw,
    };
  }
  const carriesId = Object.prototype.hasOwnProperty.call(raw, "id");
  if ((member.kind === "request") !== carriesId) {
    return {
      _tag: "VendorExtensionFailure",
      reason: "malformed_envelope",
      detail: `${member.kind} envelope has incorrect id presence`,
      raw,
    };
  }
  if (
    carriesId &&
    raw.id !== null &&
    typeof raw.id !== "string" &&
    !(typeof raw.id === "number" && Number.isSafeInteger(raw.id))
  ) {
    return {
      _tag: "VendorExtensionFailure",
      reason: "malformed_envelope",
      detail: "vendor request id must be null, a string, or a safe integer",
      raw,
    };
  }
  if (
    Object.prototype.hasOwnProperty.call(raw, "result") ||
    Object.prototype.hasOwnProperty.call(raw, "error")
  ) {
    return {
      _tag: "VendorExtensionFailure",
      reason: "malformed_envelope",
      detail: "vendor request or notification must not carry result or error",
      raw,
    };
  }
  return { _tag: "DecodedVendorExtension", raw };
};
