import {
  CloudComputerCommandError,
  assertCloudComputerRuntimeAcknowledgement,
  assertCloudComputerRuntimeReservation,
  type CloudComputerCommand,
  type CloudComputerRuntimeAcknowledgement,
  type CloudComputerRuntimeReservation,
} from "./cloud-computer-command.js";

export type CloudComputerDurableDispatchState = Readonly<{
  commandRef: string;
  requestDigest: string;
  dispatchRef: string;
  stage: "prepared" | "exposed" | "reservation_recorded" | "acknowledged";
  providerCommandRef: string | null;
  reservationRef: string | null;
  providerExecutionRef: string | null;
  acknowledgementEventRef: string | null;
  acknowledgementEventDigest: string | null;
  reservation: CloudComputerRuntimeReservation | null;
  acknowledgement: CloudComputerRuntimeAcknowledgement | null;
}>;

export type CloudComputerDurableDispatchPort<Claim> = Readonly<{
  load: (commandRef: string) => Promise<CloudComputerDurableDispatchState | null>;
  prepare: (
    input: Readonly<{
      command: CloudComputerCommand;
      sessionRef: string;
      attachmentEpoch: number;
      dispatchRef: string;
      observedAt: string;
    }>,
  ) => Promise<void>;
  expose: (
    input: Readonly<{
      command: CloudComputerCommand;
      sessionRef: string;
      attachmentEpoch: number;
      dispatchRef: string;
      observedAt: string;
    }>,
  ) => Promise<Claim>;
  markUncertain: (
    input: Readonly<{
      command: CloudComputerCommand;
      sessionRef: string;
      attachmentEpoch: number;
      dispatchRef: string;
      observedAt: string;
    }>,
  ) => Promise<void>;
  recordReservation: (
    input: Readonly<{
      command: CloudComputerCommand;
      sessionRef: string;
      attachmentEpoch: number;
      dispatchRef: string;
      providerCommandRef: string;
      reservation: CloudComputerRuntimeReservation;
      observedAt: string;
    }>,
  ) => Promise<void>;
  recordAcknowledgement: (
    input: Readonly<{
      command: CloudComputerCommand;
      sessionRef: string;
      attachmentEpoch: number;
      dispatchRef: string;
      providerCommandRef: string;
      reservation: CloudComputerRuntimeReservation;
      acknowledgement: CloudComputerRuntimeAcknowledgement;
      observedAt: string;
    }>,
  ) => Promise<void>;
}>;

export type CloudComputerCommandTransportPort<Claim> = Readonly<{
  write: (
    input: Readonly<{
      claim: Claim;
      command: CloudComputerCommand;
      dispatchRef: string;
    }>,
  ) => Promise<
    Readonly<{
      providerCommandRef: string;
      reservation: CloudComputerRuntimeReservation;
      acknowledgement: CloudComputerRuntimeAcknowledgement;
    }>
  >;
}>;

export type CloudComputerDispatchFaultPort = Readonly<{
  afterPrepare?: () => void | Promise<void>;
  afterExposure?: () => void | Promise<void>;
  afterTransport?: () => void | Promise<void>;
  afterReservation?: () => void | Promise<void>;
}>;

const assertMapping = (
  command: CloudComputerCommand,
  reservation: CloudComputerRuntimeReservation,
  acknowledgement: CloudComputerRuntimeAcknowledgement,
): void => {
  assertCloudComputerRuntimeReservation(reservation);
  assertCloudComputerRuntimeAcknowledgement(acknowledgement);
  if (
    reservation.commandRef !== command.commandRef ||
    reservation.requestDigest !== command.requestDigest ||
    reservation.runtimeRef !== command.runtimeRef ||
    reservation.runtimeGeneration !== command.runtimeGeneration ||
    reservation.providerLeaseRef !== command.providerLeaseRef ||
    acknowledgement.commandRef !== command.commandRef ||
    acknowledgement.requestDigest !== command.requestDigest ||
    acknowledgement.reservationRef !== reservation.reservationRef ||
    acknowledgement.providerExecutionRef !== reservation.providerExecutionRef ||
    acknowledgement.fence !== reservation.fence
  ) {
    throw new CloudComputerCommandError("conflict", "dispatchMapping");
  }
};

