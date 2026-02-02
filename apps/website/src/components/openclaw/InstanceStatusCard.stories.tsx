import type { Meta, StoryObj } from '@storybook/react-vite'
import { fn } from 'storybook/test'
import type { InstanceSummary } from '@/lib/openclawApi'
import { InstanceStatusCard } from './InstanceStatusCard'

const meta = {
  title: 'OpenClaw/InstanceStatusCard',
  component: InstanceStatusCard,
  parameters: { layout: 'centered' },
  tags: ['autodocs'],
  args: { onCreate: fn() },
} satisfies Meta<typeof InstanceStatusCard>

export default meta
type Story = StoryObj<typeof meta>

const now = Date.now()
const readyInstance: InstanceSummary = {
  status: 'ready',
  runtime_name: 'openclaw-prod-1',
  created_at: now - 86400000,
  updated_at: now,
  last_ready_at: now,
}

const provisioningInstance: InstanceSummary = {
  status: 'provisioning',
  runtime_name: null,
  created_at: now - 60000,
  updated_at: now,
  last_ready_at: null,
}

const errorInstance: InstanceSummary = {
  status: 'error',
  runtime_name: 'openclaw-failed',
  created_at: now - 3600000,
  updated_at: now,
  last_ready_at: null,
}

export const NotCreated: Story = {
  args: { instance: null },
}

export const Ready: Story = {
  args: { instance: readyInstance },
}

export const Provisioning: Story = {
  args: { instance: provisioningInstance },
}

export const Error: Story = {
  args: { instance: errorInstance },
}

export const Creating: Story = {
  args: {
    instance: null,
    isCreating: true,
  },
}
