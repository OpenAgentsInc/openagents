import type { Meta, StoryObj } from '@storybook/nextjs'
import { WorkspaceChat } from './WorkspaceChat'
import { ToastProvider } from '../Toast'

const meta = {
  title: 'Features/Workspace/WorkspaceChat',
  component: WorkspaceChat,
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'AI-powered chat interface for project workspace. Features mock conversation history, real-time message input, and simulated AI responses.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    projectName: {
      control: 'text',
      description: 'Name of the current project'
    }
  },
  decorators: [
    (Story) => (
      <ToastProvider>
        <Story />
      </ToastProvider>
    )
  ]
} satisfies Meta<typeof WorkspaceChat>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {
    projectName: 'Bitcoin Tracker'
  }
}

export const FullHeight: Story = {
  args: {
    projectName: 'E-commerce Dashboard'
  },
  render: (args) => (
    <div className="h-screen bg-black">
      <WorkspaceChat {...args} />
    </div>
  )
}

export const InContainer: Story = {
  args: {
    projectName: 'Portfolio Website'
  },
  render: (args) => (
    <div className="h-screen bg-black p-4">
      <div className="h-full border border-cyan-500/20">
        <WorkspaceChat {...args} />
      </div>
    </div>
  )
}

export const DifferentProject: Story = {
  args: {
    projectName: 'Todo App with AI'
  },
  render: (args) => (
    <div className="h-screen bg-black">
      <WorkspaceChat {...args} />
    </div>
  )
}