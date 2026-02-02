import type { Meta, StoryObj } from '@storybook/react-vite'
import type { RuntimeStatusData } from '@/lib/openclawApi'
import { OpenClawUsageScreen } from '@/components/openclaw/screens/OpenClawUsageScreen'

const meta = {
  title: 'OpenClaw/Screens/Usage',
  component: OpenClawUsageScreen,
  parameters: { layout: 'fullscreen' },
  tags: ['autodocs'],
} satisfies Meta<typeof OpenClawUsageScreen>

export default meta

type Story = StoryObj<typeof meta>

const status: RuntimeStatusData = {
  gateway: { status: 'running' },
  lastBackup: new Date(Date.now() - 3600000).toISOString(),
  container: { instanceType: 'standard-4' },
  version: { clawdbot: '2026.1.24-3' },
}

export const Loading: Story = {
  args: {
    status: null,
    loading: true,
    error: null,
    working: null,
  },
}

export const Ready: Story = {
  args: {
    status,
    loading: false,
    error: null,
    working: null,
  },
}

export const BackupWorking: Story = {
  args: {
    status,
    loading: false,
    error: null,
    working: 'backup',
  },
}

export const RestartWorking: Story = {
  args: {
    status,
    loading: false,
    error: null,
    working: 'restart',
  },
}

export const ErrorState: Story = {
  args: {
    status: null,
    loading: false,
    error: 'Backup failed',
    working: null,
  },
}
