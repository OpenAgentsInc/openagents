import type { Meta, StoryObj } from '@storybook/react-vite'
import type { BillingSummary } from '@/lib/openclawApi'
import { CreditsWidget } from './CreditsWidget'

const meta = {
  title: 'OpenClaw/CreditsWidget',
  component: CreditsWidget,
  parameters: { layout: 'centered' },
  tags: ['autodocs'],
} satisfies Meta<typeof CreditsWidget>

export default meta
type Story = StoryObj<typeof meta>

const mockSummary: BillingSummary = {
  user_id: 'user_123',
  balance_usd: 42.5,
}

export const WithBalance: Story = {
  args: { summary: mockSummary },
}

export const ZeroBalance: Story = {
  args: {
    summary: { user_id: 'user_123', balance_usd: 0 },
  },
}

export const Empty: Story = {
  args: { summary: null },
}
