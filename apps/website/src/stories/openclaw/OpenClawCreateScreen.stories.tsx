import type { Meta, StoryObj } from '@storybook/react-vite'
import type { InstanceSummary } from '@/lib/openclawApi'
import { OpenClawCreateScreen } from '@/components/openclaw/screens/OpenClawCreateScreen'

const meta = {
  title: 'OpenClaw/Screens/Create',
  component: OpenClawCreateScreen,
  parameters: { layout: 'fullscreen' },
  tags: ['autodocs'],
} satisfies Meta<typeof OpenClawCreateScreen>

export default meta

type Story = StoryObj<typeof meta>

const now = Date.now()

const provisioningInstance: InstanceSummary = {
  status: 'provisioning',
  runtime_name: null,
  created_at: now - 60000,
  updated_at: now,
  last_ready_at: null,
}

const readyInstance: InstanceSummary = {
  status: 'ready',
  runtime_name: 'openclaw-prod-1',
  created_at: now - 86400000,
  updated_at: now,
  last_ready_at: now - 3600000,
}

export const NotCreated: Story = {
  args: {
    instance: null,
    creating: false,
    error: null,
  },
}

export const Creating: Story = {
  args: {
    instance: null,
    creating: true,
    error: null,
  },
}

export const Provisioning: Story = {
  args: {
    instance: provisioningInstance,
    creating: false,
    error: null,
  },
}

export const Ready: Story = {
  args: {
    instance: readyInstance,
    creating: false,
    error: null,
  },
}

export const ErrorState: Story = {
  args: {
    instance: null,
    creating: false,
    error: 'Unable to load instance.',
  },
}
