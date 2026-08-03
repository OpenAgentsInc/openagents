import {
  decodeStrictBugCandidateExecuteRequest,
  decodeStrictBugCandidateExecuteResult,
  decodeStrictBugCandidateReadResult,
  type StrictBugCandidate,
  type StrictBugCandidateExecuteRequest,
  type StrictBugCandidateExecuteResult,
  type StrictBugCandidateReadResult,
} from "../../../../../packages/all-work-contract/src/generated.js";

import { verifyGitHubWebhookSignature } from "./agent-definition-webhook-routes";
import { methodNotAllowed, noStoreJsonResponse } from "./http/responses";
import { parseJsonRecord } from "./json-boundary";

export const STRICT_BUG_GITHUB_WEBHOOK_PATH = "/v1/work/webhooks/github/strict-bugs" as const;
export const STRICT_BUG_GITHUB_WEBHOOK_MAX_BYTES = 128 * 1024;

const ingressPrincipalRef = "principal:github:webhook";
const ingressCapabilityRef = "capability:strict-bug-candidate:ingest";
const requiredConfirmations = [
  "specific_reproducible_bug",
  "searched_existing_reports",
  "sensitive_material_removed",
  "exact_reproduction_and_evidence",
  "malformed_report_policy_understood",
] as const;

export type StrictBugCandidateGateway = Readonly<{
  read: (candidateRef: string) => Promise<StrictBugCandidateReadResult>;
  execute: (request: StrictBugCandidateExecuteRequest) => Promise<StrictBugCandidateExecuteResult>;
}>;

export type StrictBugWebhookRouteDependencies = Readonly<{
  githubSecret?: string | undefined;
  gateway?: StrictBugCandidateGateway | undefined;
}>;

type GitHubIssuePayload = Readonly<{
  action: string;
  issueNumber: number;
  sourceUrl: string;
  title: string;
  body: string;
  reporterLogin: string;
  repository: "openagents" | "omega";
  occurredAt: string;
}>;

const response = (body: unknown, init: ResponseInit = {}) => noStoreJsonResponse(body, init);

const invalid = (reason: string) =>
  response({ error: "invalid_strict_bug_webhook", reason }, { status: 400 });

const section = (body: string, label: string): string | undefined => {
  const escaped = label.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
  const match = new RegExp(`(?:^|\\n)### ${escaped}\\s*\\n+([\\s\\S]*?)(?=\\n### |$)`, "u").exec(
    body,
  );
  const value = match?.[1]?.trim();
  return value === undefined || value === "" || value === "_No response_" ? undefined : value;
};

const parsePayload = (value: unknown): GitHubIssuePayload | undefined => {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return undefined;
  }
  const payload = value as Record<string, unknown>;
  const action = typeof payload.action === "string" ? payload.action : "";
  const issue = payload.issue as Record<string, unknown> | undefined;
  const repository = payload.repository as Record<string, unknown> | undefined;
  const user = issue?.user as Record<string, unknown> | undefined;
  const labels = Array.isArray(issue?.labels) ? issue.labels : [];
  const fullName = repository?.full_name;
  const repositorySlug =
    fullName === "OpenAgentsInc/openagents"
      ? "openagents"
      : fullName === "OpenAgentsInc/omega"
        ? "omega"
        : undefined;
  if (
    action !== "opened" ||
    repositorySlug === undefined ||
    typeof issue?.number !== "number" ||
    !Number.isSafeInteger(issue.number) ||
    issue.number <= 0 ||
    typeof issue.html_url !== "string" ||
    typeof issue.title !== "string" ||
    !issue.title.startsWith("[Bug]: ") ||
    typeof issue.body !== "string" ||
    typeof issue.created_at !== "string" ||
    typeof user?.login !== "string" ||
    !labels.some(
      (label) =>
        typeof label === "object" && label !== null && Reflect.get(label, "name") === "bug",
    )
  ) {
    return undefined;
  }
  return {
    action,
    issueNumber: issue.number,
    sourceUrl: issue.html_url,
    title: issue.title,
    body: issue.body,
    reporterLogin: user.login,
    repository: repositorySlug,
    occurredAt: issue.created_at,
  };
};

