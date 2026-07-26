import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

const schema = "openagents.forge.omega_dogfood.v1";
const expectedRepository = {
  forgeCloneUrl: "https://openagents.com/git/openagents/omega.git",
  forgeRepositoryRef: "omega",
  forgeTenantRef: "openagents",
  githubCloneUrl: "https://github.com/OpenAgentsInc/omega.git",
  githubRepository: "OpenAgentsInc/omega",
  sourceRef: "refs/heads/main",
} as const;

const stages = [
  "admission",
  "object_hosting",
  "signed_state",
  "journey",
  "mirror",
  "backup_restore",
  "divergence",
] as const;

type RecordValue = Record<string, unknown>;
type Stage = (typeof stages)[number];

export class ForgeOmegaDogfoodEvidenceError extends Error {
  override readonly name = "ForgeOmegaDogfoodEvidenceError";
}

const fail = (message: string): never => {
  throw new ForgeOmegaDogfoodEvidenceError(message);
};

const record = (value: unknown, label: string): RecordValue =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as RecordValue)
    : fail(`${label} must be an object`);

const string = (value: unknown, label: string): string =>
  typeof value === "string" && value.length > 0
    ? value
    : fail(`${label} must be a non-empty string`);

const boolean = (value: unknown, label: string): boolean =>
  typeof value === "boolean" ? value : fail(`${label} must be boolean`);

const stringArray = (value: unknown, label: string): ReadonlyArray<string> => {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string" || item === "")) {
    fail(`${label} must be an array of non-empty strings`);
  }
  return value as ReadonlyArray<string>;
};

const objectId = (value: unknown, label: string): string => {
  const result = string(value, label);
  return /^[a-f0-9]{40}(?:[a-f0-9]{24})?$/iu.test(result)
    ? result.toLowerCase()
    : fail(`${label} must be a Git object id`);
};

const exact = (value: unknown, expected: string, label: string): void => {
  if (value !== expected) fail(`${label} must be ${expected}`);
};

const assertNoFixture = (value: unknown, label: string): void => {
  if (typeof value === "string" && /(fixture|example|fake|simulat)/iu.test(value)) {
    fail(`${label} must not name a fixture or simulated repository`);
  }
  if (Array.isArray(value)) {
    value.forEach((item, index) => assertNoFixture(item, `${label}[${index}]`));
  } else if (typeof value === "object" && value !== null) {
    for (const [key, item] of Object.entries(value as RecordValue)) {
      assertNoFixture(item, `${label}.${key}`);
    }
  }
};

const validateStage = (value: unknown, stage: Stage, status: "prepared" | "completed"): void => {
  const entry = record(value, `$.stages.${stage}`);
  const state = string(entry.state, `$.stages.${stage}.state`);
  const receiptRefs = stringArray(entry.receiptRefs, `$.stages.${stage}.receiptRefs`);
  if (status === "prepared") {
    if (state !== "not_started") fail(`$.stages.${stage}.state must be not_started when prepared`);
    if (receiptRefs.length !== 0) fail(`$.stages.${stage}.receiptRefs must be empty when prepared`);
    return;
  }
  if (state !== "completed") fail(`$.stages.${stage}.state must be completed`);
  if (receiptRefs.length === 0) fail(`$.stages.${stage}.receiptRefs must not be empty`);
};

/**
 * Validates a public-safe receipt for the real OpenAgentsInc/omega Forge
 * dogfood migration. A prepared record is intentionally not completion proof.
 */
