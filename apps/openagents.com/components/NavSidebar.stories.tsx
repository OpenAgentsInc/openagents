import type { Meta, StoryObj } from '@storybook/react'
import { NavSidebar } from './NavSidebar'
import { FrameOctagon, styleFrameClipOctagon } from '@arwes/react'

const meta = {
  title: 'Components/NavSidebar',
  component: NavSidebar,
  parameters: {
    layout: 'padded',
    nextjs: {
      navigation: {
        pathname: '/',
      },
    },
  },
  tags: ['autodocs'],
} satisfies Meta<typeof NavSidebar>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  render: () => (
    <div className="w-64 h-[600px] bg-black">
      <NavSidebar />
    </div>
  ),
}

export const InContainer: Story = {
  render: () => (
    <div className="w-64 h-[600px] relative bg-black">
      <div 
        className="absolute inset-0"
        style={{
          clipPath: styleFrameClipOctagon({ squareSize: 8 })
        }}
      >
        <FrameOctagon
          style={{
            // @ts-expect-error CSS variables
            '--arwes-frames-bg-color': 'hsla(180, 69%, 15%, 0.1)',
            '--arwes-frames-line-color': 'hsla(180, 69%, 15%, 0.5)'
          }}
          squareSize={8}
        />
      </div>
      <div className="relative p-4 h-full">
        <NavSidebar />
      </div>
    </div>
  ),
}

export const DifferentActiveRoutes: Story = {
  parameters: {
    docs: {
      description: {
        story: 'This shows how the sidebar would look with different active routes. In a real app, the active state is determined by the current route.',
      },
    },
  },
  render: () => (
    <div className="flex gap-4">
      <div className="w-64 h-[600px] bg-black/50 rounded p-4">
        <p className="text-cyan-300 text-xs mb-4">Active: Home</p>
        <div className="h-full">
          <NavSidebar />
        </div>
      </div>
      
      <div className="w-64 h-[600px] bg-black/50 rounded p-4">
        <p className="text-cyan-300 text-xs mb-4">Active: Chat</p>
        <div className="h-full">
          <NavSidebar />
        </div>
      </div>
    </div>
  ),
}

export const Responsive: Story = {
  parameters: {
    viewport: {
      defaultViewport: 'mobile1',
    },
  },
  render: () => (
    <div className="w-full h-screen bg-black p-4">
      <NavSidebar />
    </div>
  ),
}