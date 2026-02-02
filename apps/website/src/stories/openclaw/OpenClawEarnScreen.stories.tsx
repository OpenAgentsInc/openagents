import type { Meta, StoryObj } from '@storybook/react-vite'
import { OpenClawEarnScreen, type InvoicePayload } from '@/components/openclaw/screens/OpenClawEarnScreen'

const meta = {
  title: 'OpenClaw/Screens/Earn',
  component: OpenClawEarnScreen,
  parameters: { layout: 'fullscreen' },
  tags: ['autodocs'],
} satisfies Meta<typeof OpenClawEarnScreen>

export default meta

type Story = StoryObj<typeof meta>

const invoice: InvoicePayload = {
  payment_request: 'lnbc1230n1pwx0w2xpp5a5u7z0xdlzz0w3d3k5t0ksk4dq9xw5rn6up8eqw4xv5s4tc9qdqqcqzzsxqyz5vqsp5p98u8cy7tksxwr0s9gyywjx9k24j85ed6d8g2l6z2d6d4v30s9qyyssqjyjsa2y7u0a3ggw2ct5x8j8ql0lglxk3qxg3n0e0m86ljk8snnn2zsx5g5p5u9llr3y3fznz3sp2s8l6y49u7ckr8lfa9m7jx5gqx2c5s9',
  amount_sats: 12000,
  description: 'OpenClaw monthly credits',
  expires_at: new Date(Date.now() + 3600000).toISOString(),
  expires_at_ms: Date.now() + 3600000,
}

const qrPlaceholder = (
  <div className="flex size-[220px] items-center justify-center rounded-xl border border-dashed border-border text-xs text-muted-foreground">
    QR preview
  </div>
)

export const Loading: Story = {
  args: {
    state: 'loading',
    invoice: null,
    error: null,
    lastFetched: null,
    copyState: 'idle',
    qrNode: null,
  },
}

export const Empty: Story = {
  args: {
    state: 'empty',
    invoice: null,
    error: null,
    lastFetched: Date.now(),
    copyState: 'idle',
    qrNode: null,
  },
}

export const ErrorState: Story = {
  args: {
    state: 'error',
    invoice: null,
    error: 'Invoice fetch failed.',
    lastFetched: Date.now(),
    copyState: 'idle',
    qrNode: null,
  },
}

export const Ready: Story = {
  args: {
    state: 'ready',
    invoice,
    error: null,
    lastFetched: Date.now(),
    copyState: 'idle',
    qrNode: qrPlaceholder,
  },
}

export const Copied: Story = {
  args: {
    state: 'ready',
    invoice,
    error: null,
    lastFetched: Date.now(),
    copyState: 'copied',
    qrNode: qrPlaceholder,
  },
}

export const CopyFailed: Story = {
  args: {
    state: 'ready',
    invoice,
    error: null,
    lastFetched: Date.now(),
    copyState: 'failed',
    qrNode: qrPlaceholder,
  },
}