const parseSeverity = (value: string | undefined) => {
  const match = /^(S[0-3])\b/iu.exec(value ?? "")?.[1]?.toLowerCase();
  return match === "s0" || match === "s1" || match === "s2" || match === "s3" ? match : undefined;
};

const hasAllConfirmations = (body: string): boolean => {
  const confirmations = section(body, "Required confirmations");
  if (confirmations === undefined) return false;
  const checked = confirmations.split("\n").filter((line) => /^- \[[xX]\] /u.test(line));
  return (
    checked.some((line) => line.includes("specific reproducible bug")) &&
    checked.some((line) => line.includes("searched existing GitHub issues")) &&
    checked.some((line) => line.includes("removed secrets")) &&
    checked.some((line) => line.includes("exact reproduction steps")) &&
    checked.some((line) => line.includes("malformed or loose reports"))
  );
};

const commandFor = (
  payload: GitHubIssuePayload,
  deliveryId: string,
  expectedRevision: number,
): StrictBugCandidateExecuteRequest | undefined => {
  const affectedSurface = section(payload.body, "Affected surface");
  const actualBehavior = section(payload.body, "Actual behavior");
  const expectedBehavior = section(payload.body, "Expected behavior");
  const reproductionSteps = section(payload.body, "Reproduction steps");
  const publicSafeEvidence = section(payload.body, "Public-safe evidence");
  const severity = parseSeverity(section(payload.body, "Severity"));
  const environment = section(payload.body, "Environment");
  const safetyRedaction = section(payload.body, "Safety and redaction");
  if (
    affectedSurface === undefined ||
    actualBehavior === undefined ||
    expectedBehavior === undefined ||
    reproductionSteps === undefined ||
    publicSafeEvidence === undefined ||
    severity === undefined ||
    environment === undefined ||
    safetyRedaction === undefined ||
    !hasAllConfirmations(payload.body)
  ) {
    return undefined;
  }
  const candidateRef = `strict-bug-candidate:${payload.repository}:${payload.issueNumber}`;
  const deliveryRef = `source:github:webhook:${deliveryId}`;
  try {
    return decodeStrictBugCandidateExecuteRequest({
      intentRef: `intent:strict-bug:${payload.repository}:${payload.issueNumber}:${deliveryId}`,
      idempotencyKey: `github-delivery:${deliveryRef}`,
      expectedRevision,
      effectivePrincipalRef: ingressPrincipalRef,
      capabilityRef: ingressCapabilityRef,
      occurredAt: payload.occurredAt,
      githubWriteCount: 0,
      command: {
        command: "ingest",
        candidateRef,
        sourceRef: `source:github:${payload.repository}:issue:${payload.issueNumber}`,
        deliveryRef,
        repositoryRef: `repository:${payload.repository}`,
        issueNumber: payload.issueNumber,
        sourceUrl: payload.sourceUrl,
        title: payload.title,
        affectedSurface,
        actualBehavior,
        expectedBehavior,
        reproductionSteps,
        publicSafeEvidence,
        severity,
        environment,
        safetyRedaction,
        requiredConfirmations,
        reporterRef: `source:github:user:${payload.reporterLogin}`,
        attachmentRefs: [],
        signatureVerificationRef: `evidence:github-webhook-signature:${deliveryId}`,
      },
    });
  } catch {
    return undefined;
  }
};

const sameDelivery = (candidate: StrictBugCandidate | undefined, deliveryId: string): boolean =>
  candidate?.deliveryRef === `source:github:webhook:${deliveryId}`;

