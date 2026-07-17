import {
  Button,
  ComponentValueBinding,
  IntentRef,
  Spacer,
  Stack,
  StaticPayload,
  Text,
  TextField,
  type View,
} from "@effect-native/core"

import type { MobileAccessibilityProfile } from "./khala-core"
import { mobileInteractiveStyle } from "./khala-core"
import type { MobileSettingsSection, MobileSettingsState } from "../settings/mobile-settings"

type AccountPresentation = Readonly<{
  title: string
  detail: string
  control: "sign_in" | "sign_out" | "none"
}>

const sectionLabels: ReadonlyArray<Readonly<{ id: MobileSettingsSection; label: string; detail: string }>> = [
  { id: "account", label: "Account", detail: "Local identity and verified OpenAgents Sync" },
  { id: "environments", label: "Environments", detail: "Pairing, health, capabilities, and reconnect" },
  { id: "notifications", label: "Notifications", detail: "Permission, registration health, and preferences" },
  { id: "appearance", label: "Appearance", detail: "System appearance and OpenAgents visual identity" },
  { id: "accessibility", label: "Accessibility", detail: "Text scale, reduced motion, and screen-reader support" },
  { id: "storage", label: "Storage & cache", detail: "Local-first data and bounded offline cache" },
  { id: "diagnostics", label: "Diagnostics", detail: "Connection and installation inspectors" },
  { id: "legal", label: "Legal", detail: "Privacy, terms, and open-source notices" },
]

const sectionButton = (
  id: MobileSettingsSection,
  label: string,
  detail: string,
  accessibility: MobileAccessibilityProfile,
): View => Button({
  key: `settings-section-${id}`,
  label: `${label}\n${detail}`,
  variant: "ghost",
  onPress: IntentRef("SettingsSectionSelected", StaticPayload({ section: id })),
  a11y: { label: `${label}. ${detail}` },
  style: { width: "full", ...mobileInteractiveStyle(accessibility) },
})

const rootView = (state: MobileSettingsState, accessibility: MobileAccessibilityProfile): ReadonlyArray<View> => [
  Text({ key: "settings-title", content: "Settings", variant: "title", color: "textPrimary" }),
  Text({
    key: "settings-subtitle",
    content: "Device controls stay local. Shared environments and consequential actions require verified authority.",
    variant: "body",
    color: "textMuted",
  }),
  ...(state.incomingShare === null
    ? []
    : [sectionButton("share", "Incoming share", "Review before inserting into the composer", accessibility)]),
  ...sectionLabels.map(section => sectionButton(section.id, section.label, section.detail, accessibility)),
]

const environmentView = (state: MobileSettingsState, accessibility: MobileAccessibilityProfile): ReadonlyArray<View> => [
  Text({ key: "environment-education", content: "Pair only an environment you control. Pairing grants the listed capabilities; it never exposes credentials in this view.", variant: "body", color: "textMuted" }),
  TextField({
    key: "environment-pairing-code",
    value: state.pairingCode,
    placeholder: "Pairing code",
    onChange: IntentRef("EnvironmentPairingCodeChanged", ComponentValueBinding()),
    a11y: { label: "Environment pairing code" },
    style: { width: "full", ...mobileInteractiveStyle(accessibility) },
  }),
  Button({
    key: "environment-pair",
    label: state.submittingEnvironment ? "Pairing…" : "Pair environment",
    variant: "primary",
    disabled: state.submittingEnvironment || state.pairingCode.trim().length === 0,
    onPress: IntentRef("EnvironmentPairRequested", StaticPayload({})),
    style: { width: "full", ...mobileInteractiveStyle(accessibility) },
  }),
  Button({
    key: "environment-refresh",
    label: state.environmentState === "loading" ? "Refreshing…" : "Refresh health",
    variant: "secondary",
    disabled: state.environmentState === "loading",
    onPress: IntentRef("EnvironmentDirectoryRequested", StaticPayload({})),
    style: { width: "full", ...mobileInteractiveStyle(accessibility) },
  }),
  ...(state.environments?.environments ?? []).flatMap(environment => [
    Button({
      key: `environment-${environment.environmentRef}`,
      label: `${environment.label} · ${environment.health}\n${environment.detail}`,
      variant: state.selectedEnvironmentRef === environment.environmentRef ? "secondary" : "ghost",
      onPress: IntentRef("EnvironmentInspected", StaticPayload({ environmentRef: environment.environmentRef })),
      a11y: { label: `${environment.label}, ${environment.health}, inspect environment` },
      style: { width: "full", ...mobileInteractiveStyle(accessibility) },
    }),
    ...(state.selectedEnvironmentRef !== environment.environmentRef ? [] : [
      Text({ key: `environment-detail-${environment.environmentRef}`, content: `${environment.kind.replace("_", " ")} · ${environment.capabilities.join(", ") || "No capabilities"}\n${environment.lastSeenAt ?? "No confirmed heartbeat"}`, variant: "caption", color: "textMuted" }),
      Button({
        key: `environment-reconnect-${environment.environmentRef}`,
        label: state.submittingEnvironment ? "Reconnecting…" : "Reconnect",
        variant: "secondary",
        disabled: state.submittingEnvironment,
        onPress: IntentRef("EnvironmentReconnectRequested", StaticPayload({ environmentRef: environment.environmentRef })),
        style: { width: "full", ...mobileInteractiveStyle(accessibility) },
      }),
    ]),
  ]),
  ...(state.environmentReceipt === null ? [] : [Text({ key: "environment-receipt", content: `Confirmed · ${state.environmentReceipt.summary}`, variant: "caption", color: "textPrimary" })]),
]

