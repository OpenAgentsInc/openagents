/* oxlint-disable openagents/no-manual-effect-runtime-in-tests -- isolated service-layer proof. */
import { execFile } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";

import { makeMemoryBlobStore } from "@openagentsinc/oa-infra/blob-store-memory";
import { ForgeMergeOutcomeReceiptDraft } from "@openagentsinc/forge-protocol";
import { Effect, Layer, ManagedRuntime } from "effect";
import { finalizeEvent, generateSecretKey } from "nostr-effect/pure";
import { afterEach, describe, expect, test } from "vitest";

import { ForgeGitAdmission, makeMemoryAdmissionLayer } from "./admission.js";
import { makeTestConfiguration } from "./config.js";
import { ForgeGitProjector, layerProjector } from "./projector.js";
import { makeRepositoryLayer } from "./repository.js";

const paths: Array<string> = [];
const execFileAsync = promisify(execFile);

afterEach(async () => {
  await Promise.all(paths.splice(0).map((path) => rm(path, { force: true, recursive: true })));
});

describe("Forge NIP-34 admission projector", () => {
  test("provisions only a verified 30617 from the declared maintainer", async () => {
    const root = await mkdtemp(join(tmpdir(), "oa-forge-projector-"));
    paths.push(root);
    const configuration = makeTestConfiguration({
      gitBinary: "git",
      maxReceivePackBytes: 1024 * 1024,
      mirrorEnabled: false,
      repositoryRoot: root,
    });
    const admissionLayer = makeMemoryAdmissionLayer({
      admittedRepositories: [],
    });
    const projectorLayer = layerProjector.pipe(
      Layer.provide(
        Layer.mergeAll(admissionLayer, makeRepositoryLayer(configuration, makeMemoryBlobStore())),
      ),
    );
    const runtime = ManagedRuntime.make(Layer.merge(admissionLayer, projectorLayer));
    const secret = generateSecretKey();
    const event = finalizeEvent(
      {
        content: "",
        created_at: 1_785_000_000,
        kind: 30617,
        tags: [
          ["d", "demo"],
          ["clone", "https://openagents.test/git/tenant/demo.git"],
          ["maintainers"],
        ],
      },
      secret,
    );
    event.tags[2]?.push(event.pubkey);
    // finalize again after adding the signer key to the event's maintainer set.
    const signed = finalizeEvent(
      {
        content: "",
        created_at: event.created_at,
        kind: 30617,
        tags: event.tags,
      },
      secret,
    );
    const result = await runtime.runPromise(
      Effect.gen(function* () {
        const projector = yield* ForgeGitProjector;
        const disposition = yield* projector.project({
          actorBindingRef: "forge_actor.human.invited",
          event: signed,
          repositoryRef: "demo",
          tenantRef: "tenant",
        });
        const admission = yield* ForgeGitAdmission;
        const projected = yield* admission.listProjectedEvents({
          repositoryRef: "demo",
          tenantRef: "tenant",
        });
        return { disposition, projected };
      }),
    );
    expect(result.disposition).toBe("authorized");
    expect(result.projected).toHaveLength(1);
    expect(result.projected[0]).toMatchObject({
      actorBindingRef: "forge_actor.human.invited",
      authorPubkey: signed.pubkey,
      eventId: signed.id,
      kind: 30617,
    });
    const comment = finalizeEvent(
      {
        content: "Admitted NIP-22 review conversation.",
        created_at: signed.created_at + 1,
        kind: 1111,
        tags: [
          ["a", `30617:${signed.pubkey}:demo`],
          ["E", signed.id],
        ],
      },
      generateSecretKey(),
    );
    const commentResult = await runtime.runPromise(
      Effect.gen(function* () {
        const projector = yield* ForgeGitProjector;
        const disposition = yield* projector.project({
          actorBindingRef: "forge_actor.human.invited",
          event: comment,
          repositoryRef: "demo",
          tenantRef: "tenant",
        });
        const admission = yield* ForgeGitAdmission;
        return {
          disposition,
          projected: yield* admission.listProjectedEvents({
            repositoryRef: "demo",
            tenantRef: "tenant",
          }),
        };
      }),
    );
    expect(commentResult.disposition).toBe("authorized");
    expect(commentResult.projected.at(-1)).toMatchObject({
      eventId: comment.id,
      kind: 1111,
    });

    await expect(
      runtime.runPromise(
        Effect.gen(function* () {
          const projector = yield* ForgeGitProjector;
          return yield* projector.project({
            actorBindingRef: "forge_actor.human.invited",
            event: {
              ...(JSON.parse(JSON.stringify(signed)) as typeof signed),
              content: "tampered",
            },
            repositoryRef: "demo",
            tenantRef: "tenant",
          });
        }),
      ),
    ).rejects.toMatchObject({ code: "forge_git_nostr_signature_invalid" });
    await runtime.dispose();
  });

  test("holds an object-first pull-request event in purgatory without projecting it", async () => {
    const root = await mkdtemp(join(tmpdir(), "oa-forge-projector-"));
    paths.push(root);
    const configuration = makeTestConfiguration({
      gitBinary: "git",
      maxReceivePackBytes: 1024 * 1024,
      mirrorEnabled: false,
      repositoryRoot: root,
    });
    const runtime = ManagedRuntime.make(
      layerProjector.pipe(
        Layer.provide(
          Layer.mergeAll(
            makeMemoryAdmissionLayer({
              admittedRepositories: [{ repositoryRef: "demo", tenantRef: "tenant" }],
            }),
            makeRepositoryLayer(configuration, makeMemoryBlobStore()),
          ),
        ),
      ),
    );
    const event = finalizeEvent(
      {
        content: "From aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa Mon Sep 17 00:00:00 2001\n",
        created_at: 1_785_000_000,
        kind: 1618,
        tags: [
          ["a", "30617:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:demo"],
        ],
      },
      generateSecretKey(),
    );
    const announcementSecret = generateSecretKey();
    const announcement = finalizeEvent(
      {
        content: "",
        created_at: 1_785_000_000,
        kind: 30617,
        tags: [["d", "demo"], ["maintainers"]],
      },
      announcementSecret,
    );
    announcement.tags[1]?.push(announcement.pubkey);
    const signedAnnouncement = finalizeEvent(
      {
        content: "",
        created_at: announcement.created_at,
        kind: 30617,
        tags: announcement.tags,
      },
      announcementSecret,
    );
    const result = await runtime.runPromise(
      Effect.gen(function* () {
        const projector = yield* ForgeGitProjector;
        yield* projector.project({
          actorBindingRef: "forge_actor.human.invited",
          event: signedAnnouncement,
          repositoryRef: "demo",
          tenantRef: "tenant",
        });
        return yield* projector.project({
          actorBindingRef: "forge_actor.human.invited",
          event,
          repositoryRef: "demo",
          tenantRef: "tenant",
        });
      }),
    );
    expect(result).toBe("purgatory");
    await runtime.dispose();
  });

  test("refuses a signed ref state until its exact durable gate receipt exists", async () => {
    const root = await mkdtemp(join(tmpdir(), "oa-forge-projector-"));
    paths.push(root);
    const configuration = makeTestConfiguration({
      gitBinary: "git",
      maxReceivePackBytes: 1024 * 1024,
      mirrorEnabled: false,
      repositoryRoot: root,
    });
    const runtime = ManagedRuntime.make(
      layerProjector.pipe(
        Layer.provide(
          Layer.mergeAll(
            makeMemoryAdmissionLayer({ admittedRepositories: [] }),
            makeRepositoryLayer(configuration, makeMemoryBlobStore()),
          ),
        ),
      ),
    );
    const secret = generateSecretKey();
    const draftAnnouncement = finalizeEvent(
      {
        content: "",
        created_at: 1_785_000_000,
        kind: 30617,
        tags: [["d", "demo"], ["maintainers"]],
      },
      secret,
    );
    draftAnnouncement.tags[1]?.push(draftAnnouncement.pubkey);
    const announcement = finalizeEvent(
      {
        content: "",
        created_at: draftAnnouncement.created_at,
        kind: 30617,
        tags: draftAnnouncement.tags,
      },
      secret,
    );
    await runtime.runPromise(
      Effect.gen(function* () {
        const projector = yield* ForgeGitProjector;
        yield* projector.project({
          actorBindingRef: "forge_actor.human.invited",
          event: announcement,
          repositoryRef: "demo",
          tenantRef: "tenant",
        });
      }),
    );
    const payloadPath = join(root, "payload.txt");
    await writeFile(payloadPath, "durable gate proof\n");
    const barePath = join(root, "tenant", "demo.git");
    const object = (
      await execFileAsync("git", ["--git-dir", barePath, "hash-object", "-w", payloadPath])
    ).stdout.trim();
    const receiptRef = "receipt.forge.merge.exact-tip";
    const state = finalizeEvent(
      {
        content: "",
        created_at: 1_785_000_001,
        kind: 30618,
        tags: [
          ["d", "demo"],
          ["refs/heads/main", object],
          ["forge-merge-receipt", "refs/heads/main", receiptRef],
        ],
      },
      secret,
    );
    await expect(
      runtime.runPromise(
        Effect.gen(function* () {
          const projector = yield* ForgeGitProjector;
          return yield* projector.project({
            actorBindingRef: "forge_actor.human.invited",
            event: state,
            repositoryRef: "demo",
            tenantRef: "tenant",
          });
        }),
      ),
    ).rejects.toMatchObject({ code: "forge_git_merge_receipt_refused" });
    await runtime.dispose();
    const draft = ForgeMergeOutcomeReceiptDraft.make({
      authorityGeneration: 1,
      changeRef: "change.forge.exact-tip",
      decidedAt: "2026-07-26T00:00:00.000Z",
      gateResults: [],
      maintainerBindingRef: "forge_actor.human.invited",
      newObjectId: object,
      oldObjectId: "0".repeat(40),
      policyVersion: "policy.forge.v1",
      proposalEventIds: ["proposal.forge.1"],
      receiptRef,
      redacted: true,
      repositoryRef: "demo",
      schema: "openagents.forge.merge.outcome.receipt.v1",
      targetRef: "refs/heads/main",
      tenantRef: "tenant",
    });
    const authorizedAdmissionLayer = makeMemoryAdmissionLayer({
      admittedRepositories: [],
      preparedMergeReceipts: [draft],
    });
    const authorizedRepositoryLayer = makeRepositoryLayer(configuration, makeMemoryBlobStore());
    const authorizedRuntime = ManagedRuntime.make(
      Layer.mergeAll(
        authorizedAdmissionLayer,
        layerProjector.pipe(
          Layer.provide(Layer.mergeAll(authorizedAdmissionLayer, authorizedRepositoryLayer)),
        ),
      ),
    );
    await authorizedRuntime
      .runPromise(
        Effect.gen(function* () {
          const projector = yield* ForgeGitProjector;
          yield* projector.project({
            actorBindingRef: "forge_actor.human.invited",
            event: announcement,
            repositoryRef: "demo",
            tenantRef: "tenant",
          });
          return yield* projector.project({
            actorBindingRef: "forge_actor.human.invited",
            event: state,
            repositoryRef: "demo",
            tenantRef: "tenant",
          });
        }),
      )
      .then((result) => expect(result).toBe("authorized"));
    await authorizedRuntime.runPromise(
      Effect.gen(function* () {
        const admission = yield* ForgeGitAdmission;
        const input = {
          newObjectId: object,
          repositoryRef: "demo",
          targetRef: "refs/heads/main",
          tenantRef: "tenant",
        };
        expect(yield* admission.readAppliedMergeReceiptRef(input)).toBeUndefined();
        yield* admission.recordCommittedReceive({
          repositoryRef: "demo",
          stateEventIds: [state.id],
          tenantRef: "tenant",
        });
        expect(yield* admission.readAppliedMergeReceiptRef(input)).toBe(receiptRef);
      }),
    );
    await expect(
      authorizedRuntime.runPromise(
        Effect.gen(function* () {
          const projector = yield* ForgeGitProjector;
          return yield* projector.project({
            actorBindingRef: "forge_actor.human.invited",
            event: state,
            repositoryRef: "demo",
            tenantRef: "tenant",
          });
        }),
      ),
    ).rejects.toMatchObject({ code: "forge_git_merge_receipt_refused" });
    await authorizedRuntime.dispose();
  });
});
