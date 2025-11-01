import React from 'react'
import { View, Switch as RNSwitch, type SwitchProps as RNSwitchProps } from 'react-native'
import { Text } from '@/components/ui/Text'
import { Colors } from '@/constants/theme'

export interface SwitchProps extends RNSwitchProps {
  label?: string
}

export function Switch({ label, value, onValueChange, disabled, ...rest }: SwitchProps) {
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}>
      {label ? <Text variant="label" tone={disabled ? 'tertiary' : 'default'}>{label}</Text> : null}
      <RNSwitch
        value={!!value}
        onValueChange={onValueChange}
        disabled={disabled}
        trackColor={{ false: Colors.border, true: Colors.quaternary }}
        thumbColor={disabled ? Colors.gray : Colors.foreground}
        {...rest}
      />
    </View>
  )
}

