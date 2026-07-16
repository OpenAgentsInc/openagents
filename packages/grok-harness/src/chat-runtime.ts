import { randomUUID } from "node:crypto";
import {
  createBoundedAcpNativeEvidenceStore,
  type AcpNativeEvidenceStore,
  type AcpProjectionEvent,
} from "@openagentsinc/agent-client-runtime-bridge";

import {
  createGrokAcpClient,
  initializeAndAuth,
  type CreateGrokAcpClientOptions,
  type GrokAcpClient,
} from "./acp-client.ts";
import { createGrokAcpEventProjector } from "./event-projector.ts";
import { createGrokSessionStore, type GrokSessionStore } from "./session-store.ts";
import type { NeutralChatTurnEvent } from "./types.ts";
import {
  createGrokAcpPeerRuntime,
  type CreateGrokAcpPeerRuntimeOptions,
} from "./grok-peer-runtime.ts";

export type GrokAcpChatRuntime = {
  readonly startThread: (input?: {
    readonly desktopSessionId?: string;
    readonly cwd?: string;
  }) => Promise<{
    readonly threadId: string;
    readonly desktopSessionId: string;
    readonly grokSessionId: string;
  }>;
  readonly startTurn: (input: {
    readonly threadId: string;
    readonly desktopSessionId: string;
    readonly grokSessionId: string;
    readonly prompt: string;
    readonly onEvent?: (event: NeutralChatTurnEvent) => void;
  }) => Promise<{
    readonly turnId: string;
    readonly text: string;
    readonly stopReason: string;
  }>;
  readonly interruptTurn: () => Promise<void>;
  readonly dispose: () => void;
  readonly canonicalEvents: () => readonly AcpProjectionEvent[];
  readonly nativeEvidence: (rawEventRef: string) => unknown;
};

export type CreateGrokAcpChatRuntimeOptions = {
  readonly acp?: CreateGrokAcpClientOptions;
  readonly sessionStore?: GrokSessionStore;
  /** Inject a pre-built client (tests). */
  readonly clientFactory?: () => GrokAcpClient;
  readonly nativeEvidenceStore?: AcpNativeEvidenceStore;
  readonly onCanonicalEvent?: (event: AcpProjectionEvent) => void;
  readonly peerRuntime?: Omit<
    CreateGrokAcpPeerRuntimeOptions,
    | "cwd"
    | "onUpdate"
    | "settleTurn"
    | "admission"
    | "probe"
    | "createTransport"
    | "evidence"
    | "now"
  >;
};

