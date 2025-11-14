import type { Meta, StoryObj } from '@storybook/react-vite';
import {
  Accordion,
  AccordionItem,
  AccordionTrigger,
  AccordionContent,
} from "@openagentsinc/ui";

const meta = {
  title: 'UI/Accordion',
  component: Accordion,
  argTypes: {
    type: { control: 'select', options: ['single', 'multiple'] },
    collapsible: { control: 'boolean' },
    width: { control: 'number' },
  },
  args: {
    type: 'single',
    collapsible: true,
    width: 420,
  },
} satisfies Meta<typeof Accordion>;

export default meta;
type Story = StoryObj<typeof meta>;

const items = [
  {
    id: 'item-1',
    title: 'What is OpenAgents?',
    content:
      'OpenAgents is a desktop chat application for interacting with AI assistants.',
  },
  {
    id: 'item-2',
    title: 'What stack do we use?',
    content: 'Tauri + React + TypeScript + assistant-ui + Tailwind.',
  },
  {
    id: 'item-3',
    title: 'Does it support tools?',
    content: 'Yes, client-side tools via makeAssistantTool.',
  },
];

export const Basic: Story = {
  render: ({ type, collapsible, width }) => (
    <Accordion
      type={type as 'single' | 'multiple'}
      collapsible={!!collapsible}
      style={{ width: Number(width) }}
      defaultValue={type === 'single' ? items[0].id : items.map((i) => i.id)}
    >
      {items.map((item) => (
        <AccordionItem key={item.id} value={item.id}>
          <AccordionTrigger>{item.title}</AccordionTrigger>
          <AccordionContent>{item.content}</AccordionContent>
        </AccordionItem>
      ))}
    </Accordion>
  ),
};

export const Multiple: Story = {
  args: { type: 'multiple', collapsible: false },
};

