import {
  Badge,
  Button,
  Card,
  Divider,
  IntentRef,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  type View,
} from "@effect-native/core";

import {
  availableAcpProviderActions,
  type AcpAuthState,
  type AcpProfileState,
  type AcpProviderAction,
  type AcpProviderProjection,
  type AcpProviderSettingsState,
} from "../acp-provider-contract.ts";

export * from "../acp-provider-contract.ts";

const authLabel = (state: AcpAuthState): string =>
  ({
    unknown: "Authentication not observed",
    required: "Authentication required",
    pending: "Waiting for authentication",
    authenticated: "Authenticated by peer",
    cancelled: "Authentication cancelled",
    denied: "Authentication denied",
    expired: "Authentication expired",
    failed: "Authentication failed",
  })[state];
const profileTone = (state: AcpProfileState): "success" | "warn" | "neutral" =>
  state === "supported" ? "success" : state === "incompatible" ? "warn" : "neutral";
const actionLabel: Readonly<Record<AcpProviderAction, string>> = {
  probe: "Probe again",
  select_alternate: "Choose executable",
  authenticate: "Authenticate",
  reauthenticate: "Re-authenticate",
  logout: "Log out",
  new_session: "New session",
  cancel: "Stop",
  recover: "Repair session",
};

const providerCard = (provider: AcpProviderProjection): View => {
  const installLabel =
    provider.install === "detected"
      ? "Detected"
      : provider.install === "not_installed"
        ? "Not installed"
        : "Checking";
  const authority = [
    provider.capabilities.filesystem ? "filesystem" : null,
    provider.capabilities.terminal ? "terminal" : null,
  ].filter(Boolean);
  return Card(
    {
      key: `settings-acp-${provider.provider}`,
      padding: "3",
      radius: "md",
      style: { width: "full", borderColor: "borderSubtle", borderWidth: 1 },
    },
    [
      Stack(
        {
          key: `settings-acp-${provider.provider}-head`,
          direction: "row",
          gap: "2",
          align: "center",
        },
        [
          Text({
            key: `settings-acp-${provider.provider}-name`,
            content: provider.displayName,
            variant: "heading",
            color: "textPrimary",
          }),
          Badge({
            key: `settings-acp-${provider.provider}-install`,
            label: installLabel,
            tone: provider.install === "detected" ? "success" : "neutral",
            a11y: { label: `${provider.displayName}: ${installLabel}` },
          }),
          Badge({
            key: `settings-acp-${provider.provider}-profile`,
            label: provider.profileState,
            tone: profileTone(provider.profileState),
            a11y: { label: `${provider.displayName} profile: ${provider.profileState}` },
          }),
          Spacer({ key: `settings-acp-${provider.provider}-fill`, flex: true }),
          ...availableAcpProviderActions(provider).map((action) =>
            Button({
              key: `settings-acp-${provider.provider}-${action}`,
              label: actionLabel[action],
              variant:
                action === "cancel"
                  ? "secondary"
                  : action === "new_session" ||
                      action === "authenticate" ||
                      action === "reauthenticate"
                    ? "primary"
                    : "ghost",
              onPress: IntentRef(
                "DesktopAcpProviderActionRequested",
                StaticPayload(`${provider.provider}:${action}`),
              ),
              a11y: { label: `${actionLabel[action]} for ${provider.displayName}` },
            }),
          ),
        ],
      ),
      Text({
        key: `settings-acp-${provider.provider}-protocol`,
        content: `${provider.protocol} control of a local CLI · ${provider.profileRef}`,
        variant: "body",
        color: "textMuted",
      }),
      Text({
        key: `settings-acp-${provider.provider}-binary`,
        content:
          provider.install === "not_installed"
            ? `Install through ${provider.provider === "grok" ? "xAI" : "Cursor"}'s official distribution, then probe again. OpenAgents never runs copied install commands or changes PATH.`
            : `${provider.executable.source === "validated-alternate" ? "Validated alternate" : "Trusted PATH"} · ${provider.executable.displayPath ?? "path withheld"} · version ${provider.version ?? "not observed"} · probe ${provider.probe.state}`,
        variant: "body",
        color: "textMuted",
      }),
      Text({
        key: `settings-acp-${provider.provider}-auth`,
        content: `${authLabel(provider.auth.state)} · advertised: ${provider.auth.advertisedMethods.length === 0 ? "none observed" : provider.auth.advertisedMethods.join(", ")}`,
        variant: "body",
        color:
          provider.auth.state === "failed" ||
          provider.auth.state === "denied" ||
          provider.auth.state === "expired"
            ? "danger"
            : "textPrimary",
        a11y: {
          role: "region",
          label: `${provider.displayName} authentication status: ${authLabel(provider.auth.state)}`,
        },
      }),
      Text({
        key: `settings-acp-${provider.provider}-session`,
        content: `Session ${provider.session.state}${provider.session.sessionRef === null ? "" : ` · ${provider.session.sessionRef}`}${provider.session.processRef === null ? "" : ` · process ${provider.session.processRef}`}`,
        variant: "body",
        color: "textPrimary",
        a11y: {
          role: "region",
          label: `${provider.displayName} session status: ${provider.session.state}`,
        },
      }),
      Text({
        key: `settings-acp-${provider.provider}-authority`,
        content:
          authority.length === 0
            ? "Filesystem and terminal authority are not active for this session."
            : `Active session authority: ${authority.join(" and ")}.`,
        variant: "label",
        color: "textMuted",
      }),
      ...provider.configuration.map((option) =>
        Text({
          key: `settings-acp-${provider.provider}-config-${option.id}`,
          content: `${option.label}: ${option.value ?? "not selected"} · ${option.provenance}${option.provenance === "peer-extension" ? " · experimental" : ""} · ${option.state}`,
          variant: "label",
          color: option.state === "error" ? "danger" : "textMuted",
        }),
      ),
      ...(provider.diagnosticCodes.length === 0
        ? []
        : [
            Text({
              key: `settings-acp-${provider.provider}-diagnostics`,
              content: `Diagnostics: ${provider.diagnosticCodes.join(", ")}`,
              variant: "label",
              color: "danger",
            }),
          ]),
    ],
  );
};

