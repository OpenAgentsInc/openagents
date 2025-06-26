import type { Meta, StoryObj } from '@storybook/nextjs'
import { ButtonSimple } from './ButtonSimple'
import { Send, Download, Settings, ChevronRight } from 'lucide-react'

const meta = {
  title: 'Components/ButtonSimple',
  component: ButtonSimple,
  parameters: {
    layout: 'centered',
  },
  tags: ['autodocs'],
  argTypes: {
    children: {
      control: 'text',
      description: 'Button content',
    },
    className: {
      control: 'text',
      description: 'Additional CSS classes',
    },
    disabled: {
      control: 'boolean',
      description: 'Disable the button',
    },
    animated: {
      control: 'object',
      description: 'Animation configuration',
    },
  },
} satisfies Meta<typeof ButtonSimple>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  args: {
    children: 'Click Me',
  },
}

export const WithIcon: Story = {
  args: {
    children: (
      <>
        <Send size={14} />
        <span>Send Message</span>
      </>
    ),
  },
}

export const IconOnly: Story = {
  args: {
    children: <Settings size={16} />,
    className: 'px-3',
  },
}

export const Disabled: Story = {
  args: {
    children: 'Disabled Button',
    disabled: true,
  },
}

export const Large: Story = {
  args: {
    children: 'Large Button',
    className: 'text-sm px-6 h-10',
  },
}

export const ButtonGroup: Story = {
  args: {
    children: 'Button'
  },
  render: () => (
    <div className="flex gap-2">
      <ButtonSimple>
        <ChevronRight size={14} />
        <span>Next</span>
      </ButtonSimple>
      <ButtonSimple>
        <Download size={14} />
        <span>Download</span>
      </ButtonSimple>
      <ButtonSimple>
        <Settings size={14} />
        <span>Settings</span>
      </ButtonSimple>
    </div>
  ),
}

export const DifferentStates: Story = {
  args: {
    children: 'Button'
  },
  render: () => (
    <div className="flex flex-col gap-4">
      <div className="flex items-center gap-4">
        <span className="text-cyan-300 text-sm w-24">Normal:</span>
        <ButtonSimple>Normal State</ButtonSimple>
      </div>
      <div className="flex items-center gap-4">
        <span className="text-cyan-300 text-sm w-24">Hover:</span>
        <ButtonSimple className="text-yellow-200">Hover State (simulated)</ButtonSimple>
      </div>
      <div className="flex items-center gap-4">
        <span className="text-cyan-300 text-sm w-24">Disabled:</span>
        <ButtonSimple disabled>Disabled State</ButtonSimple>
      </div>
    </div>
  ),
}