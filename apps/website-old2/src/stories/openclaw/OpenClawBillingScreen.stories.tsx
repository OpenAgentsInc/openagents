import type { Meta, StoryObj } from '@storybook/react-vite'
import type { BillingSummary } from '@/lib/openclawApi'
import { OpenClawBillingScreen } from '@/components/openclaw/screens/OpenClawBillingScreen'

const meta = {
  title: 'OpenClaw/Screens/Billing',
  component: OpenClawBillingScreen,
  parameters: { layout: 'fullscreen' },
  tags: ['autodocs'],
} satisfies Meta<typeof OpenClawBillingScreen>

export default meta

type Story = StoryObj<typeof meta>

const summary: BillingSummary = {
  user_id: 'user_123',
  balance_usd: 42.5,
}

export const WithBalance: Story = {
  args: {
    summary,
    error: null,
  },
}

export const Empty: Story = {
  args: {
    summary: null,
    error: null,
  },
}

export const ErrorState: Story = {
  args: {
    summary: null,
    error: 'Failed to load billing',
  },
}
