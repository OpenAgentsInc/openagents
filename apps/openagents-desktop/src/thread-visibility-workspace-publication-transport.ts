import {
  decodeThreadDisclosureReceipt,
  type ThreadDisclosureReceipt,
} from "@openagentsinc/agent-runtime-schema";
import { createHash } from "node:crypto";
import { Effect, Schedule, Schema as S } from "effect";

import type { DesktopThreadVisibilityAuthorizationDecision } from "./thread-visibility-audience-authorization.ts";

export const DesktopThreadVisibilityWorkspacePublicationRequestSchemaLiteral =
  "openagents.desktop_thread_visibility_workspace_publication_request.v1" as const;

export const DesktopThreadVisibilityWorkspacePublicationPath = "/api/share" as const;

type AuthorizedDecision = Extract<
  DesktopThreadVisibilityAuthorizationDecision,
  { status: "authorized" }
>;

type WorkspaceAuthorization = Omit<AuthorizedDecision, "basis"> &
  Readonly<{ basis: "owner" | "workspace_member" }>;

type WorkspacePublicationSource =
  | Readonly<{ kind: "agent-run"; id: string }>
  | Readonly<{ kind: "team-thread"; id: string; teamId: string }>;

export type DesktopThreadVisibilityWorkspacePublicationDependencies = Readonly<{
  baseUrl: string;
  accessToken: () => string | null;
  fetch?: typeof fetch;
}>;

export type DesktopThreadVisibilityWorkspacePublicationResult =
  | Readonly<{
      status: "published";
      shareRef: string;
      url: string;
      receiptRef: string;
      threadRef: string;
      visibilityVersion: number;
      workspaceRef: string;
    }>
  | Readonly<{
      status: "rejected";
      reason:
        | "invalid_request"
        | "unsupported_visibility"
        | "authentication_required"
        | "publication_forbidden"
        | "publication_rejected"
        | "publication_outcome_unknown";
    }>;

const Ref = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
);

const TeamName = S.String.check(S.isMinLength(1), S.isMaxLength(120));

const ShareResponse = S.Struct({
  id: Ref,
  url: S.String.check(S.isMinLength(1), S.isMaxLength(2_048)),
  audienceLabel: S.String.check(S.isMinLength(1), S.isMaxLength(256)),
  status: S.Literal("active"),
});

class PublicationOutcomeAmbiguous extends S.TaggedErrorClass<PublicationOutcomeAmbiguous>()(
  "PublicationOutcomeAmbiguous",
  {},
) {}

const field = (value: unknown, key: string): unknown =>
  typeof value === "object" && value !== null ? Reflect.get(value, key) : undefined;

const ownKeysAre = (value: unknown, allowed: ReadonlyArray<string>): boolean => {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const actual = Object.keys(value).sort();
  const expected = [...allowed].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
};

const exactAudience = (value: unknown): boolean =>
  field(value, "kind") === "workspace_members" && ownKeysAre(value, ["kind", "workspaceRef"]);

const exactAdministratorAccess = (value: unknown): boolean => {
  const kind = field(value, "kind");
  return kind === "none"
    ? ownKeysAre(value, ["kind"])
    : kind === "workspace_admins"
      ? ownKeysAre(value, ["kind", "workspaceRef"])
      : false;
};

const exactVisibilityReceipt = (value: unknown): boolean =>
  ownKeysAre(value, [
    "schema",
    "receiptRef",
    "intentRef",
    "idempotencyKey",
    "threadRef",
    "observedAt",
    "kind",
    "result",
  ]) &&
  ownKeysAre(field(value, "result"), ["status", "visibilityVersion", "target"]) &&
  ownKeysAre(field(field(value, "result"), "target"), ["audience", "administratorAccess"]) &&
  exactAudience(field(field(field(value, "result"), "target"), "audience")) &&
  exactAdministratorAccess(field(field(field(value, "result"), "target"), "administratorAccess"));

const decodeRef = (value: unknown): string | null => {
  try {
    return S.decodeUnknownSync(Ref)(value);
  } catch {
    return null;
  }
};

const exactWorkspaceAuthorization = (value: unknown): value is WorkspaceAuthorization =>
  ownKeysAre(value, ["status", "basis", "receiptRef", "threadRef", "visibilityVersion"]) &&
  field(value, "status") === "authorized" &&
  (field(value, "basis") === "owner" || field(value, "basis") === "workspace_member") &&
  decodeRef(field(value, "receiptRef")) !== null &&
  decodeRef(field(value, "threadRef")) !== null &&
  Number.isSafeInteger(field(value, "visibilityVersion")) &&
  Number(field(value, "visibilityVersion")) >= 1;