export const validateForgeOmegaDogfoodEvidence = (value: unknown) => {
  const root = record(value, "$");
  exact(root.schema, schema, "$.schema");
  const status = string(root.status, "$.status");
  if (status !== "prepared" && status !== "completed") {
    fail("$.status must be prepared or completed");
  }
  const repository = record(root.repository, "$.repository");
  exact(
    repository.githubRepository,
    expectedRepository.githubRepository,
    "$.repository.githubRepository",
  );
  exact(
    repository.githubCloneUrl,
    expectedRepository.githubCloneUrl,
    "$.repository.githubCloneUrl",
  );
  exact(
    repository.forgeTenantRef,
    expectedRepository.forgeTenantRef,
    "$.repository.forgeTenantRef",
  );
  exact(
    repository.forgeRepositoryRef,
    expectedRepository.forgeRepositoryRef,
    "$.repository.forgeRepositoryRef",
  );
  exact(repository.forgeCloneUrl, expectedRepository.forgeCloneUrl, "$.repository.forgeCloneUrl");
  exact(repository.sourceRef, expectedRepository.sourceRef, "$.repository.sourceRef");
  objectId(repository.githubHeadObjectId, "$.repository.githubHeadObjectId");

  const safeguards = record(root.safeguards, "$.safeguards");
  if (boolean(safeguards.realRepositoryOnly, "$.safeguards.realRepositoryOnly") !== true) {
    fail("$.safeguards.realRepositoryOnly must be true");
  }
  if (
    boolean(safeguards.githubCriticalPathReads, "$.safeguards.githubCriticalPathReads") !== false
  ) {
    fail("$.safeguards.githubCriticalPathReads must be false");
  }
  if (boolean(safeguards.containsCredentials, "$.safeguards.containsCredentials") !== false) {
    fail("$.safeguards.containsCredentials must be false");
  }
  if (
    boolean(safeguards.publicCutoverApplied, "$.safeguards.publicCutoverApplied") !==
    (status === "completed")
  ) {
    fail("$.safeguards.publicCutoverApplied must match completion status");
  }

  const stageRecords = record(root.stages, "$.stages");
  for (const stage of stages) validateStage(stageRecords[stage], stage, status);

  const issueRefs = stringArray(root.issueRefs, "$.issueRefs");
  for (const issue of [9244, 9245, 9246, 9247, 9248, 9249, 9250, 9251]) {
    if (!issueRefs.includes(`OpenAgentsInc/openagents#${issue}`)) {
      fail(`$.issueRefs must include OpenAgentsInc/openagents#${issue}`);
    }
  }
  assertNoFixture(root, "$");
  return root;
};

export const verifyForgeOmegaSource = (input: {
  readonly expectedHeadObjectId: string;
  readonly sourceRef?: string | undefined;
}): string => {
  const sourceRef = input.sourceRef ?? expectedRepository.sourceRef;
  if (sourceRef !== expectedRepository.sourceRef) {
    fail(`sourceRef must be ${expectedRepository.sourceRef}`);
  }
  const output = execFileSync("git", ["ls-remote", expectedRepository.githubCloneUrl, sourceRef], {
    encoding: "utf8",
    maxBuffer: 64 * 1024,
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
  const [actualHeadObjectId, actualRef] = output.split(/\s+/u);
  if (actualHeadObjectId === undefined || actualRef !== sourceRef) {
    fail("real Omega source head was not available from its declared GitHub origin");
  }
  const expectedHeadObjectId = objectId(input.expectedHeadObjectId, "expectedHeadObjectId");
  if (actualHeadObjectId.toLowerCase() !== expectedHeadObjectId) {
    fail("real Omega source head changed; refresh the prepared dogfood record before migration");
  }
  return expectedHeadObjectId;
};

if (process.argv[1]?.endsWith("forge-omega-dogfood.ts")) {
  const source = process.argv[2];
  if (source === undefined) {
    throw new ForgeOmegaDogfoodEvidenceError(
      "usage: node --import tsx scripts/forge-omega-dogfood.ts <receipt.json>",
    );
  }
  const receipt = validateForgeOmegaDogfoodEvidence(
    JSON.parse(readFileSync(source, "utf8")) as unknown,
  );
  const repository = receipt.repository as Record<string, unknown>;
  verifyForgeOmegaSource({ expectedHeadObjectId: string(repository.githubHeadObjectId, "head") });
  process.stdout.write("valid Forge Omega dogfood evidence\n");
}
