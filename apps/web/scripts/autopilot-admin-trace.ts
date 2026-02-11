#!/usr/bin/env bun

import { mkdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

type SendResult = {
  readonly ok: boolean;
  readonly testUserId: string;
  readonly threadId: string;
  readonly runId: string;
  readonly userMessageId: string;
  readonly assistantMessageId: string;
};

type SnapshotResult = {
  readonly ok: boolean;
  readonly snapshot: {
    readonly messages: ReadonlyArray<{
      readonly messageId: string;
      readonly role: string;
      readonly status: string;
      readonly text?: string | null;
      readonly runId?: string | null;
      readonly createdAtMs?: number;
      readonly updatedAtMs?: number;
    }>;
    readonly parts: ReadonlyArray<{
      readonly runId?: string;
      readonly seq?: number;
      readonly part?: {
        readonly type?: string;
        readonly signatureId?: string;
      };
    }>;
  };
};

type TraceResult = {
  readonly ok: boolean;
  readonly trace: {
    readonly summary: {
      readonly messageCount: number;
      readonly partCount: number;
      readonly runCount: number;
      readonly receiptCount: number;
      readonly featureRequestCount: number;
      readonly dseBlobCount: number;
      readonly dseVarCount: number;
    };
    readonly messages: ReadonlyArray<{
      readonly messageId?: string;
      readonly role?: string;
      readonly runId?: string;
      readonly status?: string;
      readonly text?: string | null;
      readonly createdAtMs?: number;
      readonly updatedAtMs?: number;
    }>;
    readonly runs: ReadonlyArray<{
      readonly runId?: string;
      readonly status?: string;
      readonly createdAtMs?: number;
      readonly updatedAtMs?: number;
      readonly cancelRequested?: boolean;
    }>;
    readonly parts: ReadonlyArray<{
      readonly runId?: string;
      readonly seq?: number;
      readonly part?: {
        readonly type?: string;
        readonly signatureId?: string;
        readonly modelId?: string;
        readonly provider?: string;
        readonly modelRoute?: string;
        readonly modelFallbackId?: string;
        readonly reason?: string;
        readonly timeToFirstTokenMs?: number;
        readonly timeToCompleteMs?: number;
      };
    }>;
    readonly featureRequests: ReadonlyArray<unknown>;
  };
};

const parseArgs = (argv: ReadonlyArray<string>) => {
  const out = {
    baseUrl: "https://openagents.com",
    text: "Autopilot admin trace test: summarize your current capabilities in one sentence.",
    threadId: "",
    timeoutMs: 180_000,
    pollMs: 1_000,
    resetThread: true,
    includeDseState: true,
  };

  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const next = argv[i + 1];
    if (a === "--base-url" && typeof next === "string") {
      out.baseUrl = next;
      i++;
      continue;
    }
    if (a === "--text" && typeof next === "string") {
      out.text = next;
      i++;
      continue;
    }
    if (a === "--thread-id" && typeof next === "string") {
      out.threadId = next;
      i++;
      continue;
    }
    if (a === "--timeout-ms" && typeof next === "string") {
      const n = Number(next);
      if (Number.isFinite(n) && n > 0) out.timeoutMs = Math.floor(n);
      i++;
      continue;
    }
    if (a === "--poll-ms" && typeof next === "string") {
      const n = Number(next);
      if (Number.isFinite(n) && n > 0) out.pollMs = Math.floor(n);
      i++;
      continue;
    }
    if (a === "--no-reset") {
      out.resetThread = false;
      continue;
    }
    if (a === "--no-dse-state") {
      out.includeDseState = false;
      continue;
    }
  }
  return out;
};

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

const getSecret = (): string | null => {
  const explicit = process.env.OA_AUTOPILOT_ADMIN_SECRET;
  if (typeof explicit === "string" && explicit.length > 0) return explicit;
  const fallback = process.env.OA_DSE_ADMIN_SECRET;
  return typeof fallback === "string" && fallback.length > 0 ? fallback : null;
};