export const handleStrictBugGitHubWebhookRequest = async (
  request: Request,
  dependencies: StrictBugWebhookRouteDependencies,
): Promise<Response> => {
  if (new URL(request.url).pathname !== STRICT_BUG_GITHUB_WEBHOOK_PATH) {
    return response({ error: "not_found" }, { status: 404 });
  }
  if (request.method !== "POST") return methodNotAllowed(["POST"]);
  const secret = dependencies.githubSecret?.trim();
  if (secret === undefined || secret === "") {
    return response({ error: "strict_bug_webhook_secret_not_configured" }, { status: 503 });
  }
  if (dependencies.gateway === undefined) {
    return response({ error: "strict_bug_candidate_gateway_not_configured" }, { status: 503 });
  }
  const declaredLength = Number(request.headers.get("content-length") ?? "0");
  if (Number.isFinite(declaredLength) && declaredLength > STRICT_BUG_GITHUB_WEBHOOK_MAX_BYTES) {
    return response({ error: "strict_bug_webhook_too_large" }, { status: 413 });
  }
  const body = await request.text();
  if (new TextEncoder().encode(body).byteLength > STRICT_BUG_GITHUB_WEBHOOK_MAX_BYTES) {
    return response({ error: "strict_bug_webhook_too_large" }, { status: 413 });
  }
  if (
    !(await verifyGitHubWebhookSignature({
      body,
      headers: request.headers,
      secret,
    }))
  ) {
    return response({ error: "strict_bug_webhook_unauthorized" }, { status: 401 });
  }
  if (request.headers.get("x-github-event")?.trim() !== "issues") {
    return invalid("Only GitHub issues events are accepted.");
  }
  const deliveryId = request.headers.get("x-github-delivery")?.trim() ?? "";
  if (!/^[A-Za-z0-9][A-Za-z0-9-]{0,119}$/u.test(deliveryId)) {
    return invalid("The GitHub delivery identity is missing or invalid.");
  }
  const payload = parsePayload(parseJsonRecord(body));
  if (payload === undefined) {
    return invalid("Only opened issues in the admitted repositories are accepted.");
  }
  const candidateRef = `strict-bug-candidate:${payload.repository}:${payload.issueNumber}`;
  if (commandFor(payload, deliveryId, 0) === undefined) {
    return invalid("The issue does not contain the complete strict bug form.");
  }
  try {
    const current = await dependencies.gateway.read(candidateRef);
    const existing = current.ledger.candidates[0];
    if (sameDelivery(existing, deliveryId)) {
      return response(
        { candidateRef, idempotent: true, disposition: existing?.disposition },
        { status: 200 },
      );
    }
    if (existing !== undefined) {
      return response(
        { error: "strict_bug_candidate_source_conflict", candidateRef },
        { status: 409 },
      );
    }
    const command = commandFor(payload, deliveryId, current.ledger.revision);
    if (command === undefined) return invalid("The strict bug command is invalid.");
    const result = await dependencies.gateway.execute(command);
    return response(
      {
        candidateRef,
        idempotent: false,
        disposition: "pending",
        intentRef: result.receipt.intentRef,
      },
      { status: 202 },
    );
  } catch {
    return response({ error: "strict_bug_candidate_gateway_unavailable" }, { status: 503 });
  }
};

export const makeStrictBugCandidateHttpGateway = (
  input: Readonly<{
    endpoint?: string | undefined;
    token?: string | undefined;
    fetchImpl?: typeof fetch | undefined;
  }>,
): StrictBugCandidateGateway | undefined => {
  const endpointText = input.endpoint?.trim();
  const token = input.token?.trim();
  if (endpointText === undefined || endpointText === "" || token === undefined || token === "") {
    return undefined;
  }
  let endpoint: URL;
  try {
    endpoint = new URL(endpointText);
  } catch {
    return undefined;
  }
  if (
    endpoint.protocol !== "https:" ||
    endpoint.username !== "" ||
    endpoint.password !== "" ||
    endpoint.hash !== ""
  ) {
    return undefined;
  }
  const fetchImpl = input.fetchImpl ?? globalThis.fetch;
  const call = async (frame: unknown): Promise<unknown> => {
    const result = await fetchImpl(endpoint, {
      method: "POST",
      headers: {
        accept: "application/json",
        authorization: `Bearer ${token}`,
        "content-type": "application/json",
      },
      body: JSON.stringify(frame),
    });
    if (!result.ok) throw new Error("strict bug candidate gateway refused");
    return await result.json();
  };
  return {
    read: async (candidateRef) => {
      const value = (await call({
        method: "strict_bug.candidate.read",
        id: `strict-bug-read:${candidateRef}`,
        version: "omega-effectd.v2",
        params: { candidateRef },
      })) as { result?: unknown };
      return decodeStrictBugCandidateReadResult(value.result);
    },
    execute: async (request) => {
      const value = (await call({
        method: "strict_bug.candidate.execute",
        id: `strict-bug-execute:${request.command.candidateRef}`,
        version: "omega-effectd.v2",
        params: request,
      })) as { result?: unknown };
      return decodeStrictBugCandidateExecuteResult(value.result);
    },
  };
};
