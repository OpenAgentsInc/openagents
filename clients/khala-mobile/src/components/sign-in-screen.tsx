import { useState } from "react"
import { ActivityIndicator, Pressable, Text, TextInput, View } from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { tx } from "../i18n/copy"

/**
 * Shown whenever there is no signed-in session yet. Per the owner mandate
 * (2026-07-04, verbatim): "IF THERES A DEVICE ON TAILNET THATS AUTHED, USE
 * THAT AUTOMATICALLY - NO LOGIN SCREEN" — so the PRIMARY path here is a
 * Tailnet auto-discovery status (`KhalaAuthProvider` already ran or is
 * running it before this ever renders manual fields), not a form. Manual
 * entry survives only as a secondary "advanced" fallback for the case where
 * auto-discovery genuinely can't find a signed-in Mac (phone off Tailnet, no
 * desktop running/signed in, first-time setup, phone-only user).
 */
export const SignInScreen = () => {
  const { discoveryOutcome, retryDiscovery, signIn, status } = useKhalaAuth()
  const [showManualForm, setShowManualForm] = useState(false)

  if (!showManualForm) {
    return (
      <AutoDiscoveryPanel
        discoveryOutcome={discoveryOutcome}
        onShowManualForm={() => setShowManualForm(true)}
        onRetry={retryDiscovery}
        status={status}
      />
    )
  }

  return <ManualSignInForm onBack={() => setShowManualForm(false)} signIn={signIn} />
}

const discoveryMessage = (
  status: ReturnType<typeof useKhalaAuth>["status"],
  discoveryOutcome: ReturnType<typeof useKhalaAuth>["discoveryOutcome"]
): string => {
  if (status === "discovering") return tx("signIn.discovery.looking")
  if (discoveryOutcome?.state === "reachable_not_signed_in") {
    return discoveryOutcome.hostname === null
      ? tx("signIn.discovery.reachableNotSignedIn")
      : tx("signIn.discovery.reachableNotSignedInOnHost", { hostname: discoveryOutcome.hostname })
  }
  return tx("signIn.discovery.noSignedInMac")
}

const AutoDiscoveryPanel = ({
  discoveryOutcome,
  onRetry,
  onShowManualForm,
  status
}: {
  discoveryOutcome: ReturnType<typeof useKhalaAuth>["discoveryOutcome"]
  onRetry: () => Promise<void>
  onShowManualForm: () => void
  status: ReturnType<typeof useKhalaAuth>["status"]
}) => {
  const [retrying, setRetrying] = useState(false)
  const discovering = status === "discovering"

  const handleRetry = async () => {
    if (retrying || discovering) return
    setRetrying(true)
    try {
      await onRetry()
    } finally {
      setRetrying(false)
    }
  }

  return (
    <SafeAreaView className="flex-1 bg-bg" edges={["top", "bottom", "left", "right"]}>
      <View className="flex-1 justify-center px-6">
        <Text className="mb-1 text-center font-sans text-2xl font-semibold text-text">{tx("app.title")}</Text>

        <View className="mb-8 mt-6 items-center">
          {discovering || retrying ? <ActivityIndicator color="#4fd0ff" /> : null}
          <Text className="mt-4 text-center font-sans text-sm text-textMuted">
            {discoveryMessage(status, discoveryOutcome)}
          </Text>
          <Text className="mt-2 text-center font-mono text-xs text-textFaint">
            {tx("signIn.discovery.help")}
          </Text>
        </View>

        <Pressable
          accessibilityRole="button"
          className={`items-center rounded-xl py-3 ${discovering || retrying ? "bg-surfaceMuted" : "bg-accent"}`}
          disabled={discovering || retrying}
          onPress={handleRetry}
        >
          {retrying ? (
            <ActivityIndicator color="#000" />
          ) : (
            <Text className="font-sans text-base font-semibold text-bg">{tx("signIn.retry")}</Text>
          )}
        </Pressable>

        <Pressable accessibilityRole="button" className="mt-4 items-center py-2" onPress={onShowManualForm}>
          <Text className="font-mono text-xs uppercase tracking-wide text-textFaint">{tx("signIn.manualInstead")}</Text>
        </Pressable>
      </View>
    </SafeAreaView>
  )
}

const ManualSignInForm = ({
  onBack,
  signIn
}: {
  onBack: () => void
  signIn: ReturnType<typeof useKhalaAuth>["signIn"]
}) => {
  const [ownerUserId, setOwnerUserId] = useState("")
  const [token, setToken] = useState("")
  const [submitting, setSubmitting] = useState(false)
  const [errorMessage, setErrorMessage] = useState<string | null>(null)

  const canSubmit = ownerUserId.trim().length > 0 && token.trim().length > 0 && !submitting

  const handleSubmit = async () => {
    if (!canSubmit) return
    setSubmitting(true)
    setErrorMessage(null)
    const result = await signIn({ ownerUserId, token })
    setSubmitting(false)
    if (!result.ok) setErrorMessage(result.messageSafe)
  }

  return (
    <SafeAreaView className="flex-1 bg-bg" edges={["top", "bottom", "left", "right"]}>
      <View className="flex-1 justify-center px-6">
        <Text className="mb-1 text-center font-sans text-2xl font-semibold text-text">{tx("app.title")}</Text>
        <Text className="mb-8 text-center font-sans text-sm text-textMuted">
          {tx("signIn.manual.subtitle")}
        </Text>

        <Text className="mb-1 font-mono text-xs uppercase tracking-wide text-textFaint">{tx("signIn.manual.ownerUserId")}</Text>
        <TextInput
          autoCapitalize="none"
          autoCorrect={false}
          className="mb-4 rounded-xl border border-border bg-surfaceRaised px-3 py-2 font-mono text-sm text-text"
          onChangeText={setOwnerUserId}
          placeholder="user_..."
          placeholderTextColor="#7e8a98"
          value={ownerUserId}
        />

        <Text className="mb-1 font-mono text-xs uppercase tracking-wide text-textFaint">{tx("signIn.manual.token")}</Text>
        <TextInput
          autoCapitalize="none"
          autoCorrect={false}
          className="mb-2 rounded-xl border border-border bg-surfaceRaised px-3 py-2 font-mono text-sm text-text"
          onChangeText={setToken}
          placeholder="oa_agent_..."
          placeholderTextColor="#7e8a98"
          secureTextEntry
          value={token}
        />

        {errorMessage === null ? null : (
          <Text className="mb-4 font-mono text-xs text-danger">{errorMessage}</Text>
        )}

        <Pressable
          accessibilityRole="button"
          className={`mt-4 items-center rounded-xl py-3 ${canSubmit ? "bg-accent" : "bg-surfaceMuted"}`}
          disabled={!canSubmit}
          onPress={handleSubmit}
        >
          {submitting ? <ActivityIndicator color="#000" /> : <Text className="font-sans text-base font-semibold text-bg">{tx("signIn.manual.submit")}</Text>}
        </Pressable>

        <Pressable accessibilityRole="button" className="mt-4 items-center py-2" onPress={onBack}>
          <Text className="font-mono text-xs uppercase tracking-wide text-textFaint">
            {tx("signIn.manual.backToDiscovery")}
          </Text>
        </Pressable>

        <Text className="mt-6 text-center font-mono text-xs text-textFaint">
          Find your token and user id from a linked OpenAgents Pylon or the desktop app's account settings.
        </Text>
      </View>
    </SafeAreaView>
  )
}
