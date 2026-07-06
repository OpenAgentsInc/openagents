import type { NativeStackNavigationProp } from "@react-navigation/native-stack"
import Constants from "expo-constants"
import { useEffect, useState } from "react"
import * as Notifications from "expo-notifications"
import { Linking, Modal, ScrollView, View } from "react-native"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { KHALA_ACCOUNT_DELETION_POLICY_COPY } from "../auth/mobile-openauth"
import { AppHeader } from "../components/app-header"
import { Frame, usePowerOnVisible } from "../components/frame"
import { KhalaButton } from "../components/khala-button"
import { KhalaScreen } from "../components/khala-screen"
import { KhalaText } from "../components/khala-text"
import type { AppDrawerScreenProps, AppStackParamList } from "../navigators/navigationTypes"
import type { OnDeviceReadinessRow } from "../native/on-device-readiness-core"
import { useOnDeviceReadiness } from "../native/use-on-device-readiness"
import { registerForPushNotificationsAsync } from "../push/push-notifications-client"
import { fetchKhalaMobileCreditsBalance } from "../sync/khala-mobile-credits-api"
import { formatUsdCents, isLowBalance } from "../sync/khala-mobile-credits-format-core"
import {
  fetchKhalaMobileModelPreference,
  putKhalaMobileModelPreference,
  type KhalaModelPreference,
} from "../sync/khala-mobile-model-preference-api"
import { modelDisplayLabel, modelPreferenceFallbackMessage } from "../sync/khala-mobile-model-preference-format-core"
import { MOTION_STAGGER_MS } from "../theme/motion"

const SectionLabel = ({ children }: { children: string }) => (
  <KhalaText className="mb-2" variant="label">
    {children}
  </KhalaText>
)

/**
 * MM-H1 (#8487): the mobile-only MVP pivot's Settings rework. The prior
 * desktop-oriented Fleet section (env-var fleet-run id, "credential never
 * leaves the desktop" copy) is removed entirely — Settings must contain
 * nothing that requires a desktop (acceptance criterion). Models remains an
 * honest "not yet available" stub (MM-F1, #8484, per-user model config, not
 * merged yet). Notifications is real (backed by the merged MM-G1 push
 * infrastructure, #8485/#8486). Credits (MM-D3, #8480) is now live-attempting:
 * it queries the proposed `/api/mobile/credits/balance` contract
 * (`khala-mobile-credits-api.ts`) and degrades to the same honest stub copy
 * if that endpoint isn't built yet, never fabricating a number.
 */
const AccountSection = () => {
  const { deleteAccount, ownerUserId, signOut } = useKhalaAuth()
  const [confirmingDeletion, setConfirmingDeletion] = useState(false)
  const [deleting, setDeleting] = useState(false)
  const [deleteError, setDeleteError] = useState<string | null>(null)

  const closeConfirmation = () => {
    if (deleting) return
    setConfirmingDeletion(false)
    setDeleteError(null)
  }

  const handleDeleteAccount = async () => {
    if (deleting) return
    setDeleting(true)
    setDeleteError(null)
    try {
      await deleteAccount()
      setConfirmingDeletion(false)
    } catch (error) {
      setDeleteError(error instanceof Error ? error.message : "Account deletion failed. Please retry.")
    } finally {
      setDeleting(false)
    }
  }

  return (
    <View className="gap-2">
      <SectionLabel>Account</SectionLabel>
      <KhalaText numberOfLines={1} variant="faint">
        {ownerUserId}
      </KhalaText>
      <KhalaText variant="muted">Signed in with GitHub.</KhalaText>
      <KhalaButton onPress={() => void signOut()} text="Sign out" variant="danger" />
      <KhalaButton onPress={() => setConfirmingDeletion(true)} text="Delete account" variant="danger" />
      <Modal
        animationType="fade"
        onRequestClose={closeConfirmation}
        transparent
        visible={confirmingDeletion}
      >
        <View className="flex-1 justify-end bg-black/70 px-4 py-6">
          <View className="gap-4 rounded-lg border border-border bg-surface p-4">
            <View className="gap-2">
              <KhalaText variant="heading">Delete account</KhalaText>
              <KhalaText variant="muted">{KHALA_ACCOUNT_DELETION_POLICY_COPY}</KhalaText>
              {deleteError === null ? null : <KhalaText variant="danger">{deleteError}</KhalaText>}
            </View>
            <View className="gap-2">
              <KhalaButton
                loading={deleting}
                onPress={() => void handleDeleteAccount()}
                text="Delete account"
                variant="danger"
              />
              <KhalaButton
                disabled={deleting}
                onPress={closeConfirmation}
                text="Cancel"
                variant="secondary"
              />
            </View>
          </View>
        </View>
      </Modal>
    </View>
  )
}

