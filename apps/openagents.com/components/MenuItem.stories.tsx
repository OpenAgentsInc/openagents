import type { Meta, StoryObj } from '@storybook/react'
import { MenuItem } from './MenuItem'
import { Home, MessageSquare, Users, Settings, FileText } from 'lucide-react'

const meta = {
  title: 'Components/MenuItem',
  component: MenuItem,
  parameters: {
    layout: 'centered',
  },
  tags: ['autodocs'],
  argTypes: {
    active: {
      control: 'boolean',
      description: 'Whether the menu item is currently active',
    },
    className: {
      control: 'text',
      description: 'Additional CSS classes',
    },
    children: {
      control: 'text',
      description: 'Menu item content',
    },
    animated: {
      control: 'object',
      description: 'Animation configuration',
    },
  },
} satisfies Meta<typeof MenuItem>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  args: {
    children: 'Menu Item',
  },
}

export const Active: Story = {
  args: {
    children: 'Active Item',
    active: true,
  },
}

export const WithIcon: Story = {
  args: {
    children: (
      <>
        <Home size={14} />
        <span>Home</span>
      </>
    ),
  },
}

export const ActiveWithIcon: Story = {
  args: {
    children: (
      <>
        <MessageSquare size={14} />
        <span>Chat</span>
      </>
    ),
    active: true,
  },
}

export const Navigation: Story = {
  render: () => (
    <nav className="bg-black/50 p-4 rounded">
      <ul className="flex gap-2">
        <MenuItem active>
          <Home size={14} />
          <span>Home</span>
        </MenuItem>
        <MenuItem>
          <MessageSquare size={14} />
          <span>Chat</span>
        </MenuItem>
        <MenuItem>
          <Users size={14} />
          <span>Agents</span>
        </MenuItem>
        <MenuItem>
          <FileText size={14} />
          <span>Docs</span>
        </MenuItem>
        <MenuItem>
          <Settings size={14} />
          <span>Settings</span>
        </MenuItem>
      </ul>
    </nav>
  ),
}

export const VerticalMenu: Story = {
  render: () => (
    <nav className="bg-black/50 p-4 rounded w-48">
      <ul className="flex flex-col gap-1">
        <MenuItem active className="justify-start">
          <Home size={14} />
          <span>Home</span>
        </MenuItem>
        <MenuItem className="justify-start">
          <MessageSquare size={14} />
          <span>Chat</span>
        </MenuItem>
        <MenuItem className="justify-start">
          <Users size={14} />
          <span>Agents</span>
        </MenuItem>
        <MenuItem className="justify-start">
          <FileText size={14} />
          <span>Documentation</span>
        </MenuItem>
        <MenuItem className="justify-start">
          <Settings size={14} />
          <span>Settings</span>
        </MenuItem>
      </ul>
    </nav>
  ),
}

export const IconOnly: Story = {
  render: () => (
    <nav className="bg-black/50 p-4 rounded">
      <ul className="flex gap-2">
        <MenuItem active>
          <Home size={16} />
        </MenuItem>
        <MenuItem>
          <MessageSquare size={16} />
        </MenuItem>
        <MenuItem>
          <Users size={16} />
        </MenuItem>
        <MenuItem>
          <Settings size={16} />
        </MenuItem>
      </ul>
    </nav>
  ),
}