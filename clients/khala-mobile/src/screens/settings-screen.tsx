import type { NativeStackNavigationProp } from "@react-navigation/native-stack"
import Constants from "expo-constants"
import { useEffect, useState } from "react"
import * as Notifications from "expo-notifications"
import { Linking, ScrollView, View } from "react-native"

import { useKhalaAuth } from "../auth/khala-auth-context"
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
  const { ownerUserId, signOut } = useKhalaAuth()
  return (
    <View className="gap-2">
      <SectionLabel>Account</SectionLabel>
      <KhalaText numberOfLines={1} variant="faint">
        {ownerUserId}
      </KhalaText>
      <KhalaText variant="muted">Signed in with GitHub.</KhalaText>
      <KhalaButton onPress={() => void signOut()} text="Sign out" variant="danger" />
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

/** MM-F1 (#8484) owns per-user model preference end to end (server storage +
 * executor honor). Until that lands, Khala Code always runs the operator
 * default model — this section says so honestly rather than showing a
 * picker with no effect. */
const ModelsSection = () => (
  <View className="gap-2">
    <SectionLabel>Models</SectionLabel>
    <KhalaText variant="muted">
      Khala Code currently runs the default model for every task.
    </KhalaText>
    <KhalaText variant="faint">
      Choosing your own model is coming soon.
    </KhalaText>
  </View>
)

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
