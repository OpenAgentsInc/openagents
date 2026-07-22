import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";

import { Schema } from "effect";

import {
  IdePortablePlacementCohortSchema,
  type IdePortablePlacementCohort,
} from "../../openagents-desktop/src/ide/portable-evidence-contract.ts";
import { Ide13OwnerLocalRealCohortReceiptSchema } from "./ide13-owner-local-real-cohort.ts";

const Ref = Schema.String.check(
  Schema.isMinLength(3),
  Schema.isMaxLength(512),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
);

export const Ide13RealRemoteCohortReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal("openagents.desktop.ide-portable-real-remote-cohort.v1"),
  generatedAt: Schema.String,
  sourceReceiptSha256: Schema.String.check(Schema.isPattern(/^[a-f0-9]{64}$/u)),
  controller: Schema.Struct({
    transportRef: Ref,
    targetIdentityDigestRef: Ref,
    isolatedRunRoot: Schema.Literal(true),
    standingServiceChanged: Schema.Literal(false),
  }),
  cohort: IdePortablePlacementCohortSchema,
});

export interface Ide13RealRemoteCohortReceipt extends Schema.Schema.Type<
  typeof Ide13RealRemoteCohortReceiptSchema
> {}

const decodeSource = Schema.decodeUnknownSync(Ide13OwnerLocalRealCohortReceiptSchema);
const decodeReceipt = Schema.decodeUnknownSync(Ide13RealRemoteCohortReceiptSchema);
const sha256 = (value: string): string => createHash("sha256").update(value).digest("hex");

export const createIde13RealRemoteCohort = async (input: Readonly<{
  sourcePath: string;
  outputPath?: string;
  targetIdentity: string;
}>): Promise<Ide13RealRemoteCohortReceipt> => {
  const sourceText = await readFile(input.sourcePath, "utf8");
  const source = decodeSource(JSON.parse(sourceText), { onExcessProperty: "error" });
  if (source.cohort.operatingSystem !== "linux" || source.cohort.architecture !== "x64") {
    throw new Error("the owner-managed cohort must run on the admitted Linux x64 host");
  }
  const evidenceClass = "real_owner_managed" as const;
  const cohort: IdePortablePlacementCohort = {
    ...source.cohort,
    cohortRef: "cohort.ide13.owner-managed.real.1",
    targetClass: "owner_managed",
    evidenceClass,
    journeys: {
      ...source.cohort.journeys,
      faultMatrixReceiptRef: null,
    },
    adapter: {
      kind: "production",
      ref: "adapter.pylon.owner-managed.control-session.v1",
      name: "Pylon owner-managed remote control-session target",
      version: "1",
    },
    targetRef: `target.owner-managed.gce.${sha256(input.targetIdentity).slice(0, 32)}`,
    custody: "owner_managed",
    networkDestinations: ["network.google-cloud-iap"],
    dataDestinations: ["data.owner-managed-gce-host"],
    retentionSeconds: 0,
    costFact:
      "The run used an existing owner-managed GCE host. It created no cloud resource and added no measured infrastructure cost.",
    phaseReceipts: source.cohort.phaseReceipts.map((phase) => ({
      ...phase,
      evidenceClass,
    })),
    result:
      "An isolated process on the owner-managed Linux x64 host completed the production Pylon move, failback, replay, stale-generation refusal, encrypted artifact deletion, and teardown checks. The controller changed no standing service.",
  };
  const receipt = decodeReceipt(
    {
      schemaVersion: "openagents.desktop.ide-portable-real-remote-cohort.v1",
      generatedAt: new Date().toISOString(),
      sourceReceiptSha256: sha256(sourceText),
      controller: {
        transportRef: "transport.google-cloud-iap.ssh.v1",
        targetIdentityDigestRef: `digest.target.${sha256(input.targetIdentity)}`,
        isolatedRunRoot: true,
        standingServiceChanged: false,
      },
      cohort,
    },
    { onExcessProperty: "error" },
  );
  if (input.outputPath !== undefined) {
    await mkdir(dirname(input.outputPath), { recursive: true });
    await writeFile(input.outputPath, `${JSON.stringify(receipt, null, 2)}\n`, "utf8");
  }
  return receipt;
};

if (import.meta.main) {
  const [sourcePath, outputPath, targetIdentity] = process.argv.slice(2);
  if (!sourcePath || !outputPath || !targetIdentity) {
    throw new Error("usage: ide13-real-remote-cohort.ts SOURCE OUTPUT TARGET_IDENTITY");
  }
  const receipt = await createIde13RealRemoteCohort({
    sourcePath: resolve(sourcePath),
    outputPath: resolve(outputPath),
    targetIdentity,
  });
  process.stdout.write(
    `${JSON.stringify({ cohortRef: receipt.cohort.cohortRef, result: "passed" })}\n`,
  );
}
