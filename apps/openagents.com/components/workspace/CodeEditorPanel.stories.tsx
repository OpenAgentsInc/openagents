import type { Meta, StoryObj } from '@storybook/nextjs'
import { CodeEditorPanel } from './CodeEditorPanel'

const meta = {
  title: 'Features/Workspace/CodeEditorPanel',
  component: CodeEditorPanel,
  parameters: {
    layout: 'fullscreen',
    docs: {
      description: {
        component: 'Complete code editor panel with file tree, tabs, and Monaco editor. Shows a Bitcoin price tracker demo project.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    projectId: {
      control: 'text',
      description: 'Project ID'
    }
  }
} satisfies Meta<typeof CodeEditorPanel>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Default: Story = {
  args: {
    projectId: 'demo-project'
  }
}

export const FullHeight: Story = {
  args: {
    projectId: 'demo-project'
  },
  render: (args) => (
    <div className="h-screen bg-black flex flex-col">
      <div className="h-16 bg-offblack border-b border-cyan-900/30 flex items-center px-4">
        <h1 className="text-cyan-500 font-mono text-lg">Bitcoin Tracker Project</h1>
        <div className="ml-8 flex items-center">
          <button className="px-4 py-1.5 text-sm font-mono uppercase tracking-wider bg-cyan-500/20 text-cyan-300 border border-cyan-500/50">
            Code
          </button>
          <button className="px-4 py-1.5 text-sm font-mono uppercase tracking-wider ml-2 text-cyan-500/60 hover:text-cyan-400 border border-transparent">
            Preview
          </button>
        </div>
      </div>
      <div className="flex-1">
        <CodeEditorPanel {...args} className="h-full" />
      </div>
    </div>
  )
}

export const InContainer: Story = {
  args: {
    projectId: 'demo-project'
  },
  render: (args) => (
    <div className="h-screen bg-black p-4">
      <div className="h-full border border-cyan-500/20">
        <CodeEditorPanel {...args} />
      </div>
    </div>
  )
}

export const Playground: Story = {
  args: {
    projectId: 'demo-project'
  }
}

export const TwoColumnWorkspace: Story = {
  args: {
    projectId: 'demo-project'
  },
  render: (args) => (
    <div className="h-screen bg-black flex flex-col">
      <div className="h-16 bg-offblack border-b border-cyan-900/30 flex items-center px-4">
        <h1 className="text-cyan-500 font-mono text-lg">Bitcoin Tracker Project</h1>
        <div className="ml-8 flex items-center">
          <button className="px-4 py-1.5 text-sm font-mono uppercase tracking-wider bg-cyan-500/20 text-cyan-300 border border-cyan-500/50">
            Code
          </button>
          <button className="px-4 py-1.5 text-sm font-mono uppercase tracking-wider ml-2 text-cyan-500/60 hover:text-cyan-400 border border-transparent">
            Preview
          </button>
        </div>
      </div>
      <div className="flex-1 grid grid-cols-2 gap-4 p-4">
        <div className="h-full bg-offblack border border-cyan-900/30 p-4 overflow-hidden">
          <h2 className="text-cyan-500 text-sm font-mono uppercase mb-4">Chat</h2>
          <div className="text-cyan-300/60 text-sm font-sans">Chat interface goes here</div>
        </div>
        <div className="h-full">
          <CodeEditorPanel {...args} className="h-full" />
        </div>
      </div>
    </div>
  )
}