import type { Meta, StoryObj } from '@storybook/react-vite';
import { NavProjectsAssistant } from '@/components/nav-projects-assistant';
import { SidebarProvider, Sidebar } from "@openagentsinc/ui";
import { Folder, FolderOpen, FolderTree } from 'lucide-react';

const meta = {
  title: 'Sidebar/NavProjectsAssistant',
  component: NavProjectsAssistant,
  decorators: [
    (Story) => (
      <div className="dark w-full min-h-[80vh] bg-background text-foreground">
        <SidebarProvider>
          <Sidebar>
            <Story />
          </Sidebar>
        </SidebarProvider>
      </div>
    ),
  ],
  parameters: {
    layout: 'centered',
  },
} satisfies Meta<typeof NavProjectsAssistant>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const CustomProjects: Story = {
  args: {
    projects: [
      {
        name: "Web Dashboard",
        icon: FolderOpen,
      },
      {
        name: "Mobile App",
        icon: Folder,
      },
      {
        name: "API Service",
        icon: FolderTree,
      },
    ],
  },
};
