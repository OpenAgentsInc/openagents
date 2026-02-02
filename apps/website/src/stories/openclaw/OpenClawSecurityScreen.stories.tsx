import type { Meta, StoryObj } from '@storybook/react-vite'
import type { RuntimeDevicesData } from '@/lib/openclawApi'
import { OpenClawSecurityScreen } from '@/components/openclaw/screens/OpenClawSecurityScreen'

const meta = {
  title: 'OpenClaw/Screens/Security',
  component: OpenClawSecurityScreen,
  parameters: { layout: 'fullscreen' },
  tags: ['autodocs'],
} satisfies Meta<typeof OpenClawSecurityScreen>

export default meta

type Story = StoryObj<typeof meta>

const devices: RuntimeDevicesData = {
  pending: [
    { requestId: 'req_abc123', client: { platform: 'macOS', mode: 'desktop' } },
    { requestId: 'req_def456', client: { platform: 'web' } },
  ],
  paired: [
    { deviceId: 'dev_xyz789', client: { platform: 'macOS', mode: 'desktop' }, pairedAt: new Date().toISOString() },
  ],
}

export const Loading: Story = {
  args: {
    devices: null,
    loading: true,
    error: null,
    approvingId: null,
  },
}

export const Empty: Story = {
  args: {
    devices: { pending: [], paired: [] },
    loading: false,
    error: null,
    approvingId: null,
  },
}

export const WithDevices: Story = {
  args: {
    devices,
    loading: false,
    error: null,
    approvingId: null,
  },
}

export const Approving: Story = {
  args: {
    devices,
    loading: false,
    error: null,
    approvingId: 'req_abc123',
  },
}

export const ErrorState: Story = {
  args: {
    devices: null,
    loading: false,
    error: 'Failed to load devices.',
    approvingId: null,
  },
}
