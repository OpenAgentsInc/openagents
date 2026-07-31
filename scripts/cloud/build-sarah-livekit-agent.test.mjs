import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(fileURLToPath(new URL(".", import.meta.url)), "../..");

test("Sarah worker Cloud Build stays on the existing digest-pinned production lane", async () => {
  const [configuration, dockerfile, cloudBuildIgnore, buildScript] = await Promise.all([
    readFile(resolve(repositoryRoot, "docker/cloud/cloudbuild-sarah-livekit-agent.yaml"), "utf8"),
    readFile(resolve(repositoryRoot, "apps/sarah-livekit-agent/Dockerfile"), "utf8"),
    readFile(resolve(repositoryRoot, ".gcloudignore.sarah-livekit-agent"), "utf8"),
    readFile(resolve(repositoryRoot, "scripts/cloud/build-sarah-livekit-agent.sh"), "utf8"),
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
  assert.match(buildScript, /status --porcelain --untracked-files=normal/u);
  assert.match(buildScript, /\.gcloudignore\.sarah-livekit-agent/u);
  assert.match(buildScript, /--ignore-file "\$\{ignore_file\}"/u);
  assert.match(cloudBuildIgnore, /^\*\*$/mu);
  assert.match(cloudBuildIgnore, /^!apps\/sarah-livekit-agent\/\*\*$/mu);
  assert.match(cloudBuildIgnore, /^!packages\/audio-contract\/\*\*$/mu);
  assert.match(cloudBuildIgnore, /^!scripts\/node-test-suites\.mjs$/mu);
  assert.doesNotMatch(cloudBuildIgnore, /oa-updates|openagents-desktop|openagents-mobile/u);
  assert.match(buildScript, /--async/u);
  assert.match(buildScript, /image_summary\.digest/u);
  assert.match(buildScript, /SARAH_LIVEKIT_AGENT_IMAGE=/u);
});
