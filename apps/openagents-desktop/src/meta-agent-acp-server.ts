import { createServer, type Server, type Socket } from "node:net";

import { Effect } from "effect";
import {
  makeAcpAgentServerConnection,
  makeReferenceAdapter,
  metaAgentHarness,
  type AcpServerPermissionRequest,
  type AgentHarness,
  type HarnessToolApprovalDecision,
} from "@openagentsinc/agent-harness-contract";
import type { KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema";

/**
 * META-2 (#9181): the loopback ACP SERVER. This is the inversion of the four
 * ACP CLIENT peers Desktop already consumes in production
 * (`provider-lane-acp*`, the SDK `acp-adapter.ts`): here Desktop exposes the
 * meta-agent AS an ACP agent so an external ACP host (Zed, or — for headless
 * conformance — the SDK's own ACP client adapter) can drive it over a local
 * socket exactly like any other agent.
 *
 * Everything protocol-level is the SDK's proven `makeAcpAgentServerConnection`
 * (ai#39, published in the `@openagentsinc/agent-harness-contract` rc train).
 * This module only owns three desktop-local responsibilities:
 *
 * 1. TRANSPORT. A `node:net` server bound to loopback ONLY (127.0.0.1), one
 *    ACP connection per accepted socket, newline-delimited JSON-RPC 2.0 framing
 *    (ACP ndjson): each inbound line is one message object handed to the
 *    connection's `receive`; each outbound message the connection emits is one
 *    `JSON.stringify(message) + "\n"` written back. The listener never binds a
 *    non-loopback interface, by construction.
 *
 * 2. GATE. Default OFF. Desktop main constructs the server only when
 *    `OPENAGENTS_DESKTOP_ACP_SERVER=1` (or an equivalent owner setting maps to
 *    that flag). With the flag unset there is zero behavior change and no
 *    listener at all.
 *
 * 3. FAIL-CLOSED PERMISSIONING. The ACP surface inherits the SDK server's
 *    deny-by-default posture and, for v0, HARDENS it: the default permission
 *    decider returns `deny` for every `operator_escalation_required` tool call,
 *    so an external ACP host can never make the meta-agent execute a gated tool
 *    without an explicit owner permission broker injected through
 *    `decidePermission`. No tool runs on an unproven approval.
 *
 * Read-only posture: the surface is `operator_read`-shaped. It exposes the
 * conversation/prompt surface, never a mutation or credential path; ACP carries
 * no bearer (the server advertises `authMethods: []`), so loopback binding plus
 * deny-by-default are the v0 containment boundary.
 *
 * v0 BACKING AND THE REAL-DISPATCHER SEAM. The server is backed by the REAL
 * SDK `metaAgentHarness` — the same fleet-wrapping `AgentHarness` every
 * single-runtime adapter conforms to — but over a fixture/echo member for v0
 * (`makeFixtureMetaAgentHarness`), because wiring the live Codex/Claude/Grok
 * member harnesses requires the dispatch lane's runtime files
 * (`codex-local-runtime.ts` / `claude-local-runtime.ts` /
 * `codex-app-server-*` / `*-harness-attempt*`), which are owned by the default-on
 * dispatch-collapse lane and must not be disturbed here. The exact plug-in
 * point is the `makeHarness` option: main passes a factory that returns
 * `metaAgentHarness({ members, route })` built from the dispatch lane's real
 * member harnesses, with no change to this module. That real-fleet backing is
 * the follow-on tracked under the meta-agent epic (#9179).
 */

/** The opt-in gate main checks before constructing the server at all. */
export const META_AGENT_ACP_SERVER_ENV_FLAG = "OPENAGENTS_DESKTOP_ACP_SERVER";

/** The only interface the server is ever allowed to bind. */
export const META_AGENT_ACP_SERVER_LOOPBACK_HOST = "127.0.0.1";

/** Documented read-only posture of the surface (no mutation/credential scope). */
export const META_AGENT_ACP_SERVER_SCOPES = ["operator_read"] as const;

/** The ACP event lane every harness session started by this server is labelled with. */
const META_AGENT_ACP_SERVER_SOURCE: KhalaRuntimeSource = { lane: "agent_client_protocol" };

const LOOPBACK_HOSTS: ReadonlySet<string> = new Set(["127.0.0.1", "::1", "localhost"]);

/** Max bytes buffered for one un-terminated inbound line before the socket is dropped. */
const MAX_LINE_BYTES = 1024 * 1024;

/** True when Desktop main should start the loopback ACP server. Default OFF. */
export const isMetaAgentAcpServerEnabled = (
  env: Readonly<Record<string, string | undefined>>,
): boolean => env[META_AGENT_ACP_SERVER_ENV_FLAG] === "1";

/**
 * v0 deny-by-default permission decider. Every gated (`operator_escalation_
 * required`) tool call is denied because no owner permission broker is wired in
 * v0. Injecting `decidePermission` replaces this with a real, still fail-closed,
 * owner flow.
 */
export const denyMetaAgentAcpPermission = (
  _request: AcpServerPermissionRequest,
): Effect.Effect<HarnessToolApprovalDecision> => Effect.succeed("deny");

/**
 * The v0 fixture backing: the REAL SDK `metaAgentHarness` over one echo member.
 * It proves the whole session/prompt/permission surface and the conformance
 * loop without touching any live dispatch runtime. Swap it for a factory that
 * builds `metaAgentHarness` over the live member fleet at the `makeHarness` seam.
 */
export const makeFixtureMetaAgentHarness = (): AgentHarness =>
  metaAgentHarness({
    harnessId: "openagents",
    members: [
      {
        id: "openagents",
        harness: makeReferenceAdapter({
          harnessId: "openagents-acp-echo",
          scriptWords: ["OpenAgents ", "meta-agent ", "over ", "ACP"],
        }),
      },
    ],
    route: () => "openagents",
  });

export interface MetaAgentAcpServerOptions {
  /**
   * A single harness instance shared by every connection. Prefer `makeHarness`
   * for per-connection isolation; when both are omitted the v0 fixture is used.
   */
  readonly harness?: AgentHarness;
  /** Per-connection harness factory — the real-fleet seam (see module docs). */
  readonly makeHarness?: () => AgentHarness;
  /** Loopback host. Must be a loopback address; defaults to 127.0.0.1. */
  readonly host?: string;
  /** Loopback port. Omitted (or 0) binds an ephemeral port. */
  readonly port?: number;
  /** Event-source labelling for sessions started by this server. */
  readonly source?: KhalaRuntimeSource;
  /** Fail-closed permission decider. Defaults to deny-by-default. */
  readonly decidePermission?: (
    request: AcpServerPermissionRequest,
  ) => Effect.Effect<HarnessToolApprovalDecision>;
  /** Milliseconds to wait for a permission outcome before denying. Default 60000. */
  readonly permissionTimeoutMillis?: number;
}

export interface MetaAgentAcpServer {
  /** The loopback host the listener is bound to. */
  readonly host: string;
  /** The bound loopback port. */
  readonly port: number;
  /**
   * `tcp://127.0.0.1:<port>` — the loopback ACP endpoint (newline-delimited
   * JSON-RPC). An ACP host reaches it directly over TCP, or through a thin
   * stdio↔TCP bridge command (see the Zed demo in the META-2 runbook).
   */
  readonly url: string;
  /** Number of ACP connections currently open. */
  readonly connectionCount: () => number;
  /** Close the listener and shut down every open connection. */
  readonly stop: () => Promise<void>;
}

const assertLoopbackHost = (host: string): void => {
  if (!LOOPBACK_HOSTS.has(host)) {
    throw new Error(
      `meta-agent ACP server refuses to bind non-loopback host ${JSON.stringify(host)}; ` +
        "the surface is loopback-only by construction.",
    );
  }
};

/**
 * Start the loopback ACP server. The caller is responsible for the gate — main
 * only calls this when {@link isMetaAgentAcpServerEnabled} is true. Every
 * accepted socket becomes one SDK ACP connection over the given harness.
 */
export const startMetaAgentAcpServer = (
  options: MetaAgentAcpServerOptions = {},
): Promise<MetaAgentAcpServer> => {
  const host = options.host ?? META_AGENT_ACP_SERVER_LOOPBACK_HOST;
  assertLoopbackHost(host);
  const source = options.source ?? META_AGENT_ACP_SERVER_SOURCE;
  const decidePermission = options.decidePermission ?? denyMetaAgentAcpPermission;
  const permissionTimeoutMillis = options.permissionTimeoutMillis ?? 60_000;
  const harnessFor = (): AgentHarness =>
    options.makeHarness?.() ?? options.harness ?? makeFixtureMetaAgentHarness();

  const openConnections = new Set<Socket>();

  const onSocket = (socket: Socket): void => {
    openConnections.add(socket);
    socket.setEncoding("utf8");

    // Each outbound message the SDK connection emits is one ndjson line.
    const send = (message: unknown): Effect.Effect<void> =>
      Effect.sync(() => {
        if (!socket.destroyed) socket.write(`${JSON.stringify(message)}\n`);
      });

    // Build the SDK ACP connection over a fresh harness for this socket.
    void Effect.runPromise(
      makeAcpAgentServerConnection({
        harness: harnessFor(),
        send,
        source,
        decidePermission,
        permissionTimeoutMillis,
      }),
    )
      .then((connection) => {
        // Inbound delivery MUST be message-serial for one connection: chain each
        // receive so a `session/prompt` cannot race the `session/new` before it.
        let inbound: Promise<void> = Promise.resolve();
        const deliver = (message: unknown): void => {
          inbound = inbound.then(() =>
            Effect.runPromise(connection.receive(message)).catch(() => {}),
          );
        };

        let buffer = "";
        socket.on("data", (chunk: string) => {
          buffer += chunk;
          if (buffer.length > MAX_LINE_BYTES && !buffer.includes("\n")) {
            // A single unterminated line larger than the cap: drop the socket.
            socket.destroy();
            return;
          }
          let newlineIndex = buffer.indexOf("\n");
          while (newlineIndex !== -1) {
            const line = buffer.slice(0, newlineIndex).trim();
            buffer = buffer.slice(newlineIndex + 1);
            if (line.length > 0) {
              try {
                deliver(JSON.parse(line));
              } catch {
                // A non-JSON line is ignored; a conformant ACP host never sends one.
              }
            }
            newlineIndex = buffer.indexOf("\n");
          }
        });

        const teardown = (): void => {
          openConnections.delete(socket);
          void Effect.runPromise(connection.shutdown()).catch(() => {});
        };
        socket.on("close", teardown);
        socket.on("error", teardown);
      })
      .catch(() => {
        openConnections.delete(socket);
        socket.destroy();
      });
  };

  const server: Server = createServer(onSocket);

  return new Promise<MetaAgentAcpServer>((resolve, reject) => {
    server.once("error", reject);
    server.listen(options.port ?? 0, host, () => {
      const address = server.address();
      if (address === null || typeof address === "string") {
        server.close();
        reject(new Error("meta-agent ACP server failed to bind a loopback port"));
        return;
      }
      resolve({
        host,
        port: address.port,
        url: `tcp://${host}:${address.port}`,
        connectionCount: () => openConnections.size,
        stop: () =>
          new Promise<void>((resolveStop) => {
            for (const socket of openConnections) socket.destroy();
            openConnections.clear();
            server.close(() => resolveStop());
          }),
      });
    });
  });
};

/**
 * Convenience for Desktop main: start the server only when the gate is on,
 * otherwise resolve `null`. Keeps the gate decision in one place.
 */
export const startMetaAgentAcpServerIfEnabled = (
  env: Readonly<Record<string, string | undefined>>,
  options: MetaAgentAcpServerOptions = {},
): Promise<MetaAgentAcpServer | null> =>
  isMetaAgentAcpServerEnabled(env) ? startMetaAgentAcpServer(options) : Promise.resolve(null);
