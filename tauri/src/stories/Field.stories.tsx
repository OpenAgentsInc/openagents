import type { Meta, StoryObj } from '@storybook/react-vite';
import {
  FieldSet,
  FieldLegend,
  FieldGroup,
  Field,
  FieldLabel,
  FieldContent,
  FieldDescription,
  FieldError,
} from "@openagentsinc/ui";
import { Input } from "@openagentsinc/ui";
import { Checkbox } from "@openagentsinc/ui";

const meta = {
  title: 'UI/Field',
  component: FieldSet,
  argTypes: {
    orientation: { control: 'select', options: ['vertical', 'horizontal', 'responsive'] },
    showError: { control: 'boolean' },
    disabled: { control: 'boolean' },
  },
  args: {
    orientation: 'vertical',
    showError: false,
    disabled: false,
  },
} satisfies Meta<typeof FieldSet>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ orientation, showError, disabled }) => (
    <FieldSet>
      <FieldLegend>Profile</FieldLegend>
      <FieldGroup>
        <Field orientation={orientation as any} data-disabled={!!disabled}>
          <FieldLabel>Display name</FieldLabel>
          <FieldContent>
            <Input placeholder="Ada Lovelace" disabled={!!disabled} />
            <FieldDescription>Your public name shown to others.</FieldDescription>
            {showError && <FieldError>Display name is required.</FieldError>}
          </FieldContent>
        </Field>

        <Field orientation={orientation as any}>
          <FieldLabel>Email</FieldLabel>
          <FieldContent>
            <Input type="email" placeholder="ada@example.com" />
          </FieldContent>
        </Field>

        <Field orientation={orientation as any}>
          <FieldLabel>Notifications</FieldLabel>
          <FieldContent>
            <div className="flex items-center gap-2">
              <Checkbox id="notif" />
              <label htmlFor="notif" className="text-sm">Email me about updates</label>
            </div>
          </FieldContent>
        </Field>
      </FieldGroup>
    </FieldSet>
  ),
};

