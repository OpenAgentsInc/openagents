import type { Meta, StoryObj } from '@storybook/react-vite';
import { useEffect, useState } from 'react';
import { Calendar } from '@/components/ui/calendar';

type Mode = 'single' | 'multiple' | 'range';

const meta = {
  title: 'UI/Calendar',
  component: Calendar,
  argTypes: {
    mode: { control: 'select', options: ['single', 'range', 'multiple'] },
    numberOfMonths: { control: 'number' },
    showOutsideDays: { control: 'boolean' },
    captionLayout: { control: 'select', options: ['label', 'dropdown'] },
    buttonVariant: { control: 'select', options: ['ghost', 'outline', 'secondary', 'default'] },
  },
  args: {
    mode: 'single' as Mode,
    numberOfMonths: 1,
    showOutsideDays: true,
    captionLayout: 'label' as const,
    buttonVariant: 'ghost' as const,
  },
} satisfies Meta<typeof Calendar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Single: Story = {
  render: ({ mode, numberOfMonths, showOutsideDays, captionLayout, buttonVariant }) => {
    const [selected, setSelected] = useState<Date | undefined>(new Date());
    useEffect(() => {
      // reset selection if mode changes
      setSelected(new Date());
    }, [mode]);
    return (
      <Calendar
        mode={mode as Mode}
        selected={selected as any}
        onSelect={(v: any) => setSelected(v as Date)}
        numberOfMonths={Number(numberOfMonths)}
        showOutsideDays={!!showOutsideDays}
        captionLayout={captionLayout as any}
        buttonVariant={buttonVariant as any}
      />
    );
  },
};

export const Range: Story = {
  args: { mode: 'range', numberOfMonths: 2, captionLayout: 'dropdown', buttonVariant: 'outline' },
  render: ({ mode, numberOfMonths, showOutsideDays, captionLayout, buttonVariant }) => {
    const [range, setRange] = useState<{ from?: Date; to?: Date }>({});
    useEffect(() => setRange({}), [mode]);
    return (
      <Calendar
        mode={mode as Mode}
        selected={range as any}
        onSelect={(v: any) => setRange(v as any)}
        numberOfMonths={Number(numberOfMonths)}
        showOutsideDays={!!showOutsideDays}
        captionLayout={captionLayout as any}
        buttonVariant={buttonVariant as any}
        showWeekNumber
      />
    );
  },
};

