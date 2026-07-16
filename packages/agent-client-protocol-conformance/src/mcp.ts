export type AcpCredentialReference = Readonly<{
  id: string;
  expiresAt: string;
}>;

export type ReferencedMcpServer = Readonly<{
  name: string;
  type: "http" | "sse";
  url: string;
  credentialRef?: AcpCredentialReference;
}>;

export type AcpCredentialReferenceResolver = (
  reference: AcpCredentialReference,
) => string | undefined | Promise<string | undefined>;

export class McpReferenceError extends Error {
  constructor(readonly kind: "expired" | "invalid") {
    super(`MCP credential reference is ${kind}`);
    this.name = "McpReferenceError";
  }
}

export const materializeMcpServers = async (
  servers: ReadonlyArray<ReferencedMcpServer>,
  options: Readonly<{ now: Date; resolve: AcpCredentialReferenceResolver }>,
): Promise<ReadonlyArray<unknown>> =>
  Promise.all(
    servers.map(async (server) => {
      if (server.credentialRef === undefined) return server;
      const expiresAt = Date.parse(server.credentialRef.expiresAt);
      if (!Number.isFinite(expiresAt) || server.credentialRef.id.trim().length === 0) {
        throw new McpReferenceError("invalid");
      }
      if (expiresAt <= options.now.getTime()) {
        throw new McpReferenceError("expired");
      }
      const secret = await options.resolve(server.credentialRef);
      if (secret === undefined || secret.trim().length === 0)
        throw new McpReferenceError("invalid");
      return {
        name: server.name,
        type: server.type,
        url: server.url,
        headers: [{ name: "Authorization", value: `Bearer ${secret}` }],
      };
    }),
  );
