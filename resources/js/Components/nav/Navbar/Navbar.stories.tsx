import type { Meta, StoryObj } from '@storybook/react';
import { Navbar } from '.';

const meta = {
  title: 'OpenAgents/Navigation/Navbar',
  component: Navbar,
  parameters: { layout: 'fullscreen' },
  argTypes: {},
} satisfies Meta<typeof Navbar>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Primary: Story = {}