const readJson = async <T>(response: Response): Promise<T> => {
  const text = await response.text();
  if (!text) throw new Error(`empty_response status=${response.status}`);
  return JSON.parse(text) as T;
};

const fetchJson = async <T>(input: {
  readonly url: string;
  readonly method: "GET" | "POST";
  readonly secret: string;
  readonly body?: unknown;
}): Promise<{ readonly body: T; readonly requestId: string | null }> => {
  const res = await fetch(input.url, {
    method: input.method,
    headers: {
      authorization: `Bearer ${input.secret}`,
      "content-type": "application/json",
    },
    body: input.body === undefined ? undefined : JSON.stringify(input.body),
  });

  if (!res.ok) {
    const msg = await res.text();
    throw new Error(`http_${res.status}: ${msg}`);
  }
  const requestId = res.headers.get("x-oa-request-id");
  return {
    body: await readJson<T>(res),
    requestId,
  };
};

const toRecord = (value: unknown): Record<string, unknown> | null =>
  value && typeof value === "object" ? (value as Record<string, unknown>) : null;

const extractRunStatus = (snapshot: SnapshotResult["snapshot"], runId: string): string | null => {
  for (const msg of snapshot.messages) {
    if (msg.runId === runId && msg.role === "assistant") return msg.status;
  }
  return null;
};

const analyzeTrace = (trace: TraceResult["trace"], runId: string) => {
  const partTypeCounts = new Map<string, number>();
  let dseSignatureCount = 0;
  const signatureIds = new Set<string>();
  let finishWithModelMetadataCount = 0;
  let finishCount = 0;

  for (const row of trace.parts) {
    const type = typeof row.part?.type === "string" ? row.part.type : "unknown";
    partTypeCounts.set(type, (partTypeCounts.get(type) ?? 0) + 1);
    if (type === "dse.signature") {
      dseSignatureCount++;
      if (typeof row.part?.signatureId === "string" && row.part.signatureId.length > 0) {
        signatureIds.add(row.part.signatureId);
      }
    }
    if (type === "finish") {
      finishCount++;
      if (
        typeof row.part?.modelId === "string" ||
        typeof row.part?.provider === "string" ||
        typeof row.part?.modelRoute === "string" ||
        typeof row.part?.modelFallbackId === "string"
      ) {
        finishWithModelMetadataCount++;
      }
    }
  }

  const run = trace.runs.find((r) => r.runId === runId) ?? null;
  const assistantMessage =
    trace.messages.find((m) => m.runId === runId && m.role === "assistant") ?? null;

  return {
    traceSummary: trace.summary,
    run,
    assistantMessage,
    featureRequestCount: trace.featureRequests.length,
    dseSignatureCount,
    signatureIds: Array.from(signatureIds).sort((a, b) => a.localeCompare(b)),
    finishCount,
    finishWithModelMetadataCount,
    partTypeCounts: Array.from(partTypeCounts.entries())
      .map(([type, count]) => ({ type, count }))
      .sort((a, b) => a.type.localeCompare(b.type)),
  };
};

