/**
 * SARAH-NR-03 load-proof harness.
 *
 * Measures publish (EVENT→OK) and subscribe (REQ→EOSE) rates and latencies
 * against a local mock, a startTestRelay host, or a remote relay URL.
 */
import {
  classifyClientError,
  connectRelay,
  publishEvent,
  subscribeOnce,
  type LoadProofSocket,
} from "./client.js";
import { createSignedEvent, generatePrivateKeyHex } from "./event.js";
import { buildPhaseMetrics, emptyErrorClasses, evaluateThresholds } from "./metrics.js";
import { startMockRelay, type MockRelayHandle } from "./mock-relay.js";
import {
  DEFAULT_LOCAL_LOAD_PROOF_CONFIG,
  LOCAL_LOAD_PROOF_THRESHOLDS,
  NOSTR_EFFECT_NODE_EXPORTS,
  NOSTR_EFFECT_NODE_PIN,
  REMOTE_LOAD_PROOF_THRESHOLDS,
  type LoadProofConfig,
  type LoadProofErrorClass,
  type LoadProofReport,
  type LoadProofThresholds,
} from "./types.js";

export type StartedHost = {
  readonly mode: "local_started" | "mock";
  readonly relayUrl: string;
  readonly nostrEffectPin: string | null;
  stop: () => Promise<void>;
};

type StartTestRelayFn = (port: number) => Promise<{
  port: number;
  stop: () => unknown;
}>;

const tryImportStartTestRelay = async (): Promise<StartTestRelayFn | null> => {
  try {
    const mod = (await import(
      /* @vite-ignore */ "nostr-effect/relay/node"
    )) as { startTestRelay?: StartTestRelayFn };
    return typeof mod.startTestRelay === "function" ? mod.startTestRelay : null;
  } catch {
    return null;
  }
};

/**
 * Start a local host for the load proof.
 * Prefer nostr-effect startTestRelay. Fall back to the package mock.
 */
export const startLocalLoadProofHost = async (options?: {
  preferMock?: boolean;
  port?: number;
}): Promise<StartedHost> => {
  if (!options?.preferMock) {
    const startTestRelay = await tryImportStartTestRelay();
    if (startTestRelay) {
      const port = options?.port ?? 31_000 + Math.floor(Math.random() * 9_000);
      const handle = await startTestRelay(port);
      return {
        mode: "local_started",
        relayUrl: `ws://127.0.0.1:${handle.port}`,
        nostrEffectPin: NOSTR_EFFECT_NODE_PIN,
        stop: async () => {
          const result = handle.stop();
          if (
            result &&
            typeof result === "object" &&
            "pipe" in result &&
            typeof (result as { pipe: unknown }).pipe === "function"
          ) {
            const { Effect } = await import("effect");
            await Effect.runPromise(result as never);
            return;
          }
          await Promise.resolve(result);
        },
      };
    }
  }

  const mock: MockRelayHandle = await startMockRelay(options?.port ?? 0);
  return {
    mode: "mock",
    relayUrl: mock.url,
    nostrEffectPin: null,
    stop: () => mock.stop(),
  };
};

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

const drainAuth = async (
  socket: LoadProofSocket,
  timeoutMs: number,
): Promise<void> => {
  try {
    await socket.waitFor((m) => m[0] === "AUTH", Math.min(timeoutMs, 500));
  } catch {
    /* AUTH is optional for open relays */
  }
};

const runPublishWorkers = async (input: {
  config: LoadProofConfig;
  privateKeyHex: string;
}): Promise<{
  latenciesMs: number[];
  failures: number;
  errorClasses: Record<LoadProofErrorClass, number>;
}> => {
  const latenciesMs: number[] = [];
  let failures = 0;
  const errorClasses = emptyErrorClasses();
  const endAt = Date.now() + input.config.durationMs;
  let counter = 0;

  const worker = async () => {
    let socket: LoadProofSocket | null = null;
    try {
      socket = await connectRelay(
        input.config.relayUrl,
        input.config.connectTimeoutMs,
      );
      await drainAuth(socket, input.config.operationTimeoutMs);
      while (Date.now() < endAt) {
        counter += 1;
        const event = createSignedEvent({
          privateKeyHex: input.privateKeyHex,
          content: `load-proof-pub-${counter}`,
          tags: [["client", "openagents.sarah.relay_load_proof"]],
        });
        try {
          const ms = await publishEvent(
            socket,
            event,
            input.config.operationTimeoutMs,
          );
          latenciesMs.push(ms);
        } catch (error) {
          failures += 1;
          errorClasses[classifyClientError(error)] += 1;
        }
        await sleep(input.config.publishIntervalMs);
      }
    } catch (error) {
      failures += 1;
      errorClasses[classifyClientError(error)] += 1;
    } finally {
      socket?.close();
    }
  };

  await Promise.all(
    Array.from({ length: input.config.publishers }, () => worker()),
  );
  return { latenciesMs, failures, errorClasses };
};

