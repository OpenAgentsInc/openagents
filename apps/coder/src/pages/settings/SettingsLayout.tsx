import React from "react";
import { Outlet, useRouter } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";
import { Link } from "@tanstack/react-router";
import { Button } from "@/components/ui";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { react19 } from "@openagents/core";

// Make React Router components compatible with React 19
const OutletCompat = react19.router(Outlet);

export default function SettingsLayout() {
  const router = useRouter();
  const currentPath = router.state.location.pathname;
  
  // Determine which tab should be active based on the current route
  const activeTab = currentPath.includes("/settings/prompts") ? "prompts" : "models";
  
  return (
    <ScrollArea className="h-screen w-full">
      <div className="container py-6 space-y-6 font-mono">
        <div className="flex items-center">
          <Link to="/" className="mr-4">
            <Button variant="ghost" size="icon">
              <ArrowLeft className="h-4 w-4" />
            </Button>
          </Link>
          <h1 className="text-2xl font-bold">Settings</h1>
        </div>

        {/* Navigation Tabs */}
        <Tabs value={activeTab} className="w-full">
          <TabsList className="grid w-full grid-cols-2">
            <Link to="/settings/models">
              <TabsTrigger value="models" className="w-full">Models</TabsTrigger>
            </Link>
            <Link to="/settings/prompts">
              <TabsTrigger value="prompts" className="w-full">Prompts</TabsTrigger>
            </Link>
          </TabsList>
        </Tabs>

        {/* Content */}
        <div className="mt-6">
          <OutletCompat />
        </div>
      </div>
    </ScrollArea>
  );
}