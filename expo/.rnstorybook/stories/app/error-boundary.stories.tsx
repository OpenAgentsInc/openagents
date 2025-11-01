import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text, Pressable } from 'react-native'
import { ErrorBoundary } from '@/components/ErrorBoundary'
import { Colors } from '@/constants/theme'

const meta = { title: 'App/ErrorBoundary' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

function Boom() {
  const [throwNow, setThrow] = React.useState(false)
  React.useEffect(() => {
    if (throwNow) throw new Error('Example error for ErrorBoundary')
  }, [throwNow])
  return (
    <View style={{ padding: 16 }}>
      <Text style={{ color: Colors.foreground }}>Press to trigger an error.</Text>
      <Pressable onPress={() => setThrow(true)} style={{ marginTop: 12, borderWidth: 1, borderColor: Colors.border, padding: 8 }}>
        <Text style={{ color: Colors.secondary }}>Throw</Text>
      </Pressable>
    </View>
  )
}

export const Basic: Story = {
  render: () => (
    <ErrorBoundary>
      <Boom />
    </ErrorBoundary>
  ),
}
