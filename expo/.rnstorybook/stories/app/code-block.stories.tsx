import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
import { CodeBlock } from '@/components/CodeBlock'
import { Colors } from '@/constants/theme'

const meta = { title: 'App/CodeBlock' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

export const Typescript: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16, gap: 12 }}>
      <CodeBlock code={'const x: number = 42\nconsole.log(x)'} language="ts" />
      <Text style={{ color: Colors.secondary }}>Component: CodeBlock</Text>
      <Text style={{ color: Colors.foreground }}>Syntax-highlighted code using prism-react-renderer.</Text>
    </View>
  ),
}

export const Bash: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <CodeBlock code={'rg -n "openagents"'} language="bash" />
    </View>
  ),
}
