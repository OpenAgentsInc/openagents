import type { Meta, StoryObj } from '@storybook/react-vite';
import { Slider } from '@/components/ui/slider';

const meta = {
  title: 'UI/Slider',
  component: Slider,
  argTypes: {
    value: { control: { type: 'range', min: 0, max: 100, step: 1 } },
    min: { control: 'number' },
    max: { control: 'number' },
    orientation: { control: 'select', options: ['horizontal', 'vertical'] },
    width: { control: 'number' },
    height: { control: 'number' },
  },
  args: {
    value: 40,
    min: 0,
    max: 100,
    orientation: 'horizontal',
    width: 320,
    height: 200,
  },
} satisfies Meta<typeof Slider>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ value, min, max, orientation, width, height }) => (
    <div
      style={{
        width: orientation === 'horizontal' ? Number(width) : 'auto',
        height: orientation === 'vertical' ? Number(height) : 'auto',
      }}
    >
      <Slider value={[Number(value)]} min={Number(min)} max={Number(max)} orientation={orientation as any} />
    </div>
  ),
};

export const Range: Story = {
  args: { value: undefined },
  render: ({ min, max, orientation, width, height }) => (
    <div
      style={{
        width: orientation === 'horizontal' ? Number(width) : 'auto',
        height: orientation === 'vertical' ? Number(height) : 'auto',
      }}
    >
      <Slider defaultValue={[20, 80]} min={Number(min)} max={Number(max)} orientation={orientation as any} />
    </div>
  ),
};

