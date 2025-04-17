import { RiGithubLine } from '@remixicon/react';
import * as React from 'react';
import { Link, useLocation } from 'react-router';

import { HelpButton } from '@/components/layout/sidebar/help-button';
import { NavInbox } from '@/components/layout/sidebar/nav-inbox';
import { NavTeams } from '@/components/layout/sidebar/nav-teams';
import { NavWorkspace } from '@/components/layout/sidebar/nav-workspace';
import { NavAccount } from '@/components/layout/sidebar/nav-account';
import { NavFeatures } from '@/components/layout/sidebar/nav-features';
import { NavTeamsSettings } from '@/components/layout/sidebar/nav-teams-settings';
import { OrgSwitcher } from '@/components/layout/sidebar/org-switcher';
import { GitHubTokenInput } from '@/components/layout/sidebar/github-token-input';
import { Button } from '@/components/ui/button';
import { Sidebar, SidebarContent, SidebarFooter, SidebarHeader } from '@/components/ui/sidebar';
import { X } from 'lucide-react';
import { BackToApp } from '@/components/layout/sidebar/back-to-app';

export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  const [open, setOpen] = React.useState(true);
  const { pathname } = useLocation();
  const isSettings = pathname.includes('/settings');
  return (
    <Sidebar collapsible="offcanvas" {...props}>
      <SidebarHeader>{isSettings ? <BackToApp /> : <OrgSwitcher />}</SidebarHeader>
      <SidebarContent>
        {isSettings ? (
          <>
            <NavAccount />
            <NavFeatures />
            <NavTeamsSettings />
          </>
        ) : (
          <>
            {/* <NavInbox /> */}
            <NavWorkspace />
            {/* <NavTeams /> */}
          </>
        )}
      </SidebarContent>
      <SidebarFooter className="border-t">
        <div className="space-y-2 py-1">
          {/* <HelpButton /> */}
          <GitHubTokenInput />
        </div>
      </SidebarFooter>
    </Sidebar>
  );
}
