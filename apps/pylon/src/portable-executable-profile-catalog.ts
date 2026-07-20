export type PylonPortableExecutableProfileAdmission = Readonly<{
  executableProfileRef: string;
  installedArtifactRef: string;
  signatureRef: string;
  versionRef: string;
}>;

export type PylonPortableExecutableProfileCatalog = Readonly<{
  resolve: (executableProfileRef: string) => PylonPortableExecutableProfileAdmission | null;
}>;

/**
 * No executable profile is admitted. A future repository change must add a
 * signed installed artifact before this catalog can authorize one.
 */
const admittedProfiles: ReadonlyArray<PylonPortableExecutableProfileAdmission> = [];

export const repositoryOwnedPylonPortableExecutableProfileCatalog:
  PylonPortableExecutableProfileCatalog = {
  resolve: (executableProfileRef) =>
    admittedProfiles.find((profile) =>
      profile.executableProfileRef === executableProfileRef) ?? null,
};
