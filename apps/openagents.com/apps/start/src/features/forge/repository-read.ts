import { Context, Effect, Layer, Schema as S } from "effect";

export const FORGE_READ_SCHEMA = "openagents.forge.repository_web_read.v1" as const;
export const FORGE_MAX_TEXT_BYTES = 512_000;
export const FORGE_MAX_IMAGE_BYTES = 2_000_000;
export const FORGE_MAX_DIFF_BYTES = 1_000_000;

const NonEmpty = S.String.check(S.isMinLength(1));

export const ForgeRepositoryReadRequest = S.Struct({
  owner: NonEmpty,
  repo: NonEmpty,
  ref: S.optionalKey(NonEmpty),
  path: S.optionalKey(S.String),
  view: S.Literals(["code", "commits", "commit", "diff"]),
  commit: S.optionalKey(NonEmpty),
  base: S.optionalKey(NonEmpty),
});
export interface ForgeRepositoryReadRequest extends S.Schema.Type<
  typeof ForgeRepositoryReadRequest
> {}

const ForgeMaintainer = S.Struct({
  displayName: NonEmpty,
  nostrPubkey: S.optionalKey(NonEmpty),
});

const ForgeRef = S.Struct({
  name: NonEmpty,
  objectId: NonEmpty,
  kind: S.Literals(["branch", "tag"]),
  isDefault: S.Boolean,
});

const ForgeCommitSummary = S.Struct({
  objectId: NonEmpty,
  shortId: NonEmpty,
  subject: NonEmpty,
  authorName: NonEmpty,
  authoredAt: NonEmpty,
  parentIds: S.Array(NonEmpty),
  additions: S.Number,
  deletions: S.Number,
  changedFiles: S.Number,
});

const ForgeTreeEntry = S.Struct({
  name: NonEmpty,
  path: NonEmpty,
  kind: S.Literals(["directory", "file", "symlink", "submodule"]),
  size: S.Number,
  objectId: NonEmpty,
});

const ForgeTextFile = S.TaggedStruct("text", {
  path: NonEmpty,
  objectId: NonEmpty,
  byteSize: S.Number,
  language: S.optionalKey(NonEmpty),
  content: S.String,
  highlightedLines: S.optionalKey(
    S.Array(
      S.Array(
        S.Struct({
          content: S.String,
          color: S.optionalKey(S.String),
          fontStyle: S.optionalKey(S.Number),
        }),
      ),
    ),
  ),
});

const ForgeMarkdownFile = S.TaggedStruct("markdown", {
  path: NonEmpty,
  objectId: NonEmpty,
  byteSize: S.Number,
  content: S.String,
  assets: S.optionalKey(
    S.Array(
      S.Struct({
        path: NonEmpty,
        sourceUrl: NonEmpty,
        mimeType: S.Literals([
          "image/png",
          "image/jpeg",
          "image/gif",
          "image/webp",
          "image/svg+xml",
        ]),
      }),
    ),
  ),
});

const ForgeImageFile = S.TaggedStruct("image", {
  path: NonEmpty,
  objectId: NonEmpty,
  byteSize: S.Number,
  mimeType: S.Literals(["image/png", "image/jpeg", "image/gif", "image/webp", "image/svg+xml"]),
  sourceUrl: NonEmpty,
});

const ForgeRefusal = S.TaggedStruct("refusal", {
  path: NonEmpty,
  objectId: NonEmpty,
  byteSize: S.Number,
  reason: S.Literals(["binary", "too_large", "unsupported_image"]),
});

export const ForgeFileContent = S.TaggedUnion({
  text: ForgeTextFile.fields,
  markdown: ForgeMarkdownFile.fields,
  image: ForgeImageFile.fields,
  refusal: ForgeRefusal.fields,
});
export type ForgeFileContent = S.Schema.Type<typeof ForgeFileContent>;

const ForgeCommitDetail = S.Struct({
  ...ForgeCommitSummary.fields,
  body: S.String,
});