const exactSource = (value: unknown): value is WorkspacePublicationSource => {
  const kind = field(value, "kind");
  if (kind === "agent-run") {
    return ownKeysAre(value, ["kind", "id"]) && decodeRef(field(value, "id")) !== null;
  }
  if (kind === "team-thread") {
    return (
      ownKeysAre(value, ["kind", "id", "teamId"]) &&
      decodeRef(field(value, "id")) !== null &&
      decodeRef(field(value, "teamId")) !== null
    );
  }
  return false;
};

type DecodedRequest = Readonly<{
  receipt: ThreadDisclosureReceipt;
  authorization: WorkspaceAuthorization;
  source: WorkspacePublicationSource;
  teamId: string;
  teamName: string;
  workspaceRef: string;
}>;

const teamIdFromWorkspaceRef = (workspaceRef: string): string | null => {
  if (!workspaceRef.startsWith("scope.team.")) return null;
  const teamId = workspaceRef.slice("scope.team.".length);
  return decodeRef(teamId);
};

const decodeRequest = (input: unknown): DecodedRequest | null => {
  if (
    !ownKeysAre(input, ["schema", "receipt", "authorization", "source", "teamName"]) ||
    field(input, "schema") !== DesktopThreadVisibilityWorkspacePublicationRequestSchemaLiteral
  ) {
    return null;
  }
  const rawReceipt = field(input, "receipt");
  const authorization = field(input, "authorization");
  const source = field(input, "source");
  if (
    !exactVisibilityReceipt(rawReceipt) ||
    !exactWorkspaceAuthorization(authorization) ||
    !exactSource(source)
  ) {
    return null;
  }

  let receipt: ThreadDisclosureReceipt;
  let teamName: string;
  try {
    receipt = decodeThreadDisclosureReceipt(rawReceipt);
    teamName = S.decodeUnknownSync(TeamName)(field(input, "teamName"));
  } catch {
    return null;
  }
  if (teamName.trim() !== teamName) return null;
  if (
    receipt.kind !== "thread.visibility.set" ||
    receipt.result.status !== "visibility_applied" ||
    authorization.receiptRef !== receipt.receiptRef ||
    authorization.threadRef !== receipt.threadRef ||
    authorization.visibilityVersion !== receipt.result.visibilityVersion ||
    source.id !== receipt.threadRef
  ) {
    return null;
  }
  if (receipt.result.target.audience.kind !== "workspace_members") return null;
  const workspaceRef = receipt.result.target.audience.workspaceRef;
  const teamId = teamIdFromWorkspaceRef(workspaceRef);
  if (teamId === null || (source.kind === "team-thread" && source.teamId !== teamId)) return null;
  return { receipt, authorization, source, teamId, teamName, workspaceRef };
};

const serviceOrigin = (baseUrl: string): URL | null => {
  try {
    const url = new URL(baseUrl);
    if (
      (url.protocol !== "https:" && url.protocol !== "http:") ||
      url.username !== "" ||
      url.password !== ""
    ) {
      return null;
    }
    return url;
  } catch {
    return null;
  }
};

const decodePublishedResponse = (
  text: string,
  configuredOrigin: string,
  expectedAudienceLabel: string,
): typeof ShareResponse.Type | null => {
  if (text.length === 0 || text.length > 4_096) return null;
  try {
    const parsed = JSON.parse(text) as unknown;
    if (!ownKeysAre(parsed, ["id", "url", "audienceLabel", "status"])) return null;
    const decoded = S.decodeUnknownSync(ShareResponse)(parsed);
    const url = new URL(decoded.url);
    return url.origin === configuredOrigin &&
      url.username === "" &&
      url.password === "" &&
      decoded.audienceLabel === expectedAudienceLabel
      ? decoded
      : null;
  } catch {
    return null;
  }
};

const publicationIdempotencyKey = (receipt: ThreadDisclosureReceipt): string =>
  `desktop-workspace-share.${createHash("sha256")
    .update(
      JSON.stringify([
        receipt.schema,
        receipt.receiptRef,
        receipt.intentRef,
        receipt.idempotencyKey,
        receipt.threadRef,
        receipt.kind,
        receipt.result,
      ]),
    )
    .digest("hex")}`;

