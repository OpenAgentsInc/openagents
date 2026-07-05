import { useState } from "react"
import { View } from "react-native"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { tx } from "../i18n/copy"
import { KhalaButton } from "./khala-button"
import { KhalaEmptyState } from "./khala-empty-state"
import { KhalaScreen } from "./khala-screen"
import { KhalaText } from "./khala-text"
import { KhalaTextField } from "./khala-text-field"

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
    <KhalaScreen contentClassName="justify-center px-6">
      <KhalaText className="mb-1 text-center" variant="heading">
        {tx("app.title")}
      </KhalaText>

      <KhalaEmptyState
        className="mb-4 mt-6 py-6"
        detail={tx("signIn.discovery.help")}
        loading={discovering || retrying}
        title={discoveryMessage(status, discoveryOutcome)}
        tone={discoveryOutcome?.state === "reachable_not_signed_in" ? "accent" : "muted"}
      />

      <KhalaButton
        disabled={discovering || retrying}
        loading={retrying}
        onPress={handleRetry}
        text={tx("signIn.retry")}
        variant="primary"
      />

      <KhalaButton
        className="mt-3"
        onPress={onShowManualForm}
        text={tx("signIn.manualInstead")}
        textClassName="font-mono text-xs uppercase tracking-wide text-textFaint"
        variant="ghost"
      />
    </KhalaScreen>
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
    <KhalaScreen contentClassName="justify-center px-6">
      <View>
        <KhalaText className="mb-1 text-center" variant="heading">
          {tx("app.title")}
        </KhalaText>
        <KhalaText className="mb-8 text-center" variant="muted">
          {tx("signIn.manual.subtitle")}
        </KhalaText>

        <KhalaTextField
          autoCapitalize="none"
          className="mb-4"
          label={tx("signIn.manual.ownerUserId")}
          onChangeText={setOwnerUserId}
          placeholder="user_..."
          value={ownerUserId}
        />

        <KhalaTextField
          autoCapitalize="none"
          className="mb-2"
          label={tx("signIn.manual.token")}
          onChangeText={setToken}
          placeholder="oa_agent_..."
          secureTextEntry
          value={token}
        />

        {errorMessage === null ? null : (
          <KhalaText className="mb-4" variant="danger">{errorMessage}</KhalaText>
        )}

        <KhalaButton
          disabled={!canSubmit}
          loading={submitting}
          onPress={handleSubmit}
          text={tx("signIn.manual.submit")}
          variant="primary"
        />

        <KhalaButton
          className="mt-3"
          onPress={onBack}
          text={tx("signIn.manual.backToDiscovery")}
          textClassName="font-mono text-xs uppercase tracking-wide text-textFaint"
          variant="ghost"
        />

        <KhalaText className="mt-6 text-center" variant="faint">
          Find your token and user id from a linked OpenAgents Pylon or the desktop app's account settings.
        </KhalaText>
      </View>
    </KhalaScreen>
  )
}
