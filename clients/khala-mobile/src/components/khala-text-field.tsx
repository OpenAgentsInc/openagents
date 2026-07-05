import { forwardRef } from "react"
import { TextInput, View, type TextInputProps } from "react-native"

import { khalaMobileTheme } from "../theme/tokens"
import { KhalaText } from "./khala-text"

export type KhalaTextFieldProps = TextInputProps &
  Readonly<{
    className?: string
    disabled?: boolean
    errorText?: string | null
    label: string
    mono?: boolean
  }>

export const KhalaTextField = forwardRef<TextInput, KhalaTextFieldProps>(
  (
    {
      accessibilityLabel,
      accessibilityState,
      className = "",
      disabled = false,
      editable,
      errorText,
      label,
      mono = true,
      placeholderTextColor = khalaMobileTheme.textFaint,
      ...props
    },
    ref,
  ) => {
    const inputDisabled = disabled || editable === false
    const invalid = errorText !== undefined && errorText !== null && errorText.length > 0

    return (
      <View className={className}>
        <KhalaText className="mb-1" variant="label">
          {label}
        </KhalaText>
        <TextInput
          {...props}
          accessibilityLabel={accessibilityLabel ?? label}
          accessibilityState={{ ...accessibilityState, disabled: inputDisabled }}
          aria-invalid={invalid}
          autoCorrect={props.autoCorrect ?? false}
          className={`min-h-11 rounded-lg border px-3 py-2 text-sm text-text ${
            mono ? "font-mono" : "font-sans"
          } ${invalid ? "border-danger bg-danger/10" : "border-border bg-surfaceRaised"} ${
            inputDisabled ? "opacity-50" : ""
          }`.trim()}
          editable={!inputDisabled}
          placeholderTextColor={placeholderTextColor}
          ref={ref}
        />
        {invalid ? (
          <KhalaText className="mt-1" variant="danger">
            {errorText}
          </KhalaText>
        ) : null}
      </View>
    )
  },
)

KhalaTextField.displayName = "KhalaTextField"
