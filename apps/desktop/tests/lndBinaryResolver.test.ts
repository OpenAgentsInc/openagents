import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { describe, expect, it } from "vitest";

import {
  LndBinaryResolverError,
  resolveAndVerifyLndBinary,
  resolveLndBinaryPath,
  verifyLndBinaryIntegrity,
} from "../src/main/lndBinaryResolver";

const sha256Hex = (input: Buffer | string): string =>
  crypto.createHash("sha256").update(input).digest("hex");

const writeBundledRuntime = (options: {
  rootDir: string;
  target: string;
  binaryFileName: string;
  binaryContents: Buffer;
  sha256: string;
}) => {
  const lndRoot = path.join(options.rootDir, "build-resources", "lnd");
  const targetDir = path.join(lndRoot, options.target);
  fs.mkdirSync(targetDir, { recursive: true });

  const binaryPath = path.join(targetDir, options.binaryFileName);
  fs.writeFileSync(binaryPath, options.binaryContents);

  const runtimeManifest = {
    version: "v0.20.0-beta",
    generatedAt: new Date().toISOString(),
    targets: {
      [options.target]: {
        binaryFileName: options.binaryFileName,
        sha256: options.sha256,
        source: "release",
      },
    },
  };

  fs.writeFileSync(
    path.join(lndRoot, "runtime-manifest.json"),
    `${JSON.stringify(runtimeManifest, null, 2)}\n`,
    "utf8",
  );

  return { lndRoot, binaryPath };
};

describe("lnd binary resolver", () => {
  it("resolves bundled binary path in development mode", () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "oa-desktop-lnd-resolve-"));
    try {
      const binaryContents = Buffer.from("lnd-binary-test");
      const expectedSha = sha256Hex(binaryContents);
      const { binaryPath } = writeBundledRuntime({
        rootDir: tmp,
        target: "darwin-arm64",
        binaryFileName: "lnd",
        binaryContents,
        sha256: expectedSha,
      });

      const resolved = resolveLndBinaryPath({
        appPath: tmp,
        resourcesPath: "/ignored",
        isPackaged: false,
        platform: "darwin",
        arch: "arm64",
        env: {},
      });

      expect(resolved.source).toBe("bundled");
      expect(resolved.target).toBe("darwin-arm64");
      expect(resolved.binaryPath).toBe(binaryPath);

      const verified = resolveAndVerifyLndBinary({
        appPath: tmp,
        resourcesPath: "/ignored",
        isPackaged: false,
        platform: "darwin",
        arch: "arm64",
        env: {},
      });

      expect(verified.sha256).toBe(expectedSha);
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("fails closed on checksum mismatch", () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "oa-desktop-lnd-checksum-"));
    try {
      const binaryContents = Buffer.from("lnd-binary-with-mismatch");
      writeBundledRuntime({
        rootDir: tmp,
        target: "darwin-arm64",
        binaryFileName: "lnd",
        binaryContents,
        sha256: "0".repeat(64),
      });

      expect(() =>
        resolveAndVerifyLndBinary({
          appPath: tmp,
          resourcesPath: "/ignored",
          isPackaged: false,
          platform: "darwin",
          arch: "arm64",
          env: {},
        }),
      ).toThrowError(LndBinaryResolverError);

      try {
        resolveAndVerifyLndBinary({
          appPath: tmp,
          resourcesPath: "/ignored",
          isPackaged: false,
          platform: "darwin",
          arch: "arm64",
          env: {},
        });
      } catch (error) {
        expect(error).toBeInstanceOf(LndBinaryResolverError);
        expect((error as LndBinaryResolverError).code).toBe("checksum_mismatch");
      }
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("verifies dev override binary when checksum is provided", () => {
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "oa-desktop-lnd-dev-override-"));
    try {
      const binaryPath = path.join(tmp, "lnd");
      const binaryContents = Buffer.from("local-dev-lnd-binary");
      fs.writeFileSync(binaryPath, binaryContents);
      fs.chmodSync(binaryPath, 0o755);

      const expectedSha = sha256Hex(binaryContents);
      const checked = verifyLndBinaryIntegrity(binaryPath, expectedSha);
      expect(checked.valid).toBe(true);

      const resolved = resolveAndVerifyLndBinary({
        appPath: "/ignored",
        resourcesPath: "/ignored",
        isPackaged: false,
        platform: "darwin",
        arch: "arm64",
        env: {
          OA_DESKTOP_LND_DEV_BINARY_PATH: binaryPath,
          OA_DESKTOP_LND_DEV_BINARY_SHA256: expectedSha,
        },
      });

      expect(resolved.source).toBe("dev_override");
      expect(resolved.binaryPath).toBe(binaryPath);
      expect(resolved.sha256).toBe(expectedSha);

      expect(() =>
        resolveAndVerifyLndBinary({
          appPath: "/ignored",
          resourcesPath: "/ignored",
          isPackaged: false,
          platform: "darwin",
          arch: "arm64",
          env: {
            OA_DESKTOP_LND_DEV_BINARY_PATH: binaryPath,
            OA_DESKTOP_LND_DEV_BINARY_SHA256: "f".repeat(64),
          },
        }),
      ).toThrowError(LndBinaryResolverError);
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  });
});
