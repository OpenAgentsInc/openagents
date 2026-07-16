/**
 * Provider-facing Agent Client Protocol settings projection (#8895).
 *
 * This module deliberately owns presentation state only. Executable probing,
 * secret custody, authentication, process/session lifecycle, and capability
 * admission remain in the trusted peer runtimes. The renderer accepts one
 * bounded projection and derives every visible action from it; it never treats
 * a token file, environment variable, or executable name as proof.
 */
import { defineIntent } from "@effect-native/core";
import { Exit, Schema } from "@effect-native/core/effect";

export const acpProviderIds = ["grok", "cursor"] as const;
export const AcpProviderStatusChannel = "openagents-desktop/acp-provider-status" as const;
export const AcpProviderActionChannel = "openagents-desktop/acp-provider-action" as const;
export const AcpProviderSupportExportChannel =
  "openagents-desktop/acp-provider-support-export" as const;
export type AcpProviderId = (typeof acpProviderIds)[number];
export type AcpProfileState = "supported" | "experimental" | "incompatible" | "unprobed";
export type AcpAuthState =
  | "unknown"
  | "required"
  | "pending"
  | "authenticated"
  | "cancelled"
  | "denied"
  | "expired"
  | "failed";
export type AcpSessionState =
  | "none"
  | "starting"
  | "running"
  | "replaying"
  | "waiting_for_input"
  | "cancelling"
  | "cancel_escalated"
  | "recovering"
  | "recovered"
  | "non_recoverable"
  | "degraded"
  | "failed";

export type AcpConfigurationOption = Readonly<{
  id: string;
  label: string;
  value: string | null;
  provenance: "stable" | "peer-extension";
  state: "ready" | "pending" | "error" | "reconciled";
}>;

export type AcpProviderProjection = Readonly<{
  provider: AcpProviderId;
  displayName: "Grok CLI" | "Cursor Agent CLI";
  profileRef: "grok-cli" | "cursor-agent";
  protocol: "Agent Client Protocol";
  install: "checking" | "not_installed" | "detected";
  executable: Readonly<{
    source: "trusted-path" | "validated-alternate";
    displayPath: string | null;
  }>;
  version: string | null;
  profileState: AcpProfileState;
  auth: Readonly<{
    state: AcpAuthState;
    advertisedMethods: ReadonlyArray<"cached_token" | "xai.api_key" | "cursor_login">;
    safeLogout: boolean;
  }>;
  probe: Readonly<{
    state: "not_run" | "passed" | "failed";
    code: string | null;
    observedAt: string | null;
  }>;
  session: Readonly<{
    state: AcpSessionState;
    sessionRef: string | null;
    processRef: string | null;
    canNew: boolean;
    canList: boolean;
    canLoad: boolean;
    canResume: boolean;
    canCancel: boolean;
  }>;
  capabilities: Readonly<{
    filesystem: boolean;
    terminal: boolean;
    permissions: boolean;
    questions: boolean;
  }>;
  configuration: ReadonlyArray<AcpConfigurationOption>;
  diagnosticCodes: ReadonlyArray<string>;
  conformanceRef: string | null;
}>;

export type AcpProviderSettingsState =
  | Readonly<{ state: "loading" }>
  | Readonly<{ state: "unavailable"; message: string }>
  | Readonly<{ state: "loaded"; providers: ReadonlyArray<AcpProviderProjection>; dropped: number }>;

const unprobedProvider = (provider: AcpProviderId): AcpProviderProjection => ({
  provider,
  displayName: provider === "grok" ? "Grok CLI" : "Cursor Agent CLI",
  profileRef: provider === "grok" ? "grok-cli" : "cursor-agent",
  protocol: "Agent Client Protocol",
  install: "checking",
  executable: { source: "trusted-path", displayPath: null },
  version: null,
  profileState: "unprobed",
  auth: { state: "unknown", advertisedMethods: [], safeLogout: false },
  probe: { state: "not_run", code: null, observedAt: null },
  session: {
    state: "none",
    sessionRef: null,
    processRef: null,
    canNew: false,
    canList: false,
    canLoad: false,
    canResume: false,
    canCancel: false,
  },
  capabilities: { filesystem: false, terminal: false, permissions: false, questions: false },
  configuration: [],
  diagnosticCodes: [],
  conformanceRef: null,
});

export const initialAcpProviderSettingsState = (): AcpProviderSettingsState => ({
  state: "loaded",
  providers: [unprobedProvider("grok"), unprobedProvider("cursor")],
  dropped: 0,
});

