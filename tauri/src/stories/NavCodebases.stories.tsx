import type { Meta, StoryObj } from '@storybook/react-vite';
import { NavCodebases } from '@/components/nav-codebases';
import { SidebarProvider, Sidebar } from "@openagentsinc/ui";
import { Code2, FolderGit2, GitBranch, GitFork } from 'lucide-react';

const meta = {
  title: 'Sidebar/NavCodebases',
  component: NavCodebases,
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
} satisfies Meta<typeof NavCodebases>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const CustomCodebases: Story = {
  args: {
    codebases: [
      {
        name: "react-native-app",
        icon: FolderGit2,
        branch: "feature/auth",
      },
      {
        name: "backend-api",
        icon: Code2,
        branch: "main",
      },
      {
        name: "shared-components",
        icon: GitBranch,
        branch: "develop",
      },
      {
        name: "monorepo",
        icon: GitFork,
        branch: "main",
      },
    ],
  },
};
