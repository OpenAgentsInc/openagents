/** Exact local source and separately served private battle artifact pins. */
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile, realpath } from "node:fs/promises";
import { resolve, sep } from "node:path";
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");

/** Population samples must progress independently of direct GPU probe replies. */
export function battleStatusFreshness(samples, boundary = {}) {
  const integer = (n) => Number.isSafeInteger(n) && n >= 0;
  const { stoppedSimulationSteps, directAfterTick } = boundary;
  const points = Array.isArray(samples)
    ? samples.map((sample) => ({
        frame: sample?.frame,
        serial: sample?.battle?.stats?.sampleSerial,
        tick: sample?.battle?.stats?.simulationTicks,
      }))
    : [];
  const valid =
    points.length >= 2 &&
    points.every((point) =>
      [point.frame, point.serial, point.tick].every(integer),
    );
  const monotonic = (key) =>
    valid &&
    points.every(
      (point, index) => !index || point[key] >= points[index - 1][key],
    );
  const first = valid ? points[0] : null,
    last = valid ? points.at(-1) : null,
    lag =
      last && integer(stoppedSimulationSteps)
        ? stoppedSimulationSteps - last.tick
        : null;
  const deltas = valid
    ? Object.fromEntries(
        ["frame", "serial", "tick"].map((key) => [key, last[key] - first[key]]),
      )
    : null;
  const checks = [
    { id: "samples.valid", ok: valid },
    ...["frame", "serial", "tick"].map((key) => ({
      id: `${key}.monotonic-and-advancing`,
      ok: monotonic(key) && deltas[key] > 0,
    })),
    {
      id: "latest-status.within-stop-boundary",
      ok: lag !== null && lag >= 0 && lag <= 120,
    },
  ];
  return {
    ok: checks.every((check) => check.ok),
    samples: points.length,
    first,
    last,
    deltas,
    referenceKind: "benchmark-stop-simulation-steps",
    referenceTick: integer(stoppedSimulationSteps)
      ? stoppedSimulationSteps
      : null,
    referenceScope:
      "Frozen actual fixed-step commands at benchmark stop; this is not a GPU readback timestamp.",
    directAfterTick: integer(directAfterTick) ? directAfterTick : null,
    postStopProbeTickAdvance:
      integer(directAfterTick) && integer(stoppedSimulationSteps)
        ? directAfterTick - stoppedSimulationSteps
        : null,
    latestTickLag: lag,
    maximumTickLag: 120,
    checks,
  };
}

/** Fresh status alone is insufficient: the direct GPU probes must show combat. */
export function battleCombatProgress(before, after, count) {
  const integer = (n) => Number.isSafeInteger(n) && n >= 0;
  const valid = (probe) =>
    probe?.count === count &&
    probe.finite === true &&
    integer(probe.tick) &&
    probe.gpu?.ticks === probe.tick &&
    ["alive", "dead", "attacks", "hits", "damage", "deaths"].every((key) =>
      integer(probe.gpu?.[key]),
    );
  const probesValid = valid(before) && valid(after);
  const deltas = probesValid
    ? Object.fromEntries(
        ["ticks", "attacks", "hits", "damage", "deaths"].map((key) => [
          key,
          after.gpu[key] - before.gpu[key],
        ]),
      )
    : null;
  const checks = [
    { id: "probes.valid", ok: probesValid },
    {
      id: "population.exact",
      ok:
        probesValid &&
        before.gpu.alive + before.gpu.dead === count &&
        after.gpu.alive + after.gpu.dead === count,
    },
    ...["ticks", "attacks", "hits", "damage", "deaths"].map((key) => ({
      id: `${key}.advancing`,
      ok: probesValid && deltas[key] > 0,
    })),
  ];
  return { ok: checks.every((check) => check.ok), deltas, checks };
}

