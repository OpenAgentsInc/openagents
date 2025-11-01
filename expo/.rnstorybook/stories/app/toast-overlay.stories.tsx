import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
import { ToastOverlay } from '@/components/ToastOverlay'
import { Colors } from '@/constants/theme'
import { toast } from '@/lib/toast-store'

const meta = { title: 'App/ToastOverlay' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

function Demo() {
  React.useEffect(() => {
    toast('Info message', { type: 'info', duration: 2000 })
    toast('Success', { type: 'success', duration: 2400 })
    toast('Error occurred', { type: 'error', duration: 2600 })
  }, [])
  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <Text style={{ color: Colors.secondary, margin: 16 }}>Toasts enqueue on mount and auto-dismiss.</Text>
      <ToastOverlay />
    </View>
  )
}

export const Basic: Story = {
  render: () => <Demo />,
}
