import {
  FORGE_COLLABORATION_SCHEMA,
  ForgeCollaborationProjection as ForgeCollaborationProjectionSchema,
  type ForgeCollaborationProjection as ForgeCollaborationProjectionType,
  ForgeCollaborationRequest as ForgeCollaborationRequestSchema,
  type ForgeCollaborationRequest as ForgeCollaborationRequestType,
} from "@openagentsinc/forge-protocol";
import { Context, Effect, Layer, Schema as S } from "effect";

export { FORGE_COLLABORATION_SCHEMA };
export const ForgeCollaborationProjection = ForgeCollaborationProjectionSchema;
export const ForgeCollaborationRequest = ForgeCollaborationRequestSchema;
export type ForgeCollaborationProjection = ForgeCollaborationProjectionType;
export type ForgeCollaborationRequest = ForgeCollaborationRequestType;

const NonEmpty = S.String.check(S.isMinLength(1));

export const ForgeCollaborationFailure = S.TaggedUnion({
  not_found: { detail: NonEmpty },
  authentication_required: { detail: NonEmpty },
  unavailable: { detail: NonEmpty, retryable: S.Boolean },
  malformed_response: { detail: NonEmpty },
});
export type ForgeCollaborationFailure = S.Schema.Type<typeof ForgeCollaborationFailure>;

export const ForgeCollaborationResult = S.TaggedUnion({
  loaded: { projection: ForgeCollaborationProjection },
  failed: { failure: ForgeCollaborationFailure },
});
export type ForgeCollaborationResult = S.Schema.Type<typeof ForgeCollaborationResult>;

export class ForgeCollaborationTransportError extends S.TaggedErrorClass<ForgeCollaborationTransportError>()(
  "ForgeCollaborationTransportError",
  { operation: S.String, cause: S.Defect() },
) {}

export interface ForgeCollaborationReaderInterface {
  readonly read: (
    request: ForgeCollaborationRequest,
    authorizationCookie: string | undefined,
  ) => Effect.Effect<ForgeCollaborationResult, ForgeCollaborationTransportError>;
}

export class ForgeCollaborationReader extends Context.Service<
  ForgeCollaborationReader,
  ForgeCollaborationReaderInterface
>()("@openagents.com/ForgeCollaborationReader") {}

export const makeForgeCollaborationReaderTestLayer = (
  read: ForgeCollaborationReaderInterface["read"],
) => Layer.succeed(ForgeCollaborationReader, ForgeCollaborationReader.of({ read }));
