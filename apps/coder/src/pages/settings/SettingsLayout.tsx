import React from "react";
import { Outlet, Link, useLocation, useNavigate } from "@tanstack/react-router";
import { 
  ArrowLeft, 
  Settings, 
  Sliders, 
  MessageSquare, 
  Server, 
  Database,
  Home,
  KeyRound
} from "lucide-react";
import { 
  SidebarProvider,
  Sidebar,
  SidebarHeader,
  SidebarContent,
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
  SidebarInset
} from '@/components/ui/sidebar';
import { Button } from "@/components/ui";
import { react19 } from "@openagents/core";
import { useDarkMode } from "@/hooks/use-dark-mode";
import ToggleTheme from '@/components/ToggleTheme';

// Make React Router components compatible with React 19
const OutletCompat = react19.router(Outlet);

export default function SettingsLayout() {
  const { isDark, toggleDarkMode } = useDarkMode();

  // Use the proper Tanstack Router location hook
  const location = useLocation();
  const currentPath = location.pathname;
  
  return (
    <SidebarProvider defaultOpen={true}>
      <div className="flex h-screen w-full flex-col text-primary font-mono">
        <div className="relative flex h-full w-full flex-1 overflow-hidden z-0">
          <div className="mt-[30px] relative flex h-full w-full flex-row overflow-hidden">
            {/* Settings Sidebar */}
            <Sidebar>
              <SidebarHeader className="border-y h-14 mt-[30px]">
                <div className="flex items-center justify-between px-2 h-full">
                  <Link to="/">
                    <Button variant="ghost" className="flex items-center hover:bg-muted/40">
                      <ArrowLeft className="mr-2 h-4 w-4" />
                      Back to Chat
                    </Button>
                  </Link>
                </div>
              </SidebarHeader>

              <SidebarContent>
                <SidebarMenu>
                  <SidebarMenuItem>
                    <Link to="/settings/models">
                      <SidebarMenuButton 
                        isActive={currentPath.includes("/models") && !currentPath.includes("/local-models")}
                        className={currentPath.includes("/models") && !currentPath.includes("/local-models") ? 
                          "relative before:absolute before:left-0 before:top-1/2 before:-translate-y-1/2 before:h-5 before:w-1 before:bg-primary before:rounded-r-md" : ""}
                      >
                        <Database className="h-4 w-4" />
                        <span>Models</span>
                      </SidebarMenuButton>
                    </Link>
                  </SidebarMenuItem>
                  
                  <SidebarMenuItem>
                    <Link to="/settings/api-keys">
                      <SidebarMenuButton 
                        isActive={currentPath.includes("/api-keys")}
                        className={currentPath.includes("/api-keys") ? 
                          "relative before:absolute before:left-0 before:top-1/2 before:-translate-y-1/2 before:h-5 before:w-1 before:bg-primary before:rounded-r-md" : ""}
                      >
                        <KeyRound className="h-4 w-4" />
                        <span>API Keys</span>
                      </SidebarMenuButton>
                    </Link>
                  </SidebarMenuItem>
                  
                  <SidebarMenuItem>
                    <Link to="/settings/local-models">
                      <SidebarMenuButton 
                        isActive={currentPath.includes("/local-models")}
                        className={currentPath.includes("/local-models") ? 
                          "relative before:absolute before:left-0 before:top-1/2 before:-translate-y-1/2 before:h-5 before:w-1 before:bg-primary before:rounded-r-md" : ""}
                      >
                        <Server className="h-4 w-4" />
                        <span>Local Models</span>
                      </SidebarMenuButton>
                    </Link>
                  </SidebarMenuItem>
                  
                  <SidebarMenuItem>
                    <Link to="/settings/prompts">
                      <SidebarMenuButton 
                        isActive={currentPath.includes("/prompts")}
                        className={currentPath.includes("/prompts") ? 
                          "relative before:absolute before:left-0 before:top-1/2 before:-translate-y-1/2 before:h-5 before:w-1 before:bg-primary before:rounded-r-md" : ""}
                      >
                        <MessageSquare className="h-4 w-4" />
                        <span>Prompts</span>
                      </SidebarMenuButton>
                    </Link>
                  </SidebarMenuItem>
                  
                  <SidebarMenuItem>
                    <Link to="/settings/preferences">
                      <SidebarMenuButton 
                        isActive={currentPath.includes("/preferences")}
                        className={currentPath.includes("/preferences") ? 
                          "relative before:absolute before:left-0 before:top-1/2 before:-translate-y-1/2 before:h-5 before:w-1 before:bg-primary before:rounded-r-md" : ""}
                      >
                        <Sliders className="h-4 w-4" />
                        <span>Preferences</span>
                      </SidebarMenuButton>
                    </Link>
                  </SidebarMenuItem>
                </SidebarMenu>
              </SidebarContent>

              {/* SidebarFooter removed */}
            </Sidebar>

            {/* Main Content Area */}
            <SidebarInset>
              <div className="grid grid-rows-[auto_minmax(0,1fr)] h-[calc(100vh-30px)]">
                <div className="border-y bg-background p-3 flex items-center justify-between z-10 h-14">
                  <div className="text-lg font-semibold">
                    {currentPath.includes("/models") && !currentPath.includes("/local-models") && "Models"}
                    {currentPath.includes("/api-keys") && "API Keys"}
                    {currentPath.includes("/local-models") && "Local Models"}
                    {currentPath.includes("/prompts") && "Prompts"}
                    {currentPath.includes("/preferences") && "Preferences"}
                  </div>
                  <div className="flex items-center gap-2">
                    <Link to="/">
                      <Button 
                        size="icon" 
                        className="bg-transparent text-primary hover:bg-primary/5">
                        <Home size={16} />
                      </Button>
                    </Link>
                    <ToggleTheme />
                  </div>
                </div>

                {/* Content Area */}
                <div className="overflow-auto p-6">
                  <div className="max-w-3xl mx-auto">
                    <OutletCompat />
                  </div>
                </div>
              </div>
            </SidebarInset>
          </div>
        </div>
      </div>
    </SidebarProvider>
  );
}