import type { Meta, StoryObj } from '@storybook/nextjs'
import { NavSidebar } from './NavSidebar'
import { FrameOctagon, styleFrameClipOctagon } from '@arwes/react'

const meta = {
  title: 'Components/NavSidebar',
  component: NavSidebar,
  parameters: {
    layout: 'padded',
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

export const WithChatActive: Story = {
  parameters: {
    nextjs: {
      navigation: {
        pathname: '/chat',
      },
    },
  },
  render: () => (
    <div className="w-64 h-[600px] bg-black">
      <NavSidebar />
    </div>
  ),
}

export const WithAgentsActive: Story = {
  parameters: {
    nextjs: {
      navigation: {
        pathname: '/agents',
      },
    },
  },
  render: () => (
    <div className="w-64 h-[600px] bg-black">
      <NavSidebar />
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