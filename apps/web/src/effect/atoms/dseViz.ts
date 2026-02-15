import { Atom, Result } from "@effect-atom/atom";
import { Cause, Effect } from "effect";

import { api } from "../../../convex/_generated/api";
import { ConvexService } from "../convex";
import { AppAtomRuntime } from "./appRuntime";

import type {
  DseCompileReportPageData,
  DseEvalReportPageData,
  DseOpsRunDetailPageData,
  DseOpsRunsPageData,
  DseSignaturePageData,
} from "../../lib/pageData/dse";

function safeStableStringify(value: unknown, indent = 2, maxChars = 100_000): string {
  if (value == null) return String(value);
  if (typeof value === "string") return value;
  try {
    const s = JSON.stringify(value, null, indent);
    if (typeof s !== "string") return String(value);
    if (s.length <= maxChars) return s;
    return `${s.slice(0, maxChars)}\n… (truncated, chars=${s.length})`;
  } catch {
    return String(value);
  }
}

function errorTextFromResult<TValue, TError>(
  result: Result.Result<TValue, TError>,
  fallback: string,
): string | null {
  if (Result.isFailure(result)) {
    const pretty = Cause.pretty(result.cause as Cause.Cause<unknown>);
    return pretty.trim() ? pretty : fallback;
  }
  return null;
}

const DseOpsRunsResultAtom = Atom.family((userId: string) =>
  AppAtomRuntime.atom(
    Effect.gen(function* () {
      void userId;
      const convex = yield* ConvexService;
      const res = yield* convex.query(api.dse.opsRuns.listRuns, { limit: 50 });
      return res as any;
    }),
  )
    .pipe(
      Atom.keepAlive,
      Atom.withLabel(`DseOpsRunsResultAtom(${userId})`),
    )
);

type OpsRunDetailKey = { readonly runId: string };
const encodeKey = (k: OpsRunDetailKey): string => safeStableStringify(k, 0, 1000);
const decodeKey = (raw: string): OpsRunDetailKey => {
  try {
    const v = JSON.parse(raw) as any;
    const runId = typeof v?.runId === "string" ? v.runId : "";
    return { runId };
  } catch {
    return { runId: "" };
  }
};

export const DseOpsRunsPageDataAtom = Atom.family((userId: string) =>
  Atom.make((get) => {
    const result = get(DseOpsRunsResultAtom(userId));
    const err = errorTextFromResult(result, "Failed to load ops runs.");
    if (err) return { errorText: err, runs: null } satisfies DseOpsRunsPageData;
    if (Result.isSuccess(result)) {
      const runs = Array.isArray((result.value as any)?.runs) ? ((result.value as any).runs as any[]) : [];
      return { errorText: null, runs } satisfies DseOpsRunsPageData;
    }
    return { errorText: null, runs: null } satisfies DseOpsRunsPageData;
  }).pipe(Atom.keepAlive, Atom.withLabel(`DseOpsRunsPageDataAtom(${userId})`)),
);

const DseOpsRunDetailResultAtom = Atom.family((key: string) =>
  AppAtomRuntime.atom(
    Effect.gen(function* () {
      const { runId } = decodeKey(key);
      if (!runId) return { ok: false, error: "invalid_runId" };

      const convex = yield* ConvexService;
      const [runRes, eventsRes] = yield* Effect.all([
        convex.query(api.dse.opsRuns.getRun, { runId }),
        convex.query(api.dse.opsRuns.listRunEvents, { runId, limit: 500 }),
      ]);
      return { runRes: runRes as any, eventsRes: eventsRes as any };
    }),
  )
    .pipe(Atom.keepAlive, Atom.withLabel(`DseOpsRunDetailResultAtom(${key})`)),
);

export const makeOpsRunDetailKey = (runId: string): string => encodeKey({ runId });

type SignatureKey = { readonly signatureId: string };
const encodeSigKey = (k: SignatureKey): string => safeStableStringify(k, 0, 2000);
const decodeSigKey = (raw: string): SignatureKey => {
  try {
    const v = JSON.parse(raw) as any;
    const signatureId = typeof v?.signatureId === "string" ? v.signatureId : "";
    return { signatureId };
  } catch {
    return { signatureId: "" };
  }
};

