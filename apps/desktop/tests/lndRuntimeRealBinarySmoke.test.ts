import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

import { describe, expect, it } from "vitest";

import { materializeLndRuntimeConfig } from "../src/main/lndRuntimeConfig";

const runRealLndSmoke = process.env.OA_DESKTOP_RUN_REAL_LND_TESTS === "1";

const resolveHostTarget = (): "darwin-amd64" | "darwin-arm64" | "linux-amd64" | "linux-arm64" | "windows-amd64" => {
  switch (`${process.platform}:${process.arch}`) {
    case "darwin:arm64":
      return "darwin-arm64";
    case "darwin:x64":
      return "darwin-amd64";
    case "linux:x64":
      return "linux-amd64";
    case "linux:arm64":
      return "linux-arm64";
    case "win32:x64":
      return "windows-amd64";
    default:
      throw new Error(`Unsupported platform for real LND smoke: ${process.platform}/${process.arch}`);
  }
};

const ensureStagedBinaryForHost = (): string => {
  const target = resolveHostTarget();
  const appRoot = path.resolve(process.cwd());
  const stageRoot = path.join(appRoot, "build-resources", "lnd");
  const runtimeManifestPath = path.join(stageRoot, "runtime-manifest.json");

  if (!fs.existsSync(runtimeManifestPath)) {
    const prepare = spawnSync("node", ["./scripts/prepare-lnd-binaries.mjs", "--targets", target], {
      cwd: appRoot,
      encoding: "utf8",
    });
    if (prepare.status !== 0) {
      throw new Error(
        `Failed to stage LND binary.\nstdout:\n${prepare.stdout}\nstderr:\n${prepare.stderr}`,
      );
    }
  }

  const runtimeManifest = JSON.parse(fs.readFileSync(runtimeManifestPath, "utf8")) as {
    targets?: Record<string, { binaryFileName?: string }>;
  };
  const targetEntry = runtimeManifest.targets?.[target];
  const binaryFileName = targetEntry?.binaryFileName;
  if (!binaryFileName) {
    throw new Error(`runtime-manifest missing binary metadata for target ${target}`);
  }

  const binaryPath = path.join(stageRoot, target, binaryFileName);
  if (!fs.existsSync(binaryPath)) {
    throw new Error(`Staged LND binary not found at ${binaryPath}`);
  }
  return binaryPath;
};

describe.skipIf(!runRealLndSmoke)("lnd runtime real binary smoke", () => {
  it("accepts generated launch args without flag parser failures", () => {
    const binaryPath = ensureStagedBinaryForHost();
    const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "oa-lnd-real-smoke-"));

    try {
      const runtime = materializeLndRuntimeConfig({
        userDataPath: tempRoot,
        network: "testnet",
        alias: "OpenAgentsDesktop",
        rpcListen: "127.0.0.1:10009",
        restListen: "127.0.0.1:8080",
        p2pListen: "127.0.0.1:19735",
        debugLevel: "info",
        neutrinoPeers: [],
      });

      const result = spawnSync(binaryPath, [...runtime.launchArgs, "--help"], {
        cwd: runtime.paths.runtimeDir,
        encoding: "utf8",
        timeout: 20_000,
      });

      const output = `${result.stdout ?? ""}\n${result.stderr ?? ""}`.toLowerCase();
      expect(output.includes("cannot have an argument")).toBe(false);
      expect(output.includes("failed to load config")).toBe(false);
      expect(result.error).toBeUndefined();
      expect(result.status).toBe(0);
    } finally {
      fs.rmSync(tempRoot, { recursive: true, force: true });
    }
  });
});
