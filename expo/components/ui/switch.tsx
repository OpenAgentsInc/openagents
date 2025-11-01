import React from 'react'
import { View, Switch as RNSwitch, type StyleProp, type ViewStyle } from 'react-native'
import { Colors } from '@/constants/theme'
import { Text } from './text'

export type SwitchProps = {
  label?: string
  value: boolean
  onValueChange: (next: boolean) => void
  disabled?: boolean
  style?: StyleProp<ViewStyle>
  testID?: string
}

export function Switch({ label, value, onValueChange, disabled = false, style, testID }: SwitchProps) {
  return (
    <View style={[{ flexDirection: 'row', alignItems: 'center', gap: 8 }, style]}>
      {!!label && <Text variant="body" tone={disabled ? 'tertiary' : 'default'}>{label}</Text>}
      <RNSwitch
        testID={testID}
        value={value}
        onValueChange={onValueChange}
        disabled={disabled}
        trackColor={{ false: Colors.border, true: Colors.quaternary }}
        thumbColor={value ? Colors.foreground : Colors.secondary}
      />
    </View>
  )}

export default Switch

