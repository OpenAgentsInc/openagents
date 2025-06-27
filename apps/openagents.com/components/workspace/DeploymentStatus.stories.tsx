import type { Meta, StoryObj } from '@storybook/nextjs'
import { DeploymentStatus } from './DeploymentStatus'

const meta = {
  title: 'Workspace/DeploymentStatus',
  component: DeploymentStatus,
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component: 'Animated deployment status indicator showing progress through build and deployment stages with visual feedback.'
      }
    }
  },
  tags: ['autodocs'],
  argTypes: {
    status: {
      control: 'select',
      options: ['idle', 'generating', 'deploying', 'complete', 'error'],
      description: 'Current deployment status'
    },
    deploymentUrl: {
      control: 'text',
      description: 'URL of deployed project (shown when complete)'
    }
  }
} satisfies Meta<typeof DeploymentStatus>

export default meta
type Story = StoryObj<typeof meta>

// Stories
export const Idle: Story = {
  args: {
    status: 'idle'
  }
}

export const Generating: Story = {
  args: {
    status: 'generating'
  }
}

export const Deploying: Story = {
  args: {
    status: 'deploying'
  }
}

export const Complete: Story = {
  args: {
    status: 'complete',
    deploymentUrl: 'https://bitcoin-tracker.openagents.dev'
  }
}

export const Error: Story = {
  args: {
    status: 'error'
  }
}

export const InDarkContainer: Story = {
  args: {
    status: 'deploying'
  },
  render: (args) => (
    <div className="bg-black p-8 min-h-[400px]">
      <DeploymentStatus {...args} />
    </div>
  )
}

export const AllStates: Story = {
  args: {
    status: 'complete'
  },
  render: () => (
    <div className="bg-black p-8 space-y-6">
      <div>
        <h3 className="text-cyan-500 font-mono text-sm mb-4">All Deployment States</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          <DeploymentStatus status="idle" />
          <DeploymentStatus status="generating" />
          <DeploymentStatus status="deploying" />
          <DeploymentStatus status="complete" deploymentUrl="https://example.com" />
          <DeploymentStatus status="error" />
        </div>
      </div>
    </div>
  )
}