type PublicationAttemptResult =
  | Readonly<{ status: "published"; share: typeof ShareResponse.Type }>
  | Readonly<{
      status: "rejected";
      reason:
        | "authentication_required"
        | "publication_forbidden"
        | "publication_rejected";
    }>;

const publishOnce = Effect.fn(
  "DesktopThreadVisibilityWorkspacePublicationTransport.publishOnce",
)(function* (
  dependencies: DesktopThreadVisibilityWorkspacePublicationDependencies,
  origin: URL,
  token: string,
  idempotencyKey: string,
  body: string,
  expectedAudienceLabel: string,
) {
  const response = yield* Effect.tryPromise({
    try: (signal) =>
      (dependencies.fetch ?? fetch)(
        new URL(DesktopThreadVisibilityWorkspacePublicationPath, origin),
        {
          method: "POST",
          headers: {
            authorization: `Bearer ${token}`,
            "content-type": "application/json",
            "Idempotency-Key": idempotencyKey,
          },
          body,
          signal,
        },
      ),
    catch: () => new PublicationOutcomeAmbiguous(),
  });

  if (response.status === 401) {
    return { status: "rejected", reason: "authentication_required" } as const;
  }
  if (response.status === 403) {
    return { status: "rejected", reason: "publication_forbidden" } as const;
  }
  if (
    response.status === 408 ||
    response.status === 425 ||
    response.status === 429 ||
    response.status >= 500
  ) {
    return yield* new PublicationOutcomeAmbiguous();
  }
  if (!response.ok) {
    return { status: "rejected", reason: "publication_rejected" } as const;
  }

  const expectedReplayHeader =
    response.status === 201 ? "false" : response.status === 200 ? "true" : null;
  if (
    expectedReplayHeader === null ||
    response.headers.get("Idempotency-Replayed") !== expectedReplayHeader
  ) {
    return yield* new PublicationOutcomeAmbiguous();
  }

  const text = yield* Effect.tryPromise({
    try: () => response.text(),
    catch: () => new PublicationOutcomeAmbiguous(),
  });
  const published = decodePublishedResponse(text, origin.origin, expectedAudienceLabel);
  return published === null
    ? yield* new PublicationOutcomeAmbiguous()
    : ({ status: "published", share: published } as const);
});

/**
 * Publish an already-applied workspace visibility through the existing server
 * share builder. The server remains authoritative for source access and live
 * team membership; only source and audience refs cross this boundary.
 */
export const publishDesktopThreadWorkspaceVisibility = Effect.fn(
  "DesktopThreadVisibilityWorkspacePublicationTransport.publish",
)(function* (
  dependencies: DesktopThreadVisibilityWorkspacePublicationDependencies,
  input: unknown,
) {
  const request = decodeRequest(input);
  if (request === null) return { status: "rejected", reason: "invalid_request" } as const;
  if (
    request.receipt.kind !== "thread.visibility.set" ||
    request.receipt.result.status !== "visibility_applied" ||
    request.receipt.result.target.audience.kind !== "workspace_members"
  ) {
    return { status: "rejected", reason: "unsupported_visibility" } as const;
  }

  const origin = serviceOrigin(dependencies.baseUrl);
  if (origin === null) return { status: "rejected", reason: "invalid_request" } as const;
  const token = dependencies.accessToken()?.trim() ?? "";
  if (token === "") {
    return { status: "rejected", reason: "authentication_required" } as const;
  }

  const idempotencyKey = publicationIdempotencyKey(request.receipt);
  const body = JSON.stringify({
    source: request.source,
    audience: {
      _tag: "TeamMembers",
      teamId: request.teamId,
      teamName: request.teamName,
    },
  });
  const expectedAudienceLabel = `Shared with members of ${request.teamName}`;
  const attempt: PublicationAttemptResult | null = yield* publishOnce(
    dependencies,
    origin,
    token,
    idempotencyKey,
    body,
    expectedAudienceLabel,
  ).pipe(
    Effect.retry(Schedule.recurs(1)),
    Effect.match({ onFailure: () => null, onSuccess: (value) => value }),
  );
  if (attempt === null) {
    return { status: "rejected", reason: "publication_outcome_unknown" } as const;
  }
  if (attempt.status === "rejected") return attempt;
  return {
    status: "published",
    shareRef: attempt.share.id,
    url: attempt.share.url,
    receiptRef: request.receipt.receiptRef,
    threadRef: request.receipt.threadRef,
    visibilityVersion: request.receipt.result.visibilityVersion,
    workspaceRef: request.workspaceRef,
  } as const;
});