export const DseOpsRunDetailPageDataAtom = Atom.family((key: string) =>
  Atom.make((get) => {
    const { runId } = decodeKey(key);
    const result = get(DseOpsRunDetailResultAtom(key));
    const err = errorTextFromResult(result, "Failed to load ops run detail.");
    if (err) return { runId, errorText: err, run: null, events: null } satisfies DseOpsRunDetailPageData;
    if (!Result.isSuccess(result)) return { runId, errorText: null, run: null, events: null } satisfies DseOpsRunDetailPageData;

    const runRow = (result.value as any)?.runRes?.run ?? null;
    const run =
      runRow && typeof runRow === "object"
        ? {
            runId: String(runRow.runId ?? runId),
            status:
              runRow.status === "running" || runRow.status === "finished" || runRow.status === "failed"
                ? runRow.status
                : "running",
            startedAtMs: Number(runRow.startedAtMs ?? 0),
            endedAtMs: typeof runRow.endedAtMs === "number" ? Number(runRow.endedAtMs) : null,
            commitSha: typeof runRow.commitSha === "string" ? String(runRow.commitSha) : null,
            baseUrl: typeof runRow.baseUrl === "string" ? String(runRow.baseUrl) : null,
            actorUserId: typeof runRow.actorUserId === "string" ? String(runRow.actorUserId) : null,
            signatureIds: Array.isArray(runRow.signatureIds) ? runRow.signatureIds.map((s: any) => String(s)) : null,
            notes: typeof runRow.notes === "string" ? String(runRow.notes) : null,
            linksJson: runRow.links == null ? null : safeStableStringify(runRow.links, 2, 60_000),
            summaryJson: runRow.summaryJson == null ? null : safeStableStringify(runRow.summaryJson, 2, 60_000),
            updatedAtMs: Number(runRow.updatedAtMs ?? 0),
          }
        : null;

    const rawEvents: any[] = Array.isArray((result.value as any)?.eventsRes?.events)
      ? ((result.value as any).eventsRes.events as any[])
      : [];
    const events = rawEvents
      .map((ev) => ({
        tsMs: Number(ev.tsMs ?? 0),
        level: ev.level === "info" || ev.level === "warn" || ev.level === "error" ? ev.level : "info",
        phase: typeof ev.phase === "string" ? String(ev.phase) : null,
        message: String(ev.message ?? ""),
        jsonPreview: ev.json == null ? null : safeStableStringify(ev.json, 2, 60_000),
      }))
      .sort((a, b) => a.tsMs - b.tsMs);

    return { runId, errorText: null, run, events } satisfies DseOpsRunDetailPageData;
  }).pipe(Atom.keepAlive, Atom.withLabel(`DseOpsRunDetailPageDataAtom(${key})`)),
);

const DseSignatureResultAtom = Atom.family((key: string) =>
  AppAtomRuntime.atom(
    Effect.gen(function* () {
      const { signatureId } = decodeSigKey(key);
      if (!signatureId) return { ok: false, error: "invalid_signatureId" };

      const convex = yield* ConvexService;
      const [
        activeRes,
        activeHistRes,
        canaryRes,
        canaryHistRes,
        reportsRes,
        evalReportsRes,
        examplesRes,
        receiptsRes,
      ] = yield* Effect.all([
        convex.query(api.dse.active.getActive, { signatureId }),
        convex.query(api.dse.active.listActiveHistory, { signatureId, limit: 50 }),
        convex.query(api.dse.canary.getCanary, { signatureId }),
        convex.query(api.dse.canary.listCanaryHistory, { signatureId, limit: 50 }),
        convex.query(api.dse.compileReports.listReports, { signatureId, limit: 20 }),
        convex.query(api.dse.evalReports.listReports, { signatureId, limit: 20 }),
        convex.query(api.dse.examples.listExamples, { signatureId, limit: 60 }),
        convex.query(api.dse.receipts.listPredictReceiptsBySignatureIdAdmin, { signatureId, limit: 50 }),
      ]);

      return {
        activeRes: activeRes as any,
        activeHistRes: activeHistRes as any,
        canaryRes: canaryRes as any,
        canaryHistRes: canaryHistRes as any,
        reportsRes: reportsRes as any,
        evalReportsRes: evalReportsRes as any,
        examplesRes: examplesRes as any,
        receiptsRes: receiptsRes as any,
      };
    }),
  )
    .pipe(Atom.keepAlive, Atom.withLabel(`DseSignatureResultAtom(${key})`)),
);

