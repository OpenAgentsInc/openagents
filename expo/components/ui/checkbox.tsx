import React from 'react'
import { Pressable, View, type StyleProp, type ViewStyle } from 'react-native'
import { Colors } from '@/constants/theme'
import { Text } from './text'
import { Ionicons } from '@expo/vector-icons'

export type CheckboxProps = {
  label?: string
  checked: boolean
  onChange: (next: boolean) => void
  disabled?: boolean
  style?: StyleProp<ViewStyle>
  testID?: string
}

export function Checkbox({ label, checked, onChange, disabled = false, style, testID }: CheckboxProps) {
  return (
    <Pressable
      onPress={() => onChange(!checked)}
      accessibilityRole="checkbox"
      accessibilityState={{ checked, disabled }}
      disabled={disabled}
      testID={testID}
      style={[{ opacity: disabled ? 0.6 : 1 }, style]}
    >
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <View
          style={{
            width: 20,
            height: 20,
            borderWidth: 1,
            borderColor: checked ? Colors.quaternary : Colors.border,
            backgroundColor: checked ? Colors.quaternary : Colors.card,
            alignItems: 'center',
            justifyContent: 'center',
          }}
        >
          {checked ? <Ionicons name="checkmark" size={14} color={Colors.foreground} /> : null}
        </View>
        {!!label && <Text variant="body">{label}</Text>}
      </View>
    </Pressable>
  )
}

export default Checkbox