const main = async () => {
  const args = parseArgs(process.argv.slice(2));
  const secret = getSecret();
  if (!secret) {
    throw new Error("Missing OA_AUTOPILOT_ADMIN_SECRET (or OA_DSE_ADMIN_SECRET) in environment.");
  }

  const sendUrl = `${args.baseUrl.replace(/\/+$/, "")}/api/autopilot/admin/send`;
  const sendPayload = {
    text: args.text,
    resetThread: args.resetThread,
    ...(args.threadId ? { threadId: args.threadId } : {}),
  };
  const sendResponse = await fetchJson<SendResult>({
    url: sendUrl,
    method: "POST",
    secret,
    body: sendPayload,
  });
  const send = sendResponse.body;
  if (!send.ok) throw new Error("admin_send_not_ok");

  const startedAt = Date.now();
  const snapshotBase = `${args.baseUrl.replace(/\/+$/, "")}/api/autopilot/admin/snapshot`;
  let latestSnapshot: SnapshotResult["snapshot"] | null = null;
  let runStatus: string | null = null;
  const snapshotRequestIds: string[] = [];

  while (Date.now() - startedAt < args.timeoutMs) {
    const snapshot = await fetchJson<SnapshotResult>({
      url: `${snapshotBase}?threadId=${encodeURIComponent(send.threadId)}&maxMessages=600&maxParts=12000`,
      method: "GET",
      secret,
    });
    if (snapshot.requestId) snapshotRequestIds.push(snapshot.requestId);
    latestSnapshot = snapshot.body.snapshot;
    runStatus = extractRunStatus(snapshot.body.snapshot, send.runId);
    if (runStatus === "final" || runStatus === "error" || runStatus === "canceled") break;
    await sleep(args.pollMs);
  }

  if (runStatus !== "final" && runStatus !== "error" && runStatus !== "canceled") {
    throw new Error(`run_timeout runId=${send.runId} threadId=${send.threadId}`);
  }

  const traceBase = `${args.baseUrl.replace(/\/+$/, "")}/api/autopilot/admin/trace`;
  const traceUrl =
    `${traceBase}?threadId=${encodeURIComponent(send.threadId)}&maxMessages=1000&maxParts=20000&maxRuns=1000` +
    `&maxReceipts=5000&maxFeatureRequests=1000&maxDseRowsPerRun=2000&includeDseState=${args.includeDseState ? "1" : "0"}`;

  const traceResponse = await fetchJson<TraceResult>({
    url: traceUrl,
    method: "GET",
    secret,
  });

  if (!traceResponse.body.ok) throw new Error("admin_trace_not_ok");
  const analysis = analyzeTrace(traceResponse.body.trace, send.runId);

  const output = {
    generatedAt: new Date().toISOString(),
    input: {
      baseUrl: args.baseUrl,
      text: args.text,
      resetThread: args.resetThread,
      includeDseState: args.includeDseState,
      timeoutMs: args.timeoutMs,
      pollMs: args.pollMs,
    },
    send,
    requestIds: {
      send: sendResponse.requestId,
      snapshots: snapshotRequestIds,
      trace: traceResponse.requestId,
    },
    runStatus,
    latestSnapshotSummary: {
      messageCount: latestSnapshot?.messages.length ?? 0,
      partCount: latestSnapshot?.parts.length ?? 0,
    },
    analysis,
    trace: traceResponse.body.trace,
  };

  const runLabel = `${Date.now()}-${send.runId}`;
  const outDir = join(process.cwd(), "output", "autopilot-admin-trace");
  const outFile = join(outDir, `${runLabel}.json`);
  await mkdir(outDir, { recursive: true });
  await writeFile(outFile, `${JSON.stringify(output, null, 2)}\n`, "utf8");

  const line = (v: unknown) => console.log(String(v));
  line(`ok: true`);
  line(`testUserId: ${send.testUserId}`);
  line(`threadId: ${send.threadId}`);
  line(`runId: ${send.runId}`);
  line(`runStatus: ${runStatus}`);
  line(`dseSignatureCount: ${analysis.dseSignatureCount}`);
  line(`featureRequestCount: ${analysis.featureRequestCount}`);
  line(`trace.messageCount: ${analysis.traceSummary.messageCount}`);
  line(`trace.partCount: ${analysis.traceSummary.partCount}`);
  line(`trace.runCount: ${analysis.traceSummary.runCount}`);
  line(`artifact: ${outFile}`);
};

main().catch((err) => {
  const rec = toRecord(err);
  const msg = typeof rec?.message === "string" ? rec.message : String(err);
  console.error(`error: ${msg}`);
  process.exitCode = 1;
});
