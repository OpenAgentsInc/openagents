/* oxlint-disable openagents/no-manual-effect-runtime-in-tests -- isolated service-layer proof. */
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { makeMemoryBlobStore } from "@openagentsinc/oa-infra/blob-store-memory";
import { Effect, Layer, ManagedRuntime } from "effect";
import { finalizeEvent, generateSecretKey } from "nostr-effect/pure";
import { afterEach, describe, expect, test } from "vitest";

import { makeMemoryAdmissionLayer } from "./admission.js";
import { makeTestConfiguration } from "./config.js";
import { ForgeGitProjector, layerProjector } from "./projector.js";
import { makeRepositoryLayer } from "./repository.js";

const paths: Array<string> = [];

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
      { content: "", created_at: event.created_at, kind: 30617, tags: event.tags },
      secret,
    );
    const result = await runtime.runPromise(
      Effect.gen(function* () {
        const projector = yield* ForgeGitProjector;
        return yield* projector.project({
          actorBindingRef: "forge_actor.human.invited",
          event: signed,
          repositoryRef: "demo",
          tenantRef: "tenant",
        });
      }),
    );
    expect(result).toBe("authorized");

    await expect(
      runtime.runPromise(
        Effect.gen(function* () {
          const projector = yield* ForgeGitProjector;
          return yield* projector.project({
            actorBindingRef: "forge_actor.human.invited",
            event: { ...(JSON.parse(JSON.stringify(signed)) as typeof signed), content: "tampered" },
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
        tags: [["a", "30617:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:demo"]],
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
      { content: "", created_at: announcement.created_at, kind: 30617, tags: announcement.tags },
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
});