const PublicRef = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(120),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
);
const ShortText = Schema.String.check(Schema.isMaxLength(200));
const ProjectionSchema = Schema.Struct({
  provider: Schema.Literals(acpProviderIds),
  displayName: Schema.Literals(["Grok CLI", "Cursor Agent CLI"]),
  profileRef: Schema.Literals(["grok-cli", "cursor-agent"]),
  protocol: Schema.Literal("Agent Client Protocol"),
  install: Schema.Literals(["checking", "not_installed", "detected"]),
  executable: Schema.Struct({
    source: Schema.Literals(["trusted-path", "validated-alternate"]),
    displayPath: Schema.NullOr(ShortText),
  }),
  version: Schema.NullOr(ShortText),
  profileState: Schema.Literals(["supported", "experimental", "incompatible", "unprobed"]),
  auth: Schema.Struct({
    state: Schema.Literals([
      "unknown",
      "required",
      "pending",
      "authenticated",
      "cancelled",
      "denied",
      "expired",
      "failed",
    ]),
    advertisedMethods: Schema.Array(
      Schema.Literals(["cached_token", "xai.api_key", "cursor_login"]),
    ).check(Schema.isMaxLength(3)),
    safeLogout: Schema.Boolean,
  }),
  probe: Schema.Struct({
    state: Schema.Literals(["not_run", "passed", "failed"]),
    code: Schema.NullOr(PublicRef),
    observedAt: Schema.NullOr(ShortText),
  }),
  session: Schema.Struct({
    state: Schema.Literals([
      "none",
      "starting",
      "running",
      "replaying",
      "waiting_for_input",
      "cancelling",
      "cancel_escalated",
      "recovering",
      "recovered",
      "non_recoverable",
      "degraded",
      "failed",
    ]),
    sessionRef: Schema.NullOr(PublicRef),
    processRef: Schema.NullOr(PublicRef),
    canNew: Schema.Boolean,
    canList: Schema.Boolean,
    canLoad: Schema.Boolean,
    canResume: Schema.Boolean,
    canCancel: Schema.Boolean,
  }),
  capabilities: Schema.Struct({
    filesystem: Schema.Boolean,
    terminal: Schema.Boolean,
    permissions: Schema.Boolean,
    questions: Schema.Boolean,
  }),
  configuration: Schema.Array(
    Schema.Struct({
      id: PublicRef,
      label: ShortText,
      value: Schema.NullOr(ShortText),
      provenance: Schema.Literals(["stable", "peer-extension"]),
      state: Schema.Literals(["ready", "pending", "error", "reconciled"]),
    }),
  ).check(Schema.isMaxLength(64)),
  diagnosticCodes: Schema.Array(PublicRef).check(Schema.isMaxLength(32)),
  conformanceRef: Schema.NullOr(PublicRef),
});
const ResultSchema = Schema.Union([
  Schema.Struct({
    state: Schema.Literal("ok"),
    providers: Schema.Array(Schema.Unknown).check(Schema.isMaxLength(8)),
  }),
  Schema.Struct({ state: Schema.Literal("unavailable"), message: ShortText }),
]);

const providerIdentityIsExact = (value: AcpProviderProjection): boolean =>
  value.provider === "grok"
    ? value.displayName === "Grok CLI" && value.profileRef === "grok-cli"
    : value.displayName === "Cursor Agent CLI" && value.profileRef === "cursor-agent";

/** Fail closed per row, require exactly one Grok and one Cursor projection. */
export const decodeAcpProviderSettings = (value: unknown): AcpProviderSettingsState => {
  const outer = Schema.decodeUnknownExit(ResultSchema)(value);
  if (!Exit.isSuccess(outer))
    return { state: "unavailable", message: "ACP provider status is unavailable on this build." };
  if (outer.value.state === "unavailable")
    return { state: "unavailable", message: outer.value.message };
  const providers: AcpProviderProjection[] = [];
  let dropped = 0;
  for (const candidate of outer.value.providers) {
    const decoded = Schema.decodeUnknownExit(ProjectionSchema)(candidate);
    if (
      !Exit.isSuccess(decoded) ||
      !providerIdentityIsExact(decoded.value as AcpProviderProjection)
    ) {
      dropped += 1;
      continue;
    }
    providers.push(decoded.value as AcpProviderProjection);
  }
  const unique = new Map(providers.map((provider) => [provider.provider, provider]));
  if (unique.size !== 2 || !unique.has("grok") || !unique.has("cursor")) {
    return {
      state: "unavailable",
      message: "ACP provider status was incomplete and was withheld.",
    };
  }
  return { state: "loaded", providers: [unique.get("grok")!, unique.get("cursor")!], dropped };
};

export type AcpProviderAction =
  | "probe"
  | "select_alternate"
  | "authenticate"
  | "reauthenticate"
  | "logout"
  | "new_session"
  | "cancel"
  | "recover";

export const DesktopAcpProviderActionRequested = defineIntent(
  "DesktopAcpProviderActionRequested",
  Schema.String,
);
export const DesktopAcpSupportExportRequested = defineIntent(
  "DesktopAcpSupportExportRequested",
  Schema.Null,
);

export type AcpProviderSettingsBridge = Readonly<{
  status: () => Promise<unknown>;
  action: (provider: AcpProviderId, action: AcpProviderAction) => Promise<unknown>;
  supportExport: () => Promise<unknown>;
}>;

export const unavailableAcpProviderSettingsBridge: AcpProviderSettingsBridge = {
  status: async () => ({
    state: "unavailable",
    message: "ACP provider control is unavailable on this build.",
  }),
  action: async () => ({
    state: "unavailable",
    message: "ACP provider control is unavailable on this build.",
  }),
  supportExport: async () => ({
    ok: false,
    notice: "ACP support export is unavailable on this build.",
  }),
};

