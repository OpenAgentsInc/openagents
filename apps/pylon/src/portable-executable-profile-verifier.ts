import { createHash, verify as verifySignature } from "node:crypto";
import { readFileSync, statSync } from "node:fs";
import { dirname, isAbsolute, normalize, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import type { PylonPortableExecutableProfileAdmission } from "./portable-executable-profile-catalog.js";

const applicationRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const SHA256 = /^[a-f0-9]{64}$/u;
const NPM_REGISTRY_KEY_ID = "SHA256:DhQ8wR5APBvFHLF/+Tc+AYvPOdTpcIDqOhxsBHRwC7U";
const NPM_REGISTRY_PUBLIC_KEY = Buffer.from(
  "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEY6Ya7W++7aUPzvMTrezH6Ycx3c+HOKYCcNGybJZSCJq/fd7Qa8uuAKtdIkUQtQiEKERhAmE5lMMJhP8OkDOa2g==",
  "base64",
);
const EXACT_PACKAGE_VERSIONS = new Map([
  ["typescript-language-server", "5.3.0"],
  ["typescript", "5.9.2"],
]);
// These files are the executable closure. Their digests match the official npm
// package bytes. The npm signatures verify the source packages.
const EXACT_EXECUTABLE_MATERIAL_PATHS = new Set([
  "node_modules/typescript-language-server/lib/cli.mjs",
  "node_modules/typescript-language-server/package.json",
  "node_modules/typescript/lib/tsserver.js",
  "node_modules/typescript/lib/_tsserver.js",
  "node_modules/typescript/lib/typescript.js",
  "node_modules/typescript/package.json",
]);

export type PylonPortableVerifiedExecutableProfile = Readonly<{
  admission: PylonPortableExecutableProfileAdmission;
  nodeEntrypointPath: string;
  fixedArgv: readonly ["--stdio"];
  typescriptServerPath: string;
}>;

const containedPath = (root: string, relativePath: string): string | null => {
  if (relativePath.length === 0 || isAbsolute(relativePath)) return null;
  const candidate = resolve(root, normalize(relativePath));
  const relation = relative(root, candidate);
  return relation.length > 0 && !relation.startsWith("..") && !isAbsolute(relation)
    ? candidate
    : null;
};

const fileDigestMatches = (path: string, expected: string): boolean => {
  try {
    return (
      statSync(path).isFile() &&
      SHA256.test(expected) &&
      createHash("sha256").update(readFileSync(path)).digest("hex") === expected
    );
  } catch {
    return false;
  }
};

const packageSignatureIsValid = (
  authority: NonNullable<PylonPortableExecutableProfileAdmission["packageAuthorities"]>[number],
): boolean => {
  const expectedVersion = EXACT_PACKAGE_VERSIONS.get(authority.packageName);
  if (
    expectedVersion !== authority.version ||
    authority.npmSignatureKeyId !== NPM_REGISTRY_KEY_ID ||
    !authority.registryIntegrity.startsWith("sha512-")
  )
    return false;
  try {
    return verifySignature(
      "sha256",
      Buffer.from(
        `${authority.packageName}@${authority.version}:${authority.registryIntegrity}`,
        "utf8",
      ),
      { key: NPM_REGISTRY_PUBLIC_KEY, format: "der", type: "spki" },
      Buffer.from(authority.npmSignature, "base64"),
    );
  } catch {
    return false;
  }
};

export const verifyPylonPortableExecutableProfile = (
  admission: PylonPortableExecutableProfileAdmission,
  runtime: Readonly<{
    platform?: NodeJS.Platform;
    architecture?: string;
    applicationRoot?: string;
  }> = {},
): PylonPortableVerifiedExecutableProfile | null => {
  const platform = runtime.platform ?? process.platform;
  const architecture = runtime.architecture ?? process.arch;
  const verifiedApplicationRoot = runtime.applicationRoot ?? applicationRoot;
  if (!isAbsolute(verifiedApplicationRoot)) return null;
  if (
    admission.platforms === undefined ||
    !admission.platforms.some((item) => item === platform) ||
    admission.architectures === undefined ||
    !admission.architectures.some((item) => item === architecture) ||
    admission.helperKind !== "lsp" ||
    admission.fixedArgv === undefined ||
    admission.fixedArgv.length !== 1 ||
    admission.fixedArgv[0] !== "--stdio" ||
    admission.materials === undefined ||
    admission.materials.length !== EXACT_EXECUTABLE_MATERIAL_PATHS.size ||
    admission.materials.some(
      (material) => !EXACT_EXECUTABLE_MATERIAL_PATHS.has(material.relativePath),
    ) ||
    admission.packageAuthorities === undefined ||
    admission.packageAuthorities.length !== 2 ||
    new Set(admission.packageAuthorities.map((authority) => authority.packageName)).size !== 2 ||
    admission.packageAuthorities.some((authority) => !packageSignatureIsValid(authority))
  )
    return null;

  const materials = admission.materials.map((material) => ({
    ...material,
    path: containedPath(verifiedApplicationRoot, material.relativePath),
  }));
  if (
    materials.some(
      (material) => material.path === null || !fileDigestMatches(material.path, material.sha256),
    )
  )
    return null;

  if (
    admission.nodeEntrypointRelativePath === undefined ||
    admission.typescriptServerRelativePath === undefined
  )
    return null;
  const nodeEntrypointPath = containedPath(
    verifiedApplicationRoot,
    admission.nodeEntrypointRelativePath,
  );
  const typescriptServerPath = containedPath(
    verifiedApplicationRoot,
    admission.typescriptServerRelativePath,
  );
  if (
    nodeEntrypointPath === null ||
    typescriptServerPath === null ||
    !materials.some((material) => material.path === nodeEntrypointPath) ||
    !materials.some((material) => material.path === typescriptServerPath)
  )
    return null;

  return {
    admission,
    nodeEntrypointPath,
    fixedArgv: admission.fixedArgv,
    typescriptServerPath,
  };
};
