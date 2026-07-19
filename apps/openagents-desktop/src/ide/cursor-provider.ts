import { Context, Schema, Stream } from "effect";
import {
  IdeCursorCapabilitiesSchema,
  type IdeCursorCapabilities,
  type IdeCursorProviderInput,
} from "./cursor-contract.ts";

export class IdeCursorProviderFailure extends Schema.TaggedErrorClass<IdeCursorProviderFailure>()(
  "IdeCursor.ProviderFailure",
  {
    operation: Schema.String,
    reason: Schema.Literals([
      "unavailable",
      "rejected",
      "rate_limited",
      "interrupted",
      "invalid_event",
    ]),
    detail: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(2_000)),
  },
) {}

export interface IdeCursorProviderShape {
  readonly capabilities: IdeCursorCapabilities;
  readonly generate: (
    input: IdeCursorProviderInput,
  ) => Stream.Stream<unknown, IdeCursorProviderFailure>;
}

export class IdeCursorProvider extends Context.Service<IdeCursorProvider, IdeCursorProviderShape>()(
  "@openagentsinc/openagents/IdeCursorProvider",
) {}

export const decodeIdeCursorCapabilities = (value: unknown) =>
  Schema.decodeUnknownEffect(IdeCursorCapabilitiesSchema)(value);
