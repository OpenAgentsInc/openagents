import { Effect, Layer } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  makeOrganizationMembershipAuthorityState,
  OrganizationMembershipAuthority,
  OrganizationMembershipAuthorityLive,
  OrganizationMembershipStateStore,
  type OrganizationMembershipAuthorityState,
} from "../src/organization-membership-authority.ts";
import { decodeOrganizationMembership } from "../src/generated.ts";

const membership = decodeOrganizationMembership({
  contractVersion: "openagents.all_work_boundary.v1",
  membershipRef: "membership:openagents:owner",
  accountRef: "account:omega:owner",
  accountGeneration: 4,
  effectivePrincipalRef: "principal:omega:local-owner",
  organizationRef: "organization:openagents",
  displayName: "OpenAgents",
  sourceRevision: 1,
  state: "verified",
  observedAt: "2026-08-03T17:30:00Z",
});

const state = makeOrganizationMembershipAuthorityState({
  revision: 1,
  observedAt: "2026-08-03T17:30:00Z",
  memberships: [membership],
});

const layer = (initial: OrganizationMembershipAuthorityState) =>
  OrganizationMembershipAuthorityLive.pipe(
    Layer.provide(
      Layer.succeed(OrganizationMembershipStateStore, {
        load: Effect.succeed(initial),
      }),
    ),
  );

const effectIt = (name: string, test: () => Effect.Effect<void, unknown, never>) =>
  it(name, () => Effect.runPromise(test()));

describe("OrganizationMembershipAuthority", () => {
  effectIt("returns only an exact account, principal, and generation match", () =>
    Effect.gen(function* () {
      const authority = yield* OrganizationMembershipAuthority;
      const result = yield* authority.read({
        accountRef: "account:omega:owner",
        accountGeneration: 4,
        effectivePrincipalRef: "principal:omega:local-owner",
      });
      expect(result.ledger.memberships).toEqual([membership]);

      const staleGeneration = yield* authority.read({
        accountRef: "account:omega:owner",
        accountGeneration: 3,
        effectivePrincipalRef: "principal:omega:local-owner",
      });
      expect(staleGeneration.ledger.memberships).toEqual([]);
    }).pipe(Effect.provide(layer(state))),
  );

  it("refuses duplicate membership identities before persistence", () => {
    try {
      makeOrganizationMembershipAuthorityState({
        revision: 2,
        observedAt: "2026-08-03T17:31:00Z",
        memberships: [membership, membership],
      });
      throw new Error("duplicate membership was accepted");
    } catch (error) {
      expect(error).toMatchObject({ reason: "invalid_state", detail: "duplicate_membership" });
    }
  });
});
