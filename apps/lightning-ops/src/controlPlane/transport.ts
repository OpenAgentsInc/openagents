import { Context, Effect, Layer } from "effect";

import { ControlPlaneTransportError } from "../errors.js";

export type ControlPlaneTransportApi = Readonly<{
  query: (
    functionName: string,
    args: Record<string, unknown>,
  ) => Effect.Effect<unknown, ControlPlaneTransportError>;
  mutation: (
    functionName: string,
    args: Record<string, unknown>,
  ) => Effect.Effect<unknown, ControlPlaneTransportError>;
}>;

export class ControlPlaneTransportService extends Context.Tag(
  "@openagents/lightning-ops/ControlPlaneTransportService",
)<ControlPlaneTransportService, ControlPlaneTransportApi>() {}

export const makeControlPlaneTransportTestLayer = (transport: ControlPlaneTransportApi) =>
  Layer.succeed(ControlPlaneTransportService, transport);
