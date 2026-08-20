#!/usr/bin/env node

import * as NodeRuntime from "@effect/platform-node/NodeRuntime";
import { Console, Effect } from "effect";

import { openagentsCommand, VERSION } from "./cli.js";
import { errorMessage, exitCodeFor, type CliError } from "./errors.js";
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
    return Console.error(`openagents: ${message}`).pipe(
      Effect.andThen(Effect.sync(() => void (process.exitCode = exitCode))),
    );
  }),
  Effect.provide(runtimeLayer),
);

NodeRuntime.runMain(program);
