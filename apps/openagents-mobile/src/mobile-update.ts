export type MobileUpdatePhase = "checking" | "downloaded" | "current" | "error";

export type MobileUpdateProjection = Readonly<{
  appLabel: string;
  copyText: string;
  phase: MobileUpdatePhase;
  releaseFingerprint: string;
  runtimeFingerprint: string;
  statusLabel: string;
  voiceEnvironment: string;
  voiceHost: string;
}>;

const short = (value: string | null | undefined, length: number): string => {
  const normalized = value?.trim();
  return normalized === undefined || normalized === "" ? "unavailable" : normalized.slice(0, length);
};

export const projectMobileUpdate = (input: Readonly<{
  appVersion: string;
  buildNumber: string;
  updateId: string | null;
  runtimeVersion: string | null;
  isEmbeddedLaunch: boolean;
  isChecking: boolean;
  isDownloading: boolean;
  isUpdatePending: boolean;
  hasError: boolean;
  voiceEnvironment: string;
  voiceHost: string;
}>): MobileUpdateProjection => {
  const runtimeFingerprint = short(input.runtimeVersion, 10);
  const releaseFingerprint =
    input.updateId === null
      ? `embedded-${runtimeFingerprint}`
      : short(input.updateId, 8);
  const phase: MobileUpdatePhase =
    input.isChecking || input.isDownloading
      ? "checking"
      : input.isUpdatePending
        ? "downloaded"
        : input.hasError
          ? "error"
          : "current";
  const statusLabel =
    phase === "checking"
      ? "Checking for update"
      : phase === "downloaded"
        ? "Update downloaded—restart to apply"
        : phase === "error"
          ? "Update check failed—try again"
          : input.isEmbeddedLaunch
            ? "Embedded update running"
            : "Current update applied";
  const appLabel = `${input.appVersion} (${input.buildNumber})`;
  return {
    appLabel,
    phase,
    releaseFingerprint,
    runtimeFingerprint,
    statusLabel,
    voiceEnvironment: input.voiceEnvironment,
    voiceHost: input.voiceHost,
    copyText: [
      `OpenAgents ${appLabel}`,
      `Update ${releaseFingerprint}`,
      `Runtime ${runtimeFingerprint}`,
      `State ${statusLabel}`,
      `Sarah voice ${input.voiceEnvironment} ${input.voiceHost}`,
    ].join("\n"),
  };
};
