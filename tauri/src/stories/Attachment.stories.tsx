import type { Meta, StoryObj } from '@storybook/react-vite';
import { ComposerAttachments, ComposerAddAttachment } from '@/components/assistant-ui/attachment';
import { MyRuntimeProvider } from '@/runtime/MyRuntimeProvider';
import { ComposerPrimitive } from '@openagentsinc/assistant-ui-runtime';

const meta = {
  title: 'Assistant UI/Attachment',
  component: ComposerAttachments,
  decorators: [
    (Story) => (
      <MyRuntimeProvider>
        <div className="dark w-full max-w-xl bg-background text-foreground border rounded-md p-3">
          <ComposerPrimitive.Root className="flex flex-col gap-2">
            <Story />
            <div className="flex items-center gap-2">
              <ComposerPrimitive.Input className="flex-1 bg-transparent border rounded px-3 py-2" placeholder="Type a message..." />
              <ComposerAddAttachment />
            </div>
          </ComposerPrimitive.Root>
        </div>
      </MyRuntimeProvider>
    ),
  ],
} satisfies Meta<typeof ComposerAttachments>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Composer: Story = {};

