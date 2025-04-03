import React from "react";
import { Outlet, Link, useLocation } from "@tanstack/react-router";
import {
  Home,
  Database,
  KeyRound,
  Server,
  MessageSquare,
  Sliders,
  ArrowLeft,
  Globe,
  Wrench
} from "lucide-react";
import {
  SidebarProvider,
  Sidebar,
  SidebarContent,
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
  SidebarInset,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarGroupContent
} from '@/components/ui/sidebar';
import { Button } from "@/components/ui";
import { react19 } from "@openagents/core";
import ToggleTheme from '@/components/ToggleTheme';

// Make React Router components compatible with React 19
const OutletCompat = react19.router(Outlet);

export default function SettingsLayout() {
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
              <SidebarContent>
                <div className="px-4 mt-[45px] mb-2">
                  <Link to="/">
                    <Button
                      variant="outline"
                      className="w-full justify-start gap-2 bg-transparent border-primary/20 hover:bg-primary/5"
                    >
                      <ArrowLeft className="h-4 w-4" />
                      Back to Chat
                    </Button>
                  </Link>
                </div>

                <SidebarGroup>
                  <SidebarGroupLabel>Models & API</SidebarGroupLabel>
                  <SidebarGroupContent>
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
                            <span>LM Studio</span>
                          </SidebarMenuButton>
                        </Link>
                      </SidebarMenuItem>
                      
                      <SidebarMenuItem>
                        <Link to="/settings/mcp-clients">
                          <SidebarMenuButton
                            isActive={currentPath.includes("/mcp-clients")}
                            className={currentPath.includes("/mcp-clients") ?
                              "relative before:absolute before:left-0 before:top-1/2 before:-translate-y-1/2 before:h-5 before:w-1 before:bg-primary before:rounded-r-md" : ""}
                          >
                            <Wrench className="h-4 w-4" />
                            <span>MCP Clients</span>
                          </SidebarMenuButton>
                        </Link>
                      </SidebarMenuItem>
                    </SidebarMenu>
                  </SidebarGroupContent>
                </SidebarGroup>

                <SidebarGroup>
                  <SidebarGroupLabel>Customization</SidebarGroupLabel>
                  <SidebarGroupContent>
                    <SidebarMenu>
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
                  </SidebarGroupContent>
                </SidebarGroup>
              </SidebarContent>

              {/* Add SidebarFooter */}
              <div className="absolute bottom-0 left-0 right-0 p-3 border-t flex items-center justify-between">
                <ToggleTheme />
                <Link to="/">
                  <Button
                    size="icon"
                    className="flex items-center justify-center h-8 w-8 bg-transparent text-primary hover:bg-primary/5">
                    <Home size={20} />
                  </Button>
                </Link>
              </div>
            </Sidebar>

            {/* Main Content Area */}
            <SidebarInset>
              <div className="grid grid-rows-[minmax(0,1fr)] h-[calc(100vh-30px)]">
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
