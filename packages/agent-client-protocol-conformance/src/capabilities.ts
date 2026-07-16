import {
  AgentStdioHandlerError,
  type AgentStdioReverseHandler,
} from "@openagentsinc/agent-stdio-transport";

export type CapabilityFamily =
  | "filesystem"
  | "terminal"
  | "auth"
  | "modes"
  | "configuration"
  | "session-lifecycle";
export type CapabilityDisposition = "allowed" | "unsupported" | "peer-violation";

export const evaluateCapability = (
  input: Readonly<{
    family: CapabilityFamily;
    advertised: boolean;
    peerInvoked: boolean;
  }>,
): CapabilityDisposition => {
  if (input.advertised) return "allowed";
  return input.peerInvoked ? "peer-violation" : "unsupported";
};

export type NegotiatedClientCapabilities = Readonly<{
  fs: Readonly<{ readTextFile: boolean; writeTextFile: boolean }>;
  terminal: boolean;
}>;

const reverseCapability = (method: string, capabilities: NegotiatedClientCapabilities): boolean => {
  if (method === "fs/read_text_file") return capabilities.fs.readTextFile;
  if (method === "fs/write_text_file") return capabilities.fs.writeTextFile;
  if (method.startsWith("terminal/")) return capabilities.terminal;
  return true;
};

export const capabilityGuardedReverseHandler =
  (
    method: string,
    capabilities: NegotiatedClientCapabilities,
    handler: AgentStdioReverseHandler,
  ): AgentStdioReverseHandler =>
  async (params, context) => {
    if (!reverseCapability(method, capabilities)) {
      throw new AgentStdioHandlerError(-32_601, `method ${method} was not advertised`);
    }
    return handler(params, context);
  };

export type PeerNegotiation = Readonly<{
  authMethodIds: ReadonlyArray<string>;
  modes: ReadonlyArray<string>;
  configOptionIds: ReadonlyArray<string>;
  sessionCapabilities: ReadonlyArray<"list" | "delete" | "resume" | "close">;
}>;

export class UnadvertisedPeerMethodError extends Error {
  constructor(readonly method: string) {
    super(`peer method ${method} was not advertised`);
    this.name = "UnadvertisedPeerMethodError";
  }
}

const record = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" ? (value as Record<string, unknown>) : {};

export const peerNegotiationFromResponses = (
  initialize: unknown,
  session: unknown,
): PeerNegotiation => {
  const init = record(initialize);
  const sessionValue = record(session);
  const capabilities = record(init.agentCapabilities);
  const sessionCapabilities = record(capabilities.sessionCapabilities);
  const modes = record(sessionValue.modes);
  return {
    authMethodIds: Array.isArray(init.authMethods)
      ? init.authMethods
          .map((value) => record(value).id)
          .filter((value): value is string => typeof value === "string")
      : [],
    modes: Array.isArray(modes.availableModes)
      ? modes.availableModes
          .map((value) => record(value).id)
          .filter((value): value is string => typeof value === "string")
      : [],
    configOptionIds: Array.isArray(sessionValue.configOptions)
      ? sessionValue.configOptions
          .map((value) => record(value).id)
          .filter((value): value is string => typeof value === "string")
      : [],
    sessionCapabilities: (["list", "delete", "resume", "close"] as const).filter(
      (name) => sessionCapabilities[name] !== undefined,
    ),
  };
};

export const requireNegotiatedPeerMethod = (method: string, negotiation: PeerNegotiation): void => {
  const allowed = (() => {
    if (method === "authenticate") return negotiation.authMethodIds.length > 0;
    if (method === "session/set_mode") return negotiation.modes.length > 0;
    if (method === "session/set_config_option") return negotiation.configOptionIds.length > 0;
    if (method === "session/list") return negotiation.sessionCapabilities.includes("list");
    if (method === "session/delete") return negotiation.sessionCapabilities.includes("delete");
    if (method === "session/resume") return negotiation.sessionCapabilities.includes("resume");
    if (method === "session/close") return negotiation.sessionCapabilities.includes("close");
    return true;
  })();
  if (!allowed) throw new UnadvertisedPeerMethodError(method);
};
