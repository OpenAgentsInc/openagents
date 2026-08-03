import { describe, expect, test } from "vite-plus/test";

import type {
  StrictBugCandidateExecuteRequest,
  StrictBugCandidateExecuteResult,
  StrictBugCandidateReadResult,
} from "../../../../../packages/all-work-contract/src/generated.js";

import {
  handleStrictBugGitHubWebhookRequest,
  type StrictBugCandidateGateway,
} from "./strict-bug-webhook-routes";

const secret = "strict-bug-webhook-secret";
const deliveryId = "delivery-strict-bug-1";
const encoder = new TextEncoder();

const issueBody = `### Affected surface

Omega Work Index

### Actual behavior

The row disappears after reconnect.

### Expected behavior

The row remains visible.

### Reproduction steps

1. Open Work.\n2. Reconnect.\n3. Observe the missing row.

### Public-safe evidence

receipt:public-safe:work-reconnect

### Severity

S1 - supported user or agent path is broken

### Environment

Omega 0.2.0-rc1 on macOS 15.

### Safety and redaction

I removed all sensitive material.

### Required confirmations

- [x] I am reporting a specific reproducible bug, not a feature request.
- [x] I searched existing GitHub issues and the Product Promises Forum.
- [x] I removed secrets, bearer tokens, and private material.
- [x] I included exact reproduction steps and public-safe evidence.
- [x] I understand malformed or loose reports should be rejected.
`;

const payload = (overrides: Record<string, unknown> = {}) => ({
  action: "opened",
  issue: {
    body: issueBody,
    created_at: "2026-08-03T17:00:00.000Z",
    html_url: "https://github.com/OpenAgentsInc/openagents/issues/9400",
    labels: [{ name: "bug" }],
    number: 9400,
    title: "[Bug]: Work row disappears after reconnect",
    user: { login: "public-reporter" },
  },
  repository: { full_name: "OpenAgentsInc/openagents" },
  ...overrides,
});

const signature = async (body: string): Promise<string> => {
  const key = await crypto.subtle.importKey(
    "raw",
    encoder.encode(secret),
    { hash: "SHA-256", name: "HMAC" },
    false,
    ["sign"],
  );
  const digest = await crypto.subtle.sign("HMAC", key, encoder.encode(body));
  return `sha256=${[...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("")}`;
};

const request = async (value: unknown, signatureOverride?: string): Promise<Request> => {
  const body = JSON.stringify(value);
  return new Request("https://openagents.com/v1/work/webhooks/github/strict-bugs", {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-github-delivery": deliveryId,
      "x-github-event": "issues",
      "x-hub-signature-256": signatureOverride ?? (await signature(body)),
    },
    body,
  });
};

const emptyRead = (): StrictBugCandidateReadResult =>
  ({
    ledger: {
      revision: 7,
      candidates: [],
    },
  }) as unknown as StrictBugCandidateReadResult;

describe("strict public bug GitHub webhook transport", () => {
  test("verifies, normalizes, and forwards one typed untrusted candidate command", async () => {
    const executed: StrictBugCandidateExecuteRequest[] = [];
    const gateway: StrictBugCandidateGateway = {
      read: async () => emptyRead(),
      execute: async (command) => {
        executed.push(command);
        return {
          receipt: {
            intentRef: command.intentRef,
            previousRevision: 7,
            revision: 8,
          },
        } as unknown as StrictBugCandidateExecuteResult;
      },
    };
    const result = await handleStrictBugGitHubWebhookRequest(await request(payload()), {
      githubSecret: secret,
      gateway,
    });

    expect(result.status).toBe(202);
    expect(executed).toHaveLength(1);
    expect(executed[0]).toMatchObject({
      capabilityRef: "capability:strict-bug-candidate:ingest",
      effectivePrincipalRef: "principal:github:webhook",
      expectedRevision: 7,
      githubWriteCount: 0,
      idempotencyKey: "github-delivery:source:github:webhook:delivery-strict-bug-1",
      command: {
        candidateRef: "strict-bug-candidate:openagents:9400",
        command: "ingest",
        deliveryRef: "source:github:webhook:delivery-strict-bug-1",
        repositoryRef: "repository:openagents",
        severity: "s1",
        signatureVerificationRef: "evidence:github-webhook-signature:delivery-strict-bug-1",
      },
    });
  });

  test("treats an identical signed delivery as an idempotent replay", async () => {
    let executions = 0;
    const gateway: StrictBugCandidateGateway = {
      read: async () =>
        ({
          ledger: {
            revision: 8,
            candidates: [
              {
                deliveryRef: "source:github:webhook:delivery-strict-bug-1",
                disposition: "pending",
              },
            ],
          },
        }) as unknown as StrictBugCandidateReadResult,
      execute: async () => {
        executions += 1;
        throw new Error("must not execute a replay");
      },
    };
    const result = await handleStrictBugGitHubWebhookRequest(await request(payload()), {
      githubSecret: secret,
      gateway,
    });
    expect(result.status).toBe(200);
    expect(executions).toBe(0);
    expect(await result.json()).toMatchObject({ idempotent: true });
  });

  test("refuses bad signatures, wrong repositories, and incomplete forms", async () => {
    const gateway: StrictBugCandidateGateway = {
      read: async () => emptyRead(),
      execute: async () => {
        throw new Error("must not execute refused ingress");
      },
    };
    const unauthorized = await handleStrictBugGitHubWebhookRequest(
      await request(payload(), "sha256=00"),
      { githubSecret: secret, gateway },
    );
    expect(unauthorized.status).toBe(401);

    const wrongRepository = await handleStrictBugGitHubWebhookRequest(
      await request(payload({ repository: { full_name: "someone/private" } })),
      { githubSecret: secret, gateway },
    );
    expect(wrongRepository.status).toBe(400);

    const incomplete = payload();
    (incomplete.issue as { body: string }).body = issueBody.replace(
      "- [x] I included exact reproduction steps",
      "- [ ] I included exact reproduction steps",
    );
    const incompleteResult = await handleStrictBugGitHubWebhookRequest(await request(incomplete), {
      githubSecret: secret,
      gateway,
    });
    expect(incompleteResult.status).toBe(400);
  });
});