const runSubscribeWorkers = async (input: {
  config: LoadProofConfig;
  authorPubkey: string;
}): Promise<{
  latenciesMs: number[];
  failures: number;
  errorClasses: Record<LoadProofErrorClass, number>;
}> => {
  const latenciesMs: number[] = [];
  let failures = 0;
  const errorClasses = emptyErrorClasses();
  const endAt = Date.now() + input.config.durationMs;

  const worker = async () => {
    let socket: LoadProofSocket | null = null;
    try {
      socket = await connectRelay(
        input.config.relayUrl,
        input.config.connectTimeoutMs,
      );
      await drainAuth(socket, input.config.operationTimeoutMs);
      while (Date.now() < endAt) {
        try {
          const ms = await subscribeOnce(
            socket,
            {
              authors: [input.authorPubkey],
              kinds: [1],
              limit: 10,
            },
            input.config.operationTimeoutMs,
          );
          latenciesMs.push(ms);
        } catch (error) {
          failures += 1;
          errorClasses[classifyClientError(error)] += 1;
        }
        await sleep(10);
      }
    } catch (error) {
      failures += 1;
      errorClasses[classifyClientError(error)] += 1;
    } finally {
      socket?.close();
    }
  };

  await Promise.all(
    Array.from({ length: input.config.subscribers }, () => worker()),
  );
  return { latenciesMs, failures, errorClasses };
};

export const resolveThresholds = (remote: boolean): LoadProofThresholds =>
  remote ? REMOTE_LOAD_PROOF_THRESHOLDS : LOCAL_LOAD_PROOF_THRESHOLDS;

export const runLoadProof = async (input: {
  relayUrl: string;
  hostMode: LoadProofReport["hostMode"];
  nostrEffectPin?: string | null;
  config?: Partial<Omit<LoadProofConfig, "relayUrl">>;
  remote?: boolean;
}): Promise<LoadProofReport> => {
  const thresholds =
    input.config?.thresholds ?? resolveThresholds(Boolean(input.remote));
  const config: LoadProofConfig = {
    ...DEFAULT_LOCAL_LOAD_PROOF_CONFIG,
    ...input.config,
    relayUrl: input.relayUrl,
    thresholds,
  };

  const privateKeyHex = generatePrivateKeyHex();
  const seedEvent = createSignedEvent({
    privateKeyHex,
    content: "load-proof-seed",
  });

  // Seed one event so subscribe filters can match stored content.
  {
    const seedSocket = await connectRelay(
      config.relayUrl,
      config.connectTimeoutMs,
    );
    try {
      await drainAuth(seedSocket, config.operationTimeoutMs);
      await publishEvent(seedSocket, seedEvent, config.operationTimeoutMs);
    } finally {
      seedSocket.close();
    }
  }

  const measuredAt = new Date().toISOString();
  const started = Date.now();

  const [publishRaw, subscribeRaw] = await Promise.all([
    runPublishWorkers({ config, privateKeyHex }),
    runSubscribeWorkers({ config, authorPubkey: seedEvent.pubkey }),
  ]);

  const durationMs = Date.now() - started;
  const publish = buildPhaseMetrics({
    phase: "publish",
    durationMs,
    latenciesMs: publishRaw.latenciesMs,
    failures: publishRaw.failures,
    errorClasses: publishRaw.errorClasses,
  });
  const subscribe = buildPhaseMetrics({
    phase: "subscribe",
    durationMs,
    latenciesMs: subscribeRaw.latenciesMs,
    failures: subscribeRaw.failures,
    errorClasses: subscribeRaw.errorClasses,
  });

  const { pass, failures } = evaluateThresholds({
    publish,
    subscribe,
    thresholds,
  });

  const notes: string[] = [
    `Node entry pin ${NOSTR_EFFECT_NODE_PIN}`,
    `exports host=${NOSTR_EFFECT_NODE_EXPORTS.host} postgres=${NOSTR_EFFECT_NODE_EXPORTS.postgres}`,
  ];
  if (input.hostMode === "mock") {
    notes.push(
      "Host was the package mock relay (nostr-effect startTestRelay unavailable).",
    );
  }
  if (input.hostMode === "local_started") {
    notes.push("Host was nostr-effect startTestRelay (MemoryEventStore).");
  }
  if (input.hostMode === "remote") {
    notes.push("Host was a remote relay URL (production or staging).");
  }

  return {
    schema: "openagents.sarah.relay_load_proof.v1",
    packet: "SARAH-NR-03",
    measuredAt,
    relayUrl: config.relayUrl,
    hostMode: input.hostMode,
    nostrEffectPin: input.nostrEffectPin ?? null,
    durationMs,
    publish,
    subscribe,
    thresholds,
    pass,
    failures,
    notes,
  };
};
