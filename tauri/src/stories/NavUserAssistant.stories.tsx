import type { Meta, StoryObj } from '@storybook/react-vite';
import { NavUserAssistant } from '@/components/nav-user-assistant';
import { SidebarProvider, Sidebar, SidebarFooter } from "@openagentsinc/ui";

const meta = {
  title: 'Sidebar/NavUserAssistant',
  component: NavUserAssistant,
  decorators: [
    (Story) => (
      <div className="dark w-full min-h-[80vh] bg-background text-foreground flex items-end">
        <SidebarProvider>
          <Sidebar>
            <SidebarFooter>
              <Story />
            </SidebarFooter>
          </Sidebar>
        </SidebarProvider>
      </div>
    ),
  ],
  parameters: {
    layout: 'centered',
  },
} satisfies Meta<typeof NavUserAssistant>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const CustomUser: Story = {
  args: {
    user: {
      name: "John Doe",
      email: "john@example.com",
      avatar: "https://api.dicebear.com/7.x/avataaars/svg?seed=John",
    },
  },
};

export const NoAvatar: Story = {
  args: {
    user: {
      name: "Jane Smith",
      email: "jane@example.com",
    },
  },
};
