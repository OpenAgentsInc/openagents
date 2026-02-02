import type { Meta, StoryObj } from '@storybook/react-vite'
import { SignUpFormInner } from './SignUpForm'

const meta = {
  title: 'Auth/SignUpForm',
  component: SignUpFormInner,
  parameters: { layout: 'centered' },
  tags: ['autodocs'],
} satisfies Meta<typeof SignUpFormInner>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {}
