import React from 'react'
import { Pressable, View } from 'react-native'
import { Ionicons } from '@expo/vector-icons'
import { Text } from '@/components/ui/Text'
import { Colors } from '@/constants/theme'

export interface CheckboxProps {
  label?: string
  value: boolean
  onValueChange?: (next: boolean) => void
  disabled?: boolean
}

export function Checkbox({ label, value, onValueChange, disabled }: CheckboxProps) {
  const toggle = () => { if (!disabled) onValueChange?.(!value) }
  return (
    <Pressable accessibilityRole="checkbox" onPress={toggle} disabled={disabled} style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
      <View style={{ width: 18, height: 18, borderWidth: 1, borderColor: Colors.border, backgroundColor: value ? Colors.foreground : Colors.transparent, alignItems: 'center', justifyContent: 'center' }}>
        {value ? <Ionicons name="checkmark" size={12} color={Colors.primaryForeground} /> : null}
      </View>
      {label ? <Text variant="body" tone={disabled ? 'tertiary' : 'default'}>{label}</Text> : null}
    </Pressable>
  )
}