export const acpProviderSettingsView = (
  state: AcpProviderSettingsState,
  supportNotice: string | null = null,
): ReadonlyArray<View> => [
  Divider({ key: "settings-acp-divider" }),
  Text({
    key: "settings-acp-title",
    content: "Local Agent Client Protocol providers",
    variant: "label",
    color: "textMuted",
  }),
  Text({
    key: "settings-acp-copy",
    content:
      "Grok CLI and Cursor Agent CLI are separate trusted peer profiles. Compatibility is release- and evidence-specific; one passing peer does not imply support for every ACP agent.",
    variant: "body",
    color: "textMuted",
  }),
  Button({
    key: "settings-acp-support-export",
    label: "Export redacted support bundle",
    variant: "secondary",
    onPress: IntentRef("DesktopAcpSupportExportRequested"),
    a11y: { label: "Export a redacted Grok and Cursor Agent Client Protocol support bundle" },
  }),
  ...(supportNotice === null
    ? []
    : [
        Text({
          key: "settings-acp-support-notice",
          content: supportNotice,
          variant: "label",
          color: "textMuted",
          a11y: { role: "region", label: supportNotice },
        }),
      ]),
  ...(state.state === "loading"
    ? [
        Text({
          key: "settings-acp-loading",
          content: "Checking local ACP providers…",
          variant: "body",
          color: "textMuted",
          a11y: { role: "region", label: "Checking local Agent Client Protocol providers" },
        }),
      ]
    : state.state === "unavailable"
      ? [
          Text({
            key: "settings-acp-unavailable",
            content: state.message,
            variant: "body",
            color: "danger",
            a11y: {
              role: "region",
              label: `Agent Client Protocol providers unavailable: ${state.message}`,
            },
          }),
        ]
      : state.providers.map(providerCard)),
];
