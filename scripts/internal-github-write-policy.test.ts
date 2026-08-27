import { readFileSync } from "node:fs";
import path from "node:path";

import { describe, expect, test } from "vite-plus/test";

const root = path.resolve(import.meta.dirname, "..");
const read = (relative: string): string => readFileSync(path.join(root, relative), "utf8");

describe("internal GitHub writer cutover audit", () => {
  test("every retained internal issue or progress-comment writer has the native Omega fence", () => {
    const guardedSources = [
      "apps/openagents.com/workers/api/scripts/marching-orders-agent.ts",
      "apps/openagents.com/workers/api/src/artanis-operator-tools.ts",
      "apps/pylon/scripts/codex-supervisor/standing-tasks.sh",
      "apps/pylon/scripts/codex-supervisor/replenishment.sh",
    ];
    for (const source of guardedSources) {
      expect(read(source), source).toContain("internal_issue_");
    }
  });

  test("preserves GitHub completion comments as explicit callback transport", () => {
    const source = read("apps/openagents.com/workers/api/src/agent-definition-bot-integration.ts");
    expect(source).toContain("completionCallback");
    expect(source).toContain("createIssueComment");
    expect(source).not.toContain("internal_claim_comment");

    const backlogFaucet = read("apps/openagents.com/scripts/backlog-faucet-list.ts");
    expect(backlogFaucet).toContain("listedIssueCommentBody");
    expect(backlogFaucet).not.toContain("internal_claim_comment");
  });

  test("keeps strict public bug intake as ingress instead of an internal issue writer", () => {
    const form = read(".github/ISSUE_TEMPLATE/strict-bug.yml");
    const authority = read("packages/all-work-contract/src/strict-bug-candidate-authority.ts");
    expect(form).toContain("Strict Bug Report");
    expect(authority).toContain('disposition: "pending"');
    expect(authority).toContain("signatureVerificationRef");
  });
});
