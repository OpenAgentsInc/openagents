import type { Meta, StoryObj } from '@storybook/react-vite'
import { HomeHero } from './HomeHero'

const meta = {
  title: 'Home/HomeHero',
  component: HomeHero,
  parameters: {
    layout: 'fullscreen',
  },
  tags: ['autodocs'],
} satisfies Meta<typeof HomeHero>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {}
