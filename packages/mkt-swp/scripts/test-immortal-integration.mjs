import { spawnSync } from "node:child_process";
import { accessSync } from "node:fs";
import path from "node:path";

const expectedRevision = "d62a4f7c6c34a11d191fe78316fd8d4ce4da1d34";
const sourceDirectory = process.env.IMMORTAL_SOURCE_DIR;

if (sourceDirectory === undefined || sourceDirectory === "") {
  throw new Error("set IMMORTAL_SOURCE_DIR to a clean Immortal checkout at the pinned revision");
}

const revision = command(
  "git",
  ["-C", sourceDirectory, "rev-parse", "HEAD"],
  process.cwd(),
  "pipe",
).stdout.trim();
if (revision !== expectedRevision) {
  throw new Error(`Immortal checkout is ${revision}; expected ${expectedRevision}`);
}

command(
  "cargo",
  [
    "build",
    "--locked",
    "--release",
    "-p",
    "immortal-client-web",
    "--target",
    "wasm32-unknown-unknown",
  ],
  sourceDirectory,
  "inherit",
  { IMMORTAL_SOURCE_REVISION: expectedRevision },
);

const artifactPaths = {
  IMMORTAL_BROWSER_WASM_PATH: path.join(
    sourceDirectory,
    "target/wasm32-unknown-unknown/release/immortal_client_web.wasm",
  ),
  IMMORTAL_REQUESTER_FIXTURE_PATH: path.join(
    sourceDirectory,
    "tests/fixtures/nipmkt/swp-requester-api-v2.json",
  ),
  IMMORTAL_SESSION_FIXTURE_PATH: path.join(
    sourceDirectory,
    "tests/fixtures/nipmkt/swp-full-sessions-v1.json",
  ),
  IMMORTAL_ENGINE_FIXTURE_PATH: path.join(
    sourceDirectory,
    "tests/fixtures/nipmkt/swp-client-engine-v1.json",
  ),
};

for (const artifactPath of Object.values(artifactPaths)) accessSync(artifactPath);

command(
  "pnpm",
  [
    "exec",
    "vp",
    "test",
    "--root",
    "../..",
    "--run",
    "packages/mkt-swp/src/immortal-browser-abi.integration.test.ts",
  ],
  process.cwd(),
  "inherit",
  artifactPaths,
);

function command(executable, arguments_, cwd, stdio, environment = {}) {
  const result = spawnSync(executable, arguments_, {
    cwd,
    encoding: "utf8",
    env: { ...process.env, ...environment },
    stdio,
  });
  if (result.error !== undefined) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${executable} ${arguments_.join(" ")} exited with ${String(result.status)}`);
  }
  return { stdout: result.stdout ?? "" };
}
