import { Option, Schema } from "effect";

export const REPOSITORY_CONTRACT_NAME = "openagents.repositories.v1";
export const REPOSITORY_CONTRACT_VERSION = 1;
export const REPOSITORY_CONTRACT_SHA256 =
  "96a71ee0a3d19eb77ffa0721cc76876e1e514bb821368c469f0a6d91f21c3870";

export const AuthenticatedNamespace = Schema.Struct({
  id: Schema.Union([Schema.Number, Schema.String]),
  login: Schema.String,
  type: Schema.Literals(["user", "organization"]),
});

export const AuthenticatedUser = Schema.Struct({
  id: Schema.Number,
  login: Schema.String,
  token_expires_at: Schema.String,
  namespaces: Schema.Array(AuthenticatedNamespace),
});
export interface AuthenticatedUser extends Schema.Schema.Type<typeof AuthenticatedUser> {}

export const RepositoryOwner = Schema.Struct({
  id: Schema.Union([Schema.Number, Schema.String]),
  login: Schema.String,
  type: Schema.String,
});
export interface RepositoryOwner extends Schema.Schema.Type<typeof RepositoryOwner> {}

const RepositoryFields = {
  id: Schema.String,
  name: Schema.String,
  full_name: Schema.String,
  owner: RepositoryOwner,
  private: Schema.Boolean,
  visibility: Schema.Literals(["private", "public"]),
  description: Schema.NullOr(Schema.String),
  default_branch: Schema.String,
  lifecycle_state: Schema.Literals(["provisioning", "ready", "failed"]),
  provision_error_code: Schema.NullOr(Schema.String),
  clone_url: Schema.String,
  html_url: Schema.String,
  permissions: Schema.Struct({
    admin: Schema.Boolean,
    push: Schema.Boolean,
    pull: Schema.Boolean,
  }),
  created_at: Schema.String,
  updated_at: Schema.String,
};

export const Repository = Schema.Struct(RepositoryFields);
export interface Repository extends Schema.Schema.Type<typeof Repository> {}

export const RepositoryResponse = Repository;

export const RepositoryListResponse = Schema.Struct({
  repositories: Schema.Array(Repository),
  next_cursor: Schema.NullOr(Schema.String),
});

export const RepositoryImportState = Schema.Literals(["pending", "running", "completed", "failed"]);
export type RepositoryImportState = typeof RepositoryImportState.Type;

export const RepositoryImport = Schema.Struct({
  id: Schema.String,
  provider: Schema.Literal("github"),
  source_full_name: Schema.String,
  source_default_branch: Schema.String,
  source_ref_digest: Schema.String,
  source_head_sha: Schema.NullOr(Schema.String),
  state: RepositoryImportState,
  lfs_warning: Schema.Boolean,
  attempt_count: Schema.Number,
  error_code: Schema.NullOr(Schema.String),
  started_at: Schema.NullOr(Schema.String),
  completed_at: Schema.NullOr(Schema.String),
});
export interface RepositoryImport extends Schema.Schema.Type<typeof RepositoryImport> {}

export const RepositoryImportStatusResponse = Schema.Struct({
  repository: Repository,
  import: RepositoryImport,
});
export type RepositoryImportStatusResponse = typeof RepositoryImportStatusResponse.Type;

export const RepositoryImportAcceptedResponse = Schema.Struct({
  ...RepositoryFields,
  import: RepositoryImport,
  replayed: Schema.Boolean,
});
export type RepositoryImportAcceptedResponse = typeof RepositoryImportAcceptedResponse.Type;

export const ApiErrorResponse = Schema.Struct({
  code: Schema.optionalKey(Schema.String),
  message: Schema.optionalKey(Schema.String),
  error: Schema.optionalKey(Schema.String),
  request_id: Schema.optionalKey(Schema.NullOr(Schema.String)),
});

export interface ApiErrorDetails {
  readonly message?: string;
  readonly code?: string;
  readonly requestId?: string;
}

/**
 * Reads the optional error envelope the API returns with a failed request. The
 * caller supplies its own fallback message, because a passthrough request and a
 * typed repository request describe a failure differently.
 */
export const apiErrorDetails = (body: unknown): ApiErrorDetails => {
  const decoded = Schema.decodeUnknownOption(ApiErrorResponse)(body);
  if (Option.isNone(decoded)) return {};
  const value = decoded.value;
  const message = value.message ?? value.error;
  return {
    ...(typeof message === "string" ? { message } : {}),
    ...(typeof value.code === "string" ? { code: value.code } : {}),
    ...(typeof value.request_id === "string" ? { requestId: value.request_id } : {}),
  };
};

export const repositoryFromAcceptedImport = (
  response: RepositoryImportAcceptedResponse,
): Repository => {
  const { import: _repositoryImport, replayed: _replayed, ...repository } = response;
  return repository;
};