/**
 * Crosses the provider-write boundary once. Any persisted exposure is treated
 * as possibly started and can only continue through runtime reattachment.
 */
export const createCloudComputerCommandDispatcher = <Claim>(
  input: Readonly<{
    durable: CloudComputerDurableDispatchPort<Claim>;
    transport: CloudComputerCommandTransportPort<Claim>;
    faults?: CloudComputerDispatchFaultPort;
  }>,
) => ({
  dispatch: async (
    request: Readonly<{
      command: CloudComputerCommand;
      sessionRef: string;
      attachmentEpoch: number;
      dispatchRef: string;
      observedAt: string;
    }>,
  ): Promise<Readonly<{ outcome: "dispatched" | "observation_required" }>> => {
    const prior = await input.durable.load(request.command.commandRef);
    if (prior !== null) {
      if (
        prior.requestDigest !== request.command.requestDigest ||
        prior.dispatchRef !== request.dispatchRef
      ) {
        throw new CloudComputerCommandError("conflict", "dispatchRef");
      }
      if (prior.stage === "acknowledged") return { outcome: "dispatched" };
      if (prior.stage !== "prepared") return { outcome: "observation_required" };
    } else {
      await input.durable.prepare(request);
      await input.faults?.afterPrepare?.();
    }

    const claim = await input.durable.expose(request);
    await input.faults?.afterExposure?.();
    let runtime;
    try {
      runtime = await input.transport.write({
        claim,
        command: request.command,
        dispatchRef: request.dispatchRef,
      });
      await input.faults?.afterTransport?.();
    } catch (error) {
      await input.durable.markUncertain({
        command: request.command,
        sessionRef: request.sessionRef,
        attachmentEpoch: request.attachmentEpoch,
        dispatchRef: request.dispatchRef,
        observedAt: request.observedAt,
      });
      throw error;
    }
    assertMapping(request.command, runtime.reservation, runtime.acknowledgement);
    await input.durable.recordReservation({
      ...request,
      providerCommandRef: runtime.providerCommandRef,
      reservation: runtime.reservation,
      observedAt: runtime.reservation.reservedAt,
    });
    await input.faults?.afterReservation?.();
    await input.durable.recordAcknowledgement({
      ...request,
      providerCommandRef: runtime.providerCommandRef,
      reservation: runtime.reservation,
      acknowledgement: runtime.acknowledgement,
    });
    return { outcome: "dispatched" };
  },

  acceptRuntimeAcknowledgement: async (
    request: Readonly<{
      command: CloudComputerCommand;
      sessionRef: string;
      attachmentEpoch: number;
      dispatchRef: string;
      providerCommandRef: string;
      reservation: CloudComputerRuntimeReservation;
      acknowledgement: CloudComputerRuntimeAcknowledgement;
      observedAt: string;
    }>,
  ): Promise<void> => {
    assertMapping(request.command, request.reservation, request.acknowledgement);
    const state = await input.durable.load(request.command.commandRef);
    if (state?.stage === "acknowledged") {
      if (
        state.requestDigest !== request.command.requestDigest ||
        state.dispatchRef !== request.dispatchRef ||
        state.providerCommandRef !== request.providerCommandRef ||
        state.reservationRef !== request.reservation.reservationRef ||
        state.providerExecutionRef !== request.reservation.providerExecutionRef ||
        state.acknowledgementEventRef !== request.acknowledgement.eventRef ||
        state.acknowledgementEventDigest !== request.acknowledgement.eventDigest
      ) {
        throw new CloudComputerCommandError("conflict", "dispatchAcknowledgement");
      }
      return;
    }
    if (
      state === null ||
      state.requestDigest !== request.command.requestDigest ||
      state.dispatchRef !== request.dispatchRef ||
      (state.stage !== "exposed" && state.stage !== "reservation_recorded")
    ) {
      throw new CloudComputerCommandError("conflict", "dispatchState");
    }
    if (state.stage === "exposed") {
      await input.durable.recordReservation({
        command: request.command,
        sessionRef: request.sessionRef,
        attachmentEpoch: request.attachmentEpoch,
        dispatchRef: request.dispatchRef,
        providerCommandRef: request.providerCommandRef,
        reservation: request.reservation,
        observedAt: request.reservation.reservedAt,
      });
    }
    await input.durable.recordAcknowledgement(request);
  },
});
