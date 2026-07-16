import { type AcpVendorExtensionProfile, decodeAcpVendorExtensionEnvelope } from "./vendor.ts";
import { decodeStableAcpDefinition, type SessionConfigOption } from "../stable.ts";

export const CURSOR_ACP_PROFILE = {
  protocol: "Agent Client Protocol",
  schemaRelease: "schema-v1.19.0",
  wireVersion: 1,
  profileVersion: 1,
  peer: "cursor-agent",
  gate: "explicit-peer-profile",
  methods: [
    {
      method: "cursor/ask_question",
      direction: "agent-to-client",
      kind: "request",
      payloadCodec: "opaque-native",
    },
    {
      method: "cursor/create_plan",
      direction: "agent-to-client",
      kind: "request",
      payloadCodec: "opaque-native",
    },
    {
      method: "cursor/update_todos",
      direction: "agent-to-client",
      kind: "notification",
      payloadCodec: "opaque-native",
    },
    {
      method: "cursor/list_available_models",
      direction: "client-to-agent",
      kind: "request",
      payloadCodec: "opaque-native",
    },
  ],
} as const satisfies AcpVendorExtensionProfile<"cursor-agent">;

export const CURSOR_ACP_EXTENSIONS = {
  inboundRequests: ["cursor/ask_question", "cursor/create_plan"],
  inboundNotifications: ["cursor/update_todos"],
  outboundRequests: ["cursor/list_available_models"],
} as const;

export type CursorAcpInboundRequest = (typeof CURSOR_ACP_EXTENSIONS.inboundRequests)[number];
export type CursorAcpInboundNotification =
  (typeof CURSOR_ACP_EXTENSIONS.inboundNotifications)[number];
export type CursorAcpOutboundRequest = (typeof CURSOR_ACP_EXTENSIONS.outboundRequests)[number];

export type CursorAvailableModel = Readonly<{
  value: string;
  name: string;
  configOptions?: ReadonlyArray<SessionConfigOption>;
}>;

export type CursorListAvailableModelsResponse = Readonly<{
  models: ReadonlyArray<CursorAvailableModel>;
}>;

const object = (value: unknown): Record<string, unknown> | undefined =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;

const boundedString = (value: unknown, max: number): string | undefined =>
  typeof value === "string" && value.length > 0 && value.length <= max ? value : undefined;

/** Versioned, bounded decoder for Cursor's client-to-agent model extension. */
export const decodeCursorListAvailableModelsResponse = (
  input: unknown,
): CursorListAvailableModelsResponse | undefined => {
  const value = object(input);
  if (value === undefined || !Array.isArray(value.models) || value.models.length > 512)
    return undefined;
  const models: CursorAvailableModel[] = [];
  const seen = new Set<string>();
  for (const candidate of value.models) {
    const model = object(candidate);
    const id = boundedString(model?.value, 512);
    const name = boundedString(model?.name, 512);
    if (model === undefined || id === undefined || name === undefined || seen.has(id))
      return undefined;
    if (model.configOptions !== undefined && !Array.isArray(model.configOptions)) return undefined;
    if (Array.isArray(model.configOptions) && model.configOptions.length > 128) return undefined;
    const configOptions = Array.isArray(model.configOptions)
      ? model.configOptions.map((option) =>
          decodeStableAcpDefinition("SessionConfigOption", option),
        )
      : [];
    if (configOptions.some((option) => option._tag === "DecodeFailure")) return undefined;
    seen.add(id);
    models.push(
      Object.freeze({
        value: id,
        name,
        ...(Array.isArray(model.configOptions)
          ? {
              configOptions: Object.freeze(
                configOptions.map((option) =>
                  structuredClone(
                    (option as { _tag: "Decoded"; value: SessionConfigOption }).value,
                  ),
                ),
              ),
            }
          : {}),
      }),
    );
  }
  return Object.freeze({ models: Object.freeze(models) });
};

export const decodeCursorAcpExtensionEnvelope = (input: {
  message: unknown;
  enabledPeer: "cursor-agent" | undefined;
  direction: "client-to-agent" | "agent-to-client";
}) => decodeAcpVendorExtensionEnvelope({ profile: CURSOR_ACP_PROFILE, ...input });
