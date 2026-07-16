import { type AcpVendorExtensionProfile, decodeAcpVendorExtensionEnvelope } from "./vendor.ts";

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

export const decodeCursorAcpExtensionEnvelope = (input: {
  message: unknown;
  enabledPeer: "cursor-agent" | undefined;
  direction: "client-to-agent" | "agent-to-client";
}) => decodeAcpVendorExtensionEnvelope({ profile: CURSOR_ACP_PROFILE, ...input });
