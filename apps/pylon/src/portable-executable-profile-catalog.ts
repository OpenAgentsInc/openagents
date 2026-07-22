export type PylonPortableExecutableProfileMaterial = Readonly<{
  relativePath: string;
  sha256: string;
}>;

export type PylonPortableExecutablePackageAuthority = Readonly<{
  packageName: string;
  version: string;
  license: "Apache-2.0";
  registryIntegrity: string;
  npmSignatureKeyId: string;
  npmSignature: string;
  attestationUrl: string | null;
  repositoryUrl: string;
}>;

export type PylonPortableExecutableProfileAdmission = Readonly<{
  executableProfileRef: string;
  helperKind?: "lsp";
  installedArtifactRef: string;
  signatureRef: string;
  attestationRef?: string | null;
  versionRef: string;
  platforms?: ReadonlyArray<"darwin" | "linux">;
  architectures?: ReadonlyArray<"arm64" | "x64">;
  nodeEntrypointRelativePath?: string;
  fixedArgv?: readonly ["--stdio"];
  typescriptServerRelativePath?: string;
  materials?: ReadonlyArray<PylonPortableExecutableProfileMaterial>;
  packageAuthorities?: ReadonlyArray<PylonPortableExecutablePackageAuthority>;
}>;

export type PylonPortableExecutableProfileCatalog = Readonly<{
  resolve: (executableProfileRef: string) => PylonPortableExecutableProfileAdmission | null;
}>;

export const PYLON_TYPESCRIPT_LSP_EXECUTABLE_PROFILE_REF =
  "profile.pylon.portable.lsp.typescript-language-server-5.3.0.typescript-5.9.2.v1";
export const PYLON_PORTABLE_EXECUTABLE_PROFILE_CATALOG_REF =
  "catalog.pylon.portable-executable-profile.v1";

const npmSigningKeyId = "SHA256:DhQ8wR5APBvFHLF/+Tc+AYvPOdTpcIDqOhxsBHRwC7U";

const admittedProfiles: ReadonlyArray<PylonPortableExecutableProfileAdmission> = [
  {
    executableProfileRef: PYLON_TYPESCRIPT_LSP_EXECUTABLE_PROFILE_REF,
    helperKind: "lsp",
    installedArtifactRef:
      "artifact.pylon.portable.lsp.typescript-language-server-5.3.0.typescript-5.9.2.v1",
    signatureRef: "signature.npm.typescript-language-server-5.3.0.typescript-5.9.2.2026-07-20.v1",
    attestationRef: null,
    versionRef: "version.pylon.portable.lsp.typescript-language-server-5.3.0.typescript-5.9.2.v1",
    platforms: ["darwin", "linux"],
    architectures: ["arm64", "x64"],
    nodeEntrypointRelativePath: "node_modules/typescript-language-server/lib/cli.mjs",
    fixedArgv: ["--stdio"],
    typescriptServerRelativePath: "node_modules/typescript/lib/tsserver.js",
    materials: [
      {
        relativePath: "node_modules/typescript-language-server/lib/cli.mjs",
        sha256: "86ef128358dcd71d0c684d786d7751ee09fbbbbb2a521482b160007b8228be2e",
      },
      {
        relativePath: "node_modules/typescript-language-server/package.json",
        sha256: "090384304250e5b70e2a3f54aa99c0ae8465ebbdf136b05695c6a60eadcbbbd5",
      },
      {
        relativePath: "node_modules/typescript/lib/tsserver.js",
        sha256: "e3ccfeec65ec5c470b8ffc5611878a31182650c7e8a062c38a719f83b523edcb",
      },
      {
        relativePath: "node_modules/typescript/lib/_tsserver.js",
        sha256: "0efcc88cdf0593cc1dcc8b6afd605c7bed9eca6f648c70c8f0df8695114bf6d7",
      },
      {
        relativePath: "node_modules/typescript/lib/typescript.js",
        sha256: "e5f1f6b3e82228a89873cc7b941b2465185e839c0692860f83e3e63e53f94c2b",
      },
      {
        relativePath: "node_modules/typescript/package.json",
        sha256: "5a0bb7f286c4b3f1413a42c05f902311b161f70e5f52d9da10490443bfd595a3",
      },
    ],
    packageAuthorities: [
      {
        packageName: "typescript-language-server",
        version: "5.3.0",
        license: "Apache-2.0",
        registryIntegrity:
          "sha512-5puofxZHgFdAYtfNpmwCAvgtaYgg8wrUnH30m7Ze3QuguId5RNRadKASpOpyDxTyUdAF51FjhTdjntLw/EuWcQ==",
        npmSignatureKeyId: npmSigningKeyId,
        npmSignature:
          "MEYCIQDTTAaxeyB0i8aYMkSjAH1qZbStK+FA4v2ynVQn/t8/+AIhAIxggMboyHWJkCDQw1+eYTEfXa69bin7tvNJWOwBaHB1",
        attestationUrl:
          "https://registry.npmjs.org/-/npm/v1/attestations/typescript-language-server@5.3.0",
        repositoryUrl:
          "https://github.com/typescript-language-server/typescript-language-server.git",
      },
      {
        packageName: "typescript",
        version: "5.9.2",
        license: "Apache-2.0",
        registryIntegrity:
          "sha512-CWBzXQrc/qOkhidw1OzBTQuYRbfyxDXJMVJ1XNwUHGROVmuaeiEm3OslpZ1RV96d7SKKjZKrSJu3+t/xlw3R9A==",
        npmSignatureKeyId: npmSigningKeyId,
        npmSignature:
          "MEQCIBtsPj1YAoqGWRd5FeipJP5DWQeOABakzg15i/tN+6H7AiBay9mfRYuiOW/Cdab2vU8CuvLZvt4In8yl8HBP4IeOOQ==",
        attestationUrl: null,
        repositoryUrl: "https://github.com/microsoft/TypeScript.git",
      },
    ],
  },
];

export const repositoryOwnedPylonPortableExecutableProfileCatalog: PylonPortableExecutableProfileCatalog =
  {
    resolve: (executableProfileRef) =>
      admittedProfiles.find((profile) => profile.executableProfileRef === executableProfileRef) ??
      null,
  };
