import Constants from "expo-constants"
import { useEffect, useState } from "react"
import * as Notifications from "expo-notifications"
import { Linking, Modal, View, type TextStyle, type ViewStyle } from "react-native"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { KHALA_ACCOUNT_DELETION_POLICY_COPY } from "../auth/mobile-openauth"
import { Button, Card, Header, ListItem, Screen, Text, useAppTheme } from "../ignite"
import type { ThemedStyle } from "../ignite"
import type { AppDrawerScreenProps } from "../navigators/navigationTypes"
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

/**
 * MM-H1 (#8487) Settings, rebuilt entirely on the ported Infinite Red Ignite
 * component kit (`../ignite`: `Screen`, `Header`, `Text`, `Card`, `ListItem`,
 * `Button`) so the app shows the real Ignite look on a live screen. The prior
 * NativeWind + bespoke-primitive composition is gone; the product behavior is
 * unchanged. Settings must contain nothing that requires a desktop
 * (acceptance criterion): Account, Credits (MM-D3, #8480, live-attempting with
 * an honest fallback), Models (MM-F1, #8484, real `GET/PUT` model preference),
 * Notifications (MM-G1, #8485/#8486, real push permissions), and
 * About & diagnostics (on-device native-module readiness).
 */

const SectionCard = ({ heading, children }: { heading: string; children: React.ReactNode }) => {
  const { themed } = useAppTheme()
  return (
    <Card
      style={themed($sectionCard)}
      verticalAlignment="top"
      heading={heading}
      HeadingTextProps={{ preset: "subheading" }}
      ContentComponent={<View style={themed($sectionBody)}>{children}</View>}
    />
  )
}

const AccountSection = () => {
  const { deleteAccount, ownerUserId, signOut } = useKhalaAuth()
  const { theme, themed } = useAppTheme()
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
    <SectionCard heading="Account">
      <Text size="xs" numberOfLines={1} style={themed($dim)} text={ownerUserId} />
      <Text size="xs" style={themed($dim)} text="Signed in with GitHub." />
      <Button preset="reversed" text="Sign out" onPress={() => void signOut()} />
      <Button
        preset="default"
        text="Delete account"
        textStyle={{ color: theme.colors.error }}
        onPress={() => setConfirmingDeletion(true)}
      />
      <Modal
        animationType="fade"
        onRequestClose={closeConfirmation}
        transparent
        visible={confirmingDeletion}
      >
        <View style={themed($modalScrim)}>
          <Card
            style={themed($modalCard)}
            verticalAlignment="top"
            HeadingComponent={<Text preset="subheading" text="Delete account" />}
            ContentComponent={
              <View style={themed($sectionBody)}>
                <Text size="xs" style={themed($dim)} text={KHALA_ACCOUNT_DELETION_POLICY_COPY} />
                {deleteError === null ? null : (
                  <Text size="xs" style={{ color: theme.colors.error }} text={deleteError} />
                )}
                <Button
                  preset="default"
                  text="Delete account"
                  textStyle={{ color: theme.colors.error }}
                  disabled={deleting}
                  onPress={() => void handleDeleteAccount()}
                />
                <Button
                  preset="filled"
                  text="Cancel"
                  disabled={deleting}
                  onPress={closeConfirmation}
                />
              </View>
            }
          />
        </View>
      </Modal>
    </SectionCard>
  )
}

type CreditsBalanceState =
  | Readonly<{ status: "loading" }>
  | Readonly<{ status: "unavailable" }>
  | Readonly<{ status: "ready"; balanceUsdCents: number }>

/** MM-D3 (#8480): live-attempting balance, honest fallback. */
const CreditsSection = ({ onViewHistory }: { onViewHistory: () => void }) => {
  const { baseUrl, token } = useKhalaAuth()
  const { theme, themed } = useAppTheme()
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
    <SectionCard heading="Credits">
      {state.status === "loading" ? (
        <Text size="xs" style={themed($dim)} text="Checking your balance…" />
      ) : state.status === "ready" ? (
        <>
          <Text size="xs" style={themed($dim)} text={"Balance: " + formatUsdCents(state.balanceUsdCents)} />
          {isLowBalance(state.balanceUsdCents) ? (
            <Text size="xs" style={{ color: theme.colors.error }} text="Your balance is low." />
          ) : null}
          <Button preset="filled" text="View history" onPress={onViewHistory} />
          <Button preset="filled" text="Buy more credits (coming soon)" disabled />
        </>
      ) : (
        <>
          <Text
            size="xs"
            style={themed($dim)}
            text="You received $10 in free credit when you signed in with GitHub."
          />
          <Text size="xs" style={themed($faint)} text="Balance and usage history are coming soon." />
        </>
      )}
    </SectionCard>
  )
}

type ModelsState =
  | Readonly<{ status: "loading" }>
  | Readonly<{ status: "unavailable" }>
  | Readonly<{ status: "ready"; preference: KhalaModelPreference }>

