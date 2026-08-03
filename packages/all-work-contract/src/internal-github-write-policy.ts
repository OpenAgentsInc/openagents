import { readFileSync } from "node:fs";
import path from "node:path";

import { decodeWorkCutoverState, type WorkWriter } from "./generated.ts";

export const INTERNAL_WORK_WRITER_ENV = "OPENAGENTS_INTERNAL_WORK_WRITER" as const;
export const OMEGA_EFFECTD_DATA_ROOT_ENV = "OPENAGENTS_OMEGA_EFFECTD_DATA_ROOT" as const;

export type InternalGitHubWriteOperation =
  | "internal_issue_create"
  | "internal_issue_comment"
  | "internal_claim_comment";

export type InternalGitHubWritePolicyInput = Readonly<{
  dataRoot?: string | undefined;
  env?: Readonly<Record<string, string | undefined>> | undefined;
}>;

export type InternalGitHubWriteDecision = Readonly<{
  allowed: boolean;
  operation: InternalGitHubWriteOperation;
  reason: "legacy_writer_active" | "native_writer_active";
  route: "github" | "omega";
  writer: WorkWriter;
}>;

export class InternalGitHubWritePolicyError extends Error {
  readonly code:
    | "invalid_writer_configuration"
    | "invalid_cutover_state"
    | "native_writer_active"
    | "writer_configuration_conflict";

  constructor(
    code: InternalGitHubWritePolicyError["code"],
    message: string,
    options?: ErrorOptions,
  ) {
    super(message, options);
    this.name = "InternalGitHubWritePolicyError";
    this.code = code;
  }
}

const configuredWriter = (env: Readonly<Record<string, string | undefined>>): WorkWriter | null => {
  const value = env[INTERNAL_WORK_WRITER_ENV]?.trim();
  if (value === undefined || value === "") return null;
  if (value === "legacy_github" || value === "native_omega") return value;
  throw new InternalGitHubWritePolicyError(
    "invalid_writer_configuration",
    `${INTERNAL_WORK_WRITER_ENV} must be legacy_github or native_omega`,
  );
};

const ledgerWriter = (dataRoot: string | undefined): WorkWriter | null => {
  if (dataRoot === undefined || dataRoot.trim() === "") return null;
  const statePath = path.join(dataRoot, "all-work", "work-cutover.v1.json");
  let parsed: unknown;
  try {
    parsed = JSON.parse(readFileSync(statePath, "utf8"));
  } catch (error) {
    if (typeof error === "object" && error !== null && Reflect.get(error, "code") === "ENOENT") {
      return null;
    }
    throw new InternalGitHubWritePolicyError(
      "invalid_cutover_state",
      "the internal Work writer ledger could not be read",
      { cause: error },
    );
  }
  try {
    const record = parsed as { cutover?: unknown };
    return decodeWorkCutoverState(record.cutover).writer;
  } catch (error) {
    throw new InternalGitHubWritePolicyError(
      "invalid_cutover_state",
      "the internal Work writer ledger is invalid",
      { cause: error },
    );
  }
};

export const resolveInternalWorkWriter = (
  input: InternalGitHubWritePolicyInput = {},
): WorkWriter => {
  const env = input.env ?? process.env;
  const fromConfiguration = configuredWriter(env);
  const fromLedger = ledgerWriter(input.dataRoot ?? env[OMEGA_EFFECTD_DATA_ROOT_ENV]);
  if (fromConfiguration !== null && fromLedger !== null && fromConfiguration !== fromLedger) {
    throw new InternalGitHubWritePolicyError(
      "writer_configuration_conflict",
      "configured internal Work writer conflicts with the owner-local cutover ledger",
    );
  }
  return fromLedger ?? fromConfiguration ?? "legacy_github";
};

export const decideInternalGitHubWrite = (
  operation: InternalGitHubWriteOperation,
  input: InternalGitHubWritePolicyInput = {},
): InternalGitHubWriteDecision => {
  const writer = resolveInternalWorkWriter(input);
  return writer === "legacy_github"
    ? { allowed: true, operation, reason: "legacy_writer_active", route: "github", writer }
    : { allowed: false, operation, reason: "native_writer_active", route: "omega", writer };
};

export const assertInternalGitHubWriteAllowed = (
  operation: InternalGitHubWriteOperation,
  input: InternalGitHubWritePolicyInput = {},
): InternalGitHubWriteDecision => {
  const decision = decideInternalGitHubWrite(operation, input);
  if (!decision.allowed) {
    throw new InternalGitHubWritePolicyError(
      "native_writer_active",
      `${operation} is disabled because native_omega is the internal Work writer; create or update canonical Work through Omega`,
    );
  }
  return decision;
};
