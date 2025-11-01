import React from 'react'
import { View, TextInput, type TextInputProps, type StyleProp, type ViewStyle, type TextStyle } from 'react-native'
import { Colors } from '@/constants/theme'
import { Text } from '@/components/ui/Text'

export interface TextFieldProps extends Omit<TextInputProps, 'style'> {
  label?: string
  helperText?: string
  errorText?: string
  containerStyle?: StyleProp<ViewStyle>
  inputStyle?: StyleProp<TextStyle>
  left?: React.ReactNode
  right?: React.ReactNode
}

export const TextField = React.forwardRef<TextInput, TextFieldProps>(function TextField(
  { label, helperText, errorText, containerStyle, inputStyle, left, right, multiline, ...rest },
  ref
) {
  const borderColor = errorText ? Colors.destructive : Colors.border
  const caption = errorText ?? helperText
  const captionTone = errorText ? 'danger' : 'secondary'
  return (
    <View style={[{ gap: 6 }, containerStyle]}>
      {label ? <Text variant="label">{label}</Text> : null}
      <View style={{ flexDirection: 'row', alignItems: 'center', borderWidth: 1, borderColor, backgroundColor: Colors.card }}>
        {left}
        <TextInput
          ref={ref}
          multiline={multiline}
          placeholderTextColor={Colors.tertiary}
          style={[{ flex: 1, paddingVertical: multiline ? 8 : 6, paddingHorizontal: 10 }, inputStyle]}
          {...rest}
        />
        {right}
      </View>
      {caption ? <Text tone={captionTone as any} variant="caption">{caption}</Text> : null}
    </View>
  )
})
