import type { Meta, StoryObj } from '@storybook/react-vite'
import { AuthUIView, type AuthUIUserState } from './AuthUI'

const meta = {
  title: 'Auth/AuthUI',
  component: AuthUIView,
  parameters: { layout: 'centered' },
  tags: ['autodocs'],
  argTypes: {
    user: {
      control: false,
      description: 'User state: undefined = loading, null = logged out, object = logged in',
    },
  },
} satisfies Meta<typeof AuthUIView>

export default meta
type Story = StoryObj<typeof meta>

export const Loading: Story = {
  args: { user: undefined as AuthUIUserState },
}

export const LoggedOut: Story = {
  args: { user: null },
}

export const LoggedInWithName: Story = {
  args: {
    user: { name: 'Alice', email: 'alice@example.com' },
  },
}

export const LoggedInEmailOnly: Story = {
  args: {
    user: { email: 'user@example.com' },
  },
}
