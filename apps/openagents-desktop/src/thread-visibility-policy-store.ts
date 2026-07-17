import { createHash, randomUUID } from "node:crypto";
import { chmodSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";

import {
  decodeThreadDisclosureIntent,
  decodeThreadDisclosureReceipt,
  type ThreadDisclosureIntent,
  type ThreadDisclosureReceipt,
  type ThreadDisclosureTarget,
} from "@openagentsinc/agent-runtime-schema";
import { Effect } from "effect";

const LedgerSchema = "openagents.desktop_thread_visibility_policy.v1" as const;
const MaxRecords = 512;

type VisibilityIntent = Extract<ThreadDisclosureIntent, { kind: "thread.visibility.set" }>;
type VisibilityReceipt = ThreadDisclosureReceipt &
  Readonly<{
    kind: "thread.visibility.set";
    result: Readonly<{
      status: "visibility_applied";
      visibilityVersion: number;
      target: ThreadDisclosureTarget;
    }>;
  }>;

type VisibilityRecord = Readonly<{
  intent: VisibilityIntent;
  receipt: VisibilityReceipt;
}>;

export type DesktopThreadVisibilityApplyResult =
  | Readonly<{ status: "stored" | "unchanged"; receipt: VisibilityReceipt }>
  | Readonly<{
      status: "rejected";
      reason:
        | "invalid_request"
        | "corrupt_store"
        | "identity_conflict"
        | "stale_version"
        | "capacity_exceeded"
        | "persistence_failed";
    }>;

export type DesktopThreadVisibilityLoadResult =
  | Readonly<{
      status: "found";
      visibilityVersion: number;
      target: ThreadDisclosureTarget;
      receipt: VisibilityReceipt;
    }>
  | Readonly<{ status: "missing" }>
  | Readonly<{ status: "rejected"; reason: "invalid_request" | "corrupt_store" }>;

const clone = <T>(value: T): T => structuredClone(value);
const same = (left: unknown, right: unknown): boolean =>
  JSON.stringify(left) === JSON.stringify(right);

const decodeRecord = (input: unknown): VisibilityRecord => {
  if (typeof input !== "object" || input === null) throw new Error("invalid record");
  const intent = decodeThreadDisclosureIntent(Reflect.get(input, "intent"));
  const receipt = decodeThreadDisclosureReceipt(Reflect.get(input, "receipt"));
  if (
    intent.kind !== "thread.visibility.set" ||
    receipt.kind !== "thread.visibility.set" ||
    receipt.result.status !== "visibility_applied" ||
    receipt.intentRef !== intent.intentRef ||
    receipt.idempotencyKey !== intent.idempotencyKey ||
    receipt.threadRef !== intent.threadRef ||
    !same(receipt.result.target, intent.target)
  ) {
    throw new Error("visibility record identity mismatch");
  }
  return {
    intent,
    receipt: {
      ...receipt,
      kind: "thread.visibility.set",
      result: {
        status: "visibility_applied",
        visibilityVersion: receipt.result.visibilityVersion,
        target: receipt.result.target,
      },
    },
  };
};

const validateHistory = (records: ReadonlyArray<VisibilityRecord>): void => {
  const intentRefs = new Set<string>();
  const idempotencyKeys = new Set<string>();
  const versions = new Map<string, number>();
  for (const record of records) {
    if (
      intentRefs.has(record.intent.intentRef) ||
      idempotencyKeys.has(record.intent.idempotencyKey)
    ) {
      throw new Error("duplicate visibility identity");
    }
    intentRefs.add(record.intent.intentRef);
    idempotencyKeys.add(record.intent.idempotencyKey);
    const current = versions.get(record.intent.threadRef) ?? 0;
    const expected = record.intent.expectedVisibilityVersion;
    const expectationMatches =
      current === 0
        ? expected.state === "unknown" || (expected.state === "known" && expected.value === 0)
        : expected.state === "known" && expected.value === current;
    if (!expectationMatches || record.receipt.result.visibilityVersion !== current + 1) {
      throw new Error("invalid visibility version history");
    }
    versions.set(record.intent.threadRef, current + 1);
  }
};

const decodeLedger = (input: unknown): ReadonlyArray<VisibilityRecord> => {
  if (
    typeof input !== "object" ||
    input === null ||
    Reflect.get(input, "schema") !== LedgerSchema ||
    !Array.isArray(Reflect.get(input, "records"))
  ) {
    throw new Error("invalid visibility ledger");
  }
  const rawRecords = Reflect.get(input, "records") as ReadonlyArray<unknown>;
  if (rawRecords.length > MaxRecords) throw new Error("visibility ledger exceeds bound");
  const records = rawRecords.map(decodeRecord);
  validateHistory(records);
  return records;
};

const validRef = (value: unknown): value is string =>
  typeof value === "string" &&
  value.length >= 1 &&
  value.length <= 256 &&
  /^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(value);

/**
 * Private main-process policy evidence. Applying a record does not publish
 * thread content or grant membership, administrator, or transport authority.
 */
export const openDesktopThreadVisibilityPolicyStore = (file: string) => {
  let corrupt = false;
  let records: ReadonlyArray<VisibilityRecord> = [];
  try {
    records = decodeLedger(JSON.parse(readFileSync(file, "utf8")));
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") corrupt = true;
  }

  const persist = (next: ReadonlyArray<VisibilityRecord>): boolean => {
    let temporary: string | undefined;
    try {
      const directory = path.dirname(file);
      mkdirSync(directory, { recursive: true, mode: 0o700 });
      if (process.platform !== "win32") chmodSync(directory, 0o700);
      temporary = path.join(directory, `.${path.basename(file)}.${randomUUID()}.tmp`);
      writeFileSync(temporary, JSON.stringify({ schema: LedgerSchema, records: next }), {
        encoding: "utf8",
        flag: "wx",
        mode: 0o600,
      });
      if (process.platform !== "win32") chmodSync(temporary, 0o600);
      renameSync(temporary, file);
      temporary = undefined;
      if (process.platform !== "win32") chmodSync(file, 0o600);
      return true;
    } catch {
      if (temporary !== undefined) rmSync(temporary, { force: true });
      return false;
    }
  };

  const apply = Effect.fn("DesktopThreadVisibilityPolicyStore.apply")(function* (
    input: Readonly<{
      intent: unknown;
      receiptRef: string;
      observedAt: string;
    }>,
  ) {
    return yield* Effect.sync((): DesktopThreadVisibilityApplyResult => {
      if (corrupt) return { status: "rejected", reason: "corrupt_store" };
      let intent: ThreadDisclosureIntent;
      try {
        intent = decodeThreadDisclosureIntent(input.intent);
      } catch {
        return { status: "rejected", reason: "invalid_request" };
      }
      if (intent.kind !== "thread.visibility.set") {
        return { status: "rejected", reason: "invalid_request" };
      }

      const matchingIdentity = records.find(
        (record) =>
          record.intent.intentRef === intent.intentRef ||
          record.intent.idempotencyKey === intent.idempotencyKey,
      );
      if (matchingIdentity !== undefined) {
        if (
          matchingIdentity.intent.intentRef === intent.intentRef &&
          matchingIdentity.intent.idempotencyKey === intent.idempotencyKey &&
          same(matchingIdentity.intent, intent)
        ) {
          return { status: "unchanged", receipt: clone(matchingIdentity.receipt) };
        }
        return { status: "rejected", reason: "identity_conflict" };
      }

      const current = records.findLast((record) => record.intent.threadRef === intent.threadRef);
      const currentVersion = current?.receipt.result.visibilityVersion ?? 0;
      const expectationMatches =
        current === undefined
          ? intent.expectedVisibilityVersion.state === "unknown" ||
            (intent.expectedVisibilityVersion.state === "known" &&
              intent.expectedVisibilityVersion.value === 0)
          : intent.expectedVisibilityVersion.state === "known" &&
            intent.expectedVisibilityVersion.value === currentVersion;
      if (!expectationMatches) return { status: "rejected", reason: "stale_version" };
      if (records.length >= MaxRecords) {
        return { status: "rejected", reason: "capacity_exceeded" };
      }

      let receipt: ThreadDisclosureReceipt;
      try {
        receipt = decodeThreadDisclosureReceipt({
          schema: "openagents.thread_disclosure_receipt.v1",
          receiptRef: input.receiptRef,
          intentRef: intent.intentRef,
          idempotencyKey: intent.idempotencyKey,
          threadRef: intent.threadRef,
          observedAt: input.observedAt,
          kind: intent.kind,
          result: {
            status: "visibility_applied",
            visibilityVersion: currentVersion + 1,
            target: intent.target,
          },
        });
      } catch {
        return { status: "rejected", reason: "invalid_request" };
      }
      const record = decodeRecord({ intent, receipt });
      const next = [...records, record];
      if (!persist(next)) return { status: "rejected", reason: "persistence_failed" };
      records = next;
      return { status: "stored", receipt: clone(record.receipt) };
    });
  });

  const load = Effect.fn("DesktopThreadVisibilityPolicyStore.load")(function* (threadRef: unknown) {
    return yield* Effect.sync((): DesktopThreadVisibilityLoadResult => {
      if (corrupt) return { status: "rejected", reason: "corrupt_store" };
      if (!validRef(threadRef)) return { status: "rejected", reason: "invalid_request" };
      const record = records.findLast((candidate) => candidate.intent.threadRef === threadRef);
      if (record === undefined) return { status: "missing" };
      return {
        status: "found",
        visibilityVersion: record.receipt.result.visibilityVersion,
        target: clone(record.receipt.result.target),
        receipt: clone(record.receipt),
      };
    });
  });

  return { apply, load } as const;
};

export const desktopThreadVisibilityPolicyFileName = (ownerRef: string): string =>
  `${createHash("sha256").update(ownerRef).digest("hex")}.json`;
