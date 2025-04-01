import React from "react";
import { Outlet, Link, useLocation } from "@tanstack/react-router";
import { ArrowLeft, Sun, Moon } from "lucide-react";
import { Button } from "@/components/ui";
import { ScrollArea } from "@/components/ui/scroll-area";
import { react19 } from "@openagents/core";
import { useDarkMode } from "@/hooks/use-dark-mode";

// Make React Router components compatible with React 19
const OutletCompat = react19.router(Outlet);

export default function SettingsLayout() {
  const { isDarkMode, toggleDarkMode } = useDarkMode();
  
  // Use the proper Tanstack Router location hook
  const location = useLocation();
  const isPrompts = location.pathname.includes("prompts");
  
  console.log("Location pathname:", location.pathname, "isPrompts:", isPrompts);
  
  return (
    <ScrollArea className="h-screen w-full">
      <div className="mx-auto flex min-h-screen max-w-[1200px] flex-col px-4 pb-24 pt-6 md:px-6 lg:px-8">
        {/* Background */}
        <div className="absolute inset-0 -z-50 dark:bg-sidebar !fixed">
          <div className="absolute inset-0 opacity-40" 
            style={{
              backgroundImage: "radial-gradient(closest-corner at 180px 36px, rgba(255, 1, 111, 0.19), rgba(255, 1, 111, 0.08)), linear-gradient(rgb(63, 51, 69) 15%, rgb(7, 3, 9))"
            }}>
          </div>
          <div className="absolute inset-0 bg-noise"></div>
          <div className="absolute inset-0 bg-black/40"></div>
        </div>
        
        {/* Header */}
        <header className="flex items-center justify-between pb-8">
          <Link to="/">
            <Button variant="ghost" className="flex items-center hover:bg-muted/40">
              <ArrowLeft className="mr-2 h-4 w-4" />
              Back to Chat
            </Button>
          </Link>
          
          <div className="flex flex-row items-center gap-2">
            <Button 
              variant="ghost" 
              size="icon" 
              onClick={toggleDarkMode}
              className="relative size-8 hover:bg-muted/40 hover:text-foreground"
            >
              <Moon className={`absolute size-4 transition-all duration-200 ${isDarkMode ? 'rotate-0 scale-100' : '-rotate-90 scale-0'}`} />
              <Sun className={`absolute size-4 transition-all duration-200 ${isDarkMode ? 'rotate-90 scale-0' : 'rotate-0 scale-100'}`} />
              <span className="sr-only">Toggle theme</span>
            </Button>
          </div>
        </header>
        
        {/* Content */}
        <div className="flex flex-grow flex-col gap-4 md:flex-row justify-center">
          {/* Main content area */}
          <div className="w-full">
            <div className="space-y-6">
              {/* Tabs navigation */}
              <div className="flex justify-center w-full">
                <div className="inline-flex h-9 items-center rounded-lg bg-secondary/80 p-1 text-secondary-foreground no-scrollbar overflow-auto">
                  <Link to="/settings/models">
                    <div className={`mx-0.5 rounded-md px-4 py-1.5 text-sm font-medium hover:bg-sidebar-accent/40 cursor-pointer 
                      ${!isPrompts ? "bg-background text-foreground shadow" : ""}`}>
                      Models
                    </div>
                  </Link>
                  <Link to="/settings/prompts">
                    <div className={`mx-0.5 rounded-md px-4 py-1.5 text-sm font-medium hover:bg-sidebar-accent/40 cursor-pointer
                      ${isPrompts ? "bg-background text-foreground shadow" : ""}`}>
                      Prompts
                    </div>
                  </Link>
                </div>
              </div>
              
              {/* Tab content */}
              <div className="flex justify-center w-full">
                <div className="w-full max-w-3xl mt-2 space-y-8">
                  <OutletCompat />
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </ScrollArea>
  );
}