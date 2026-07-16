import { type AcpVendorExtensionProfile, decodeAcpVendorExtensionEnvelope } from "./vendor.ts";

export const GROK_ACP_PROFILE = {
  protocol: "Agent Client Protocol",
  schemaRelease: "schema-v1.19.0",
  wireVersion: 1,
  profileVersion: 1,
  peer: "grok-cli",
  gate: "explicit-peer-profile",
  methods: [
    {
      method: "x.ai/ask_user_question",
      direction: "agent-to-client",
      kind: "request",
      payloadCodec: "opaque-native",
    },
    {
      method: "_x.ai/ask_user_question",
      direction: "agent-to-client",
      kind: "request",
      payloadCodec: "opaque-native",
    },
  ],
} as const satisfies AcpVendorExtensionProfile<"grok-cli">;

export const GROK_ACP_EXTENSIONS = {
  askUserQuestion: GROK_ACP_PROFILE.methods[0].method,
  askUserQuestionCompatibility: GROK_ACP_PROFILE.methods[1].method,
} as const;

export type GrokAcpExtensionMethod = (typeof GROK_ACP_EXTENSIONS)[keyof typeof GROK_ACP_EXTENSIONS];

export const decodeGrokAcpExtensionEnvelope = (
  message: unknown,
  enabledPeer: "grok-cli" | undefined,
) =>
  decodeAcpVendorExtensionEnvelope({
    profile: GROK_ACP_PROFILE,
    enabledPeer,
    direction: "agent-to-client",
    message,
  });
