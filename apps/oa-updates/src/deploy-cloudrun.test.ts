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
      OA_SIGNING_SECRET: "fixture-signing-key:1",
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
  test("Desktop-v2-only update (no seed) builds incrementally from the ready image digest and preserves existing mobile env/secrets", () => {
    const { build, deploy } = deployCommands({
      OA_RELEASE_SET_BUCKET: "openagents-release-fixture",
      OA_RELEASE_SET_PINS_PATH: "/app/openagents-desktop-dist/release-set-pins.json",
    });
    expect(build).toContain("cloudbuild.incremental.yaml");
    expect(build).toContain(
      `_BASE_IMAGE=${immutableBase},_IMAGE=${immutableBase.split("@")[0]}:source-${"c".repeat(40)}`,
    );
    expect(deploy).toContain(`${immutableBase.split("@")[0]}@${immutableBuiltDigest}`);
    expect(deploy).not.toContain("--source");
    expect(deploy).toContain("--update-env-vars");
    expect(deploy).not.toContain("--set-env-vars");
    expect(deploy).toContain("--update-secrets");
    const env = deploy[deploy.indexOf("--update-env-vars") + 1];
    expect(env).toContain("OA_RELEASE_SET_BUCKET=openagents-release-fixture");
    expect(env).not.toContain("OA_SEED_DIST=");
  });

  test("bare code-only update (no seed, no ReleaseSet config change) also builds incrementally", () => {
    const { build, deploy } = deployCommands({});
    expect(build).toContain("cloudbuild.incremental.yaml");
    expect(build).toContain(
      `_BASE_IMAGE=${immutableBase},_IMAGE=${immutableBase.split("@")[0]}:source-${"c".repeat(40)}`,
    );
    expect(deploy).not.toContain("--source");
  });

  test("mobile-only update (OA_SEED_DIST) does a full --source rebuild so the fresh export actually ships, and preserves existing Desktop v2 env", () => {
    const { build, deploy } = deployCommands({
      OA_SEED_DIST: "/app/dist",
      OA_SEED_RUNTIME: "fixture-runtime",
      OA_SEED_BRANCH: "openagents-production",
    });
    expect(build).toEqual([]);
    expect(deploy).toContain("--source");
    expect(deploy).not.toContain("--image");
    expect(deploy).toContain("--update-env-vars");
    const env = deploy[deploy.indexOf("--update-env-vars") + 1];
    expect(env).toContain("OA_SEED_DIST=/app/dist");
    expect(env).not.toContain("OA_RELEASE_SET_BUCKET=");
  });

  test("legacy Desktop v1 seed update (OA_DESKTOP_RELEASES_DIST) also does a full --source rebuild", () => {
    const { build, deploy } = deployCommands({
      OA_DESKTOP_RELEASES_DIST: "/app/desktop-dist",
    });
    expect(build).toEqual([]);
    expect(deploy).toContain("--source");
    const env = deploy[deploy.indexOf("--update-env-vars") + 1];
    expect(env).toContain("OA_DESKTOP_RELEASES_DIST=/app/desktop-dist");
  });

  test("explicit OA_UPDATES_DEPLOY_MODE=full forces a full rebuild even with no seed present", () => {
    const { build, deploy } = deployCommands({ OA_UPDATES_DEPLOY_MODE: "full" });
    expect(build).toEqual([]);
    expect(deploy).toContain("--source");
  });

  test("explicit OA_UPDATES_DEPLOY_MODE=incremental forces the incremental path even with only ReleaseSet config", () => {
    const { build, deploy } = deployCommands({
      OA_UPDATES_DEPLOY_MODE: "incremental",
      OA_RELEASE_SET_BUCKET: "openagents-release-fixture",
      OA_RELEASE_SET_PINS_PATH: "/app/openagents-desktop-dist/release-set-pins.json",
    });
    expect(build).toContain("cloudbuild.incremental.yaml");
    expect(deploy).not.toContain("--source");
  });

  test("refuses to combine a forced incremental mode with a seed publish instead of silently dropping the seed", () => {
    expect(() =>
      deployCommands({
        OA_UPDATES_DEPLOY_MODE: "incremental",
        OA_SEED_DIST: "/app/dist",
        OA_SEED_RUNTIME: "fixture-runtime",
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