type CreditsBalanceState =
  | Readonly<{ status: "loading" }>
  | Readonly<{ status: "unavailable" }>
  | Readonly<{ status: "ready"; balanceUsdCents: number }>

/** MM-D3 (#8480): live-attempting balance, honest fallback. Neither the
 * balance nor the transaction-history route exists on the server yet (see
 * `khala-mobile-credits-api.ts`'s header comment for the proposed contract),
 * so `status === "unavailable"` is the expected state today — the section
 * still states the one thing that IS true and shipped (#8478's $10
 * GitHub-account-keyed signup grant) rather than showing nothing. */
const CreditsSection = ({ onViewHistory }: { onViewHistory: () => void }) => {
  const { baseUrl, token } = useKhalaAuth()
  const [state, setState] = useState<CreditsBalanceState>({ status: "loading" })

  useEffect(() => {
    let cancelled = false
    void fetchKhalaMobileCreditsBalance(baseUrl, token).then(result => {
      if (cancelled) return
      setState(result.ok ? { balanceUsdCents: result.value, status: "ready" } : { status: "unavailable" })
    })
    return () => {
      cancelled = true
    }
  }, [baseUrl, token])

  return (
    <View className="gap-2">
      <SectionLabel>Credits</SectionLabel>
      {state.status === "loading" ? (
        <KhalaText variant="muted">Checking your balance…</KhalaText>
      ) : state.status === "ready" ? (
        <View className="gap-2">
          <KhalaText variant="muted">Balance: {formatUsdCents(state.balanceUsdCents)}</KhalaText>
          {isLowBalance(state.balanceUsdCents) ? (
            <KhalaText variant="danger">Your balance is low.</KhalaText>
          ) : null}
          <KhalaButton onPress={onViewHistory} text="View history" variant="secondary" />
          <KhalaButton disabled text="Buy more credits (coming soon)" variant="secondary" />
        </View>
      ) : (
        <View className="gap-2">
          <KhalaText variant="muted">
            You received $10 in free credit when you signed in with GitHub.
          </KhalaText>
          <KhalaText variant="faint">Balance and usage history are coming soon.</KhalaText>
        </View>
      )}
    </View>
  )
}

type ModelsState =
  | Readonly<{ status: "loading" }>
  | Readonly<{ status: "unavailable" }>
  | Readonly<{ status: "ready"; preference: KhalaModelPreference }>

/** MM-F1 (#8484, merged 8f38922fc4 while this lane was mid-flight) shipped
 * the real `GET/PUT /api/mobile/model-preference` route — this section now
 * wires directly against it rather than stubbing. The coding executor's own
 * honor of this preference (per #8484's scoping note) is Lane 0's follow-up,
 * separate from this store/read/write UI. */
