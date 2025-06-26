import type { Meta, StoryObj } from '@storybook/react'
import { Background } from './Background'

const meta = {
  title: 'Components/Background',
  component: Background,
  parameters: {
    layout: 'fullscreen',
  },
  tags: ['autodocs'],
  argTypes: {
    className: {
      control: 'text',
      description: 'Additional CSS classes',
    },
    animated: {
      control: 'object',
      description: 'Animation configuration',
    },
  },
} satisfies Meta<typeof Background>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  render: () => (
    <div className="relative w-full h-screen">
      <Background />
      <div className="absolute inset-0 flex items-center justify-center">
        <h1 className="text-cyan-300 text-4xl font-mono uppercase tracking-wider">
          Background Effect
        </h1>
      </div>
    </div>
  ),
}

export const WithContent: Story = {
  render: () => (
    <div className="relative w-full h-screen">
      <Background />
      <div className="absolute inset-0 flex items-center justify-center">
        <div className="max-w-2xl mx-auto p-8 text-center">
          <h1 className="text-cyan-300 text-4xl font-mono uppercase tracking-wider mb-4">
            OpenAgents
          </h1>
          <p className="text-cyan-500/80 font-mono text-sm">
            This background includes animated dots, puffs, and gradient effects
            that create a sci-fi atmosphere for the application.
          </p>
        </div>
      </div>
    </div>
  ),
}

export const MinimalView: Story = {
  parameters: {
    docs: {
      description: {
        story: 'The background without any content overlay to see the pure effect.',
      },
    },
  },
  render: () => (
    <div className="relative w-full h-screen">
      <Background />
    </div>
  ),
}

export const InCard: Story = {
  parameters: {
    layout: 'centered',
  },
  render: () => (
    <div className="relative w-[600px] h-[400px] rounded-lg overflow-hidden border border-cyan-500/30">
      <Background />
      <div className="absolute inset-0 flex items-center justify-center">
        <div className="text-center">
          <h3 className="text-cyan-300 text-xl font-mono uppercase tracking-wider mb-2">
            Card with Background
          </h3>
          <p className="text-cyan-500/60 font-mono text-xs">
            Background effects work in any container
          </p>
        </div>
      </div>
    </div>
  ),
}

export const Variations: Story = {
  render: () => (
    <div className="grid grid-cols-2 gap-4 p-4 bg-black">
      <div className="relative h-64 rounded overflow-hidden">
        <Background />
        <div className="absolute inset-0 flex items-center justify-center">
          <span className="text-cyan-300 font-mono text-sm">Default</span>
        </div>
      </div>
      
      <div className="relative h-64 rounded overflow-hidden">
        <Background className="opacity-50" />
        <div className="absolute inset-0 flex items-center justify-center">
          <span className="text-cyan-300 font-mono text-sm">50% Opacity</span>
        </div>
      </div>
      
      <div className="relative h-64 rounded overflow-hidden">
        <Background className="filter blur-sm" />
        <div className="absolute inset-0 flex items-center justify-center">
          <span className="text-cyan-300 font-mono text-sm">Blurred</span>
        </div>
      </div>
      
      <div className="relative h-64 rounded overflow-hidden">
        <Background className="filter hue-rotate-90" />
        <div className="absolute inset-0 flex items-center justify-center">
          <span className="text-cyan-300 font-mono text-sm">Hue Rotated</span>
        </div>
      </div>
    </div>
  ),
}