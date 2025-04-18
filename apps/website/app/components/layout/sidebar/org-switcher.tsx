import * as React from 'react';
import { ChevronsUpDown } from 'lucide-react';
import { Link } from 'react-router';
import { signOut } from '@/lib/auth-client';

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuPortal,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { SidebarMenu, SidebarMenuButton, SidebarMenuItem } from '@/components/ui/sidebar';
import { CreateNewIssue } from './create-new-issue';
import { ThemeToggle } from '../theme-toggle';

export function OrgSwitcher() {
  const handleSignOut = async () => {
    await signOut({
      fetchOptions: {
        onSuccess: () => {
          // Redirect to login page after signout
          window.location.href = '/login';
        },
      },
    });
  };

  return (
    <SidebarMenu>
      <SidebarMenuItem>
        <DropdownMenu>
          <div className="w-full flex items-center justify-between gap-1 pt-2">
            <div className="flex-shrink">
              <DropdownMenuTrigger asChild>
                <SidebarMenuButton
                  variant="outline"
                  size="lg"
                  className="h-8 p-1 data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
                >
                  <div className="select-none grid flex-1 text-left text-sm leading-tight">
                    <span className="truncate font-semibold">OpenAgents</span>
                  </div>
                  <ChevronsUpDown className="ml-auto" />
                </SidebarMenuButton>
              </DropdownMenuTrigger>
            </div>

            <CreateNewIssue />
          </div>
          <DropdownMenuContent
            className="w-[--radix-dropdown-menu-trigger-width] min-w-60 rounded-lg"
            side="bottom"
            align="end"
            sideOffset={4}
          >
            <DropdownMenuItem onClick={handleSignOut}>
              Log out
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </SidebarMenuItem>
    </SidebarMenu>
  );
}
