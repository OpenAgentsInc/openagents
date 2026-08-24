#!/usr/bin/env node

import * as NodeRuntime from "@effect/platform-node/NodeRuntime";
import { Console, Effect } from "effect";

import { openagentsCommand, VERSION } from "./cli.js";
import { errorCode, errorMessage, exitCodeFor, requestIdFor, type CliError } from "./errors.js";
import { runtimeLayer } from "./runtime.js";
import { Command } from "effect/unstable/cli";

const cliErrorTags = new Set([
  "OpenAgentsCli.InputError",
  "OpenAgentsCli.ConfigurationError",
  "OpenAgentsCli.CredentialPersistenceUnavailable",
  "OpenAgentsCli.CredentialStoreError",
  "OpenAgentsCli.AuthenticationRequired",
  "OpenAgentsCli.NetworkRefused",
  "OpenAgentsCli.TransportError",
  "OpenAgentsCli.ApiError",
  "OpenAgentsCli.ContractError",
  "OpenAgentsCli.ImportFailed",
  "OpenAgentsCli.ImportWaitTimeout",
  "OpenAgentsCli.ProvisioningFailed",
  "OpenAgentsCli.ProvisioningWaitTimeout",
  "OpenAgentsCli.GitExecutionError",
  "OpenAgentsCli.OutputError",
  "OpenAgentsCli.ComputerAlreadyPaired",
  "OpenAgentsCli.ComputerPairingInProgress",
  "OpenAgentsCli.ComputerDisabled",
  "OpenAgentsCli.ComputerPairingExpired",
  "OpenAgentsCli.ComputerPairingRefused",
  "OpenAgentsCli.ComputerPairingNetworkFailure",
  "OpenAgentsCli.ComputerStatusNetworkFailure",
  "OpenAgentsCli.ComputerMachineUnavailable",
  "OpenAgentsCli.ComputerMachineMismatch",
  "OpenAgentsCli.ComputerReconnectExhausted",
  "OpenAgentsCli.TraceUploadUnsupported",
]);

const isCliError = (value: unknown): value is CliError =>
  typeof value === "object" &&
  value !== null &&
  "_tag" in value &&
  typeof value._tag === "string" &&
  cliErrorTags.has(value._tag);

const program = Command.run(openagentsCommand, { version: VERSION }).pipe(
  Effect.catch((error) => {
    const message = isCliError(error) ? errorMessage(error) : String(error);
    const exitCode = isCliError(error) ? exitCodeFor(error) : 1;
    const json = process.argv.includes("--json");
    const rendered =
      json && isCliError(error)
        ? JSON.stringify({
            code: errorCode(error),
            message,
            exit_code: exitCode,
            ...(requestIdFor(error) === undefined ? {} : { request_id: requestIdFor(error) }),
          })
        : `openagents: ${message}`;
    const write = json ? Console.log(rendered) : Console.error(rendered);
    return write.pipe(Effect.andThen(Effect.sync(() => void (process.exitCode = exitCode))));
  }),
  Effect.provide(runtimeLayer),
);

NodeRuntime.runMain(program);
