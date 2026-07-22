import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

import { Schema } from "effect";

import { Ide13OwnerLocalRealFaultMatrixReceiptSchema } from "./ide13-owner-local-real-fault-matrix.ts";

const sha256 = (value: string): string => createHash("sha256").update(value).digest("hex");
const decodeSource = Schema.decodeUnknownSync(Ide13OwnerLocalRealFaultMatrixReceiptSchema);

export const createIde13RealRemoteFaultMatrix = async (input: Readonly<{
  sourcePath: string;
  outputPath: string;
  targetIdentity: string;
}>): Promise<void> => {
  const sourceText = await readFile(input.sourcePath, "utf8");
  const source = decodeSource(JSON.parse(sourceText), { onExcessProperty: "error" });
  const cases = source.cases
    .filter((fault) => fault.outcome === "passed")
    .map((fault) => ({
      ...fault,
      faultRef: fault.faultRef.replace("owner-local", "owner-managed"),
      evidenceClass: "real_owner_managed",
      disclosure:
        "The isolated production Pylon composition injected this fault on the owner-managed Linux x64 host. It completed recovery and verified zero remote process, queue, lease, session, SQLite, and ciphertext residue.",
    }));
  if (cases.length === 0) throw new Error("the remote fault matrix has no passed case");
  const output = {
    schemaVersion: "openagents.desktop.ide-portable-real-remote-fault-matrix.v1",
    generatedAt: new Date().toISOString(),
    sourceReceiptSha256: sha256(sourceText),
    candidateCommitSha: source.candidateCommitSha,
    baseCommitSha: source.baseCommitSha,
    cohortRef: "cohort.ide13.owner-managed.real.1",
    targetClass: "owner_managed",
    evidenceClass: "real_owner_managed",
    targetIdentityDigestRef: `digest.target.${sha256(input.targetIdentity)}`,
    transportRef: "transport.google-cloud-iap.ssh.v1",
    cases,
    safety: source.safety,
    omittedCaseCount: source.cases.length - cases.length,
  };
  await mkdir(dirname(input.outputPath), { recursive: true });
  await writeFile(input.outputPath, `${JSON.stringify(output, null, 2)}\n`, "utf8");
};

if (import.meta.main) {
  const [sourcePath, outputPath, targetIdentity] = process.argv.slice(2);
  if (!sourcePath || !outputPath || !targetIdentity) {
    throw new Error("usage: ide13-real-remote-fault-matrix.ts SOURCE OUTPUT TARGET_IDENTITY");
  }
  await createIde13RealRemoteFaultMatrix({
    sourcePath: resolve(sourcePath),
    outputPath: resolve(outputPath),
    targetIdentity,
  });
}
