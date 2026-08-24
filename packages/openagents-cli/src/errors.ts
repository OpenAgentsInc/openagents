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

export class ComputerAlreadyPaired extends Schema.TaggedErrorClass<ComputerAlreadyPaired>()(
  "OpenAgentsCli.ComputerAlreadyPaired",
  { message: Schema.String },
) {}

export class ComputerPairingInProgress extends Schema.TaggedErrorClass<ComputerPairingInProgress>()(
  "OpenAgentsCli.ComputerPairingInProgress",
  { message: Schema.String },
) {}

export class ComputerDisabled extends Schema.TaggedErrorClass<ComputerDisabled>()(
  "OpenAgentsCli.ComputerDisabled",
  { message: Schema.String },
) {}

export class ComputerPairingExpired extends Schema.TaggedErrorClass<ComputerPairingExpired>()(
  "OpenAgentsCli.ComputerPairingExpired",
  { message: Schema.String },
) {}

export class ComputerPairingRefused extends Schema.TaggedErrorClass<ComputerPairingRefused>()(
  "OpenAgentsCli.ComputerPairingRefused",
  { message: Schema.String },
) {}

export class ComputerPairingNetworkFailure extends Schema.TaggedErrorClass<ComputerPairingNetworkFailure>()(
  "OpenAgentsCli.ComputerPairingNetworkFailure",
  { message: Schema.String },
) {}

export class ComputerStatusNetworkFailure extends Schema.TaggedErrorClass<ComputerStatusNetworkFailure>()(
  "OpenAgentsCli.ComputerStatusNetworkFailure",
  { message: Schema.String },
) {}

export class ComputerMachineUnavailable extends Schema.TaggedErrorClass<ComputerMachineUnavailable>()(
  "OpenAgentsCli.ComputerMachineUnavailable",
  { message: Schema.String },
) {}

export class ComputerMachineMismatch extends Schema.TaggedErrorClass<ComputerMachineMismatch>()(
  "OpenAgentsCli.ComputerMachineMismatch",
  { message: Schema.String },
) {}

export class ComputerReconnectExhausted extends Schema.TaggedErrorClass<ComputerReconnectExhausted>()(
  "OpenAgentsCli.ComputerReconnectExhausted",
  { message: Schema.String },
) {}

export class TraceUploadUnsupported extends Schema.TaggedErrorClass<TraceUploadUnsupported>()(
  "OpenAgentsCli.TraceUploadUnsupported",
  { message: Schema.String },
) {}

/** A fleet promotion target reached `failed` or `reverted`. */
export class DeploymentFailed extends Schema.TaggedErrorClass<DeploymentFailed>()(
  "OpenAgentsCli.DeploymentFailed",
  {
    targetId: Schema.String,
    status: Schema.String,
    code: Schema.optionalKey(Schema.String),
    message: Schema.String,
  },
) {}

/**
 * Polling ended while the fleet target was still nonterminal. The target
 * itself has not failed; the CLI simply stopped watching.
 */
export class DeploymentWaitTimeout extends Schema.TaggedErrorClass<DeploymentWaitTimeout>()(
  "OpenAgentsCli.DeploymentWaitTimeout",
  {
    targetId: Schema.String,
    timeoutMs: Schema.Number,
    lastStatus: Schema.String,
    message: Schema.String,
  },
) {}

/** The target needs an operator-driven rolling replacement to finish. */
export class DeploymentRollingReplaceRequired extends Schema.TaggedErrorClass<DeploymentRollingReplaceRequired>()(
  "OpenAgentsCli.DeploymentRollingReplaceRequired",
  {
    targetId: Schema.String,
    message: Schema.String,
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
  | OutputError
  | ComputerAlreadyPaired
  | ComputerPairingInProgress
  | ComputerDisabled
  | ComputerPairingExpired
  | ComputerPairingRefused
  | ComputerPairingNetworkFailure
  | ComputerStatusNetworkFailure
  | ComputerMachineUnavailable
  | ComputerMachineMismatch
  | ComputerReconnectExhausted
  | TraceUploadUnsupported
  | DeploymentFailed
  | DeploymentWaitTimeout
  | DeploymentRollingReplaceRequired;

export const exitCodeFor = (error: CliError): number => {
  switch (error._tag) {
    case "OpenAgentsCli.InputError":
    case "OpenAgentsCli.ConfigurationError":
      return 2;
    case "OpenAgentsCli.ComputerAlreadyPaired":
    case "OpenAgentsCli.ComputerPairingInProgress":
      return 5;
    case "OpenAgentsCli.ComputerDisabled":
      return 8;
    case "OpenAgentsCli.ComputerPairingExpired":
      return 9;
    case "OpenAgentsCli.ComputerPairingRefused":
      return 10;
    case "OpenAgentsCli.ComputerPairingNetworkFailure":
      return 11;
    case "OpenAgentsCli.ComputerStatusNetworkFailure":
      return 12;
    case "OpenAgentsCli.ComputerMachineUnavailable":
      return 13;
    case "OpenAgentsCli.ComputerMachineMismatch":
      return 14;
    case "OpenAgentsCli.ComputerReconnectExhausted":
      return 15;
    case "OpenAgentsCli.TraceUploadUnsupported":
      return 16;
    // Deployment outcomes stay apart from each other and from transport
    // failures, so release automation can tell "the fleet rejected these
    // bytes" from "the CLI stopped watching" without parsing prose.
    case "OpenAgentsCli.DeploymentFailed":
      return 17;
    case "OpenAgentsCli.DeploymentWaitTimeout":
      return 18;
    case "OpenAgentsCli.DeploymentRollingReplaceRequired":
      return 19;
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
