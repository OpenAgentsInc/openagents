import { Schema } from "effect";

export const ForgeGitPathSegment = Schema.String.pipe(
  Schema.check(Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/)),
);

export const ForgeGitOperation = Schema.Literals(["git-upload-pack", "git-receive-pack"]);
export type ForgeGitOperation = typeof ForgeGitOperation.Type;

export const ForgeGitScope = Schema.Literals(["git:upload-pack", "git:receive-pack"]);
export type ForgeGitScope = typeof ForgeGitScope.Type;

export const ForgeGitRoute = Schema.Struct({
  kind: Schema.Literals(["advertisement", "rpc"]),
  operation: ForgeGitOperation,
  repositoryRef: ForgeGitPathSegment,
  tenantRef: ForgeGitPathSegment,
});
export interface ForgeGitRoute extends Schema.Schema.Type<typeof ForgeGitRoute> {}

export const ForgeGitSession = Schema.Struct({
  authenticatedAt: Schema.String,
  refRestrictions: Schema.Array(Schema.String),
  repositoryRef: ForgeGitPathSegment,
  subjectRef: Schema.String,
  tenantRef: ForgeGitPathSegment,
  tokenRef: Schema.String,
});
export interface ForgeGitSession extends Schema.Schema.Type<typeof ForgeGitSession> {}

export const ForgeGitRef = Schema.Struct({
  objectId: Schema.String,
  refName: Schema.String,
});
export interface ForgeGitRef extends Schema.Schema.Type<typeof ForgeGitRef> {}

export const ForgeGitPackEvidence = Schema.Struct({
  bytes: Schema.Number,
  objectKey: Schema.String,
  sha256: Schema.String,
});
export interface ForgeGitPackEvidence extends Schema.Schema.Type<typeof ForgeGitPackEvidence> {}

export const ForgeGitMirrorReceipt = Schema.Struct({
  createdAt: Schema.String,
  evidence: Schema.Array(ForgeGitPackEvidence),
  manifestKey: Schema.String,
  repositoryRef: ForgeGitPathSegment,
  tenantRef: ForgeGitPathSegment,
});
export interface ForgeGitMirrorReceipt extends Schema.Schema.Type<typeof ForgeGitMirrorReceipt> {}

export const ForgeGitBackupReceipt = Schema.Struct({
  bundleBytes: Schema.Number,
  bundleKey: Schema.String,
  bundleSha256: Schema.String,
  createdAt: Schema.String,
  headObjectId: Schema.NullOr(Schema.String),
  receiptKey: Schema.String,
  refs: Schema.Array(ForgeGitRef),
  repositoryRef: ForgeGitPathSegment,
  tenantRef: ForgeGitPathSegment,
});
export interface ForgeGitBackupReceipt extends Schema.Schema.Type<typeof ForgeGitBackupReceipt> {}

export class ForgeGitRouteError extends Schema.TaggedErrorClass<ForgeGitRouteError>()(
  "ForgeGitRouteError",
  {
    code: Schema.String,
    status: Schema.Number,
  },
) {}

export class ForgeGitAuthError extends Schema.TaggedErrorClass<ForgeGitAuthError>()(
  "ForgeGitAuthError",
  {
    code: Schema.String,
    status: Schema.Number,
  },
) {}

export class ForgeGitRepositoryError extends Schema.TaggedErrorClass<ForgeGitRepositoryError>()(
  "ForgeGitRepositoryError",
  {
    code: Schema.String,
    operation: Schema.String,
    status: Schema.Number,
    cause: Schema.optionalKey(Schema.Defect()),
  },
) {}
