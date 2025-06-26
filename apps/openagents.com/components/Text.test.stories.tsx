import type { Meta, StoryObj } from '@storybook/nextjs'
import { Text, Animator, Animated } from '@arwes/react'

// Minimal test story to verify Text component rendering
const meta = {
  title: 'Arwes/Text Test',
  component: Text,
  parameters: {
    layout: 'centered',
  },
} satisfies Meta<typeof Text>

export default meta
type Story = StoryObj<typeof meta>

// Basic test without wrapper
export const BasicText: Story = {
  args: {
    children: 'Basic text without animation wrapper',
    as: 'div',
  },
}

// Test with Animator
export const WithAnimator: Story = {
  render: () => (
    <Animator root active duration={{ enter: 1, exit: 0.5 }}>
      <Text as="div">Text with Animator wrapper</Text>
    </Animator>
  ),
}

// Test with Animator and Animated
export const WithAnimatorAndAnimated: Story = {
  render: () => (
    <Animator root active duration={{ enter: 1, exit: 0.5 }}>
      <Animated>
        <Text as="div">Text with Animator and Animated wrapper</Text>
      </Animated>
    </Animator>
  ),
}

// Test sequence animation
export const SequenceTest: Story = {
  render: () => (
    <Animator root active>
      <Animated>
        <Text as="div" manager="sequence">
          Testing sequence animation
        </Text>
      </Animated>
    </Animator>
  ),
}

// Test decipher animation
export const DecipherTest: Story = {
  render: () => (
    <Animator root active>
      <Animated>
        <Text as="div" manager="decipher">
          TESTING DECIPHER
        </Text>
      </Animated>
    </Animator>
  ),
}

// Test with inline content
export const InlineContent: Story = {
  render: () => (
    <Animator root active>
      <Animated>
        <Text as="span">This is inline text</Text>
      </Animated>
    </Animator>
  ),
}

// Test with paragraph
export const ParagraphContent: Story = {
  render: () => (
    <Animator root active>
      <Animated>
        <div>
          <Text>This is a paragraph rendered as default p tag.</Text>
        </div>
      </Animated>
    </Animator>
  ),
}