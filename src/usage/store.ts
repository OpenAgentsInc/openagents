import * as FileSystem from "@effect/platform/FileSystem";
import { createHash } from "node:crypto";
import { Effect } from "effect";
import * as S from "effect/Schema";
import { UsageRecordSchema, createEmptyTotals, type UsageRecord, type UsageSummary, type UsageTotals } from "./types.js";

const usageFilePath = (rootDir: string): string => `${rootDir}/.openagents/usage.jsonl`;

const parseRecord = (line: string): UsageRecord | null => {
  try {
    return S.decodeUnknownSync(UsageRecordSchema)(JSON.parse(line.trim()));
  } catch {
    return null;
  }
};

export const computeUsageIdempotencyKey = (record: UsageRecord): string => {
  if (record.idempotencyKey) return record.idempotencyKey;
  const serialized = JSON.stringify({ ...record, idempotencyKey: undefined });
  const digest = createHash("sha256").update(serialized).digest("hex").slice(0, 16);
  return `auto:${digest}`;
};

const readRecentRecords = (
  content: string,
  limit = 200,
): UsageRecord[] => {
  const lines = content.split("\n");
  const recent = lines.slice(Math.max(0, lines.length - limit));
  const records: UsageRecord[] = [];
  for (const line of recent) {
    if (line.trim().length === 0) continue;
    const parsed = parseRecord(line);
    if (parsed) records.push(parsed);
  }
  return records;
};

export interface AppendUsageOptions {
  rootDir: string;
  record: UsageRecord;
}

export const appendUsageRecord = ({ rootDir, record }: AppendUsageOptions): Effect.Effect<void, Error, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const filePath = usageFilePath(rootDir);
    const dir = filePath.slice(0, filePath.lastIndexOf("/"));
    yield* fs.makeDirectory(dir, { recursive: true }).pipe(
      Effect.mapError((e) => new Error(`Failed to create usage dir: ${e.message}`)),
    );

    const usageWithKey: UsageRecord = {
      ...record,
      idempotencyKey: computeUsageIdempotencyKey(record),
    };

    const exists = yield* fs.exists(filePath).pipe(
      Effect.mapError((e) => new Error(`Failed to check usage file: ${e.message}`)),
    );

    if (exists) {
      const content = yield* fs.readFileString(filePath).pipe(
        Effect.mapError((e) => new Error(`Failed to read usage file: ${e.message}`)),
      );
      const recent = readRecentRecords(content);
      const isDuplicate = recent.some((r) => computeUsageIdempotencyKey(r) === usageWithKey.idempotencyKey);
      if (isDuplicate) return;
    }

    const payload = JSON.stringify(usageWithKey);
    yield* fs.writeFile(filePath, new TextEncoder().encode(`${payload}\n`), { flag: "a" }).pipe(
      Effect.mapError((e) => new Error(`Failed to write usage record: ${e.message}`)),
    );
  });

export const readUsageRecords = (rootDir: string): Effect.Effect<UsageRecord[], Error, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const filePath = usageFilePath(rootDir);
    const exists = yield* fs.exists(filePath).pipe(
      Effect.mapError((e) => new Error(`Failed to check usage file: ${e.message}`)),
    );
    if (!exists) return [];
    const content = yield* fs.readFileString(filePath).pipe(
      Effect.mapError((e) => new Error(`Failed to read usage file: ${e.message}`)),
    );
    const records: UsageRecord[] = [];
    for (const line of content.split("\n")) {
      if (line.trim().length === 0) continue;
      const parsed = parseRecord(line);
      if (parsed) records.push(parsed);
    }
    return records;
  });

const formatPeriodKey = (date: Date, period: "day" | "week" | "month"): string => {
  const year = date.getUTCFullYear();
  const month = String(date.getUTCMonth() + 1).padStart(2, "0");
  const day = String(date.getUTCDate()).padStart(2, "0");
  switch (period) {
    case "day":
      return `${year}-${month}-${day}`;
    case "week": {
      const firstDay = new Date(Date.UTC(year, date.getUTCMonth(), date.getUTCDate() - date.getUTCDay()));
      const weekMonth = String(firstDay.getUTCMonth() + 1).padStart(2, "0");
      const weekDay = String(firstDay.getUTCDate()).padStart(2, "0");
      return `${firstDay.getUTCFullYear()}-W${weekMonth}${weekDay}`;
    }
    case "month":
    default:
      return `${year}-${month}`;
  }
};

const addTotals = (target: UsageTotals, record: UsageRecord): UsageTotals => ({
  sessions: target.sessions + 1,
  subtasks: target.subtasks + record.subtasks,
  inputTokens: target.inputTokens + record.inputTokens,
  outputTokens: target.outputTokens + record.outputTokens,
  cacheReadTokens: target.cacheReadTokens + record.cacheReadTokens,
  cacheCreationTokens: target.cacheCreationTokens + record.cacheCreationTokens,
  totalCostUsd: target.totalCostUsd + record.totalCostUsd,
});

export interface SummarizeOptions {
  rootDir: string;
  period?: "day" | "week" | "month";
}

export const summarizeUsage = ({
  rootDir,
  period = "day",
}: SummarizeOptions): Effect.Effect<UsageSummary, Error, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const records = yield* readUsageRecords(rootDir);
    const byPeriod: Record<string, UsageTotals> = {};
    let overall = createEmptyTotals();

    for (const record of records) {
      const ts = new Date(record.timestamp);
      const key = formatPeriodKey(ts, period);
      const current = byPeriod[key] ?? createEmptyTotals();
      byPeriod[key] = addTotals(current, record);
      overall = addTotals(overall, record);
    }

    return { period, byPeriod, overall };
  });
