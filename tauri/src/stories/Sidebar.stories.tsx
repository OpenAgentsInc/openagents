import type { Meta, StoryObj } from '@storybook/react-vite';
import {
  SidebarProvider,
  Sidebar,
  SidebarHeader,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarGroupContent,
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
  SidebarInset,
  SidebarTrigger,
  SidebarSeparator,
} from '@/components/ui/sidebar';
import { Button } from '@/components/ui/button';
import { Home, Settings, Folder, FileText } from 'lucide-react';

const meta = {
  title: 'UI/Sidebar',
  component: Sidebar,
  argTypes: {
    variant: { control: 'select', options: ['sidebar', 'floating', 'inset'] },
    collapsible: { control: 'select', options: ['offcanvas', 'icon', 'none'] },
    side: { control: 'select', options: ['left', 'right'] },
    width: { control: 'number' },
  },
  args: {
    variant: 'sidebar',
    collapsible: 'offcanvas',
    side: 'left',
    width: 920,
  },
} satisfies Meta<typeof Sidebar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ variant, collapsible, side, width }) => (
    <div style={{ width: Number(width), height: 360, border: '1px solid var(--border)' }}>
      <SidebarProvider defaultOpen>
        <div className="flex h-full">
          <Sidebar variant={variant as any} collapsible={collapsible as any} side={side as any}>
            <SidebarHeader>
              <div className="flex items-center justify-between">
                <div className="text-sm font-semibold">Project</div>
                <SidebarTrigger />
              </div>
            </SidebarHeader>
            <SidebarContent>
              <SidebarGroup>
                <SidebarGroupLabel>General</SidebarGroupLabel>
                <SidebarGroupContent>
                  <SidebarMenu>
                    <SidebarMenuItem>
                      <SidebarMenuButton asChild>
                        <a href="#"><Home /> Home</a>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                    <SidebarMenuItem>
                      <SidebarMenuButton asChild>
                        <a href="#"><Folder /> Files</a>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                    <SidebarMenuItem>
                      <SidebarMenuButton asChild>
                        <a href="#"><FileText /> Documents</a>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  </SidebarMenu>
                </SidebarGroupContent>
              </SidebarGroup>
              <SidebarSeparator />
              <SidebarGroup>
                <SidebarGroupLabel>Settings</SidebarGroupLabel>
                <SidebarGroupContent>
                  <SidebarMenu>
                    <SidebarMenuItem>
                      <SidebarMenuButton asChild>
                        <a href="#"><Settings /> Preferences</a>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  </SidebarMenu>
                </SidebarGroupContent>
              </SidebarGroup>
            </SidebarContent>
            <SidebarFooter>
              <Button size="sm" variant="outline">New</Button>
            </SidebarFooter>
          </Sidebar>
          <SidebarInset>
            <div className="p-4 h-full flex items-center justify-center text-muted-foreground">
              Main content area
            </div>
          </SidebarInset>
        </div>
      </SidebarProvider>
    </div>
  ),
};

