import { createHash } from "node:crypto";
import { mkdir, open, readFile } from "node:fs/promises";
import { join } from "node:path";

import { forensicCanonicalJson } from "@openagentsinc/forensic-contract";

export const LOUPE_FIRST_VERDICT_LEDGER_VERSION =
  "openagents.loupe_first_verdict_ledger.v1" as const;

/**
 * The durable first-verdict authority.
 *
 * The verifier does not accept a `commitInitialVerdict` function. Exactly-once
 * would then be a property of whatever the caller handed in, which is precisely
 * what the OFR-007 acceptance audit named as missing. It accepts a directory
 * instead, and owns the compare-and-set itself.
 *
 * The primitive is `open(path, "wx")`: `O_CREAT | O_EXCL`, which the kernel
 * resolves atomically against every other opener of that path, in this process
 * or any other, and against every other machine when the directory is on a
 * filesystem whose `O_EXCL` is honoured. A second writer never wins, never
 * partially overwrites, and never observes a torn record: it gets `EEXIST` and
 * reads what the first writer durably stored.
 *
 * `commit` always returns the value it re-read from disk after the syscall, on
 * both the winning and the losing path, so the returned verdict is the durable
 * one rather than the candidate the caller hoped for.
 */
export interface LoupeFirstVerdictLedger {
  readonly ledgerRef: string;
  readonly directory: string;
  /**
   * Durably records `candidate` as the first verdict for `verificationRef` if
   * and only if no verdict is already stored, and returns whatever verdict is
   * durably stored for it afterwards.
   */
  readonly commit: (verificationRef: string, candidate: unknown) => Promise<unknown>;
  /** Returns the durably stored verdict, or `undefined` when none exists. */
  readonly read: (verificationRef: string) => Promise<unknown>;
}

const recordPath = (directory: string, verificationRef: string): string =>
  join(directory, `${createHash("sha256").update(verificationRef).digest("hex")}.v1.json`);

const readRecord = async (path: string): Promise<unknown> => {
  const raw = await readFile(path, "utf8");
  const parsed = JSON.parse(raw) as { readonly schema?: unknown; readonly verdict?: unknown };
  if (parsed.schema !== LOUPE_FIRST_VERDICT_LEDGER_VERSION) {
    throw new Error("the durable first-verdict ledger holds a record of an unknown schema");
  }
  return parsed.verdict;
};

/**
 * Best-effort directory fsync, so a crash between the file write and the next
 * read cannot lose the directory entry that makes the lock visible. Platforms
 * that refuse to fsync a directory handle are not a reason to fail the commit,
 * because the file itself was already synced.
 */
const syncDirectory = async (directory: string): Promise<void> => {
  let handle;
  try {
    handle = await open(directory, "r");
    await handle.sync();
  } catch {
    // Not every platform permits fsync on a directory handle.
  } finally {
    await handle?.close();
  }
};

export const durableFirstVerdictLedger = (directory: string): LoupeFirstVerdictLedger => ({
  ledgerRef: `ledger.loupe-first-verdict.${createHash("sha256").update(directory).digest("hex").slice(0, 20)}`,
  directory,
  read: async (verificationRef) => {
    try {
      return await readRecord(recordPath(directory, verificationRef));
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ENOENT") return undefined;
      throw error;
    }
  },
  commit: async (verificationRef, candidate) => {
    await mkdir(directory, { recursive: true });
    const path = recordPath(directory, verificationRef);
    const record = `${forensicCanonicalJson({
      schema: LOUPE_FIRST_VERDICT_LEDGER_VERSION,
      verificationRef,
      verdict: candidate,
    })}\n`;
    let handle;
    try {
      // O_CREAT | O_EXCL. This, and nothing above it, is the compare-and-set.
      handle = await open(path, "wx", 0o600);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
      return await readRecord(path);
    }
    try {
      await handle.writeFile(record, "utf8");
      await handle.sync();
    } finally {
      await handle.close();
    }
    await syncDirectory(directory);
    // Deliberately re-read rather than returning `candidate`: what the caller
    // receives is what survived to disk.
    return await readRecord(path);
  },
});