export async function battleSourcePin(root) {
  const git = (...args) =>
    execFileSync("git", args, { cwd: root, encoding: "utf8" });
  const paths = git(
    "ls-files",
    "--cached",
    "--others",
    "--exclude-standard",
    "-z",
    "--",
    ".",
    "../../crates/aethyr",
  )
    .split("\0")
    .filter(
      (path) =>
        path &&
        (/^(apps|packages|tools|experiments|\.\.\/\.\.\/crates\/aethyr)\//.test(path) ||
          [
            "public/kernel.wasm",
            "package.json",
            "package-lock.json",
            "vite.config.ts",
            "../../crates/aethyr/Cargo.toml",
            "../../crates/aethyr/Cargo.lock",
            "tsconfig.json",
          ].includes(path)),
    )
    .sort();
  const files = {};
  for (const path of paths) {
    try {
      const bytes = await readFile(resolve(root, path));
      files[path] = { byteLength: bytes.length, sha256: sha(bytes) };
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
      // A tracked deletion changes the pin rather than hiding the dirty source.
      files[path] = { absent: true };
    }
  }
  return {
    revision: git("rev-parse", "HEAD").trim(),
    dirty: !!git("status", "--porcelain").trim(),
    sha256: sha(JSON.stringify(files)),
    files,
  };
}
export async function battleAssetPin(editorURL, configuredRoot) {
  const base = new URL(editorURL);
  const privateRoot = configuredRoot
    ? await realpath(resolve(configuredRoot))
    : undefined;
  const valid = (url) =>
    typeof url === "string" &&
    /^\/private-assets\/[A-Za-z0-9._/-]+$/.test(url) &&
    !url.includes("..");
  const served = async (url) => {
    assert(valid(url), "Choose private battle resource paths.");
    const response = await fetch(new URL(url, base));
    assert(response.ok, `Battle resource ${url} returned ${response.status}`);
    return Buffer.from(await response.arrayBuffer());
  };
  const local = async (url) => {
    assert(valid(url));
    const path = await realpath(
      resolve(privateRoot, url.slice("/private-assets/".length)),
    );
    assert(
      path.startsWith(privateRoot + sep),
      "Keep private battle resources inside their asset root.",
    );
    return readFile(path);
  };
  const sceneURL = "/private-assets/scene-ground.json",
    sceneBytes = await served(sceneURL),
    scene = JSON.parse(sceneBytes);
  assert(
    valid(scene.battleCrowdURL),
    "The running scene has no prepared battle manifest.",
  );
  const manifestURL = scene.battleCrowdURL,
    manifestBytes = await served(manifestURL),
    manifest = JSON.parse(manifestBytes);
  assert.equal(manifest.schema, "aethyrforge.battle-crowd.v1");
  if (privateRoot) {
    assert.equal(
      sha(await local(sceneURL)),
      sha(sceneBytes),
      "The configured asset root differs from the running scene",
    );
    assert.equal(
      sha(await local(manifestURL)),
      sha(manifestBytes),
      "The configured battle manifest differs from the server",
    );
  }
  const descriptors = new Map();
  const visit = (value) => {
    if (!value || typeof value !== "object") return;
    if (
      valid(value.url) &&
      typeof value.sha256 === "string" &&
      Number.isSafeInteger(value.byteLength)
    ) {
      const prior = descriptors.get(value.url);
      if (prior)
        assert.deepEqual(
          prior,
          value,
          "A battle resource has conflicting descriptors",
        );
      else descriptors.set(value.url, value);
    }
    for (const child of Object.values(value)) visit(child);
  };
  visit(manifest);
  assert(descriptors.size > 0 && descriptors.size <= 128);
  const files = {};
  let totalBytes = 0;
  for (const [url, descriptor] of [...descriptors].sort(([a], [b]) =>
    a.localeCompare(b),
  )) {
    const bytes = privateRoot ? await local(url) : await served(url),
      hash = sha(bytes);
    assert.equal(
      bytes.length,
      descriptor.byteLength,
      `Wrong battle resource length ${url}`,
    );
    assert.equal(
      hash,
      descriptor.sha256,
      `Wrong battle resource SHA256 ${url}`,
    );
    totalBytes += bytes.length;
    files[url] = { byteLength: bytes.length, sha256: hash };
  }
  return {
    source: privateRoot
      ? "configured private asset root matched to served scene/manifest"
      : "served private battle closure",
    scene: {
      url: sceneURL,
      byteLength: sceneBytes.length,
      sha256: sha(sceneBytes),
    },
    manifest: {
      url: manifestURL,
      byteLength: manifestBytes.length,
      sha256: sha(manifestBytes),
    },
    files,
    totalBytes,
    sha256: sha(JSON.stringify(files)),
  };
}
