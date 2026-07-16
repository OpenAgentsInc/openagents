import { createHash } from "node:crypto";
import { decodeStableAcpMethodPayload } from "@openagentsinc/agent-client-protocol/stable";
import type {
  CreateTerminalRequest,
  KillTerminalRequest,
  ReadTextFileRequest,
  ReleaseTerminalRequest,
  RequestPermissionRequest,
  TerminalOutputRequest,
  WaitForTerminalExitRequest,
  WriteTextFileRequest,
} from "@openagentsinc/agent-client-protocol/stable";
import {
  AgentStdioHandlerError,
  type AgentStdioReverseHandler,
} from "@openagentsinc/agent-stdio-transport";
import {
  AcpAuthorityFault,
  toAcpAuthorityProtocolError,
  type AcpAuthorityBridge,
  type AcpAuthorityContext,
} from "./authority.ts";

type ReverseHost = Readonly<{
  registerReverseHandler(method: string, handler: AgentStdioReverseHandler): () => void;
}>;
type HandlerContext = Parameters<AgentStdioReverseHandler>[1];

const methods = [
  "session/request_permission",
  "fs/read_text_file",
  "fs/write_text_file",
  "terminal/create",
  "terminal/output",
  "terminal/wait_for_exit",
  "terminal/kill",
  "terminal/release",
] as const;

export const registerAcpAuthorityReverseHandlers = (
  input: Readonly<{
    transport: ReverseHost;
    bridge: AcpAuthorityBridge;
    contextFor: (
      params: unknown,
      transport: HandlerContext,
    ) => Omit<AcpAuthorityContext, "requestRef" | "generation" | "signal">;
  }>,
): (() => void) => {
  const unregistrations = methods.map((method) =>
    input.transport.registerReverseHandler(method, async (params, transportContext) => {
      const decoded = decodeStableAcpMethodPayload({
        direction: "agent-to-client",
        method,
        phase: "params",
        payload: params,
      });
      if (decoded._tag !== "Decoded")
        throw new AgentStdioHandlerError(-32602, "Invalid ACP reverse-request parameters.", {
          reason: decoded.reason,
          retryable: false,
        });
      const requestRef = `request.${createHash("sha256")
        .update(`${transportContext.generation}:${String(transportContext.requestId)}`)
        .digest("hex")
        .slice(0, 32)}`;
      const context = {
        ...input.contextFor(decoded.value, transportContext),
        requestRef,
        generation: transportContext.generation,
        signal: transportContext.signal,
      };
      try {
        let result: unknown;
        switch (method) {
          case "session/request_permission":
            result = await input.bridge.requestPermission(
              decoded.value as RequestPermissionRequest,
              context,
            );
            break;
          case "fs/read_text_file":
            result = await input.bridge.readTextFile(decoded.value as ReadTextFileRequest, context);
            break;
          case "fs/write_text_file":
            result = await input.bridge.writeTextFile(
              decoded.value as WriteTextFileRequest,
              context,
            );
            break;
          case "terminal/create":
            result = await input.bridge.createTerminal(
              decoded.value as CreateTerminalRequest,
              context,
            );
            break;
          case "terminal/output":
            result = await input.bridge.terminalOutput(
              decoded.value as TerminalOutputRequest,
              context,
            );
            break;
          case "terminal/wait_for_exit":
            result = await input.bridge.waitForTerminalExit(
              decoded.value as WaitForTerminalExitRequest,
              context,
            );
            break;
          case "terminal/kill":
            result = await input.bridge.killTerminal(decoded.value as KillTerminalRequest, context);
            break;
          case "terminal/release":
            result = await input.bridge.releaseTerminal(
              decoded.value as ReleaseTerminalRequest,
              context,
            );
            break;
        }
        const encoded = decodeStableAcpMethodPayload({
          direction: "agent-to-client",
          method,
          phase: "result",
          payload: result,
        });
        if (encoded._tag !== "Decoded")
          throw new AgentStdioHandlerError(
            -32603,
            "ACP authority broker returned an invalid response.",
            { reason: "invalid_broker_response", retryable: false },
          );
        return result;
      } catch (error) {
        if (error instanceof AgentStdioHandlerError) throw error;
        const fault =
          error instanceof AcpAuthorityFault ? error : new AcpAuthorityFault("broker_failure");
        const protocol = toAcpAuthorityProtocolError(fault);
        throw new AgentStdioHandlerError(protocol.code, protocol.message, protocol.data);
      }
    }),
  );
  return () => {
    for (const unregister of unregistrations) unregister();
  };
};
