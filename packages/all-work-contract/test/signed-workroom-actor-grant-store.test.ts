import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { Effect } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  emptySignedWorkroomActorGrantState,
  fileSignedWorkroomActorGrantResolverLayer,
  initializeFileSignedWorkroomActorGrantState,
  makeSignedWorkroomActorGrantState,
  provisionFileSignedWorkroomActorGrantState,
  SignedWorkroomActorGrantResolver,
  type SignedWorkroomActorGrant,
} from "../src/index.ts";

const grant = (state: "active" | "revoked" = "active"): SignedWorkroomActorGrant => ({
  grantRef: "delegation-grant:omega-216:3",
  issuerPrincipalRef: `principal:nostr:${"7".repeat(64)}`,
  actorRef: "principal:agent:omega-coder",
  signerPubkey: "7".repeat(64),
  purpose: "purpose:signed-workroom:project-activity",
  workroomRef: "workroom:omega:208",
  workRef: "work:github:openagentsinc-omega:216",
  activityKinds: ["agent_activity"],
  audiences: ["workroom"],
  privacyClasses: ["workroom"],
  generation: 3,
  validFrom: "2026-08-03T09:00:00Z",
  expiresAt: "2026-08-03T11:00:00Z",
  state,
  evidenceRefs: ["evidence:actor-grant:omega-216:3"],
});

describe("signed Workroom actor grant store", () => {
  it("provisions and revokes authoritative grants with a revision fence", async () => {
    const root = await mkdtemp(path.join(tmpdir(), "openagents-signed-workroom-grants-"));
    try {
      await Effect.runPromise(
        initializeFileSignedWorkroomActorGrantState(
          root,
          emptySignedWorkroomActorGrantState("2026-08-03T09:00:00Z"),
        ),
      );
      const active = makeSignedWorkroomActorGrantState({
        revision: 1,
        updatedAt: "2026-08-03T09:01:00Z",
        grants: [grant()],
      });
      await Effect.runPromise(provisionFileSignedWorkroomActorGrantState(root, 0, active));
      const resolve = (grantRef: string) =>
        Effect.runPromise(
          Effect.gen(function* () {
            const resolver = yield* SignedWorkroomActorGrantResolver;
            return yield* resolver.resolve(grantRef);
          }).pipe(Effect.provide(fileSignedWorkroomActorGrantResolverLayer(root))),
        );
      await expect(resolve(grant().grantRef)).resolves.toMatchObject({ state: "active" });
      await expect(
        Effect.runPromise(provisionFileSignedWorkroomActorGrantState(root, 0, active)),
      ).rejects.toMatchObject({ reason: "revision_conflict" });

      const revoked = makeSignedWorkroomActorGrantState({
        revision: 2,
        updatedAt: "2026-08-03T09:02:00Z",
        grants: [grant("revoked")],
      });
      await Effect.runPromise(provisionFileSignedWorkroomActorGrantState(root, 1, revoked));
      await expect(resolve(grant().grantRef)).resolves.toMatchObject({ state: "revoked" });
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});
