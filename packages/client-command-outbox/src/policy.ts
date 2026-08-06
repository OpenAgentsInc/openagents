import type { CommandClass, DestructiveGitAuthorization } from "./model.ts";

export const CLIENT_OPERATION_CATALOG = {
  "thread.create": "durable_intent",
  "thread.message.send": "durable_intent",
  "work.order.create": "durable_intent",
  "work.order.update": "durable_intent",
  "work.order.dispatch": "durable_intent",
  "issue.create": "durable_intent",
  "issue.update": "durable_intent",
  "approval.respond": "expiring_decision",
  "input.respond": "expiring_decision",
  "runtime.interrupt": "live_control",
  "git.destructive.execute": "destructive_git",
  "attention.list": "observation",
  "thread.list": "observation",
  "thread.transcript.read": "observation",
  "work.order.read": "observation",
} as const satisfies Readonly<Record<string, CommandClass>>;

export type ClientOperation = keyof typeof CLIENT_OPERATION_CATALOG;

export const classifyClientOperation = (operation: string): CommandClass | undefined =>
  Object.prototype.hasOwnProperty.call(CLIENT_OPERATION_CATALOG, operation)
    ? CLIENT_OPERATION_CATALOG[operation as ClientOperation]
    : undefined;

export class OfflineCommandRefusedError extends Error {
  readonly operation: string;

  constructor(operation: string, detail: string) {
    super(detail);
    this.name = "OfflineCommandRefusedError";
    this.operation = operation;
  }
}

export const authorizeImmediateCommand = (input: {
  readonly operation: string;
  readonly online: boolean;
  readonly destructiveGit?: DestructiveGitAuthorization;
}): void => {
  const commandClass = classifyClientOperation(input.operation);
  if (commandClass === undefined) throw new OfflineCommandRefusedError(input.operation, "Unknown operation.");
  if (commandClass !== "live_control" && commandClass !== "destructive_git") {
    throw new OfflineCommandRefusedError(input.operation, "This operation must use the durable outbox.");
  }
  if (!input.online) {
    throw new OfflineCommandRefusedError(input.operation, "Live operations are unavailable while offline.");
  }
  if (commandClass === "destructive_git") {
    const authorization = input.destructiveGit;
    if (
      authorization === undefined ||
      authorization.preflightRef.trim() === "" ||
      authorization.confirmationRef.trim() === "" ||
      !/^sha256:[a-f0-9]{64}$/u.test(authorization.disclosureDigest)
    ) {
      throw new OfflineCommandRefusedError(
        input.operation,
        "Destructive Git requires a live preflight, fresh confirmation, and complete disclosure digest.",
      );
    }
  }
};
