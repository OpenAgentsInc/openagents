import { chmodSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";

import {
  DesktopRuntimeControlOutcomeLedgerSchema,
  decodeDesktopRuntimeControlOutcomeRecord,
  type DesktopRuntimeControlOutcomeRecord,
  type DesktopRuntimeControlOutcomeRecordResult,
} from "./runtime-control-outcome-contract.ts";

const maxRecords = 512;
type Evidence = DesktopRuntimeControlOutcomeRecord["outcome"][
  | "admission"
  | "delivery"
  | "terminal"];

const mergeAxis = <T extends Evidence>(existing: T, incoming: T): T | null => {
  if (JSON.stringify(existing) === JSON.stringify(incoming)) return existing;
  if (existing.status === "pending") return incoming;
  if (incoming.status === "pending") return existing;
  return null;
};

const mergeRecords = (
  existing: DesktopRuntimeControlOutcomeRecord,
  incoming: DesktopRuntimeControlOutcomeRecord,
): DesktopRuntimeControlOutcomeRecord | null => {
  if (existing.threadRef !== incoming.threadRef) return null;
  const admission = mergeAxis(existing.outcome.admission, incoming.outcome.admission);
  const delivery = mergeAxis(existing.outcome.delivery, incoming.outcome.delivery);
  const terminal = mergeAxis(existing.outcome.terminal, incoming.outcome.terminal);
  if (admission === null || delivery === null || terminal === null) return null;
  return {
    threadRef: existing.threadRef,
    outcome: {
      ...existing.outcome,
      observedAt:
        Date.parse(incoming.outcome.observedAt) > Date.parse(existing.outcome.observedAt)
          ? incoming.outcome.observedAt
          : existing.outcome.observedAt,
      admission,
      delivery,
      terminal,
    },
  };
};

export const openDesktopRuntimeControlOutcomeStore = (file: string) => {
  let corrupt = false;
  let records: DesktopRuntimeControlOutcomeRecord[] = [];
  try {
    const parsed = JSON.parse(readFileSync(file, "utf8")) as Readonly<{
      schema?: unknown;
      records?: unknown;
    }>;
    if (
      parsed.schema !== DesktopRuntimeControlOutcomeLedgerSchema ||
      !Array.isArray(parsed.records)
    )
      throw new Error("invalid ledger");
    const decoded = parsed.records.map(decodeDesktopRuntimeControlOutcomeRecord);
    if (decoded.some((record) => record === null)) throw new Error("invalid record");
    records = (decoded as DesktopRuntimeControlOutcomeRecord[]).slice(-maxRecords);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") corrupt = true;
  }

  const persist = (): boolean => {
    try {
      mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 });
      if (process.platform !== "win32") chmodSync(path.dirname(file), 0o700);
      const temporary = `${file}.tmp`;
      writeFileSync(
        temporary,
        JSON.stringify({
          schema: DesktopRuntimeControlOutcomeLedgerSchema,
          records,
        }),
        { encoding: "utf8", mode: 0o600 },
      );
      if (process.platform !== "win32") chmodSync(temporary, 0o600);
      renameSync(temporary, file);
      if (process.platform !== "win32") chmodSync(file, 0o600);
      return true;
    } catch {
      return false;
    }
  };

  const reject = (
    reason: NonNullable<DesktopRuntimeControlOutcomeRecordResult["reason"]>,
  ): DesktopRuntimeControlOutcomeRecordResult => ({ status: "rejected", reason });

  return {
    list: (): ReadonlyArray<DesktopRuntimeControlOutcomeRecord> =>
      records.map((record) => structuredClone(record)),
    record: (input: unknown): DesktopRuntimeControlOutcomeRecordResult => {
      if (corrupt) return reject("corrupt_ledger");
      const incoming = decodeDesktopRuntimeControlOutcomeRecord(input);
      if (incoming === null) return reject("invalid_request");
      const existingIndex = records.findIndex(
        ({ outcome }) =>
          outcome.outcomeRef === incoming.outcome.outcomeRef ||
          outcome.intentRef === incoming.outcome.intentRef ||
          outcome.idempotencyKey === incoming.outcome.idempotencyKey,
      );
      if (existingIndex < 0) {
        const prior = records;
        records = [...records, incoming].slice(-maxRecords);
        if (!persist()) {
          records = prior;
          return reject("persistence_failed");
        }
        return { status: "stored", record: incoming };
      }
      const existing = records[existingIndex]!;
      if (
        existing.outcome.outcomeRef !== incoming.outcome.outcomeRef ||
        existing.outcome.intentRef !== incoming.outcome.intentRef ||
        existing.outcome.idempotencyKey !== incoming.outcome.idempotencyKey ||
        existing.threadRef !== incoming.threadRef
      )
        return reject("identity_conflict");
      const merged = mergeRecords(existing, incoming);
      if (merged === null) return reject("evidence_conflict");
      if (JSON.stringify(merged) === JSON.stringify(existing))
        return { status: "unchanged", record: existing };
      records = records.map((record, index) => (index === existingIndex ? merged : record));
      if (!persist()) {
        records = records.map((record, index) => (index === existingIndex ? existing : record));
        return reject("persistence_failed");
      }
      return { status: "advanced", record: merged };
    },
  };
};