export const ForgeRepositoryProjection = S.Struct({
  schema: S.Literal(FORGE_READ_SCHEMA),
  servedAt: NonEmpty,
  repository: S.Struct({
    repositoryRef: NonEmpty,
    owner: NonEmpty,
    name: NonEmpty,
    description: S.String,
    nip34Coordinate: NonEmpty,
    authorityMode: S.Literals(["openagents_git_authoritative", "github_authoritative"]),
    defaultBranch: NonEmpty,
    canonicalCloneUrl: NonEmpty,
    publicWebRead: S.Boolean,
    projectionFreshness: NonEmpty,
    maintainers: S.Array(ForgeMaintainer),
  }),
  access: S.Struct({
    mode: S.Literals(["member", "public_web_read"]),
    canWrite: S.Boolean,
  }),
  selectedRef: NonEmpty,
  selectedPath: S.String,
  refs: S.Array(ForgeRef),
  commits: S.Array(ForgeCommitSummary),
  tree: S.Array(ForgeTreeEntry),
  readme: S.NullOr(ForgeMarkdownFile),
  file: S.NullOr(ForgeFileContent),
  commit: S.NullOr(ForgeCommitDetail),
  diff: S.NullOr(
    S.Struct({
      baseObjectId: NonEmpty,
      headObjectId: NonEmpty,
      unified: S.String,
      truncated: S.Boolean,
    }),
  ),
});
export interface ForgeRepositoryProjection extends S.Schema.Type<
  typeof ForgeRepositoryProjection
> {}

export const ForgeRepositoryReadFailure = S.TaggedUnion({
  not_found: {
    detail: NonEmpty,
  },
  authentication_required: {
    detail: NonEmpty,
  },
  unavailable: {
    detail: NonEmpty,
    retryable: S.Boolean,
  },
  malformed_response: {
    detail: NonEmpty,
  },
});
export type ForgeRepositoryReadFailure = S.Schema.Type<typeof ForgeRepositoryReadFailure>;

export const ForgeRepositoryReadResult = S.TaggedUnion({
  loaded: {
    projection: ForgeRepositoryProjection,
  },
  failed: {
    failure: ForgeRepositoryReadFailure,
  },
});
export type ForgeRepositoryReadResult = S.Schema.Type<typeof ForgeRepositoryReadResult>;

export class ForgeReadTransportError extends S.TaggedErrorClass<ForgeReadTransportError>()(
  "ForgeReadTransportError",
  {
    operation: S.String,
    cause: S.Defect(),
  },
) {}

export interface ForgeRepositoryReaderInterface {
  readonly read: (
    request: ForgeRepositoryReadRequest,
    authorizationCookie: string | undefined,
  ) => Effect.Effect<ForgeRepositoryReadResult, ForgeReadTransportError>;
}

export class ForgeRepositoryReader extends Context.Service<
  ForgeRepositoryReader,
  ForgeRepositoryReaderInterface
>()("@openagents.com/ForgeRepositoryReader") {}

export const makeForgeRepositoryReaderTestLayer = (read: ForgeRepositoryReaderInterface["read"]) =>
  Layer.succeed(ForgeRepositoryReader, ForgeRepositoryReader.of({ read }));

const publicProjection = (projection: ForgeRepositoryProjection): ForgeRepositoryProjection => ({
  ...projection,
  access: {
    mode: "public_web_read",
    canWrite: false,
  },
  repository: {
    ...projection.repository,
    maintainers: [],
  },
});

/**
 * Apply the final public boundary after the owned service makes its access
 * decision. This keeps member identity and write capability out of anonymous
 * serializable output even when an upstream adapter regresses.
 */
export const enforceForgePublicWebRead = (
  result: ForgeRepositoryReadResult,
  hadAuthorizationCookie: boolean,
): ForgeRepositoryReadResult => {
  if (result._tag === "failed") return result;
  const { projection } = result;

  if (hadAuthorizationCookie) return result;
  if (!projection.repository.publicWebRead) {
    return ForgeRepositoryReadResult.cases.failed.make({
      failure: ForgeRepositoryReadFailure.cases.authentication_required.make({
        detail: "This repository requires a Forge invitation.",
      }),
    });
  }

  return ForgeRepositoryReadResult.cases.loaded.make({
    projection: publicProjection(projection),
  });
};
