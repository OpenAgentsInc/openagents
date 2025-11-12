import type { Meta, StoryObj } from '@storybook/react-vite';
import { NavChats } from '@/components/nav-chats';
import { MyRuntimeProvider } from '@/runtime/MyRuntimeProvider';
import { SidebarProvider, Sidebar } from '@/components/ui/sidebar';

const meta = {
  title: 'Sidebar/NavChats',
  component: NavChats,
  decorators: [
    (Story) => (
      <MyRuntimeProvider>
        <div className="dark w-full min-h-[80vh] bg-background text-foreground">
          <SidebarProvider>
            <Sidebar>
              <Story />
            </Sidebar>
          </SidebarProvider>
        </div>
      </MyRuntimeProvider>
    ),
  ],
  parameters: {
    layout: 'centered',
  },
} satisfies Meta<typeof NavChats>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};