export const makeSignatureKey = (signatureId: string): string => encodeSigKey({ signatureId });

type CompileReportKey = { readonly signatureId: string; readonly jobHash: string; readonly datasetHash: string };
const encodeReportKey = (k: CompileReportKey): string => safeStableStringify(k, 0, 5000);
const decodeReportKey = (raw: string): CompileReportKey => {
  try {
    const v = JSON.parse(raw) as any;
    const signatureId = typeof v?.signatureId === "string" ? v.signatureId : "";
    const jobHash = typeof v?.jobHash === "string" ? v.jobHash : "";
    const datasetHash = typeof v?.datasetHash === "string" ? v.datasetHash : "";
    return { signatureId, jobHash, datasetHash };
  } catch {
    return { signatureId: "", jobHash: "", datasetHash: "" };
  }
};

export const DseSignaturePageDataAtom = Atom.family((key: string) =>
  Atom.make((get) => {
    const { signatureId } = decodeSigKey(key);
    const result = get(DseSignatureResultAtom(key));
    const err = errorTextFromResult(result, "Failed to load signature detail.");
    if (err) {
      return {
        signatureId,
        errorText: err,
        active: null,
        activeHistory: null,
        canary: null,
        canaryHistory: null,
        compileReports: null,
        evalReports: null,
        examples: null,
        receipts: null,
      } satisfies DseSignaturePageData;
    }
    if (!Result.isSuccess(result)) {
      return {
        signatureId,
        errorText: null,
        active: null,
        activeHistory: null,
        canary: null,
        canaryHistory: null,
        compileReports: null,
        evalReports: null,
        examples: null,
        receipts: null,
      } satisfies DseSignaturePageData;
    }

    const activeRow = (result.value as any).activeRes ?? {};
    const active = {
      compiled_id: typeof activeRow.compiled_id === "string" ? activeRow.compiled_id : null,
      updatedAtMs: typeof activeRow.updatedAtMs === "number" ? activeRow.updatedAtMs : null,
    };

    const activeHistoryRaw: any[] = Array.isArray((result.value as any).activeHistRes?.history)
      ? (result.value as any).activeHistRes.history
      : [];
    const activeHistory = activeHistoryRaw.map((h) => ({
      action: h.action === "set" || h.action === "clear" || h.action === "rollback" ? h.action : "set",
      fromCompiledId: typeof h.fromCompiledId === "string" ? h.fromCompiledId : null,
      toCompiledId: typeof h.toCompiledId === "string" ? h.toCompiledId : null,
      reason: typeof h.reason === "string" ? h.reason : null,
      actorUserId: typeof h.actorUserId === "string" ? h.actorUserId : null,
      createdAtMs: Number(h.createdAtMs ?? 0),
    }));

    const canaryRow = (result.value as any).canaryRes?.canary ?? null;
    const canary =
      canaryRow && typeof canaryRow === "object"
        ? {
            enabled: Boolean(canaryRow.enabled),
            control_compiled_id: String(canaryRow.control_compiled_id ?? ""),
            canary_compiled_id: String(canaryRow.canary_compiled_id ?? ""),
            rolloutPct: Number(canaryRow.rolloutPct ?? 0),
            okCount: Number(canaryRow.okCount ?? 0),
            errorCount: Number(canaryRow.errorCount ?? 0),
            minSamples: Number(canaryRow.minSamples ?? 0),
            maxErrorRate: Number(canaryRow.maxErrorRate ?? 0),
            updatedAtMs: Number(canaryRow.updatedAtMs ?? 0),
          }
        : null;

    const canaryHistoryRaw: any[] = Array.isArray((result.value as any).canaryHistRes?.history)
      ? (result.value as any).canaryHistRes.history
      : [];
    const canaryHistory = canaryHistoryRaw.map((h) => ({
      action:
        h.action === "start" || h.action === "stop" || h.action === "auto_stop" || h.action === "update"
          ? h.action
          : "start",
      control_compiled_id: typeof h.control_compiled_id === "string" ? h.control_compiled_id : null,
      canary_compiled_id: typeof h.canary_compiled_id === "string" ? h.canary_compiled_id : null,
      rolloutPct: typeof h.rolloutPct === "number" ? Number(h.rolloutPct) : null,
      okCount: typeof h.okCount === "number" ? Number(h.okCount) : null,
      errorCount: typeof h.errorCount === "number" ? Number(h.errorCount) : null,
      reason: typeof h.reason === "string" ? h.reason : null,
      actorUserId: typeof h.actorUserId === "string" ? h.actorUserId : null,
      createdAtMs: Number(h.createdAtMs ?? 0),
    }));

    const reportsRaw: any[] = Array.isArray((result.value as any).reportsRes?.reports)
      ? (result.value as any).reportsRes.reports
      : [];
    const compileReports = reportsRaw.map((r) => ({
      jobHash: String(r.jobHash ?? ""),
      datasetHash: String(r.datasetHash ?? ""),
      compiled_id: String(r.compiled_id ?? ""),
      createdAtMs: Number(r.createdAtMs ?? 0),
    }));

    const evalReportsRaw: any[] = Array.isArray((result.value as any).evalReportsRes?.reports)
      ? (result.value as any).evalReportsRes.reports
      : [];
    const evalReports = evalReportsRaw.map((r) => ({
      evalHash: String(r.evalHash ?? ""),
      compiled_id: String(r.compiled_id ?? ""),
      datasetHash: String(r.datasetHash ?? ""),
      rewardId: String(r.rewardId ?? ""),
      split: typeof r.split === "string" ? String(r.split) : null,
      n: typeof r.n === "number" ? Number(r.n) : null,
      createdAtMs: Number(r.createdAtMs ?? 0),
    }));

    const examplesRaw: any[] = Array.isArray((result.value as any).examplesRes?.examples)
      ? (result.value as any).examplesRes.examples
      : [];
    const examples = examplesRaw
      .slice(0, 60)
      .map((ex) => ({
        exampleId: String(ex.exampleId ?? ""),
        split:
          ex.split === "train" || ex.split === "dev" || ex.split === "holdout" || ex.split === "test" ? ex.split : null,
        tags: Array.isArray(ex.tags) ? ex.tags.map((t: any) => String(t)) : null,
        inputJson: safeStableStringify(ex.inputJson, 2, 60_000),
        expectedJson: safeStableStringify(ex.expectedJson, 2, 60_000),
      }))
      .filter((ex) => ex.exampleId.length > 0);

    const receiptsRaw: any[] = Array.isArray((result.value as any).receiptsRes?.receipts)
      ? (result.value as any).receiptsRes.receipts
      : [];
    const receipts = receiptsRaw
      .slice(0, 50)
      .map((r) => ({
        receiptId: String(r.receiptId ?? ""),
        compiled_id: String(r.compiled_id ?? ""),
        createdAtMs: Number(r.createdAtMs ?? 0),
        strategyId: typeof r.strategyId === "string" ? String(r.strategyId) : null,
        resultTag: r.resultTag === "Ok" || r.resultTag === "Error" ? r.resultTag : null,
        rlmTraceBlobId: typeof r.rlmTraceBlobId === "string" ? String(r.rlmTraceBlobId) : null,
        rlmTraceEventCount: typeof r.rlmTraceEventCount === "number" ? Number(r.rlmTraceEventCount) : null,
      }))
      .filter((r) => r.receiptId.length > 0);

    return {
      signatureId,
      errorText: null,
      active,
      activeHistory,
      canary,
      canaryHistory,
      compileReports,
      evalReports,
      examples,
      receipts,
    } satisfies DseSignaturePageData;
  }).pipe(Atom.keepAlive, Atom.withLabel(`DseSignaturePageDataAtom(${key})`)),
);

