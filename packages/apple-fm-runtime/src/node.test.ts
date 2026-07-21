import { describe, expect, test } from "vite-plus/test";
import { createHash } from "node:crypto";
import { chmodSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import { resolveAppleFmHelperPath, verifyAppleFmHelper, type AppleFmHelperManifest } from "./node.js";

const stageHelper = (): { resourcesPath: string; helper: string; manifest: AppleFmHelperManifest } => {
  const root = mkdtempSync(path.join(os.tmpdir(), "oa-apple-fm-node-"));
  const helper = resolveAppleFmHelperPath(root);
  mkdirSync(path.dirname(helper), { recursive: true });
  writeFileSync(helper, "signed-foundation-bridge");
  chmodSync(helper, 0o755);
  const manifest: AppleFmHelperManifest = {
    protocolVersion: 1,
    helperVersion: "0.1.1",
    architecture: process.arch,
    sha256: createHash("sha256").update("signed-foundation-bridge").digest("hex"),
  };
  return { resourcesPath: root, helper, manifest };
};

describe("Apple FM node host verify (#9155 digest-stage)", () => {
  test("makes the code signature authoritative and demotes sha256 to an unsigned-only fallback", () => {
    const { resourcesPath, helper, manifest } = stageHelper();
    // Baseline: valid signature + matching digest is accepted.
    expect(verifyAppleFmHelper({ resourcesPath, manifest, verifySignature: (candidate) => candidate === helper })).toBe(helper);
    // #9155 regression pin: a validly-signed binary whose runtime bytes no
    // longer match the pinned PRE-SIGN digest (codesign rewrote the Mach-O) is
    // ACCEPTED. The signature is authoritative; sha256 is NOT compared.
    expect(verifyAppleFmHelper({ resourcesPath, manifest: { ...manifest, sha256: "0".repeat(64) }, verifySignature: () => true })).toBe(helper);
    // Unsigned/dev build: fall back to the sha256 pin. Matching accepted, mismatch rejected.
    expect(verifyAppleFmHelper({ resourcesPath, manifest, verifySignature: () => false })).toBe(helper);
    expect(() =>
      verifyAppleFmHelper({ resourcesPath, manifest: { ...manifest, sha256: "0".repeat(64) }, verifySignature: () => false }),
    ).toThrow("apple_fm_helper_digest_mismatch");
    // Manifest/architecture mismatch is still rejected before any body read.
    expect(() =>
      verifyAppleFmHelper({ resourcesPath, manifest: { ...manifest, architecture: "sparc" }, verifySignature: () => true }),
    ).toThrow("apple_fm_helper_manifest_mismatch");
  });
});
