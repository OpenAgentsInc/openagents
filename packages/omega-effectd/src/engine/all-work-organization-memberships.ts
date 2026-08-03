import {
  emptyOrganizationMembershipAuthorityState,
  fileOrganizationMembershipStateStoreLayer,
  initializeFileOrganizationMembershipState,
  OrganizationMembershipAuthority,
  OrganizationMembershipAuthorityLive,
} from "@openagentsinc/all-work-contract";
import { Effect, Layer } from "effect";

const authorityLayer = (dataRoot: string) =>
  OrganizationMembershipAuthorityLive.pipe(
    Layer.provide(fileOrganizationMembershipStateStoreLayer(dataRoot)),
  );

export const bootstrapAllWorkOrganizationMemberships = Effect.fn(
  "OmegaEffectd.bootstrapAllWorkOrganizationMemberships",
)(function* (dataRoot: string) {
  yield* initializeFileOrganizationMembershipState(
    dataRoot,
    emptyOrganizationMembershipAuthorityState("2026-08-03T17:30:00Z"),
  );
});

export const readAllWorkOrganizationMemberships = (dataRoot: string, input: unknown) =>
  Effect.gen(function* () {
    yield* bootstrapAllWorkOrganizationMemberships(dataRoot);
    const authority = yield* OrganizationMembershipAuthority;
    return yield* authority.read(input);
  }).pipe(Effect.provide(authorityLayer(dataRoot)));
