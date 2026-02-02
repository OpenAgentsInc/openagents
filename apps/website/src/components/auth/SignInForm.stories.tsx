import type { Meta, StoryObj } from '@storybook/react-vite'
import { SignInFormInner } from './SignInForm'

const meta = {
  title: 'Auth/SignInForm',
  component: SignInFormInner,
  parameters: { layout: 'centered' },
  tags: ['autodocs'],
} satisfies Meta<typeof SignInFormInner>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {}