export const DseCompileReportPageDataAtom = Atom.family((key: string) =>
  Atom.make((get) => {
    const { signatureId, jobHash, datasetHash } = decodeReportKey(key);
    const result = get(DseCompileReportResultAtom(key));
    const err = errorTextFromResult(result, "Failed to load compile report.");
    if (err) return { signatureId, jobHash, datasetHash, errorText: err, report: null } satisfies DseCompileReportPageData;
    if (!Result.isSuccess(result))
      return { signatureId, jobHash, datasetHash, errorText: null, report: null } satisfies DseCompileReportPageData;

    const report = (result.value as any)?.report ?? null;
    if (!report) return { signatureId, jobHash, datasetHash, errorText: null, report: null } satisfies DseCompileReportPageData;

    return {
      signatureId,
      jobHash,
      datasetHash,
      errorText: null,
      report: {
        signatureId: String(report.signatureId ?? signatureId),
        jobHash: String(report.jobHash ?? jobHash),
        datasetHash: String(report.datasetHash ?? datasetHash),
        datasetId: String(report.datasetId ?? ""),
        compiled_id: String(report.compiled_id ?? ""),
        createdAtMs: Number(report.createdAtMs ?? 0),
        jsonPretty: safeStableStringify(report.json, 2, 200_000),
      },
    } satisfies DseCompileReportPageData;
  }).pipe(Atom.keepAlive, Atom.withLabel(`DseCompileReportPageDataAtom(${key})`)),
);