export async function createGrokAcpChatRuntime(
  options: CreateGrokAcpChatRuntimeOptions = {},
): Promise<GrokAcpChatRuntime> {
  if (options.clientFactory === undefined) return createSharedGrokAcpChatRuntime(options);
  const client = options.clientFactory ? options.clientFactory() : createGrokAcpClient(options.acp);
  const store = options.sessionStore ?? createGrokSessionStore();
  const nativeEvidenceStore =
    options.nativeEvidenceStore ??
    createBoundedAcpNativeEvidenceStore({ maxEntries: 8_192, maxBytes: 64 * 1_048_576 });
  const canonicalEvents: AcpProjectionEvent[] = [];
  const onCanonicalEvent = (event: AcpProjectionEvent): void => {
    if (canonicalEvents.length >= 8_192) canonicalEvents.shift();
    canonicalEvents.push(event);
    options.onCanonicalEvent?.(event);
  };

  await initializeAndAuth(client);

  let activeClient = client;
  let interrupted = false;
  const connectionRef = randomUUID();
  let processGeneration = 1;

  return {
    async startThread(input = {}) {
      const desktopSessionId = input.desktopSessionId ?? randomUUID();
      const result = await activeClient.request("session/new", {
        cwd: input.cwd ?? process.cwd(),
        mcpServers: [],
      });
      const grokSessionId = String(result.sessionId ?? "");
      if (!grokSessionId) throw new Error("session/new missing sessionId");

      await store.put({
        desktopSessionId,
        grokSessionId,
        updatedAt: new Date().toISOString(),
        capabilities: { resume: false, fork: false },
      });

      return {
        threadId: desktopSessionId,
        desktopSessionId,
        grokSessionId,
      };
    },

    async startTurn(input) {
      interrupted = false;
      const turnId = randomUUID();
      const projector = createGrokAcpEventProjector({
        threadId: input.threadId,
        turnId,
        grokSessionId: input.grokSessionId,
        connectionRef,
        processGeneration,
        nativeEvidenceStore,
        onCanonicalEvent,
      });
      let projectionQueue = Promise.resolve();

      input.onEvent?.({
        type: "thread_ready",
        threadId: input.threadId,
        turnId,
      });

      activeClient.onSessionUpdate((update, nativeSessionId) => {
        if (interrupted) return;
        projectionQueue = projectionQueue.then(async () => {
          for (const event of await projector.onUpdate(update, nativeSessionId))
            input.onEvent?.(event);
        });
      });

      const promptResult = await activeClient.request(
        "session/prompt",
        {
          sessionId: input.grokSessionId,
          prompt: [{ type: "text", text: input.prompt }],
        },
        120_000,
      );

      await projectionQueue;
      for (const event of await projector.finish(promptResult)) {
        input.onEvent?.(event);
      }

      await store.put({
        desktopSessionId: input.desktopSessionId,
        grokSessionId: input.grokSessionId,
        lastTurnId: turnId,
        updatedAt: new Date().toISOString(),
        capabilities: { resume: false, fork: false },
      });

      return {
        turnId,
        text: projector.text(),
        stopReason: String(promptResult.stopReason ?? "end_turn"),
      };
    },

    async interruptTurn() {
      interrupted = true;
      // ACP cancel is not uniformly available; kill is the hard interrupt.
      activeClient.kill();
      processGeneration += 1;
      activeClient = options.clientFactory
        ? options.clientFactory()
        : createGrokAcpClient(options.acp);
      await initializeAndAuth(activeClient);
    },

    dispose() {
      activeClient.kill();
    },
    canonicalEvents: () => [...canonicalEvents],
    nativeEvidence: (rawEventRef) =>
      "get" in nativeEvidenceStore && typeof nativeEvidenceStore.get === "function"
        ? nativeEvidenceStore.get(rawEventRef)
        : undefined,
  };
}

/**
 * Production compatibility facade. Protocol ownership is entirely delegated
 * to the admitted shared transport/session runtime; the legacy clientFactory
 * branch remains fixture-only until its callers are removed.
 */
