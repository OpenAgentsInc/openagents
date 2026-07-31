import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(fileURLToPath(new URL(".", import.meta.url)), "../..");

test("Sarah worker Cloud Build stays on the existing digest-pinned production lane", async () => {
  const [
    configuration,
    dockerfile,
    dockerIgnore,
    cloudBuildIgnore,
    buildScript,
    deployerBuildScript,
    deployerDockerfile,
  ] =
    await Promise.all([
      readFile(resolve(repositoryRoot, "docker/cloud/cloudbuild-sarah-livekit-agent.yaml"), "utf8"),
      readFile(resolve(repositoryRoot, "apps/sarah-livekit-agent/Dockerfile"), "utf8"),
      readFile(
        resolve(repositoryRoot, "apps/sarah-livekit-agent/Dockerfile.dockerignore"),
        "utf8",
      ),
      readFile(resolve(repositoryRoot, ".gcloudignore.sarah-livekit-agent"), "utf8"),
      readFile(resolve(repositoryRoot, "scripts/cloud/build-sarah-livekit-agent.sh"), "utf8"),
      readFile(
        resolve(repositoryRoot, "scripts/cloud/build-livekit-production-deployer.sh"),
        "utf8",
      ),
      readFile(
        resolve(repositoryRoot, "infra/livekit-production/deployer.Dockerfile"),
        "utf8",
      ),
    ]);

  assert.match(
    configuration,
    /us-central1-docker\.pkg\.dev\/openagentsgemini\/oa-cloud\/sarah-livekit-agent:source-only/u,
  );
  assert.doesNotMatch(configuration, /\/livekit\/sarah-livekit-agent/u);
  assert.match(configuration, /--platform\s+- linux\/amd64/u);
  assert.match(configuration, /apps\/sarah-livekit-agent\/Dockerfile/u);
  assert.equal(
    dockerfile.match(
      /node:24\.13\.1-bookworm-slim@sha256:85a395c77b811fa7f5b5e4aa69cd6eb4c3b80c7f1a8e34704dc0ce061e5b404e/gu,
    )?.length,
    2,
  );
  assert.doesNotMatch(dockerfile, /^FROM node:24\.13\.1-bookworm-slim$/mu);
  assert.equal(
    dockerfile.match(/apt-get install --yes --no-install-recommends ca-certificates/gu)?.length,
    2,
  );
  assert.match(dockerfile, /pnpm install --frozen-lockfile --ignore-scripts/u);
  assert.match(
    dockerfile,
    /deploy\s+\\\n\s+--legacy --prod --ignore-scripts --config\.allowUnusedPatches=true/u,
  );
  assert.match(buildScript, /status --porcelain --untracked-files=normal/u);
  assert.match(deployerBuildScript, /status --porcelain --untracked-files=normal/u);
  assert.match(deployerDockerfile, /releases\/download\/v4\.53\.3\/yq_linux_amd64/u);
  assert.match(
    deployerDockerfile,
    /fa52a4e758c63d38299163fbdd1edfb4c4963247918bf9c1c5d31d84789eded4/u,
  );
  assert.match(deployerDockerfile, /yq --version/u);
  assert.match(buildScript, /revision\}" != "\$\{remote_revision/u);
  assert.match(deployerBuildScript, /revision\}" != "\$\{remote_revision/u);
  assert.doesNotMatch(buildScript, /branch --show-current/u);
  assert.doesNotMatch(deployerBuildScript, /branch --show-current/u);
  assert.match(buildScript, /\.gcloudignore\.sarah-livekit-agent/u);
  assert.match(buildScript, /--ignore-file "\$\{ignore_file\}"/u);
  assert.match(cloudBuildIgnore, /^\*\*$/mu);
  assert.match(cloudBuildIgnore, /^!apps\/sarah-livekit-agent\/\*\*$/mu);
  assert.match(cloudBuildIgnore, /^!packages\/audio-contract\/\*\*$/mu);
  assert.match(cloudBuildIgnore, /^!scripts\/node-test-suites\.mjs$/mu);
  assert.doesNotMatch(cloudBuildIgnore, /oa-updates|openagents-desktop|openagents-mobile/u);
  for (const workspaceManifest of [
    "packages/nip90/package.json",
    "packages/runtime-platform/package.json",
    "types/openagents-platform/package.json",
    "types/vite-plus-matchers/package.json",
  ]) {
    assert.match(cloudBuildIgnore, new RegExp(`^!${workspaceManifest}$`, "mu"));
    assert.match(dockerIgnore, new RegExp(`^!${workspaceManifest}$`, "mu"));
  }
  assert.match(dockerIgnore, /^apps\/sarah-livekit-agent\/node_modules$/mu);
  assert.match(dockerIgnore, /^packages\/audio-contract\/node_modules$/mu);
  assert.match(buildScript, /--async/u);
  assert.match(buildScript, /image_summary\.digest/u);
  assert.match(buildScript, /SARAH_LIVEKIT_AGENT_IMAGE=/u);
});
