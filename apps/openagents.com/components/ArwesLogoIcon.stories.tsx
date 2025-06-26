import type { Meta, StoryObj } from '@storybook/nextjs'
import { ArwesLogoIcon } from './ArwesLogoIcon'

const meta = {
  title: 'Components/ArwesLogoIcon',
  component: ArwesLogoIcon,
  parameters: {
    layout: 'centered',
  },
  tags: ['autodocs'],
  argTypes: {
    className: {
      control: 'text',
      description: 'CSS classes for sizing and styling',
    },
    hasRotation: {
      control: 'boolean',
      description: 'Enable/disable rotation animation',
    },
    animated: {
      control: 'object',
      description: 'Animation configuration',
    },
  },
} satisfies Meta<typeof ArwesLogoIcon>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  args: {
    className: 'w-12 h-12',
  },
}

export const Small: Story = {
  args: {
    className: 'w-6 h-6',
  },
}

export const Large: Story = {
  args: {
    className: 'w-24 h-24',
  },
}

export const WithoutRotation: Story = {
  args: {
    className: 'w-12 h-12',
    hasRotation: false,
  },
}

export const CustomColors: Story = {
  render: () => (
    <div className="flex gap-4">
      <div className="filter hue-rotate-90">
        <ArwesLogoIcon className="w-12 h-12" />
      </div>
      <div className="filter hue-rotate-180">
        <ArwesLogoIcon className="w-12 h-12" />
      </div>
      <div className="filter hue-rotate-270">
        <ArwesLogoIcon className="w-12 h-12" />
      </div>
    </div>
  ),
}

export const Sizes: Story = {
  render: () => (
    <div className="flex items-center gap-4">
      <ArwesLogoIcon className="w-4 h-4" />
      <ArwesLogoIcon className="w-6 h-6" />
      <ArwesLogoIcon className="w-8 h-8" />
      <ArwesLogoIcon className="w-12 h-12" />
      <ArwesLogoIcon className="w-16 h-16" />
      <ArwesLogoIcon className="w-20 h-20" />
    </div>
  ),
}

export const InContext: Story = {
  render: () => (
    <div className="flex items-center gap-3 p-4 bg-black/50 rounded">
      <ArwesLogoIcon className="w-8 h-8" />
      <span className="text-cyan-300 font-mono text-sm uppercase tracking-wider">
        OpenAgents
      </span>
    </div>
  ),
}