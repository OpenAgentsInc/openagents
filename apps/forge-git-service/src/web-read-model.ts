import { Schema } from "effect";

export const ForgeWebReadSchemaVersion = "openagents.forge.repository_web_read.v1" as const;

const NonEmpty = Schema.String.pipe(Schema.check(Schema.isMinLength(1)));

export const ForgeWebReadRequest = Schema.Struct({
  base: Schema.optionalKey(NonEmpty),
  commit: Schema.optionalKey(NonEmpty),
  maxDiffBytes: Schema.Number,
  maxImageBytes: Schema.Number,
  maxTextBytes: Schema.Number,
  owner: NonEmpty,
  path: Schema.optionalKey(Schema.String),
  ref: Schema.optionalKey(NonEmpty),
  repo: NonEmpty,
  view: Schema.Literals(["code", "commits", "commit", "diff"]),
});
export interface ForgeWebReadRequest extends Schema.Schema.Type<typeof ForgeWebReadRequest> {}

export const ForgeWebReadPolicyDecision = Schema.Struct({
  access: Schema.Struct({
    canWrite: Schema.Boolean,
    mode: Schema.Literals(["member", "public_web_read"]),
  }),
  repository: Schema.Struct({
    maintainers: Schema.Array(
      Schema.Struct({
        displayName: NonEmpty,
        nostrPubkey: Schema.optionalKey(NonEmpty),
      }),
    ),
    publicWebRead: Schema.Boolean,
  }),
});
export interface ForgeWebReadPolicyDecision extends Schema.Schema.Type<
  typeof ForgeWebReadPolicyDecision
> {}

const ForgeRef = Schema.Struct({
  name: NonEmpty,
  objectId: NonEmpty,
  kind: Schema.Literals(["branch", "tag"]),
  isDefault: Schema.Boolean,
});

const ForgeCommitSummary = Schema.Struct({
  objectId: NonEmpty,
  shortId: NonEmpty,
  subject: NonEmpty,
  authorName: NonEmpty,
  authoredAt: NonEmpty,
  parentIds: Schema.Array(NonEmpty),
  additions: Schema.Number,
  deletions: Schema.Number,
  changedFiles: Schema.Number,
});

const ForgeTreeEntry = Schema.Struct({
  name: NonEmpty,
  path: NonEmpty,
  kind: Schema.Literals(["directory", "file", "symlink", "submodule"]),
  size: Schema.Number,
  objectId: NonEmpty,
});

const ForgeTextFile = Schema.TaggedStruct("text", {
  path: NonEmpty,
  objectId: NonEmpty,
  byteSize: Schema.Number,
  language: Schema.optionalKey(NonEmpty),
  content: Schema.String,
});

const ForgeMarkdownFile = Schema.TaggedStruct("markdown", {
  path: NonEmpty,
  objectId: NonEmpty,
  byteSize: Schema.Number,
  content: Schema.String,
  assets: Schema.optionalKey(
    Schema.Array(
      Schema.Struct({
        path: NonEmpty,
        sourceUrl: NonEmpty,
        mimeType: Schema.Literals([
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

const ForgeImageFile = Schema.TaggedStruct("image", {
  path: NonEmpty,
  objectId: NonEmpty,
  byteSize: Schema.Number,
  mimeType: Schema.Literals([
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/svg+xml",
  ]),
  sourceUrl: NonEmpty,
});

const ForgeFileRefusal = Schema.TaggedStruct("refusal", {
  path: NonEmpty,
  objectId: NonEmpty,
  byteSize: Schema.Number,
  reason: Schema.Literals(["binary", "too_large", "unsupported_image"]),
});

export const ForgeWebReadFile = Schema.TaggedUnion({
  text: ForgeTextFile.fields,
  markdown: ForgeMarkdownFile.fields,
  image: ForgeImageFile.fields,
  refusal: ForgeFileRefusal.fields,
});
export type ForgeWebReadFile = typeof ForgeWebReadFile.Type;

export const ForgeWebReadProjection = Schema.Struct({
  schema: Schema.Literal(ForgeWebReadSchemaVersion),
  servedAt: NonEmpty,
  repository: Schema.Struct({
    repositoryRef: NonEmpty,
    owner: NonEmpty,
    name: NonEmpty,
    description: Schema.String,
    nip34Coordinate: NonEmpty,
    authorityMode: Schema.Literals(["openagents_git_authoritative", "github_authoritative"]),
    defaultBranch: NonEmpty,
    canonicalCloneUrl: NonEmpty,
    publicWebRead: Schema.Boolean,
    projectionFreshness: NonEmpty,
    maintainers: Schema.Array(
      Schema.Struct({
        displayName: NonEmpty,
        nostrPubkey: Schema.optionalKey(NonEmpty),
      }),
    ),
  }),
  access: ForgeWebReadPolicyDecision.fields.access,
  selectedRef: NonEmpty,
  selectedPath: Schema.String,
  refs: Schema.Array(ForgeRef),
  commits: Schema.Array(ForgeCommitSummary),
  tree: Schema.Array(ForgeTreeEntry),
  readme: Schema.NullOr(ForgeMarkdownFile),
  file: Schema.NullOr(ForgeWebReadFile),
  commit: Schema.NullOr(
    Schema.Struct({
      ...ForgeCommitSummary.fields,
      body: Schema.String,
    }),
  ),
  diff: Schema.NullOr(
    Schema.Struct({
      baseObjectId: NonEmpty,
      headObjectId: NonEmpty,
      unified: Schema.String,
      truncated: Schema.Boolean,
    }),
  ),
});
export interface ForgeWebReadProjection extends Schema.Schema.Type<typeof ForgeWebReadProjection> {}
