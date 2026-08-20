import { Schema } from "effect";

export class InputError extends Schema.TaggedErrorClass<InputError>()("OpenAgentsCli.InputError", {
  message: Schema.String,
}) {}

export class ConfigurationError extends Schema.TaggedErrorClass<ConfigurationError>()(
  "OpenAgentsCli.ConfigurationError",
  {
    message: Schema.String,
  },
) {}

export class CredentialPersistenceUnavailable extends Schema.TaggedErrorClass<CredentialPersistenceUnavailable>()(
  "OpenAgentsCli.CredentialPersistenceUnavailable",
  {
    message: Schema.String,
  },
) {}

export class CredentialStoreError extends Schema.TaggedErrorClass<CredentialStoreError>()(
  "OpenAgentsCli.CredentialStoreError",
  {
    operation: Schema.String,
    message: Schema.String,
    cause: Schema.Defect(),
  },
) {}

export class AuthenticationRequired extends Schema.TaggedErrorClass<AuthenticationRequired>()(
  "OpenAgentsCli.AuthenticationRequired",
  {
    origin: Schema.String,
    message: Schema.String,
  },
) {}

export class NetworkRefused extends Schema.TaggedErrorClass<NetworkRefused>()(
  "OpenAgentsCli.NetworkRefused",
  {
    origin: Schema.String,
    message: Schema.String,
  },
) {}

export class TransportError extends Schema.TaggedErrorClass<TransportError>()(
  "OpenAgentsCli.TransportError",
  {
    operation: Schema.String,
    message: Schema.String,
    cause: Schema.Defect(),
  },
) {}

export class ApiError extends Schema.TaggedErrorClass<ApiError>()("OpenAgentsCli.ApiError", {
  operation: Schema.String,
  status: Schema.Number,
  code: Schema.optionalKey(Schema.String),
  message: Schema.String,
  requestId: Schema.optionalKey(Schema.String),
}) {}

export class ContractError extends Schema.TaggedErrorClass<ContractError>()(
  "OpenAgentsCli.ContractError",
  {
    operation: Schema.String,
    message: Schema.String,
    cause: Schema.Defect(),
  },
) {}

export class ImportFailed extends Schema.TaggedErrorClass<ImportFailed>()(
  "OpenAgentsCli.ImportFailed",
  {
    importId: Schema.String,
    message: Schema.String,
  },
) {}

export class ImportWaitTimeout extends Schema.TaggedErrorClass<ImportWaitTimeout>()(
  "OpenAgentsCli.ImportWaitTimeout",
  {
    importId: Schema.String,
    timeoutMs: Schema.Number,
    message: Schema.String,
  },
) {}

export class ProvisioningFailed extends Schema.TaggedErrorClass<ProvisioningFailed>()(
  "OpenAgentsCli.ProvisioningFailed",
  {
    repository: Schema.String,
    message: Schema.String,
  },
) {}

export class ProvisioningWaitTimeout extends Schema.TaggedErrorClass<ProvisioningWaitTimeout>()(
  "OpenAgentsCli.ProvisioningWaitTimeout",
  {
    repository: Schema.String,
    timeoutMs: Schema.Number,
    message: Schema.String,
  },
) {}

export class GitExecutionError extends Schema.TaggedErrorClass<GitExecutionError>()(
  "OpenAgentsCli.GitExecutionError",
  {
    operation: Schema.String,
    exitCode: Schema.optionalKey(Schema.Number),
    message: Schema.String,
    cause: Schema.optionalKey(Schema.Defect()),
  },
) {}

export class OutputError extends Schema.TaggedErrorClass<OutputError>()(
  "OpenAgentsCli.OutputError",
  {
    message: Schema.String,
    cause: Schema.Defect(),
  },
) {}

export type CliError =
  | InputError
  | ConfigurationError
  | CredentialPersistenceUnavailable
  | CredentialStoreError
  | AuthenticationRequired
  | NetworkRefused
  | TransportError
  | ApiError
  | ContractError
  | ImportFailed
  | ImportWaitTimeout
  | ProvisioningFailed
  | ProvisioningWaitTimeout
  | GitExecutionError
  | OutputError;

export const exitCodeFor = (error: CliError): number => {
  switch (error._tag) {
    case "OpenAgentsCli.InputError":
    case "OpenAgentsCli.ConfigurationError":
      return 2;
    case "OpenAgentsCli.AuthenticationRequired":
    case "OpenAgentsCli.CredentialPersistenceUnavailable":
    case "OpenAgentsCli.CredentialStoreError":
      return 3;
    case "OpenAgentsCli.NetworkRefused":
    case "OpenAgentsCli.TransportError":
    case "OpenAgentsCli.ContractError":
      return 6;
    case "OpenAgentsCli.ApiError":
      if (error.status === 401 || error.status === 403) return 3;
      if (error.status === 404) return 4;
      if (error.status === 409) return 5;
      if (error.status === 400 || error.status === 422) return 2;
      if (error.status >= 500) return 6;
      return 1;
    case "OpenAgentsCli.ImportFailed":
      return 7;
    case "OpenAgentsCli.ImportWaitTimeout":
      return 7;
    case "OpenAgentsCli.ProvisioningFailed":
      return 7;
    case "OpenAgentsCli.ProvisioningWaitTimeout":
      return 7;
    case "OpenAgentsCli.GitExecutionError":
      return 1;
    case "OpenAgentsCli.OutputError":
      return 1;
  }
};

export const errorMessage = (error: CliError): string => error.message;

export const errorCode = (error: CliError): string => {
  if (error._tag === "OpenAgentsCli.ApiError" && error.code !== undefined) return error.code;
  return error._tag
    .replace("OpenAgentsCli.", "")
    .replaceAll(/([a-z])([A-Z])/gu, "$1_$2")
    .toLowerCase();
};

export const requestIdFor = (error: CliError): string | undefined =>
  error._tag === "OpenAgentsCli.ApiError" ? error.requestId : undefined;
