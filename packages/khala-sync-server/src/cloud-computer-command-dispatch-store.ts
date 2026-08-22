import type { CloudComputerCommand } from "./cloud-computer-command.js";
import type { CloudComputerDurableDispatchPort } from "./cloud-computer-command-dispatch.js";
import {
  CloudComputerCommandStoreError,
  type CloudComputerCommandDispatchClaim,
  type PostgresCloudComputerCommandStore,
} from "./cloud-computer-command-store.js";

export type CloudComputerCommandDispatchStore = Pick<
  PostgresCloudComputerCommandStore,
  | "get"
  | "getDispatchAttempt"
  | "prepareDispatchAttempt"
  | "exposeDispatchAttempt"
  | "markMayHaveStarted"
  | "recordReservation"
  | "recordDispatchedAcknowledgement"
>;

const fence = (
  input: Readonly<{
    command: CloudComputerCommand;
    sessionRef: string;
    attachmentEpoch: number;
    observedAt: string;
  }>,
) => ({
  commandRef: input.command.commandRef,
  sessionRef: input.sessionRef,
  attachmentEpoch: input.attachmentEpoch,
  runtimeGeneration: input.command.runtimeGeneration,
  runtimeRef: input.command.runtimeRef,
  providerLeaseRef: input.command.providerLeaseRef,
  observedAt: input.observedAt,
});

/** Adapts the Postgres write fence to the crash-safe dispatcher contract. */
export const createPostgresCloudComputerDurableDispatchPort = (
  store: CloudComputerCommandDispatchStore,
): CloudComputerDurableDispatchPort<CloudComputerCommandDispatchClaim> => ({
  load: async (commandRef) => {
    const attempt = await store.getDispatchAttempt(commandRef);
    if (attempt === null || attempt.status === "not_exposed") return null;
    const command = await store.get(commandRef);
    if (
      attempt.runtimeGeneration !== command.runtimeGeneration ||
      attempt.runtimeRef !== command.runtimeRef ||
      attempt.providerLeaseRef !== command.providerLeaseRef
    ) {
      throw new CloudComputerCommandStoreError("conflict", "dispatch attempt fence differs");
    }
    const stage =
      attempt.status === "prepared"
        ? "prepared"
        : attempt.status === "reservation_recorded"
          ? "reservation_recorded"
          : attempt.status === "acknowledged"
            ? "acknowledged"
            : "exposed";
    return {
      commandRef,
      requestDigest: command.requestDigest,
      dispatchRef: attempt.dispatchRef,
      stage,
      providerCommandRef: attempt.providerCommandRef,
      reservationRef: attempt.reservationRef,
      providerExecutionRef: attempt.providerExecutionRef,
      acknowledgementEventRef: attempt.acknowledgementEventRef,
      acknowledgementEventDigest: attempt.acknowledgementEventDigest,
      reservation: null,
      acknowledgement: null,
    };
  },
  prepare: async (input) => {
    await store.prepareDispatchAttempt({ ...fence(input), dispatchRef: input.dispatchRef });
  },
  expose: (input) =>
    store.exposeDispatchAttempt({ ...fence(input), dispatchRef: input.dispatchRef }),
  markUncertain: async (input) => {
    await store.markMayHaveStarted({
      ...fence(input),
      dispatchRef: input.dispatchRef,
    });
  },
  recordReservation: async (input) => {
    await store.recordReservation({
      ...fence(input),
      dispatchRef: input.dispatchRef,
      reservation: input.reservation,
      reservationRef: input.reservation.reservationRef,
      providerExecutionRef: input.reservation.providerExecutionRef,
      providerCommandRef: input.providerCommandRef,
    });
  },
  recordAcknowledgement: async (input) => {
    await store.recordDispatchedAcknowledgement({
      ...fence({ ...input, observedAt: input.acknowledgement.observedAt }),
      dispatchRef: input.dispatchRef,
      reservation: input.reservation,
      acknowledgement: input.acknowledgement,
      reservationRef: input.reservation.reservationRef,
      providerExecutionRef: input.reservation.providerExecutionRef,
      providerCommandRef: input.providerCommandRef,
      acknowledgementEventRef: input.acknowledgement.eventRef,
      acknowledgementEventDigest: input.acknowledgement.eventDigest,
      expectedFence: input.acknowledgement.fence,
      expectedCommandSequence: input.acknowledgement.acceptedSequence,
    });
  },
});
