import { Schema as S } from "effect";

export const SARAH_PRINCIPAL_SCHEMA = "openagents.sarah.principal.v1" as const;
export const SARAH_AUTHORITY_PROFILE_REF = "openagents.sarah-owner-orchestrator" as const;
export const SARAH_AUTHORITY_REVISION = 6 as const;
export const ROOT_AUTHORITY_PROFILE_REF = "openagents.owner-delegated-autonomy" as const;
export const ROOT_AUTHORITY_REVISION = 8 as const;

export const SarahRefSchema = S.Trim.check(S.isMinLength(1), S.isMaxLength(256));

export const SarahCapabilitySchema = S.Struct({
  capabilityRef: SarahRefSchema,
  label: S.String.check(S.isMinLength(1), S.isMaxLength(80)),
  mode: S.Literals(["live", "brokered", "reserved"]),
  access: S.Literals(["read", "propose", "act", "none"]),
});
export interface SarahCapability extends S.Schema.Type<typeof SarahCapabilitySchema> {}

export const SarahPrincipalProjectionSchema = S.Struct({
  schema: S.Literal(SARAH_PRINCIPAL_SCHEMA),
  principalRef: S.Literal("principal.sarah"),
  displayName: S.Literal("Sarah"),
  role: S.Literal("Owner orchestrator"),
  threadRef: SarahRefSchema,
  authorityProfileRef: S.Literal(SARAH_AUTHORITY_PROFILE_REF),
  authorityRevision: S.Literal(SARAH_AUTHORITY_REVISION),
  rootAuthorityProfileRef: S.Literal(ROOT_AUTHORITY_PROFILE_REF),
  rootAuthorityRevision: S.Literal(ROOT_AUTHORITY_REVISION),
  memory: S.Literals(["durable_cited", "unavailable"]),
  capabilities: S.Array(SarahCapabilitySchema),
});
export interface SarahPrincipalProjection extends S.Schema.Type<
  typeof SarahPrincipalProjectionSchema
> {}

export const SarahPrincipalApiResponseSchema = S.Struct({
  ok: S.Literal(true),
  routeRef: S.Literal("route.mobile.sarah.principal.v1"),
  principal: SarahPrincipalProjectionSchema,
});
export interface SarahPrincipalApiResponse extends S.Schema.Type<
  typeof SarahPrincipalApiResponseSchema
> {}

export const sanitizeSarahConversationResponse = (value: string): string =>
  value
    .replace(/\s*\[source\.[^\]\n]{1,512}\]/gi, "")
    .replace(/[ \t]+([.,;:!?])/g, "$1")
    .replace(/[ \t]{2,}/g, " ")
    .replace(/\n[ \t]+/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
