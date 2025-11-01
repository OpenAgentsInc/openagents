import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
import { HapticTab } from '@/components/haptic-tab'
import { Colors } from '@/constants/theme'
import type { BottomTabBarButtonProps } from '@react-navigation/bottom-tabs'

const meta = { title: 'App/HapticTab' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

function Wrapper() {
  const props: BottomTabBarButtonProps = {
    route: { key: 'tab-1', name: 'TabOne' } as any,
    children: <Text style={{ color: Colors.foreground }}>Tab</Text>,
    onPress: () => {},
    onLongPress: () => {},
    onPressIn: () => {},
    onPressOut: () => {},
    accessibilityRole: 'button',
    accessibilityState: {},
    accessibilityLabel: 'Tab',
    testID: 'haptic-tab',
    style: { padding: 12, borderWidth: 1, borderColor: Colors.border },
  }
  return (
    <View style={{ padding: 16 }}>
      <HapticTab {...props} />
    </View>
  )
}

export const Basic: Story = {
  render: () => <Wrapper />,
}

