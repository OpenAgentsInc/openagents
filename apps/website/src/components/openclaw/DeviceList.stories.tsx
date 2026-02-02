import type { Meta, StoryObj } from '@storybook/react-vite'
import { fn } from 'storybook/test'
import type { RuntimeDevicesData } from '@/lib/openclawApi'
import { DeviceList } from './DeviceList'

const meta = {
  title: 'OpenClaw/DeviceList',
  component: DeviceList,
  parameters: { layout: 'centered' },
  tags: ['autodocs'],
  args: { onApprove: fn() },
} satisfies Meta<typeof DeviceList>

export default meta
type Story = StoryObj<typeof meta>

const emptyDevices: RuntimeDevicesData = {
  pending: [],
  paired: [],
}

const withPending: RuntimeDevicesData = {
  pending: [
    {
      requestId: 'req_abc123',
      client: { platform: 'macOS', mode: 'desktop' },
      requestedAt: new Date().toISOString(),
    },
    {
      requestId: 'req_def456',
      client: { platform: 'web' },
    },
  ],
  paired: [],
}

const withPaired: RuntimeDevicesData = {
  pending: [],
  paired: [
    {
      deviceId: 'dev_xyz789',
      client: { platform: 'macOS', mode: 'desktop' },
      pairedAt: new Date(Date.now() - 86400000).toISOString(),
    },
  ],
}

const full: RuntimeDevicesData = {
  pending: [
    {
      requestId: 'req_new',
      client: { platform: 'Linux', mode: 'cli' },
    },
  ],
  paired: [
    {
      deviceId: 'dev_1',
      client: { platform: 'macOS', mode: 'desktop' },
      pairedAt: new Date(Date.now() - 86400000).toISOString(),
    },
    {
      deviceId: 'dev_2',
      client: { platform: 'web' },
      pairedAt: new Date(Date.now() - 3600000).toISOString(),
    },
  ],
}

export const Empty: Story = {
  args: { devices: emptyDevices },
}

export const PendingOnly: Story = {
  args: { devices: withPending },
}

export const PairedOnly: Story = {
  args: { devices: withPaired },
}

export const Full: Story = {
  args: { devices: full },
}

export const Approving: Story = {
  args: {
    devices: withPending,
    approvingId: 'req_abc123',
  },
}

export const NoData: Story = {
  args: { devices: null },
}
