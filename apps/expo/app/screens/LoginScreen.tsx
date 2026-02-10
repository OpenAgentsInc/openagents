import { ComponentType, FC, useMemo, useRef, useState } from "react"
// eslint-disable-next-line no-restricted-imports
import { TextInput, TextStyle, ViewStyle } from "react-native"

import { Button } from "@/components/Button"
import { PressableIcon } from "@/components/Icon"
import { Screen } from "@/components/Screen"
import { Text } from "@/components/Text"
import { TextField, type TextFieldAccessoryProps } from "@/components/TextField"
import { useAuth } from "@/context/AuthContext"
import type { AppStackScreenProps } from "@/navigators/navigationTypes"
import { authStart, authVerify } from "@/services/authApi"
import { useAppTheme } from "@/theme/context"
import type { ThemedStyle } from "@/theme/types"

function emailValidationError(email: string): string {
  const trimmed = email.trim()
  if (!trimmed) return "can't be blank"
  if (trimmed.length < 6) return "must be at least 6 characters"
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(trimmed)) return "must be a valid email address"
  return ""
}

type LoginStep = "email" | "code"

interface LoginScreenProps extends AppStackScreenProps<"Login"> {}

export const LoginScreen: FC<LoginScreenProps> = () => {
  const codeInputRef = useRef<TextInput>(null)
  const [step, setStep] = useState<LoginStep>("email")
  const [email, setEmail] = useState("")
  const [code, setCode] = useState("")
  const [isBusy, setIsBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [codeMasked, setCodeMasked] = useState(false)
  const { setSession } = useAuth()
  const { themed, theme: { colors } } = useAppTheme()

  const emailError = step === "email" ? emailValidationError(email) : ""
  const displayError = error ?? (step === "email" ? emailError : "")

  const handleSendCode = async () => {
    if (isBusy) return
    const normalizedEmail = email.trim().toLowerCase()
    if (!normalizedEmail || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(normalizedEmail)) return
    setError(null)
    setIsBusy(true)
    const result = await authStart(normalizedEmail)
    setIsBusy(false)
    if (result.ok) {
      setEmail(normalizedEmail)
      setCode("")
      setStep("code")
      setTimeout(() => codeInputRef.current?.focus(), 300)
    } else {
      setError(result.error === "invalid_email" ? "Please enter a valid email." : "Failed to send code. Try again.")
    }
  }

  const handleVerifyCode = async () => {
    if (isBusy) return
    const normalizedCode = code.replace(/\s+/g, "")
    if (!normalizedCode) return
    setError(null)
    setIsBusy(true)
    const result = await authVerify(email, normalizedCode)
    setIsBusy(false)
    if (result.ok) {
      console.log("[LoginScreen] verify ok, token present:", !!result.token)
      setSession({
        userId: result.userId,
        email,
        token: result.token,
        user: { id: result.userId, email, firstName: null, lastName: null },
      })
    } else {
      setError(result.error === "invalid_code" ? "Invalid code. Please try again." : "Verification failed. Try again.")
    }
  }

  const handleBackToEmail = () => {
    if (isBusy) return
    setError(null)
    setCode("")
    setStep("email")
  }

  const handleResendCode = async () => {
    if (isBusy) return
    setError(null)
    setIsBusy(true)
    const result = await authStart(email)
    setIsBusy(false)
    if (!result.ok) {
      setError("Failed to resend code. Try again.")
    }
  }

  const CodeRightAccessory: ComponentType<TextFieldAccessoryProps> = useMemo(
    () =>
      function CodeRightAccessory(props: TextFieldAccessoryProps) {
        return (
          <PressableIcon
            icon={codeMasked ? "view" : "hidden"}
            color={colors.palette.neutral800}
            containerStyle={props.style}
            size={20}
            onPress={() => setCodeMasked(!codeMasked)}
          />
        )
      },
    [codeMasked, colors.palette.neutral800],
  )

  return (
    <Screen
      preset="auto"
      contentContainerStyle={themed($screenContentContainer)}
      safeAreaEdges={["top", "bottom"]}
    >
      <Text testID="login-heading" tx="loginScreen:logIn" preset="heading" style={themed($logIn)} />
      <Text tx="loginScreen:enterDetails" preset="subheading" style={themed($enterDetails)} />

      {step === "email" && (
        <>
          <TextField
            value={email}
            onChangeText={(t) => { setEmail(t); setError(null) }}
            containerStyle={themed($textField)}
            autoCapitalize="none"
            autoComplete="email"
            autoCorrect={false}
            keyboardType="email-address"
            labelTx="loginScreen:emailFieldLabel"
            placeholderTx="loginScreen:emailFieldPlaceholder"
            helper={displayError}
            status={displayError ? "error" : undefined}
            editable={!isBusy}
          />
          <Button
            testID="login-button"
            tx="loginScreen:sendCode"
            style={themed($tapButton)}
            preset="reversed"
            onPress={handleSendCode}
            disabled={isBusy || !!emailError}
          />
        </>
      )}

      {step === "code" && (
        <>
          <Text text={email} size="sm" style={themed($emailLabel)} />
          <TextField
            ref={codeInputRef}
            value={code}
            onChangeText={(t) => { setCode(t); setError(null) }}
            containerStyle={themed($textField)}
            autoCapitalize="none"
            autoComplete="one-time-code"
            autoCorrect={false}
            keyboardType="number-pad"
            labelTx="loginScreen:codeFieldLabel"
            placeholderTx="loginScreen:codeFieldPlaceholder"
            helper={displayError}
            status={displayError ? "error" : undefined}
            secureTextEntry={codeMasked}
            editable={!isBusy}
            RightAccessory={CodeRightAccessory}
          />
          <Button
            testID="login-verify-button"
            tx="loginScreen:verifyCode"
            style={themed($tapButton)}
            preset="reversed"
            onPress={handleVerifyCode}
            disabled={isBusy || code.replace(/\s+/g, "").length < 4}
          />
          <Button
            tx="loginScreen:resendCode"
            style={themed($secondaryButton)}
            preset="default"
            onPress={handleResendCode}
            disabled={isBusy}
          />
          <Button
            tx="loginScreen:backToEmail"
            style={themed($secondaryButton)}
            preset="default"
            onPress={handleBackToEmail}
            disabled={isBusy}
          />
        </>
      )}
    </Screen>
  )
}

const $screenContentContainer: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  paddingVertical: spacing.xxl,
  paddingHorizontal: spacing.lg,
})

const $logIn: ThemedStyle<TextStyle> = ({ spacing }) => ({
  marginBottom: spacing.sm,
})

const $enterDetails: ThemedStyle<TextStyle> = ({ spacing }) => ({
  marginBottom: spacing.lg,
})

const $emailLabel: ThemedStyle<TextStyle> = ({ spacing }) => ({
  marginBottom: spacing.xs,
})

const $textField: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  marginBottom: spacing.lg,
})

const $tapButton: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  marginTop: spacing.xs,
})

const $secondaryButton: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  marginTop: spacing.sm,
})

