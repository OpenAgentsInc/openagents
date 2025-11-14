import type { Meta, StoryObj } from "@storybook/react-vite";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupText,
  InputGroupInput,
  InputGroupTextarea,
} from "@openagentsinc/ui";
import { Button } from "@openagentsinc/ui";
import { Kbd } from "@openagentsinc/ui";
import { Search, X } from "lucide-react";

type InputGroupStoryArgs = {
  align: "inline-start" | "inline-end" | "block-start" | "block-end";
  placeholder?: string;
  invalid?: boolean;
  disabled?: boolean;
  width: number;
};

const InputGroupStoryComponent = (_: InputGroupStoryArgs) => null;

const meta = {
  title: "UI/InputGroup",
  component: InputGroupStoryComponent,
  argTypes: {
    align: {
      control: "select",
      options: ["inline-start", "inline-end", "block-start", "block-end"],
    },
    placeholder: { control: "text" },
    invalid: { control: "boolean" },
    disabled: { control: "boolean" },
    width: { control: "number" },
  },
  args: {
    align: "inline-start",
    placeholder: "Search…",
    invalid: false,
    disabled: false,
    width: 420,
  },
} satisfies Meta<typeof InputGroupStoryComponent>;

export default meta;
type Story = StoryObj<typeof meta>;

export const WithButtons: Story = {
  render: ({ align, placeholder, invalid, disabled, width }) => (
    <div style={{ width: Number(width), display: "grid", gap: 12 }}>
      <InputGroup data-disabled={!!disabled}>
        <InputGroupAddon align={align as any}>
          <Search className="opacity-70" />
          <InputGroupText>Find</InputGroupText>
        </InputGroupAddon>
        <InputGroupInput placeholder={placeholder as string} aria-invalid={invalid ? true : undefined} disabled={!!disabled} />
        <InputGroupAddon align="inline-end">
          <InputGroupButton size="icon-xs" variant="ghost" aria-label="Clear">
            <X />
          </InputGroupButton>
          <InputGroupButton size="sm">Search</InputGroupButton>
        </InputGroupAddon>
      </InputGroup>

      <InputGroup>
        <InputGroupAddon align="inline-start">
          <Kbd>⌘</Kbd>
          <Kbd>K</Kbd>
        </InputGroupAddon>
        <InputGroupInput placeholder="Command palette" />
      </InputGroup>
    </div>
  ),
};

export const WithTextarea: Story = {
  args: { align: 'block-start', placeholder: 'Write a message…' },
  render: ({ align, placeholder, invalid, disabled, width }) => (
    <div style={{ width: Number(width) }}>
      <InputGroup>
        <InputGroupAddon align={align as any}>Message</InputGroupAddon>
        <InputGroupTextarea rows={4} placeholder={placeholder as string} aria-invalid={invalid ? true : undefined} disabled={!!disabled} />
        <InputGroupAddon align="block-end">
          <Button size="sm" variant="secondary">Cancel</Button>
          <Button size="sm">Send</Button>
        </InputGroupAddon>
      </InputGroup>
    </div>
  ),
};
