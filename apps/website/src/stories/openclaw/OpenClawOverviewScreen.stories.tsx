import type { Meta, StoryObj } from '@storybook/react-vite'
import { Button } from '@/components/ui/button'
import type { InstanceSummary, RuntimeStatusData } from '@/lib/openclawApi'
import { OpenClawOverviewScreen } from '@/components/openclaw/screens/OpenClawOverviewScreen'

const meta = {
  title: 'OpenClaw/Screens/Overview',
  component: OpenClawOverviewScreen,
  parameters: { layout: 'fullscreen' },
  tags: ['autodocs'],
} satisfies Meta<typeof OpenClawOverviewScreen>

export default meta

type Story = StoryObj<typeof meta>

const actions = (
  <>
    <Button variant="secondary" className="w-full">Provision settings</Button>
    <Button variant="secondary" className="w-full">Pair devices</Button>
    <Button variant="secondary" className="w-full">Usage & backups</Button>
    <Button variant="secondary" className="w-full">Billing</Button>
  </>
)

const now = Date.now()

const readyInstance: InstanceSummary = {
  status: 'ready',
  runtime_name: 'openclaw-prod-1',
  created_at: now - 86400000,
  updated_at: now,
  last_ready_at: now - 3600000,
}

const provisioningInstance: InstanceSummary = {
  status: 'provisioning',
  runtime_name: null,
  created_at: now - 60000,
  updated_at: now,
  last_ready_at: null,
}

const runtimeStatus: RuntimeStatusData = {
  gateway: { status: 'running' },
  lastBackup: new Date(now - 7200000).toISOString(),
  container: { instanceType: 'standard-4' },
  version: { clawdbot: '2026.1.24-3' },
}

export const NotCreated: Story = {
  args: {
    instance: null,
    runtimeStatus: null,
    loading: false,
    creating: false,
    error: null,
    actions,
  },
}

export const Creating: Story = {
  args: {
    instance: null,
    runtimeStatus: null,
    loading: false,
    creating: true,
    error: null,
    actions,
  },
}

export const Provisioning: Story = {
  args: {
    instance: provisioningInstance,
    runtimeStatus,
    loading: false,
    creating: false,
    error: null,
    actions,
  },
}

export const Ready: Story = {
  args: {
    instance: readyInstance,
    runtimeStatus,
    loading: false,
    creating: false,
    error: null,
    actions,
  },
}

export const LoadingStatus: Story = {
  args: {
    instance: readyInstance,
    runtimeStatus: null,
    loading: true,
    creating: false,
    error: null,
    actions,
  },
}

export const ErrorState: Story = {
  args: {
    instance: null,
    runtimeStatus: null,
    loading: false,
    creating: false,
    error: 'Unable to load OpenClaw data.',
    actions,
  },
}
