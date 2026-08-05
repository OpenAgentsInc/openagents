import { execFileSync } from "node:child_process";

import { describe, expect, test } from "vite-plus/test";

const script = new URL("../scripts/deploy-cloudrun.sh", import.meta.url).pathname;

const immutableBase = `us-central1-docker.pkg.dev/openagents/updates/oa-updates@sha256:${"a".repeat(64)}`;
const immutableBuiltDigest = `sha256:${"b".repeat(64)}`;

const deployCommands = (
  environment: Record<string, string>,
): {
  readonly build: readonly string[];
  readonly deploy: readonly string[];
} => {
  const lines = execFileSync("bash", [script], {
    encoding: "utf8",
    env: {
      PATH: process.env.PATH ?? "/usr/bin:/bin",
      OA_UPDATES_DEPLOY_DRY_RUN: "1",
      OA_UPDATES_BASE_IMAGE: immutableBase,
      OA_UPDATES_BUILT_IMAGE_DIGEST: immutableBuiltDigest,
      OA_UPDATES_SOURCE_REVISION: "c".repeat(40),
      OA_PUBLIC_URL: "https://updates.openagents.test",
      ...environment,
    },
  })
    .trim()
    .split("\n");
  return {
    build: lines.filter((line) => line.startsWith("BUILD_ARG=")).map((line) => line.slice(10)),
    deploy: lines.filter((line) => line.startsWith("DEPLOY_ARG=")).map((line) => line.slice(11)),
  };
};

describe("oa-updates additive Cloud Run deploy command", () => {
  test("bare code-only update (no seed) builds incrementally from the ready image digest and preserves the existing Pylon env", () => {
    const { build, deploy } = deployCommands({});
    expect(build).toContain("cloudbuild.incremental.yaml");
    expect(build).toContain(
      "projects/openagentsgemini/serviceAccounts/oa-cloud-image-builder@openagentsgemini.iam.gserviceaccount.com",
    );
    expect(build).toContain("gs://openagentsgemini-cloud-build-source/source");
    expect(build).toContain(
      `_BASE_IMAGE=${immutableBase},_IMAGE=${immutableBase.split("@")[0]}:source-${"c".repeat(40)}`,
    );
    expect(deploy).toContain(`${immutableBase.split("@")[0]}@${immutableBuiltDigest}`);
    expect(deploy).not.toContain("--source");
    // Additive env updates are what keep the already-attached
    // OA_PYLON_RELEASES_DIST alive across a code push.
    expect(deploy).toContain("--update-env-vars");
    expect(deploy).not.toContain("--set-env-vars");
    const env = deploy[deploy.indexOf("--update-env-vars") + 1];
    expect(env).toContain("OA_PUBLIC_URL=https://updates.openagents.test");
  });

  test("a Pylon release publish (OA_PYLON_RELEASES_DIST) does a full --source rebuild so the staged binaries actually ship", () => {
    const { build, deploy } = deployCommands({
      OA_PYLON_RELEASES_DIST: "/app/pylon-dist",
    });
    expect(build).toEqual([]);
    expect(deploy).toContain("--source");
    expect(deploy).not.toContain("--image");
    expect(deploy).toContain("--update-env-vars");
    const env = deploy[deploy.indexOf("--update-env-vars") + 1];
    expect(env).toContain("OA_PYLON_RELEASES_DIST=/app/pylon-dist");
  });

  // The desktop app and its feeds were deleted, and the Expo mobile OTA
  // surface was retired on 2026-08-05 (#9325). The deploy must no longer
  // assert any desktop or mobile seed env var, and must not resurrect one from
  // a stale exported environment.
  test("never emits retired mobile OTA or desktop env vars, even when they are exported", () => {
    const { deploy } = deployCommands({
      OA_SEED_DIST: "/app/dist",
      OA_SEED_RUNTIME: "stale-runtime",
      OA_SEED_PLATFORM: "ios",
      OA_SEED_BRANCH: "openagents-production",
      OA_SEED_EXPO_CLIENT_PATH: "/app/dist/expo-client.json",
      OA_DESKTOP_RELEASES_DIST: "/app/desktop-dist",
      OA_OPENAGENTS_DESKTOP_RELEASE_DIST: "/app/openagents-desktop-dist",
      OA_DESKTOP_OTA_DIR: "/app/desktop-ota",
      OA_RELEASE_SET_BUCKET: "openagents-release-fixture",
    });
    const env = deploy[deploy.indexOf("--update-env-vars") + 1];
    for (const retired of [
      "OA_SEED_DIST",
      "OA_SEED_RUNTIME",
      "OA_SEED_PLATFORM",
      "OA_SEED_BRANCH",
      "OA_SEED_EXPO_CLIENT_PATH",
      "OA_DESKTOP_RELEASES_DIST",
      "OA_OPENAGENTS_DESKTOP_RELEASE_DIST",
      "OA_DESKTOP_OTA_DIR",
      "OA_RELEASE_SET_BUCKET",
      "OA_RELEASE_SET_PINS_PATH",
    ]) {
      expect(env).not.toContain(retired);
    }
    // A retired-surface export is no longer a seed publish, so it stays on the
    // non-destructive incremental path.
    expect(deploy).not.toContain("--source");
  });

  // The OA_SIGNING_KEY secret signed Expo manifests only. With mobile OTA
  // retired the running service must hold no signing material; Pylon release
  // signatures are minted offline and verified against the pinned public key.
  test("never attaches the retired OTA manifest code-signing secret", () => {
    const { deploy } = deployCommands({
      OA_SIGNING_SECRET: "oa-updates-codesign-key:latest",
    });
    expect(deploy).not.toContain("--update-secrets");
    expect(deploy).not.toContain("--set-secrets");
    expect(deploy.join(" ")).not.toContain("OA_SIGNING_KEY");
  });

  test("explicit OA_UPDATES_DEPLOY_MODE=full forces a full rebuild even with no seed present", () => {
    const { build, deploy } = deployCommands({ OA_UPDATES_DEPLOY_MODE: "full" });
    expect(build).toEqual([]);
    expect(deploy).toContain("--source");
  });

  test("explicit OA_UPDATES_DEPLOY_MODE=incremental forces the incremental path", () => {
    const { build, deploy } = deployCommands({
      OA_UPDATES_DEPLOY_MODE: "incremental",
    });
    expect(build).toContain("cloudbuild.incremental.yaml");
    expect(deploy).not.toContain("--source");
  });

  test("refuses to combine a forced incremental mode with a seed publish instead of silently dropping the seed", () => {
    expect(() =>
      deployCommands({
        OA_UPDATES_DEPLOY_MODE: "incremental",
        OA_PYLON_RELEASES_DIST: "/app/pylon-dist",
      }),
    ).toThrow();
  });

  test("refuses an unknown OA_UPDATES_DEPLOY_MODE value", () => {
    expect(() => deployCommands({ OA_UPDATES_DEPLOY_MODE: "bogus" })).toThrow();
  });

  test.each([
    ["missing base", { OA_UPDATES_BASE_IMAGE: "" }],
    ["mutable base tag", { OA_UPDATES_BASE_IMAGE: "registry.invalid/oa-updates:latest" }],
    ["missing built digest", { OA_UPDATES_BUILT_IMAGE_DIGEST: "" }],
  ])("refuses %s instead of risking baked release bytes", (_name, environment) => {
    expect(() => deployCommands(environment)).toThrow();
  });
});
