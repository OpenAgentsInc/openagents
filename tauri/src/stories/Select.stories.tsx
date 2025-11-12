import type { Meta, StoryObj } from '@storybook/react-vite';
import {
  Select,
  SelectTrigger,
  SelectContent,
  SelectItem,
  SelectValue,
  SelectLabel,
  SelectGroup,
} from '@/components/ui/select';

const options = ['Apple', 'Banana', 'Orange', 'Grape', 'Pineapple'] as const;

const meta = {
  title: 'UI/Select',
  component: Select,
  argTypes: {
    defaultValue: { control: 'select', options: options.map((o) => o.toLowerCase()) },
    placeholder: { control: 'text' },
    label: { control: 'text' },
    width: { control: 'number' },
  },
  args: {
    defaultValue: 'apple',
    placeholder: 'Choose a fruit',
    label: 'Fruits',
    width: 224,
  },
} satisfies Meta<typeof Select>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ defaultValue, placeholder, label, width }) => (
    <Select defaultValue={defaultValue as string}>
      <SelectTrigger className="" style={{ width }}>
        <SelectValue placeholder={placeholder as string} />
      </SelectTrigger>
      <SelectContent style={{ width }}>
        <SelectGroup>
          <SelectLabel>{label as string}</SelectLabel>
          {options.map((o) => (
            <SelectItem key={o} value={o.toLowerCase()}>{o}</SelectItem>
          ))}
        </SelectGroup>
      </SelectContent>
    </Select>
  ),
};
