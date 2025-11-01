import React from 'react'
import { TextInput, View, type StyleProp, type ViewStyle, type TextStyle } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { Text } from './text'

export type TextFieldProps = {
  label?: string
  value: string
  onChangeText: (text: string) => void
  placeholder?: string
  secureTextEntry?: boolean
  keyboardType?: React.ComponentProps<typeof TextInput>['keyboardType']
  multiline?: boolean
  editable?: boolean
  helperText?: string
  errorText?: string
  left?: React.ReactNode
  right?: React.ReactNode
  containerStyle?: StyleProp<ViewStyle>
  inputStyle?: StyleProp<TextStyle>
  testID?: string
}

export function TextField({
  label,
  value,
  onChangeText,
  placeholder,
  secureTextEntry,
  keyboardType,
  multiline = false,
  editable = true,
  helperText,
  errorText,
  left,
  right,
  containerStyle,
  inputStyle,
  testID,
}: TextFieldProps) {
  const hasError = typeof errorText === 'string' && errorText.length > 0

  return (
    <View style={containerStyle}>
      {!!label && (
        <Text variant="label" tone={hasError ? 'danger' : 'secondary'} style={{ marginBottom: 6 }}>
          {label}
        </Text>
      )}
      <View
        style={{
          borderWidth: 1,
          borderColor: hasError ? Colors.destructive : Colors.border,
          backgroundColor: Colors.card,
          borderRadius: 0,
          flexDirection: 'row',
          alignItems: 'center',
        }}
      >
        {left}
        <TextInput
          testID={testID}
          value={value}
          onChangeText={onChangeText}
          placeholder={placeholder}
          placeholderTextColor={Colors.tertiary}
          secureTextEntry={secureTextEntry}
          keyboardType={keyboardType}
          multiline={multiline}
          editable={editable}
          style={[
            {
              flex: 1,
              paddingHorizontal: 12,
              paddingVertical: multiline ? 10 : 8,
              fontFamily: Typography.primary,
              color: Colors.foreground,
              minHeight: multiline ? 84 : 40,
            },
            inputStyle,
          ]}
        />
        {right}
      </View>
      {!!helperText && !hasError && (
        <Text tone="tertiary" variant="caption" style={{ marginTop: 6 }}>
          {helperText}
        </Text>
      )}
      {!!hasError && (
        <Text tone="danger" variant="caption" style={{ marginTop: 6 }}>
          {errorText}
        </Text>
      )}
    </View>
  )}

export default TextField

