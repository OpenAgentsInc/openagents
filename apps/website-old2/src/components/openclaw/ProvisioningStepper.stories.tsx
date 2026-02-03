import type { Meta, StoryObj } from '@storybook/react-vite'
import { ProvisioningStepper } from './ProvisioningStepper'

const meta = {
  title: 'OpenClaw/ProvisioningStepper',
  component: ProvisioningStepper,
  parameters: { layout: 'centered' },
  tags: ['autodocs'],
  argTypes: {
    status: {
      control: 'select',
      options: [undefined, 'provisioning', 'ready'],
    },
  },
} satisfies Meta<typeof ProvisioningStepper>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  args: {},
}

export const Creating: Story = {
  args: { status: undefined },
}

export const Provisioning: Story = {
  args: { status: 'provisioning' },
}

export const Ready: Story = {
  args: { status: 'ready' },
}
