import { Schema } from "effect";

export const RepositoryOwner = Schema.Struct({
  login: Schema.String,
  id: Schema.Union([Schema.Number, Schema.String]),
  type: Schema.optionalKey(Schema.String),
});
export interface RepositoryOwner extends Schema.Schema.Type<typeof RepositoryOwner> {}

export const Repository = Schema.Struct({
  id: Schema.Union([Schema.Number, Schema.String]),
  name: Schema.String,
  full_name: Schema.String,
  owner: RepositoryOwner,
  private: Schema.Boolean,
  visibility: Schema.optionalKey(Schema.String),
  default_branch: Schema.optionalKey(Schema.NullOr(Schema.String)),
  html_url: Schema.optionalKey(Schema.String),
  clone_url: Schema.optionalKey(Schema.String),
  git_url: Schema.optionalKey(Schema.String),
  provisioning_state: Schema.optionalKey(Schema.String),
});
export interface Repository extends Schema.Schema.Type<typeof Repository> {}

export const RepositoryResponse = Schema.Union([
  Repository,
  Schema.Struct({ repository: Repository }),
]);

export const RepositoryListResponse = Schema.Union([
  Schema.Array(Repository),
  Schema.Struct({ repositories: Schema.Array(Repository) }),
]);

export const RepositoryImportState = Schema.Literals(["pending", "running", "completed", "failed"]);
export type RepositoryImportState = typeof RepositoryImportState.Type;

export const RepositoryImport = Schema.Struct({
  id: Schema.Union([Schema.String, Schema.Number]),
  state: RepositoryImportState,
  repository_id: Schema.optionalKey(Schema.Union([Schema.String, Schema.Number])),
  source_full_name: Schema.optionalKey(Schema.String),
  source_head_sha: Schema.optionalKey(Schema.NullOr(Schema.String)),
  error_code: Schema.optionalKey(Schema.NullOr(Schema.String)),
  error_message: Schema.optionalKey(Schema.NullOr(Schema.String)),
});
export interface RepositoryImport extends Schema.Schema.Type<typeof RepositoryImport> {}

export const RepositoryImportResponse = Schema.Union([
  RepositoryImport,
  Schema.Struct({ import: RepositoryImport }),
]);

export const RepositoryImportAcceptedResponse = Schema.Union([
  Schema.Struct({ repository: Repository, import: RepositoryImport }),
  Schema.Struct({ repository: Repository, repository_import: RepositoryImport }),
]);
export type RepositoryImportAcceptedResponse = typeof RepositoryImportAcceptedResponse.Type;

export const ApiErrorResponse = Schema.Struct({
  message: Schema.optionalKey(Schema.String),
  error: Schema.optionalKey(Schema.String),
  request_id: Schema.optionalKey(Schema.String),
});

export const unwrapRepository = (response: typeof RepositoryResponse.Type): Repository =>
  "repository" in response ? response.repository : response;

export const unwrapRepositoryList = (
  response: typeof RepositoryListResponse.Type,
): ReadonlyArray<Repository> => ("repositories" in response ? response.repositories : response);

export const unwrapRepositoryImport = (
  response: typeof RepositoryImportResponse.Type,
): RepositoryImport => ("import" in response ? response.import : response);

export const unwrapAcceptedImport = (
  response: RepositoryImportAcceptedResponse,
): { readonly repository: Repository; readonly repositoryImport: RepositoryImport } => ({
  repository: response.repository,
  repositoryImport: "import" in response ? response.import : response.repository_import,
});