const createSharedGrokAcpChatRuntime = async (
  options: CreateGrokAcpChatRuntimeOptions,
): Promise<GrokAcpChatRuntime> => {
  if (options.acp?.command !== undefined)
    throw new TypeError("production Grok ACP launch argv comes from the trusted peer profile");
  const store = options.sessionStore ?? createGrokSessionStore();
  const nativeEvidenceStore =
    options.nativeEvidenceStore ??
    createBoundedAcpNativeEvidenceStore({ maxEntries: 8_192, maxBytes: 64 * 1_048_576 });
  const canonicalEvents: AcpProjectionEvent[] = [];
  const onCanonicalEvent = (event: AcpProjectionEvent): void => {
    if (canonicalEvents.length >= 8_192) canonicalEvents.shift();
    canonicalEvents.push(event);
    options.onCanonicalEvent?.(event);
  };
  type ActiveTurn = Readonly<{
    peerSessionId: string;
    projector: ReturnType<typeof createGrokAcpEventProjector>;
    onEvent?: (event: NeutralChatTurnEvent) => void;
  }>;
  let activeTurn: ActiveTurn | undefined;
  let peer: Awaited<ReturnType<typeof createGrokAcpPeerRuntime>> | undefined;

  const createPeer = async (cwd: string) => {
    const peerOptions: CreateGrokAcpPeerRuntimeOptions = {
      ...options.peerRuntime,
      cwd,
      ...(options.peerRuntime?.environment !== undefined
        ? { environment: options.peerRuntime.environment }
        : options.acp?.env === undefined
          ? {}
          : { environment: options.acp.env }),
      onUpdate: async (record) => {
        const active = activeTurn;
        if (active === undefined || record.sessionId !== active.peerSessionId) return;
        for (const event of await active.projector.onUpdate(
          record.update as Record<string, unknown>,
          record.sessionId,
          record.notificationMeta,
        ))
          active.onEvent?.(event);
      },
      settleTurn: async (settlement) => {
        const active = activeTurn;
        if (active === undefined || settlement.peerSessionId !== active.peerSessionId) return;
        for (const event of await active.projector.finish({
          stopReason: settlement.stopReason,
          ...(settlement.completionMeta === undefined ? {} : { _meta: settlement.completionMeta }),
        }))
          active.onEvent?.(event);
      },
    };
    const created = await createGrokAcpPeerRuntime(peerOptions);
    const started = await created.start();
    if (!started.ok) throw new Error(started.safeDetail);
    return created;
  };

  return {
    async startThread(input = {}) {
      if (peer !== undefined) throw new Error("Grok ACP profile owns one session per process");
      const cwd = input.cwd ?? options.acp?.cwd ?? process.cwd();
      peer = await createPeer(cwd);
      const desktopSessionId = input.desktopSessionId ?? randomUUID();
      const attached = await peer.newSession({
        cwd,
        canonicalThreadSeed: desktopSessionId,
      });
      if (!attached.ok) throw new Error(attached.safeDetail);
      await store.put({
        desktopSessionId,
        grokSessionId: attached.value.peerSessionId,
        updatedAt: new Date().toISOString(),
        capabilities: {
          resume: peer.evidence()?.capabilities.resume ?? false,
          fork: peer.evidence()?.capabilities.fork ?? false,
        },
      });
      return {
        threadId: attached.value.threadId,
        desktopSessionId,
        grokSessionId: attached.value.peerSessionId,
      };
    },
    async startTurn(input) {
      if (peer === undefined) throw new Error("Grok ACP session has not started");
      const turnId = randomUUID();
      const projector = createGrokAcpEventProjector({
        threadId: input.threadId,
        turnId,
        grokSessionId: input.grokSessionId,
        ...(peer.evidence()?.connectionRef === undefined
          ? {}
          : { connectionRef: peer.evidence()!.connectionRef }),
        ...(peer.evidence()?.runtimeGeneration === undefined
          ? {}
          : { processGeneration: peer.evidence()!.runtimeGeneration }),
        nativeEvidenceStore,
        onCanonicalEvent,
      });
      activeTurn = {
        peerSessionId: input.grokSessionId,
        projector,
        ...(input.onEvent === undefined ? {} : { onEvent: input.onEvent }),
      };
      input.onEvent?.({ type: "thread_ready", threadId: input.threadId, turnId });
      try {
        const outcome = await peer.prompt(input.grokSessionId, [
          { type: "text", text: input.prompt },
        ]);
        if (!outcome.ok) throw new Error(outcome.safeDetail);
        await store.put({
          desktopSessionId: input.desktopSessionId,
          grokSessionId: input.grokSessionId,
          lastTurnId: turnId,
          updatedAt: new Date().toISOString(),
          capabilities: {
            resume: peer.evidence()?.capabilities.resume ?? false,
            fork: peer.evidence()?.capabilities.fork ?? false,
          },
        });
        return { turnId, text: projector.text(), stopReason: outcome.value.stopReason };
      } finally {
        activeTurn = undefined;
      }
    },
    async interruptTurn() {
      if (peer === undefined || activeTurn === undefined) return;
      const outcome = await peer.cancel(activeTurn.peerSessionId, "user");
      if (!outcome.ok) throw new Error(outcome.safeDetail);
    },
    dispose() {
      void peer?.shutdown();
    },
    canonicalEvents: () => [...canonicalEvents],
    nativeEvidence: (rawEventRef) =>
      "get" in nativeEvidenceStore && typeof nativeEvidenceStore.get === "function"
        ? nativeEvidenceStore.get(rawEventRef)
        : undefined,
  };
};