export const decodeAcpProviderActionPayload = (
  value: string,
): Readonly<{ provider: AcpProviderId; action: AcpProviderAction }> | null => {
  const [provider, action, extra] = value.split(":");
  if (extra !== undefined || !(acpProviderIds as ReadonlyArray<string>).includes(provider ?? ""))
    return null;
  const actions: ReadonlyArray<AcpProviderAction> = [
    "probe",
    "select_alternate",
    "authenticate",
    "reauthenticate",
    "logout",
    "new_session",
    "cancel",
    "recover",
  ];
  if (!actions.includes(action as AcpProviderAction)) return null;
  return { provider: provider as AcpProviderId, action: action as AcpProviderAction };
};

/** Available controls are negotiated/profile-derived; presence is never proof. */
export const availableAcpProviderActions = (
  provider: AcpProviderProjection,
): ReadonlyArray<AcpProviderAction> => {
  const runtimeLocked =
    provider.auth.state === "authenticated" ||
    provider.auth.state === "pending" ||
    ["starting", "running", "replaying", "waiting_for_input", "cancelling", "recovering"].includes(
      provider.session.state,
    );
  const actions: AcpProviderAction[] = runtimeLocked ? [] : ["probe", "select_alternate"];
  if (provider.install !== "detected" || provider.profileState === "incompatible") return actions;
  if (provider.auth.state === "required" || provider.auth.state === "cancelled")
    actions.push("authenticate");
  if (["denied", "expired", "failed"].includes(provider.auth.state)) actions.push("reauthenticate");
  if (provider.auth.state === "authenticated" && provider.auth.safeLogout) actions.push("logout");
  if (
    provider.auth.state === "authenticated" &&
    provider.session.canNew &&
    ["none", "recovered", "non_recoverable", "failed"].includes(provider.session.state)
  )
    actions.push("new_session");
  if (
    provider.session.canCancel &&
    ["starting", "running", "replaying", "waiting_for_input"].includes(provider.session.state)
  )
    actions.push("cancel");
  if (["degraded", "failed", "non_recoverable"].includes(provider.session.state))
    actions.push("recover");
  return actions;
};

export type AcpSupportBundle = Readonly<{
  schema: "openagents.desktop.acp-support.v1";
  schemaRelease: "schema-v1.19.0";
  generatedAt: string;
  providers: ReadonlyArray<
    Readonly<{
      provider: AcpProviderId;
      profileRef: string;
      version: string | null;
      profileState: AcpProfileState;
      install: AcpProviderProjection["install"];
      authState: AcpAuthState;
      probeState: AcpProviderProjection["probe"]["state"];
      probeCode: string | null;
      sessionState: AcpSessionState;
      capabilities: AcpProviderProjection["capabilities"];
      diagnosticCodes: ReadonlyArray<string>;
      conformanceRef: string | null;
      receiptRefs: ReadonlyArray<string>;
      evidenceRefs: ReadonlyArray<string>;
    }>
  >;
}>;

/** Main-owned export builder; its closed input has no secret-bearing fields. */
export const buildAcpSupportBundle = (
  input: Readonly<{
    generatedAt: string;
    providers: ReadonlyArray<
      Readonly<{
        projection: AcpProviderProjection;
        receiptRefs: ReadonlyArray<string>;
        evidenceRefs: ReadonlyArray<string>;
      }>
    >;
  }>,
): AcpSupportBundle => ({
  schema: "openagents.desktop.acp-support.v1",
  schemaRelease: "schema-v1.19.0",
  generatedAt: input.generatedAt.slice(0, 40),
  providers: input.providers.map(({ projection: provider, receiptRefs, evidenceRefs }) => ({
    provider: provider.provider,
    profileRef: provider.profileRef,
    version: provider.version,
    profileState: provider.profileState,
    install: provider.install,
    authState: provider.auth.state,
    probeState: provider.probe.state,
    probeCode: provider.probe.code,
    sessionState: provider.session.state,
    capabilities: provider.capabilities,
    diagnosticCodes: provider.diagnosticCodes,
    conformanceRef: provider.conformanceRef,
    receiptRefs: receiptRefs
      .filter((ref) => /^[A-Za-z0-9][A-Za-z0-9._:-]{0,119}$/.test(ref))
      .slice(-128),
    evidenceRefs: evidenceRefs
      .filter((ref) => /^[A-Za-z0-9][A-Za-z0-9._:-]{0,119}$/.test(ref))
      .slice(-128),
  })),
});

const HostActionSchema = Schema.Struct({
  provider: Schema.Literals(acpProviderIds),
  action: Schema.Literals([
    "probe",
    "select_alternate",
    "authenticate",
    "reauthenticate",
    "logout",
    "new_session",
    "cancel",
    "recover",
  ]),
});
export const decodeAcpProviderHostAction = (
  value: unknown,
): Readonly<{ provider: AcpProviderId; action: AcpProviderAction }> | null => {
  const decoded = Schema.decodeUnknownExit(HostActionSchema)(value);
  return Exit.isSuccess(decoded) ? decoded.value : null;
};
