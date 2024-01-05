import type { Meta, StoryObj } from '@storybook/react';
import { MessagesList } from '.';

const meta = {
  title: 'OpenAgents/AgentBuilder/MessagesList',
  component: MessagesList,
  // tags: ['autodocs'],
  argTypes: {},
  // parameters: { layout: 'fullscreen' },
  // decorators: [
  //   (Story) => (
  //     <div style={{ height: '100vh' }}>
  //       <Story />
  //     </div>
  //   ),
  // ],
} satisfies Meta<typeof MessagesList>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Primary: Story = {
  args: {
    messages: [
      {
        role: 'user',
        content: 'Hello, I am interested in this property. Can you tell me more about it?',
      },
      {
        role: 'agent',
        content: 'Sure, what would you like to know?',
      },
      {
        role: 'user',
        content: 'How many bedrooms does it have?',
      },
      {
        role: 'agent',
        content: 'It has 3 bedrooms.',
      },
      {
        role: 'user',
        content: 'How many bathrooms does it have?',
      },
      {
        role: 'agent',
        content: 'It has 2 bathrooms.',
      },
      {
        role: 'user',
        content: 'How many parking spaces does it have?',
      },
      {
        role: 'agent',
        content: 'It has 1 parking space.',
      },
      {
        role: 'user',
        content: 'How much is it?',
      },
      {
        role: 'agent',
        content: 'It is 10 BTC.',
      },
      {
        role: 'user',
        content: 'Thank you.',
      },
    ],
  },
}