/** MM-F1 (#8484): real `GET/PUT /api/mobile/model-preference`. */
const ModelsSection = () => {
  const { baseUrl, token } = useKhalaAuth()
  const { themed } = useAppTheme()
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
    <SectionCard heading="Models">
      {state.status === "loading" ? (
        <Text size="xs" style={themed($dim)} text="Loading models…" />
      ) : state.status === "unavailable" ? (
        <>
          <Text
            size="xs"
            style={themed($dim)}
            text="Khala Code currently runs the default model for every task."
          />
          <Text size="xs" style={themed($faint)} text="Choosing your own model isn't available right now." />
        </>
      ) : (
        <>
          {modelPreferenceFallbackMessage(state.preference.fallback) === null ? null : (
            <Text
              size="xs"
              style={themed($warning)}
              text={modelPreferenceFallbackMessage(state.preference.fallback) ?? ""}
            />
          )}
          {state.preference.availableModelIds.map(modelId => {
            const active = state.preference.effectiveModelId === modelId
            return (
              <Button
                key={modelId}
                preset={active ? "reversed" : "default"}
                disabled={selecting !== null}
                onPress={() => void handleSelect(modelId)}
                text={active ? `${modelDisplayLabel(modelId)} (active)` : modelDisplayLabel(modelId)}
              />
            )
          })}
        </>
      )}
    </SectionCard>
  )
}

type PushPermissionStatus = "loading" | "granted" | "denied" | "undetermined"

const NotificationsSection = () => {
  const { baseUrl, token } = useKhalaAuth()
  const { themed } = useAppTheme()
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
    <SectionCard heading="Notifications">
      {status === "loading" ? (
        <Text size="xs" style={themed($dim)} text="checking…" />
      ) : status === "granted" ? (
        <Text
          size="xs"
          style={themed($dim)}
          text="Enabled — you'll get a push when a task finishes or needs you."
        />
      ) : status === "denied" ? (
        <>
          <Text
            size="xs"
            style={themed($dim)}
            text="Notifications are turned off for Khala Code in your device Settings."
          />
          <Button preset="filled" text="Open device settings" onPress={() => void Linking.openSettings()} />
        </>
      ) : (
        <>
          <Text size="xs" style={themed($dim)} text="Get notified when a task finishes or needs your input." />
          <Button
            preset="filled"
            text="Enable notifications"
            disabled={requesting}
            onPress={() => void handleEnable()}
          />
        </>
      )}
    </SectionCard>
  )
}

const ON_DEVICE_TONE_COLOR: Record<OnDeviceReadinessRow["tone"], ThemedStyle<TextStyle>> = {
  danger: ({ colors }) => ({ color: colors.error }),
  faint: ({ colors }) => ({ color: colors.textDim }),
  success: ({ colors }) => ({ color: colors.palette.secondary300 }),
  warning: ({ colors }) => ({ color: colors.palette.accent200 }),
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
  const { themed } = useAppTheme()
  return (
    <SectionCard heading="About & diagnostics">
      <Text size="xs" style={themed($faint)} text={appVersionLabel()} />
      {readiness.status === "loading" ? (
        <Text size="xs" style={themed($dim)} text="checking…" />
      ) : readiness.status === "error" ? (
        <Text size="xs" style={themed($danger)} text="Could not read native module readiness." />
      ) : (
        readiness.rows.map(row => (
          <ListItem
            key={row.key}
            topSeparator
            height={44}
            text={row.label}
            TextProps={{ size: "xs" }}
            RightComponent={
              <View style={themed($readinessRight)}>
                <Text size="xs" style={themed(ON_DEVICE_TONE_COLOR[row.tone])} text={row.status} />
                {row.detail === undefined ? null : (
                  <Text size="xxs" style={themed($faint)} text={row.detail} />
                )}
              </View>
            }
          />
        ))
      )}
    </SectionCard>
  )
}

type SettingsScreenProps = AppDrawerScreenProps<"Settings">

export const SettingsScreen = ({ navigation }: SettingsScreenProps) => {
  const { themed } = useAppTheme()

  return (
    <Screen preset="scroll" contentContainerStyle={themed($contentContainer)}>
      <Header
        title="Settings"
        leftIcon="☰"
        onLeftPress={() => navigation.openDrawer()}
      />
      <View style={themed($body)}>
        <AccountSection />
        <CreditsSection onViewHistory={() => navigation.navigate("Main", { screen: "CreditsHistory" })} />
        <ModelsSection />
        <NotificationsSection />
        <AboutSection />
      </View>
    </Screen>
  )
}

const $contentContainer: ThemedStyle<ViewStyle> = () => ({
  flexGrow: 1,
})

const $body: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  paddingHorizontal: spacing.md,
  paddingVertical: spacing.md,
  gap: spacing.md,
})

const $sectionCard: ThemedStyle<ViewStyle> = ({ colors }) => ({
  backgroundColor: colors.palette.neutral200,
  borderColor: colors.palette.neutral400,
})

const $sectionBody: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  gap: spacing.xs,
})

const $modalScrim: ThemedStyle<ViewStyle> = ({ spacing, colors }) => ({
  flex: 1,
  justifyContent: "flex-end",
  paddingHorizontal: spacing.md,
  paddingVertical: spacing.lg,
  backgroundColor: colors.palette.overlay50,
})

const $modalCard: ThemedStyle<ViewStyle> = ({ colors }) => ({
  backgroundColor: colors.palette.neutral200,
  borderColor: colors.palette.neutral400,
})

const $readinessRight: ThemedStyle<ViewStyle> = () => ({
  alignItems: "flex-end",
  justifyContent: "center",
  flexShrink: 1,
})

const $dim: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.textDim })
const $faint: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.palette.neutral500 })
const $warning: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.palette.accent200 })
const $danger: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.error })