const notificationView = (state: MobileSettingsState, accessibility: MobileAccessibilityProfile): ReadonlyArray<View> => {
  const notification = state.notification
  return [
    Text({ key: "notification-education", content: "OpenAgents asks only after you choose Enable. System permission and native registration are inspected separately.", variant: "body", color: "textMuted" }),
    Text({ key: "notification-health", content: `Permission · ${notification.permission}\nRegistration · ${notification.registration}\n${notification.detail}`, variant: "body", color: notification.permission === "denied" ? "warning" : "textPrimary" }),
    ...(notification.permission === "granted" ? [] : [Button({
      key: "notification-permission",
      label: state.notificationLoading ? "Checking…" : "Enable notifications",
      variant: "primary",
      disabled: state.notificationLoading,
      onPress: IntentRef("NotificationPermissionRequested", StaticPayload({})),
      style: { width: "full", ...mobileInteractiveStyle(accessibility) },
    })]),
    ...(["attention", "completion", "approvals"] as const).map(preference => Button({
      key: `notification-preference-${preference}`,
      label: `${notification.preferences[preference] ? "On" : "Off"} · ${preference[0]!.toUpperCase()}${preference.slice(1)}`,
      variant: notification.preferences[preference] ? "secondary" : "ghost",
      onPress: IntentRef("NotificationPreferenceToggled", StaticPayload({ preference })),
      a11y: { label: `${preference} notifications ${notification.preferences[preference] ? "on" : "off"}` },
      style: { width: "full", ...mobileInteractiveStyle(accessibility) },
    })),
  ]
}

const staticSection = (section: MobileSettingsSection, account: AccountPresentation, accessibility: MobileAccessibilityProfile): ReadonlyArray<View> => {
  const copy: Partial<Record<MobileSettingsSection, string>> = {
    account: "This device works locally without an account. A verified OpenAgents session adds Sync and authenticated environment controls.",
    appearance: "OpenAgents follows the device color scheme and uses the shared Effect Native token system.",
    accessibility: "Dynamic text, reduced motion, 44-point targets, selectable transcript content, and semantic labels are active throughout the workbench.",
    storage: "Conversation and coding continuity use the device-local store. Shared rows remain bounded by confirmed owner scope and offline-cache accounting.",
    diagnostics: "Inspect Sync status, environment health, native notification registration, exact worktree state, controller sessions, and runtime agents from their authoritative surfaces.",
    legal: "Privacy, terms, and open-source notices are served from OpenAgents-owned release surfaces. No legal acceptance is inferred by this screen.",
  }
  if (section === "account") return [
    Text({ key: "settings-account-title", content: account.title, variant: "heading", color: "textPrimary" }),
    Text({ key: "settings-account-detail", content: account.detail, variant: "body", color: "textMuted" }),
    ...(account.control === "none" ? [] : [Button({
      key: `settings-account-${account.control}`,
      label: account.control === "sign_out" ? "Sign out" : "Link OpenAgents account",
      variant: account.control === "sign_out" ? "secondary" : "primary",
      onPress: IntentRef(account.control === "sign_out" ? "OpenAgentsSignOutPressed" : "OpenAgentsSignInPressed", StaticPayload({})),
      style: { width: "full", ...mobileInteractiveStyle(accessibility) },
    })]),
  ]
  return [Text({ key: `settings-copy-${section}`, content: copy[section] ?? "", variant: "body", color: "textMuted" })]
}

const shareView = (state: MobileSettingsState, accessibility: MobileAccessibilityProfile): ReadonlyArray<View> => state.incomingShare === null
  ? [Text({ key: "share-empty", content: "No pending share.", variant: "body", color: "textMuted" })]
  : [
      Text({ key: "share-preview", content: [state.incomingShare.title, state.incomingShare.text, state.incomingShare.url].filter(Boolean).join("\n\n"), variant: "body", color: "textPrimary" }),
      Button({ key: "share-insert", label: "Insert in composer", variant: "primary", onPress: IntentRef("IncomingShareInserted", StaticPayload({})), style: { width: "full", ...mobileInteractiveStyle(accessibility) } }),
      Button({ key: "share-dismiss", label: "Dismiss", variant: "secondary", onPress: IntentRef("IncomingShareDismissed", StaticPayload({})), style: { width: "full", ...mobileInteractiveStyle(accessibility) } }),
    ]

export const renderMobileSettingsView = (
  state: MobileSettingsState,
  accessibility: MobileAccessibilityProfile,
  account: AccountPresentation,
): View => Stack({ key: "mobile-settings", direction: "column", gap: "3", padding: "4", style: { width: "full", height: "full", backgroundColor: "background" } }, [
  ...(state.section === "root" ? [] : [Button({
    key: "settings-back",
    label: "Back to Settings",
    variant: "ghost",
    onPress: IntentRef("SettingsSectionSelected", StaticPayload({ section: "root" })),
    style: mobileInteractiveStyle(accessibility),
  })]),
  ...(state.section === "root"
    ? rootView(state, accessibility)
    : [Text({ key: "settings-section-title", content: sectionLabels.find(section => section.id === state.section)?.label ?? "Incoming share", variant: "title", color: "textPrimary" }),
      ...(state.section === "environments" ? environmentView(state, accessibility)
        : state.section === "notifications" ? notificationView(state, accessibility)
        : state.section === "share" ? shareView(state, accessibility)
        : staticSection(state.section, account, accessibility))]),
  ...(state.notice === null ? [] : [Text({ key: "settings-notice", content: state.notice, variant: "caption", color: "warning" })]),
  Spacer({ key: "settings-bottom-space", size: "8" }),
])
