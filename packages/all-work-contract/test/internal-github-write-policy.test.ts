import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import { describe, expect, it } from "@effect/vitest";

import {
  assertInternalGitHubWriteAllowed,
  decideInternalGitHubWrite,
  InternalGitHubWritePolicyError,
  resolveInternalWorkWriter,
} from "../src/internal-github-write-policy.ts";

const writeLedger = (writer: "legacy_github" | "native_omega"): string => {
  const root = mkdtempSync(path.join(tmpdir(), "internal-github-write-policy-"));
  const directory = path.join(root, "all-work");
  mkdirSync(directory, { recursive: true });
  writeFileSync(
    path.join(directory, "work-cutover.v1.json"),
    `${JSON.stringify({
      schema: "openagents.all_work_cutover_authority_state.v1",
      cutover: {
        contractVersion: "openagents.all_work_boundary.v1",
        organizationRef: "organization:openagents",
        authorizedPrincipalRefs: ["principal:omega:local-owner"],
        revision: 2,
        generation: 2,
        writer,
        sourceDigest: "a".repeat(64),
        sourceCursor: "cursor:planning:1",
        nativeHighWatermark: writer === "native_omega" ? "cursor:planning:1" : null,
        activationReceiptRef: writer === "native_omega" ? "receipt:cutover:1" : null,
        rollbackReceiptRef: null,
      },
      receipts: [],
    })}\n`,
  );
  return root;
};

describe("internal GitHub write policy", () => {
  it("keeps legacy GitHub writes available before explicit activation", () => {
    expect(resolveInternalWorkWriter({ env: {} })).toBe("legacy_github");
    expect(decideInternalGitHubWrite("internal_issue_create", { env: {} })).toMatchObject({
      allowed: true,
      route: "github",
    });
  });

  it("routes internal issue and claim writes to Omega after native activation", () => {
    const dataRoot = writeLedger("native_omega");
    expect(
      decideInternalGitHubWrite("internal_claim_comment", { dataRoot, env: {} }),
    ).toMatchObject({ allowed: false, route: "omega", writer: "native_omega" });
    expect(() =>
      assertInternalGitHubWriteAllowed("internal_issue_create", { dataRoot, env: {} }),
    ).toThrow(InternalGitHubWritePolicyError);
  });

  it("fails closed when remote configuration conflicts with the local ledger", () => {
    const dataRoot = writeLedger("native_omega");
    expect(() =>
      resolveInternalWorkWriter({
        dataRoot,
        env: { OPENAGENTS_INTERNAL_WORK_WRITER: "legacy_github" },
      }),
    ).toThrow(/conflicts/u);
  });
});
