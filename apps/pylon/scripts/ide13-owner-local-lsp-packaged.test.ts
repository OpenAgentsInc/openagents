import { execFile } from "node:child_process";
import { mkdtemp, mkdir, readFile, readdir, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import { expect, test } from "vite-plus/test";

import { makePylonPortableDestinationProductionHelpers } from "../src/portable-destination-production-helper-adapters.js";
import type { PylonPortableDestinationHelperStartInput } from "../src/portable-destination-helper-supervisor.js";
import {
  PYLON_TYPESCRIPT_LSP_EXECUTABLE_PROFILE_REF,
  repositoryOwnedPylonPortableExecutableProfileCatalog,
} from "../src/portable-executable-profile-catalog.js";
import { verifyPylonPortableExecutableProfile } from "../src/portable-executable-profile-verifier.js";

const run = promisify(execFile);
const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

test("packs Pylon and initializes the exact LSP from an isolated dependency install", async () => {
  const proofRoot = await mkdtemp(join(tmpdir(), "pylon-packaged-lsp-"));
  const packRoot = join(proofRoot, "pack");
  const extractRoot = join(proofRoot, "extract");
  const dependencyRoot = join(proofRoot, "dependencies");
  const workspaceRoot = join(proofRoot, "workspace");
  await Promise.all([
    mkdir(packRoot),
    mkdir(extractRoot),
    mkdir(dependencyRoot),
    mkdir(workspaceRoot),
  ]);
  try {
    await run("pnpm", ["--dir", appRoot, "pack", "--pack-destination", packRoot], {
      cwd: appRoot,
    });
    const archives = (await readdir(packRoot)).filter((name) => name.endsWith(".tgz"));
    expect(archives).toHaveLength(1);
    const archive = archives[0];
    if (archive === undefined) return;
    await run("tar", ["-xzf", join(packRoot, archive), "-C", extractRoot]);
    const packagedApplicationRoot = join(extractRoot, "package");
    const packagedManifest = await readFile(join(packagedApplicationRoot, "package.json"), "utf8");
    expect(packagedManifest).toMatch(/"typescript": "5\.9\.2"/u);
    expect(packagedManifest).toMatch(/"typescript-language-server": "5\.3\.0"/u);

    await writeFile(
      join(dependencyRoot, "package.json"),
      `${JSON.stringify({ name: "pylon-lsp-proof", private: true, version: "0.0.0" })}\n`,
      "utf8",
    );
    await run(
      "pnpm",
      [
        "--dir",
        dependencyRoot,
        "add",
        "--save-exact",
        "--ignore-scripts",
        "typescript-language-server@5.3.0",
        "typescript@5.9.2",
      ],
      { cwd: dependencyRoot },
    );
    await symlink(
      join(dependencyRoot, "node_modules"),
      join(packagedApplicationRoot, "node_modules"),
    );

    const admission = repositoryOwnedPylonPortableExecutableProfileCatalog.resolve(
      PYLON_TYPESCRIPT_LSP_EXECUTABLE_PROFILE_REF,
    );
    expect(admission).not.toBeNull();
    if (admission === null) return;
    const verified = verifyPylonPortableExecutableProfile(admission, {
      applicationRoot: packagedApplicationRoot,
    });
    expect(verified).not.toBeNull();
    if (verified === null) return;

    const helpers = makePylonPortableDestinationProductionHelpers({
      exactExecutableIsAvailable: () => false,
      resolveLspProfile: () => verified,
    });
    const adapter = helpers.adapters.find((candidate) => candidate.kind === "lsp");
    expect(adapter).toBeDefined();
    if (adapter === undefined) return;
    const input: PylonPortableDestinationHelperStartInput = {
      destinationRunnerSessionReservationRef: "runner-session-reservation.ide13.packaged",
      sessionRef: "session.ide13.packaged",
      destinationAttachmentRef: "attachment.ide13.packaged.2",
      destinationGeneration: 2,
      workspaceRef: "workspace.ide13.packaged",
      workingDirectory: workspaceRoot,
      authorityEvidenceRef: "evidence.ide13.packaged.authority",
      authenticationPolicyRef: "policy.ide13.packaged.authentication",
      capabilityLeaseRefs: ["lease.ide13.packaged.provider"],
      authentication: {
        state: "reauthenticated",
        policyRef: "policy.ide13.packaged.authentication",
        evidenceRef: "evidence.ide13.packaged.authority",
        observedAt: "2026-07-20T18:00:00.000Z",
        expiresAt: null,
      },
      signal: new AbortController().signal,
    };
    const handle = await adapter.start(input);
    expect(await handle.isLive()).toBe(true);
    expect(handle.evidenceRefs.every((ref) => !ref.includes(proofRoot))).toBe(true);
    await handle.dispose();
    expect(await handle.isLive()).toBe(false);
  } finally {
    await rm(proofRoot, { recursive: true, force: true });
  }
});