const ModelsSection = () => {
  const { baseUrl, token } = useKhalaAuth()
  const [state, setState] = useState<ModelsState>({ status: "loading" })
  const [selecting, setSelecting] = useState<string | null>(null)

  const refresh = async () => {
    const result = await fetchKhalaMobileModelPreference(baseUrl, token)
    setState(result.ok ? { preference: result.value, status: "ready" } : { status: "unavailable" })
  }

  useEffect(() => {
    void refresh()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const handleSelect = async (modelId: string) => {
    if (selecting !== null) return
    setSelecting(modelId)
    const result = await putKhalaMobileModelPreference(baseUrl, token, modelId)
    setSelecting(null)
    if (result.ok) setState({ preference: result.value, status: "ready" })
  }

  return (
    <View className="gap-2">
      <SectionLabel>Models</SectionLabel>
      {state.status === "loading" ? (
        <KhalaText variant="muted">Loading models…</KhalaText>
      ) : state.status === "unavailable" ? (
        <View className="gap-1">
          <KhalaText variant="muted">Khala Code currently runs the default model for every task.</KhalaText>
          <KhalaText variant="faint">Choosing your own model isn't available right now.</KhalaText>
        </View>
      ) : (
        <View className="gap-2">
          {modelPreferenceFallbackMessage(state.preference.fallback) === null ? null : (
            <KhalaText variant="warning">{modelPreferenceFallbackMessage(state.preference.fallback)}</KhalaText>
          )}
          {state.preference.availableModelIds.map(modelId => {
            const active = state.preference.effectiveModelId === modelId
            return (
              <KhalaButton
                disabled={selecting !== null}
                key={modelId}
                loading={selecting === modelId}
                onPress={() => void handleSelect(modelId)}
                text={active ? `${modelDisplayLabel(modelId)} (active)` : modelDisplayLabel(modelId)}
                variant={active ? "primary" : "secondary"}
              />
            )
          })}
        </View>
      )}
    </View>
  )
}

type PushPermissionStatus = "loading" | "granted" | "denied" | "undetermined"

const NotificationsSection = () => {
  const { baseUrl, token } = useKhalaAuth()
  const [status, setStatus] = useState<PushPermissionStatus>("loading")
  const [requesting, setRequesting] = useState(false)

  const refreshStatus = async () => {
    try {
      const permissions = await Notifications.getPermissionsAsync()
      setStatus(permissions.granted ? "granted" : permissions.canAskAgain ? "undetermined" : "denied")
    } catch {
      setStatus("undetermined")
    }
  }

  useEffect(() => {
    void refreshStatus()
  }, [])

  const handleEnable = async () => {
    if (requesting || token === "") return
    setRequesting(true)
    try {
      await registerForPushNotificationsAsync({ apiBaseUrl: baseUrl, bearerToken: token, event: "task_dispatched" })
    } finally {
      setRequesting(false)
      await refreshStatus()
    }
  }

  return (
    <View className="gap-2">
      <SectionLabel>Notifications</SectionLabel>
      {status === "loading" ? (
        <KhalaText variant="muted">checking…</KhalaText>
      ) : status === "granted" ? (
        <KhalaText variant="muted">Enabled — you'll get a push when a task finishes or needs you.</KhalaText>
      ) : status === "denied" ? (
        <View className="gap-2">
          <KhalaText variant="muted">
            Notifications are turned off for Khala Code in your device Settings.
          </KhalaText>
          <KhalaButton onPress={() => void Linking.openSettings()} text="Open device settings" variant="secondary" />
        </View>
      ) : (
        <View className="gap-2">
          <KhalaText variant="muted">
            Get notified when a task finishes or needs your input.
          </KhalaText>
          <KhalaButton
            disabled={requesting}
            loading={requesting}
            onPress={() => void handleEnable()}
            text="Enable notifications"
            variant="secondary"
          />
        </View>
      )}
    </View>
  )
}

const ON_DEVICE_TONE_COLOR: Record<OnDeviceReadinessRow["tone"], string> = {
  danger: "text-danger",
  faint: "text-textFaint",
  success: "text-success",
  warning: "text-warning",
}

const OnDeviceCard = ({ row, index }: { row: OnDeviceReadinessRow; index: number }) => {
  const visible = usePowerOnVisible(MOTION_STAGGER_MS * index)
  return (
    <Frame alwaysShowBorder visible={visible}>
      <View className="px-3 py-2">
        <View className="flex-row items-center justify-between">
          <KhalaText text={row.label} variant="caption" />
          <KhalaText className={ON_DEVICE_TONE_COLOR[row.tone]} text={row.status} variant="faint" />
        </View>
        {row.detail === undefined ? null : (
          <KhalaText className="mt-1" text={row.detail} variant="faint" />
        )}
      </View>
    </Frame>
  )
}

const appVersionLabel = (): string => {
  const version = Constants.expoConfig?.version ?? "0.0.0"
  const iosBuildNumber = (Constants.expoConfig?.ios as { buildNumber?: unknown } | undefined)?.buildNumber
  const androidVersionCode = (Constants.expoConfig?.android as { versionCode?: unknown } | undefined)?.versionCode
  const build =
    typeof iosBuildNumber === "string"
      ? ` (ios ${iosBuildNumber})`
      : typeof androidVersionCode === "number"
        ? ` (android ${androidVersionCode})`
        : ""
  return `Khala Code ${version}${build}`
}

const AboutSection = () => {
  const readiness = useOnDeviceReadiness()
  return (
    <View className="gap-2">
      <SectionLabel>About &amp; diagnostics</SectionLabel>
      <KhalaText variant="faint">{appVersionLabel()}</KhalaText>
      {readiness.status === "loading" ? (
        <KhalaText text="checking…" variant="muted" />
      ) : readiness.status === "error" ? (
        <KhalaText text="Could not read native module readiness." variant="danger" />
      ) : (
        readiness.rows.map((row, index) => <OnDeviceCard index={index} key={row.key} row={row} />)
      )}
    </View>
  )
}

type SettingsScreenProps = AppDrawerScreenProps<"Settings">

export const SettingsScreen = ({ navigation }: SettingsScreenProps) => {
  const stackNavigation = navigation.getParent<NativeStackNavigationProp<AppStackParamList>>()

  return (
    <KhalaScreen preset="fixed">
      <AppHeader showMenu title="Settings" />
      <ScrollView contentContainerClassName="gap-6 px-4 py-4">
        <AccountSection />
        <CreditsSection onViewHistory={() => stackNavigation?.navigate("CreditsHistory")} />
        <ModelsSection />
        <NotificationsSection />
        <AboutSection />
      </ScrollView>
    </KhalaScreen>
  )
}