const DseCompileReportResultAtom = Atom.family((key: string) =>
  AppAtomRuntime.atom(
    Effect.gen(function* () {
      const { signatureId, jobHash, datasetHash } = decodeReportKey(key);
      if (!signatureId || !jobHash || !datasetHash) return { ok: false, error: "invalid_key" };
      const convex = yield* ConvexService;
      const res = yield* convex.query(api.dse.compileReports.getReport, { signatureId, jobHash, datasetHash });
      return res as any;
    }),
  )
    .pipe(Atom.keepAlive, Atom.withLabel(`DseCompileReportResultAtom(${key})`)),
);

export const makeCompileReportKey = (signatureId: string, jobHash: string, datasetHash: string): string =>
  encodeReportKey({ signatureId, jobHash, datasetHash });

type EvalReportKey = { readonly signatureId: string; readonly evalHash: string };
const encodeEvalKey = (k: EvalReportKey): string => safeStableStringify(k, 0, 5000);
const decodeEvalKey = (raw: string): EvalReportKey => {
  try {
    const v = JSON.parse(raw) as any;
    const signatureId = typeof v?.signatureId === "string" ? v.signatureId : "";
    const evalHash = typeof v?.evalHash === "string" ? v.evalHash : "";
    return { signatureId, evalHash };
  } catch {
    return { signatureId: "", evalHash: "" };
  }
};

export const DseEvalReportPageDataAtom = Atom.family((key: string) =>
  Atom.make((get) => {
    const { signatureId, evalHash } = decodeEvalKey(key);
    const result = get(DseEvalReportResultAtom(key));
    const err = errorTextFromResult(result, "Failed to load eval report.");
    if (err) return { signatureId, evalHash, errorText: err, report: null } satisfies DseEvalReportPageData;
    if (!Result.isSuccess(result))
      return { signatureId, evalHash, errorText: null, report: null } satisfies DseEvalReportPageData;

    const report = (result.value as any)?.report ?? null;
    if (!report) return { signatureId, evalHash, errorText: null, report: null } satisfies DseEvalReportPageData;

    return {
      signatureId,
      evalHash,
      errorText: null,
      report: {
        signatureId: String(report.signatureId ?? signatureId),
        evalHash: String(report.evalHash ?? evalHash),
        compiled_id: String(report.compiled_id ?? ""),
        datasetId: String(report.datasetId ?? ""),
        datasetHash: String(report.datasetHash ?? ""),
        rewardId: String(report.rewardId ?? ""),
        rewardVersion: Number(report.rewardVersion ?? 0),
        split: typeof report.split === "string" ? String(report.split) : null,
        n: typeof report.n === "number" ? Number(report.n) : null,
        createdAtMs: Number(report.createdAtMs ?? 0),
        jsonPretty: safeStableStringify(report.json, 2, 300_000),
      },
    } satisfies DseEvalReportPageData;
  }).pipe(Atom.keepAlive, Atom.withLabel(`DseEvalReportPageDataAtom(${key})`)),
);

const DseEvalReportResultAtom = Atom.family((key: string) =>
  AppAtomRuntime.atom(
    Effect.gen(function* () {
      const { signatureId, evalHash } = decodeEvalKey(key);
      if (!signatureId || !evalHash) return { ok: false, error: "invalid_key" };
      const convex = yield* ConvexService;
      const res = yield* convex.query(api.dse.evalReports.getReport, { signatureId, evalHash });
      return res as any;
    }),
  )
    .pipe(Atom.keepAlive, Atom.withLabel(`DseEvalReportResultAtom(${key})`)),
);

export const makeEvalReportKey = (signatureId: string, evalHash: string): string =>
  encodeEvalKey({ signatureId, evalHash });
