#!/usr/bin/env node

import {
  assertInternalGitHubWriteAllowed,
  type InternalGitHubWriteOperation,
} from "../packages/all-work-contract/src/internal-github-write-policy.ts";

const operation = process.argv[2] as InternalGitHubWriteOperation | undefined;
if (
  operation !== "internal_issue_create" &&
  operation !== "internal_issue_comment" &&
  operation !== "internal_claim_comment"
) {
  process.stderr.write(
    "usage: internal-github-write-policy.ts <internal_issue_create|internal_issue_comment|internal_claim_comment>\n",
  );
  process.exit(2);
}

try {
  const decision = assertInternalGitHubWriteAllowed(operation);
  process.stdout.write(`${JSON.stringify(decision)}\n`);
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exit(3);
}
