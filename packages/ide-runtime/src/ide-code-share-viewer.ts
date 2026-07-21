import { Context, Effect, Layer, Result, Schema as S } from "effect";

import type { CodeShareDigestPort } from "./ide-code-share-bundle.js";
import { CodeShareBundle, verifyCodeShareBundleIntegrity } from "./ide-code-share-bundle.js";
import { IdeProjectionTimestamp } from "./ide-review-projection.js";
import { CodeShareDigestService } from "./ide-code-share-builder.js";

export const CodeShareViewUnavailableReason = S.Literals([
  "invalid_bundle",
  "tampered",
  "deleted",
  "revoked",
  "expired",
]);
export type CodeShareViewUnavailableReason = typeof CodeShareViewUnavailableReason.Type;

/**
 * The outcome of resolving a public link. It is a read-only decision. There is
 * no accepted mutation, agent, provider, terminal, repository, or attachment
 * case: a public viewer only renders a frozen snapshot or learns it is gone.
 */
export const CodeShareViewOutcome = S.TaggedUnion({
  Rendered: {
    bundle: CodeShareBundle,
  },
  Unavailable: {
    reason: CodeShareViewUnavailableReason,
  },
}).annotate({ identifier: "CodeShareViewOutcome" });
export type CodeShareViewOutcome = typeof CodeShareViewOutcome.Type;

/**
 * A public viewer command outcome. It has exactly one case: rejection. A public
 * share is immutable and read-only, so no write, approve, run, or attachment
 * command can ever be accepted. This structurally encodes the no-write-back law.
 */
export const CodeShareCommandOutcome = S.TaggedUnion({
  Rejected: {
    reason: S.Literal("public_share_read_only"),
  },
}).annotate({ identifier: "CodeShareCommandOutcome" });
export type CodeShareCommandOutcome = typeof CodeShareCommandOutcome.Type;

const decodeBundle = S.decodeUnknownResult(CodeShareBundle);
const strictOptions = { onExcessProperty: "error" as const };

/**
 * Resolve an immutable public bundle at a point in time. It verifies integrity
 * and forbidden material, then gates deletion, revocation, and expiry before it
 * renders. It never mutates the bundle and grants no write authority.
 */
export const resolveCodeShareBundle = (
  payload: unknown,
  asOf: IdeProjectionTimestamp,
  revoked: boolean,
  port: CodeShareDigestPort,
): CodeShareViewOutcome => {
  const decoded = decodeBundle(payload, strictOptions);
  if (Result.isFailure(decoded)) {
    return CodeShareViewOutcome.cases.Unavailable.make({ reason: "invalid_bundle" });
  }
  const bundle = decoded.success;
  const asOfMillis = Date.parse(asOf);
  if (!Number.isFinite(asOfMillis)) {
    return CodeShareViewOutcome.cases.Unavailable.make({ reason: "invalid_bundle" });
  }
  if (!verifyCodeShareBundleIntegrity(bundle, port)) {
    return CodeShareViewOutcome.cases.Unavailable.make({ reason: "tampered" });
  }
  if (asOfMillis >= Date.parse(bundle.retention.deleteAfter)) {
    return CodeShareViewOutcome.cases.Unavailable.make({ reason: "deleted" });
  }
  if (revoked) {
    return CodeShareViewOutcome.cases.Unavailable.make({ reason: "revoked" });
  }
  if (asOfMillis >= Date.parse(bundle.expiresAt)) {
    return CodeShareViewOutcome.cases.Unavailable.make({ reason: "expired" });
  }
  return CodeShareViewOutcome.cases.Rendered.make({ bundle });
};

/** A public share never accepts a command. The outcome is always a read-only rejection. */
export const rejectCodeShareCommand = (): CodeShareCommandOutcome =>
  CodeShareCommandOutcome.cases.Rejected.make({ reason: "public_share_read_only" });

export interface CodeShareViewerInterface {
  readonly resolve: (
    payload: unknown,
    asOf: IdeProjectionTimestamp,
    revoked: boolean,
  ) => Effect.Effect<CodeShareViewOutcome>;
  readonly command: (payload: unknown) => Effect.Effect<CodeShareCommandOutcome>;
}

export class CodeShareViewer extends Context.Service<CodeShareViewer, CodeShareViewerInterface>()(
  "ide-runtime.CodeShareViewer",
) {}

export const CodeShareViewerLayer = Layer.effect(
  CodeShareViewer,
  Effect.gen(function* () {
    const port = yield* CodeShareDigestService;
    const resolve = Effect.fn("CodeShareViewer.resolve")(
      (payload: unknown, asOf: IdeProjectionTimestamp, revoked: boolean) =>
        Effect.succeed(resolveCodeShareBundle(payload, asOf, revoked, port)),
    );
    const command = Effect.fn("CodeShareViewer.command")((_payload: unknown) =>
      Effect.succeed(rejectCodeShareCommand()),
    );
    return CodeShareViewer.of({ resolve, command });
  }),
);
