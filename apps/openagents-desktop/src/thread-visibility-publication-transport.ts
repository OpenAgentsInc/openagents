import {
  decodeThreadDisclosureReceipt,
  type ThreadDisclosureReceipt,
} from "@openagentsinc/agent-runtime-schema";
import { createHash } from "node:crypto";
import { Effect, Schedule, Schema as S } from "effect";

import {
  DesktopThreadVisibilityAuthorizationRequestSchemaLiteral,
  evaluateDesktopThreadVisibilityAudience,
  type DesktopThreadVisibilityAuthorizationDecision,
} from "./thread-visibility-audience-authorization.ts";

export const DesktopThreadVisibilityPublicationRequestSchemaLiteral =
  "openagents.desktop_thread_visibility_publication_request.v1" as const;

export const DesktopThreadVisibilityPublicationPath = "/api/share" as const;

type OwnerAuthorization = Extract<
  DesktopThreadVisibilityAuthorizationDecision,
  { status: "authorized" }
>;

type PublicationSource = Readonly<{
  kind: "agent-run" | "team-thread";
  id: string;
}>;

export type DesktopThreadVisibilityPublicationDependencies = Readonly<{
  baseUrl: string;
  accessToken: () => string | null;
  fetch?: typeof fetch;
}>;

export type DesktopThreadVisibilityPublicationResult =
  | Readonly<{
      status: "published";
      shareRef: string;
      url: string;
      receiptRef: string;
      threadRef: string;
      visibilityVersion: number;
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

const exactOwnerAuthorization = (value: unknown): value is OwnerAuthorization =>
  ownKeysAre(value, ["status", "basis", "receiptRef", "threadRef", "visibilityVersion"]) &&
  field(value, "status") === "authorized" &&
  field(value, "basis") === "owner" &&
  typeof field(value, "receiptRef") === "string" &&
  typeof field(value, "threadRef") === "string" &&
  typeof field(value, "visibilityVersion") === "number";

const exactSource = (value: unknown): value is PublicationSource => {
  if (!ownKeysAre(value, ["kind", "id"])) return false;
  const kind = field(value, "kind");
  const id = field(value, "id");
  if ((kind !== "agent-run" && kind !== "team-thread") || typeof id !== "string") return false;
  try {
    S.decodeUnknownSync(Ref)(id);
    return true;
  } catch {
    return false;
  }
};

type DecodedRequest = Readonly<{
  receipt: ThreadDisclosureReceipt;
  authorization: OwnerAuthorization;
  source: PublicationSource;
}>;

const decodeRequest = (input: unknown): DecodedRequest | null => {
  if (
    !ownKeysAre(input, ["schema", "receipt", "authorization", "source"]) ||
    field(input, "schema") !== DesktopThreadVisibilityPublicationRequestSchemaLiteral
  ) {
    return null;
  }
  const rawReceipt = field(input, "receipt");
  const authorization = field(input, "authorization");
  const source = field(input, "source");
  if (!exactOwnerAuthorization(authorization) || !exactSource(source)) return null;

  const validation = evaluateDesktopThreadVisibilityAudience({
    schema: DesktopThreadVisibilityAuthorizationRequestSchemaLiteral,
    actorRef: "publication.owner",
    ownerRef: "publication.owner",
    receipt: rawReceipt,
    authorities: [],
  });
  if (validation.status !== "authorized" || validation.basis !== "owner") return null;

  let receipt: ThreadDisclosureReceipt;
  try {
    receipt = decodeThreadDisclosureReceipt(rawReceipt);
  } catch {
    return null;
  }
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
  return { receipt, authorization, source };
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
): typeof ShareResponse.Type | null => {
  if (text.length === 0 || text.length > 4_096) return null;
  try {
    const parsed = JSON.parse(text) as unknown;
    if (!ownKeysAre(parsed, ["id", "url", "audienceLabel", "status"])) return null;
    const decoded = S.decodeUnknownSync(ShareResponse)(parsed);
    const url = new URL(decoded.url);
    return url.origin === configuredOrigin && url.username === "" && url.password === ""
      ? decoded
      : null;
  } catch {
    return null;
  }
};

const publicationIdempotencyKey = (receipt: ThreadDisclosureReceipt): string =>
  `desktop-public-share.${createHash("sha256")
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

const publishOnce = Effect.fn("DesktopThreadVisibilityPublicationTransport.publishOnce")(
  function* (
    dependencies: DesktopThreadVisibilityPublicationDependencies,
    origin: URL,
    token: string,
    idempotencyKey: string,
    body: string,
  ) {
    const response = yield* Effect.tryPromise({
      try: (signal) =>
        (dependencies.fetch ?? fetch)(
          new URL(DesktopThreadVisibilityPublicationPath, origin),
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
    const published = decodePublishedResponse(text, origin.origin);
    return published === null
      ? yield* new PublicationOutcomeAmbiguous()
      : ({ status: "published", share: published } as const);
  },
);

/**
 * Publish an already-applied public visibility through the existing server
 * share builder. Only source identity crosses the boundary; the server loads
 * and redacts its authoritative source.
 */
export const publishDesktopThreadPublicVisibility = Effect.fn(
  "DesktopThreadVisibilityPublicationTransport.publish",
)(function* (dependencies: DesktopThreadVisibilityPublicationDependencies, input: unknown) {
  const request = decodeRequest(input);
  if (request === null) return { status: "rejected", reason: "invalid_request" } as const;
  if (
    request.receipt.result.status !== "visibility_applied" ||
    request.receipt.result.target.audience.kind !== "internet_readable"
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
    audience: { _tag: "Public" },
  });
  const attempt: PublicationAttemptResult | null = yield* publishOnce(
    dependencies,
    origin,
    token,
    idempotencyKey,
    body,
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
  } as const;
});
