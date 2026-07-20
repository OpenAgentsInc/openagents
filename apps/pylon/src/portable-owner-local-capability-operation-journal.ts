import { createHash, randomUUID } from "node:crypto";
import { chmod, lstat, mkdir, open, readFile, rename, unlink } from "node:fs/promises";
import { join } from "node:path";

import {
  PortableOwnerLocalCapabilityOperationClaimRequestSchema,
  PortableOwnerLocalCapabilityOperationRecordSchema,
  PortableOwnerLocalCapabilityOperationResultRequestSchema,
} from "@openagentsinc/portable-session-contract";
import { Schema as S } from "effect";

const JOURNAL_SCHEMA =
  "openagents.pylon.portable_owner_local_capability_operation_journal.v1" as const;
const MAX_JOURNAL_BYTES = 2 * 1_024 * 1_024;
const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const forbiddenPrivateMaterial =
  /"(?:[A-Za-z0-9_]*(?:path|paths|credential|credentials|handle|handles|bytes)|token|apiKey|authorization|sessionToken|refreshToken|mnemonic|secret|hostname|processId|providerSessionId|socket|pid|authHome)"\s*:|(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)/iu;

const EntrySchema = S.Struct({
  record: PortableOwnerLocalCapabilityOperationRecordSchema,
  claimRequest: PortableOwnerLocalCapabilityOperationClaimRequestSchema,
  claimGeneration: S.NullOr(S.Number.check(S.isInt(), S.isGreaterThan(0))),
  leaseRevision: S.NullOr(S.Number.check(S.isInt(), S.isGreaterThan(0))),
  leaseExpiresAt: PortableOwnerLocalCapabilityOperationRecordSchema.fields.leaseExpiresAt,
  state: S.Literals(["claiming", "claimed", "executing", "uncertain", "completion_pending"]),
  completion: S.NullOr(PortableOwnerLocalCapabilityOperationResultRequestSchema),
});

const JournalSchema = S.Struct({
  schema: S.Literal(JOURNAL_SCHEMA),
  pylonRef: S.String,
  targetRef: S.String,
  workerInstanceRef: S.String,
  entries: S.Array(EntrySchema),
});

export type PylonPortableOwnerLocalCapabilityOperationJournalEntry = S.Schema.Type<
  typeof EntrySchema
>;

export type PylonPortableOwnerLocalCapabilityOperationJournal = Readonly<{
  entries: () => Promise<ReadonlyArray<PylonPortableOwnerLocalCapabilityOperationJournalEntry>>;
  put: (entry: PylonPortableOwnerLocalCapabilityOperationJournalEntry) => Promise<void>;
  remove: (operationRef: string) => Promise<void>;
}>;

export class PylonPortableOwnerLocalCapabilityOperationJournalError extends Error {
  override readonly name = "PylonPortableOwnerLocalCapabilityOperationJournalError";

  constructor(readonly reason: "corrupt" | "io_failed" | "unsafe_material") {
    super(`Pylon portable owner-local capability operation journal failed: ${reason}`);
  }
}

export type MakePylonPortableOwnerLocalCapabilityOperationJournalOptions = Readonly<{
  directory: string;
  pylonRef: string;
  targetRef: string;
  workerInstanceRef: string;
}>;

const journalError = (reason: PylonPortableOwnerLocalCapabilityOperationJournalError["reason"]) =>
  new PylonPortableOwnerLocalCapabilityOperationJournalError(reason);

const validateEntry = (
  entry: PylonPortableOwnerLocalCapabilityOperationJournalEntry,
  scope: Readonly<{ pylonRef: string; targetRef: string; workerInstanceRef: string }>,
): void => {
  const request = entry.record.request;
  if (
    request.pylonRef !== scope.pylonRef ||
    request.targetRef !== scope.targetRef ||
    entry.claimRequest.pylonRef !== scope.pylonRef ||
    entry.claimRequest.targetRef !== scope.targetRef ||
    entry.claimRequest.workerInstanceRef !== scope.workerInstanceRef ||
    entry.claimRequest.operationRef !== request.operationRef ||
    entry.claimRequest.sessionRef !== request.sessionRef ||
    entry.claimRequest.attachmentRef !== request.attachmentRef ||
    entry.claimRequest.attachmentGeneration !== request.attachmentGeneration
  ) {
    throw journalError("corrupt");
  }
  const claimed = entry.state !== "claiming";
  if (
    (claimed &&
      (entry.claimGeneration === null ||
        entry.leaseRevision === null ||
        entry.leaseExpiresAt === null ||
        entry.record.state !== "claimed" ||
        entry.record.claimRef !== entry.claimRequest.claimRef ||
        entry.record.workerInstanceRef !== scope.workerInstanceRef ||
        entry.record.claimGeneration !== entry.claimGeneration ||
        entry.record.leaseRevision !== entry.leaseRevision ||
        entry.record.leaseExpiresAt !== entry.leaseExpiresAt)) ||
    (!claimed &&
      (entry.claimGeneration !== null ||
        entry.leaseRevision !== null ||
        entry.leaseExpiresAt !== null ||
        entry.completion !== null ||
        entry.record.state !== "pending")) ||
    (entry.state === "completion_pending") !== (entry.completion !== null)
  ) {
    throw journalError("corrupt");
  }
  if (
    entry.completion !== null &&
    (entry.completion.claimRef !== entry.claimRequest.claimRef ||
      entry.completion.workerInstanceRef !== scope.workerInstanceRef ||
      entry.completion.claimGeneration !== entry.claimGeneration ||
      entry.completion.expectedLeaseRevision !== entry.leaseRevision ||
      entry.completion.executableProfileRef !== request.executableProfileRef)
  ) {
    throw journalError("corrupt");
  }
};

