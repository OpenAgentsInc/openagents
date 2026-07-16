import { resolve } from "node:path";

import {
  AgentStdioTransport,
  type AgentStdioReverseHandler,
  type AgentStdioTransportReceipt,
} from "@openagentsinc/agent-stdio-transport";

import { sanitizeTranscript, type AcpTranscriptEntry } from "./transcript.ts";

export type PeerAction = Readonly<{
  method: string;
  result?: unknown;
  error?: Readonly<{ code: number; message: string }>;
  notifications?: ReadonlyArray<Readonly<{ method: string; params: unknown }>>;
  notificationsAfterResponse?: ReadonlyArray<Readonly<{ method: string; params: unknown }>>;
  afterResponseTurns?: number;
  reverseRequests?: ReadonlyArray<Readonly<{ method: string; params: unknown }>>;
  delayMs?: number;
  fragmentBytes?: number;
  stderr?: string;
  raw?: string;
  exitCode?: number;
  expectParamsSha256?: string;
  ignoreReverseErrors?: boolean;
  duplicateResponse?: boolean;
  lateDuplicateMs?: number;
  exitBeforeResponse?: boolean;
}>;

export type AcpPeerScenario = Readonly<{
  name: string;
  actions: ReadonlyArray<PeerAction>;
  exitOnStart?: number;
  pauseInput?: boolean;
}>;

export type PeerScenarioResult = Readonly<{
  results: ReadonlyArray<unknown>;
  notifications: ReadonlyArray<Readonly<{ method: string; params: unknown }>>;
  transcript: ReadonlyArray<AcpTranscriptEntry>;
  receipt: AgentStdioTransportReceipt;
}>;

export type PrivateNativeObservation = Readonly<{
  generation: number;
  direction: "inbound" | "outbound";
  at: string;
  bytes: number;
  sha256: string;
  raw: unknown;
}>;

export const definePeerScenario = (scenario: AcpPeerScenario): AcpPeerScenario => scenario;

export const startPeerScenarioTransport = (
  scenario: AcpPeerScenario,
  limits: Partial<Parameters<typeof AgentStdioTransport.start>[0]["limits"]> = {},
): Promise<AgentStdioTransport> => {
  const fixture = resolve(import.meta.dirname, "../scripts/scripted-peer.mjs");
  return AgentStdioTransport.start({
    executable: process.execPath,
    args: [fixture],
    env: { OA_ACP_SCENARIO: JSON.stringify(scenario) },
    limits: { requestTimeoutMs: 2_000, reverseRequestTimeoutMs: 1_000, ...limits },
  });
};

export const runPeerScenario = async (
  scenario: AcpPeerScenario,
  requests: ReadonlyArray<
    Readonly<{
      method: string;
      params: unknown;
      kind?: "request" | "notification";
      signal?: AbortSignal;
      timeoutMs?: number;
    }>
  >,
  reverseHandlers: Readonly<Record<string, AgentStdioReverseHandler>> = {},
  options: Readonly<{
    onPrivateNative?: (rows: ReadonlyArray<PrivateNativeObservation>) => void;
    limits?: Partial<Parameters<typeof AgentStdioTransport.start>[0]["limits"]>;
    settleMs?: number;
  }> = {},
): Promise<PeerScenarioResult> => {
  const transport = await startPeerScenarioTransport(scenario, options.limits);
  const notifications: Array<{ method: string; params: unknown }> = [];
  const methods = new Set(
    scenario.actions.flatMap((action) => [
      ...(action.notifications?.map((value) => value.method) ?? []),
      ...(action.notificationsAfterResponse?.map((value) => value.method) ?? []),
    ]),
  );
  for (const method of methods)
    transport.onNotification(method, (params) => notifications.push({ method, params }));
  for (const [method, handler] of Object.entries(reverseHandlers))
    transport.registerReverseHandler(method, handler);
  const token = transport.authorizeNativeEvidence();
  let results: ReadonlyArray<unknown>;
  let evidence: ReturnType<typeof transport.readNativeEvidence>;
  try {
    results = await Promise.all(
      requests.map((request) => {
        if (request.kind === "notification") {
          transport.notify(request.method, request.params);
          return undefined;
        }
        return transport.request(request.method, request.params, {
          ...(request.signal === undefined ? {} : { signal: request.signal }),
          ...(request.timeoutMs === undefined ? {} : { timeoutMs: request.timeoutMs }),
        });
      }),
    );
    if ((options.settleMs ?? 0) > 0)
      await new Promise((resolveTurn) => setTimeout(resolveTurn, options.settleMs));
    else await new Promise((resolveTurn) => setImmediate(resolveTurn));
    evidence = transport.readNativeEvidence(token);
    const privateRows = evidence
      .filter((entry): entry is typeof entry & { raw: unknown } => entry.raw !== undefined)
      .map((entry) => Object.freeze({ ...entry, raw: entry.raw }));
    options.onPrivateNative?.(Object.freeze(privateRows));
  } finally {
    await transport.dispose();
  }
  const receipt = transport.getReceipt();
  const firstAt = evidence[0] === undefined ? 0 : Date.parse(evidence[0].at);
  const transcript = sanitizeTranscript([
    ...evidence.map((entry) => ({
      generation: entry.generation,
      direction: entry.direction,
      atMs: Math.max(0, Date.parse(entry.at) - firstAt),
      native: entry.raw ?? { sha256: entry.sha256, bytes: entry.bytes },
    })),
    ...(receipt.stderrExcerpt.length > 0
      ? [
          {
            generation: receipt.generation,
            direction: "stderr" as const,
            atMs: Math.max(
              0,
              Date.parse(receipt.endedAt ?? receipt.startedAt) - Date.parse(receipt.startedAt),
            ),
            native: receipt.stderrExcerpt,
          },
        ]
      : []),
    {
      generation: receipt.generation,
      direction: "lifecycle" as const,
      atMs: Math.max(
        0,
        Date.parse(receipt.endedAt ?? receipt.startedAt) - Date.parse(receipt.startedAt),
      ),
      native: {
        state: receipt.state,
        terminalOutcome: receipt.terminalOutcome,
        exitCode: receipt.exitCode,
        signal: receipt.signal,
      },
    },
  ]);
  return { results, notifications, transcript, receipt };
};
