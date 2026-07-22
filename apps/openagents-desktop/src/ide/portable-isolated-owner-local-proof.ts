import type { PortableSessionCommand } from "@openagentsinc/portable-session-contract";

import type { IdePortableClientCommandResult } from "./portable-client-contract.ts";

export const Ide13IsolatedOwnerLocalProofEnvironment = "OPENAGENTS_DESKTOP_IDE13_OWNER_LOCAL_PROOF";
export const Ide13IsolatedOwnerLocalProofNonceEnvironment =
  "OPENAGENTS_DESKTOP_IDE13_OWNER_LOCAL_PROOF_NONCE";

export type Ide13IsolatedOwnerLocalProofDispatcher = Readonly<{
  request: (command: PortableSessionCommand) => Promise<IdePortableClientCommandResult>;
}>;

const admittedProofCommand = (command: PortableSessionCommand, now: number): boolean => {
  const expiresAt = Date.parse(command.expiresAt);
  return command.schema === "openagents.portable_session_command.v1" &&
    command.kind === "move" && command.ownerRef === "owner.ide13.isolated" &&
    command.sessionRef === "session.ide13.isolated" &&
    command.expectedAttachmentRef === "attachment.ide13.isolated.1" &&
    command.expectedGeneration === 1 &&
    command.destinationTargetRef === "target.ide13.owner-local.2" &&
    command.checkpointRef === undefined &&
    command.commandRef === "command.ide13.isolated.move.1" &&
    command.idempotencyKey === "idempotency.ide13.isolated.move.1" &&
    Number.isFinite(expiresAt) && expiresAt > now && expiresAt <= now + 5 * 60_000;
};

export const makeIde13IsolatedOwnerLocalProofDispatcher = (input: Readonly<{
  env: NodeJS.ProcessEnv;
  isolatedAppProof: boolean;
  packaged: boolean;
  now?: () => number;
}>): Ide13IsolatedOwnerLocalProofDispatcher | null => {
  if (!input.packaged || !input.isolatedAppProof ||
    input.env[Ide13IsolatedOwnerLocalProofEnvironment] !== "1" ||
    !/^[a-f0-9]{64}$/u.test(input.env[Ide13IsolatedOwnerLocalProofNonceEnvironment] ?? "")) {
    return null;
  }
  const now = input.now ?? Date.now;
  return {
    request: async command => admittedProofCommand(command, now())
      ? { _tag: "Requested", mutationRef: "mutation.ide13.isolated.move.1" }
      : { _tag: "Refused", reason: "invalid_input" },
  };
};