export const makePylonPortableOwnerLocalCapabilityOperationJournal = (
  options: MakePylonPortableOwnerLocalCapabilityOperationJournalOptions,
): PylonPortableOwnerLocalCapabilityOperationJournal => {
  if (
    options.directory.trim() === "" ||
    ![options.pylonRef, options.targetRef, options.workerInstanceRef].every((ref) =>
      SAFE_REF.test(ref),
    )
  ) {
    throw journalError("corrupt");
  }
  const scope = {
    pylonRef: options.pylonRef,
    targetRef: options.targetRef,
    workerInstanceRef: options.workerInstanceRef,
  };
  const scopeDigest = createHash("sha256")
    .update(`${options.pylonRef}\0${options.targetRef}\0${options.workerInstanceRef}`)
    .digest("hex");
  const fileName = `portable-owner-local-capability-${scopeDigest}.json`;
  const path = join(options.directory, fileName);

  const ensureDirectory = async (): Promise<void> => {
    try {
      await mkdir(options.directory, { recursive: true, mode: 0o700 });
      const info = await lstat(options.directory);
      if (!info.isDirectory() || info.isSymbolicLink() || (info.mode & 0o077) !== 0) {
        throw journalError("unsafe_material");
      }
    } catch (error) {
      if (error instanceof PylonPortableOwnerLocalCapabilityOperationJournalError) throw error;
      throw journalError("io_failed");
    }
  };

  const empty = () => ({ schema: JOURNAL_SCHEMA, ...scope, entries: [] });

  const read = async (): Promise<S.Schema.Type<typeof JournalSchema>> => {
    await ensureDirectory();
    let bytes: Buffer;
    try {
      const info = await lstat(path);
      if (!info.isFile() || info.isSymbolicLink() || (info.mode & 0o077) !== 0) {
        throw journalError("unsafe_material");
      }
      if (info.size > MAX_JOURNAL_BYTES) throw journalError("corrupt");
      bytes = await readFile(path);
    } catch (error) {
      if (error instanceof PylonPortableOwnerLocalCapabilityOperationJournalError) throw error;
      if (error instanceof Error && "code" in error && error.code === "ENOENT") return empty();
      throw journalError("io_failed");
    }
    const raw = bytes.toString("utf8");
    if (forbiddenPrivateMaterial.test(raw)) throw journalError("unsafe_material");
    let decoded: S.Schema.Type<typeof JournalSchema>;
    try {
      decoded = S.decodeUnknownSync(S.fromJsonString(JournalSchema))(raw, {
        onExcessProperty: "error",
      });
    } catch {
      throw journalError("corrupt");
    }
    if (
      decoded.pylonRef !== scope.pylonRef ||
      decoded.targetRef !== scope.targetRef ||
      decoded.workerInstanceRef !== scope.workerInstanceRef ||
      new Set(decoded.entries.map((entry) => entry.record.request.operationRef)).size !==
        decoded.entries.length
    ) {
      throw journalError("corrupt");
    }
    for (const entry of decoded.entries) validateEntry(entry, scope);
    return decoded;
  };

  const write = async (journal: S.Schema.Type<typeof JournalSchema>): Promise<void> => {
    const encoded = `${JSON.stringify(journal)}\n`;
    if (forbiddenPrivateMaterial.test(encoded)) throw journalError("unsafe_material");
    if (Buffer.byteLength(encoded) > MAX_JOURNAL_BYTES) throw journalError("corrupt");
    await ensureDirectory();
    const temporary = join(options.directory, `.${fileName}.${randomUUID()}.tmp`);
    try {
      const handle = await open(temporary, "wx", 0o600);
      try {
        await handle.writeFile(encoded, { encoding: "utf8" });
        await handle.sync();
      } finally {
        await handle.close();
      }
      await chmod(temporary, 0o600);
      await rename(temporary, path);
      await chmod(path, 0o600);
      const directory = await open(options.directory, "r");
      try {
        await directory.sync();
      } finally {
        await directory.close();
      }
    } catch (error) {
      await unlink(temporary).catch(() => undefined);
      if (error instanceof PylonPortableOwnerLocalCapabilityOperationJournalError) throw error;
      throw journalError("io_failed");
    }
  };

  return {
    entries: async () => (await read()).entries,
    put: async (entry) => {
      const exact = S.decodeUnknownSync(EntrySchema)(entry, { onExcessProperty: "error" });
      validateEntry(exact, scope);
      const journal = await read();
      await write({
        ...journal,
        entries: [
          ...journal.entries.filter(
            (current) => current.record.request.operationRef !== exact.record.request.operationRef,
          ),
          exact,
        ],
      });
    },
    remove: async (operationRef) => {
      if (!SAFE_REF.test(operationRef)) throw journalError("corrupt");
      const journal = await read();
      const entries = journal.entries.filter(
        (entry) => entry.record.request.operationRef !== operationRef,
      );
      if (entries.length !== journal.entries.length) await write({ ...journal, entries });
    },
  };
};
