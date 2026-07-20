import { describe, expect, test } from "vite-plus/test";

import {
  PYLON_TYPESCRIPT_LSP_EXECUTABLE_PROFILE_REF,
  repositoryOwnedPylonPortableExecutableProfileCatalog,
} from "./portable-executable-profile-catalog.js";
import { verifyPylonPortableExecutableProfile } from "./portable-executable-profile-verifier.js";

describe("portable executable profile verifier", () => {
  test("admits the exact installed TypeScript LSP materials and npm authority", () => {
    const admission = repositoryOwnedPylonPortableExecutableProfileCatalog.resolve(
      PYLON_TYPESCRIPT_LSP_EXECUTABLE_PROFILE_REF,
    );
    expect(admission).not.toBeNull();
    if (admission === null) return;

    const verified = verifyPylonPortableExecutableProfile(admission);
    expect(verified).not.toBeNull();
    expect(admission.platforms).toEqual(["darwin", "linux"]);
    expect(admission.architectures).toEqual(["arm64", "x64"]);
    expect(
      admission.packageAuthorities?.map(({ packageName, version }) => `${packageName}@${version}`),
    ).toEqual(["typescript-language-server@5.3.0", "typescript@5.9.2"]);
    expect(
      admission.packageAuthorities?.every(
        (authority) =>
          authority.registryIntegrity.startsWith("sha512-") && authority.npmSignature.length > 64,
      ),
    ).toBe(true);
  });

  test("fails closed for a platform mismatch or changed installed digest", () => {
    const admission = repositoryOwnedPylonPortableExecutableProfileCatalog.resolve(
      PYLON_TYPESCRIPT_LSP_EXECUTABLE_PROFILE_REF,
    );
    expect(admission).not.toBeNull();
    if (admission === null) return;

    expect(
      verifyPylonPortableExecutableProfile(admission, {
        platform: "win32",
        architecture: "x64",
      }),
    ).toBeNull();
    expect(
      verifyPylonPortableExecutableProfile({
        ...admission,
        materials: admission.materials?.map((material, index) =>
          index === 0 ? { relativePath: material.relativePath, sha256: "0".repeat(64) } : material,
        ),
      }),
    ).toBeNull();
  });

  test("rejects forged registry authority and a swapped package identity", () => {
    const admission = repositoryOwnedPylonPortableExecutableProfileCatalog.resolve(
      PYLON_TYPESCRIPT_LSP_EXECUTABLE_PROFILE_REF,
    );
    expect(admission?.packageAuthorities).toHaveLength(2);
    const languageServer = admission?.packageAuthorities?.[0];
    const typescript = admission?.packageAuthorities?.[1];
    if (admission === null || languageServer === undefined || typescript === undefined) return;

    expect(
      verifyPylonPortableExecutableProfile({
        ...admission,
        packageAuthorities: [
          {
            ...languageServer,
            npmSignature: `${languageServer.npmSignature.slice(0, -2)}AA`,
          },
          typescript,
        ],
      }),
    ).toBeNull();
    expect(
      verifyPylonPortableExecutableProfile({
        ...admission,
        packageAuthorities: [
          {
            ...languageServer,
            registryIntegrity: `${languageServer.registryIntegrity.slice(0, -2)}AA`,
          },
          typescript,
        ],
      }),
    ).toBeNull();
    expect(
      verifyPylonPortableExecutableProfile({
        ...admission,
        packageAuthorities: [
          {
            ...languageServer,
            npmSignatureKeyId: "SHA256:forged-registry-key",
          },
          typescript,
        ],
      }),
    ).toBeNull();
    expect(
      verifyPylonPortableExecutableProfile({
        ...admission,
        packageAuthorities: [
          {
            ...languageServer,
            packageName: typescript.packageName,
            version: typescript.version,
            registryIntegrity: typescript.registryIntegrity,
            npmSignature: typescript.npmSignature,
          },
          typescript,
        ],
      }),
    ).toBeNull();
  });

  test("rejects a missing or extra executable-closure material", () => {
    const admission = repositoryOwnedPylonPortableExecutableProfileCatalog.resolve(
      PYLON_TYPESCRIPT_LSP_EXECUTABLE_PROFILE_REF,
    );
    expect(admission?.materials).toHaveLength(6);
    if (admission?.materials === undefined) return;

    expect(
      verifyPylonPortableExecutableProfile({
        ...admission,
        materials: admission.materials.slice(1),
      }),
    ).toBeNull();
    expect(
      verifyPylonPortableExecutableProfile({
        ...admission,
        materials: [
          ...admission.materials,
          {
            relativePath: "node_modules/typescript/lib/extra-entry.js",
            sha256: "0".repeat(64),
          },
        ],
      }),
    ).toBeNull();
  });
});
