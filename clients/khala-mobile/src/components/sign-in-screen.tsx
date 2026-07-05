import { useState } from "react"
import { ActivityIndicator, Pressable, Text, TextInput, View } from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import { useKhalaAuth } from "../auth/khala-auth-context"

/** Shown whenever no valid Khala Sync credentials are stored on-device —
 * the only path into the app for a real (non-dev-env) install, since a
 * shipped binary never has EXPO_PUBLIC_KHALA_SYNC_DEMO_* baked in. */
export const SignInScreen = () => {
  const { signIn } = useKhalaAuth()
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
        <Text className="mb-1 text-center font-sans text-2xl font-semibold text-text">Khala Code</Text>
        <Text className="mb-8 text-center font-sans text-sm text-textMuted">
          Sign in with your OpenAgents account to sync chats and your fleet.
        </Text>

        <Text className="mb-1 font-mono text-xs uppercase tracking-wide text-textFaint">Owner user id</Text>
        <TextInput
          autoCapitalize="none"
          autoCorrect={false}
          className="mb-4 rounded-xl border border-border bg-surfaceRaised px-3 py-2 font-mono text-sm text-text"
          onChangeText={setOwnerUserId}
          placeholder="user_..."
          placeholderTextColor="#7e8a98"
          value={ownerUserId}
        />

        <Text className="mb-1 font-mono text-xs uppercase tracking-wide text-textFaint">OpenAgents token</Text>
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
          {submitting ? <ActivityIndicator color="#000" /> : <Text className="font-sans text-base font-semibold text-bg">Sign in</Text>}
        </Pressable>

        <Text className="mt-6 text-center font-mono text-xs text-textFaint">
          Find your token and user id from a linked OpenAgents Pylon or the desktop app's account settings.
        </Text>
      </View>
    </SafeAreaView>
  )
}
