import type { Meta, StoryObj } from '@storybook/react-vite';
import { useState } from 'react';
import { InputOTP, InputOTPGroup, InputOTPSlot, InputOTPSeparator } from "@openagentsinc/ui";

const meta = {
  title: 'UI/InputOTP',
  component: InputOTP,
  argTypes: {
    maxLength: { control: 'number' },
    disabled: { control: 'boolean' },
    invalid: { control: 'boolean' },
  },
  args: {
    maxLength: 6,
    disabled: false,
    invalid: false,
  },
} satisfies Meta<typeof InputOTP>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ maxLength, disabled, invalid }) => {
    const [value, setValue] = useState<string>('');
    const len = Math.max(1, Number(maxLength));
    return (
      <InputOTP maxLength={len} value={value} onChange={setValue} containerClassName="gap-3">
        <InputOTPGroup>
          {Array.from({ length: len }).map((_, i) => (
            <InputOTPSlot key={i} index={i} aria-invalid={invalid ? true : undefined} />
          ))}
        </InputOTPGroup>
        {len >= 6 && <InputOTPSeparator />}
      </InputOTP>
    );
  },
};

