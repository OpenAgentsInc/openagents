import { Schema as S } from "effect";

export const ProbeBackendKind = S.Literals(["apple_fm_bridge", "gemini_api", "psionic_qwen35"]);
export type ProbeBackendKind = typeof ProbeBackendKind.Type;

export const ProbeBackendAttachMode = S.Literals(["attach_existing", "direct_api"]);
export type ProbeBackendAttachMode = typeof ProbeBackendAttachMode.Type;

export const ProbeBackendAuthMode = S.Literals(["none", "api_key"]);
export type ProbeBackendAuthMode = typeof ProbeBackendAuthMode.Type;

export const ProbeBackendStreamMode = S.Literals(["snapshot", "sse"]);
export type ProbeBackendStreamMode = typeof ProbeBackendStreamMode.Type;

export const ProbeBackendProfile = S.Struct({
  id: S.String,
  kind: ProbeBackendKind,
  defaultBaseUrl: S.String,
  model: S.String,
  attachMode: ProbeBackendAttachMode,
  auth: ProbeBackendAuthMode,
  readinessPath: S.String,
  streamMode: ProbeBackendStreamMode,
});
export type ProbeBackendProfile = typeof ProbeBackendProfile.Type;

export const ResolvedProbeBackendProfile = S.Struct({
  ...ProbeBackendProfile.fields,
  baseUrl: S.String,
  baseUrlSource: S.Literals([
    "explicit",
    "PROBE_APPLE_FM_BASE_URL",
    "OPENAGENTS_APPLE_FM_BASE_URL",
    "PROBE_GEMINI_BASE_URL",
    "PROBE_OMEGA_BASE_URL",
    "PYLON_PSIONIC_BASE_URL",
    "PROBE_PSIONIC_BASE_URL",
    "default",
  ]),
});
export type ResolvedProbeBackendProfile = typeof ResolvedProbeBackendProfile.Type;

export interface ResolveProbeBackendProfileOptions {
  readonly profileId?: string;
  readonly explicitBaseUrl?: string;
  readonly env?: Readonly<Record<string, string | undefined>>;
}
