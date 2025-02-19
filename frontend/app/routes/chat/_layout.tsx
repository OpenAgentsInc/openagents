import { Outlet, useLocation } from "react-router";
import { HeaderBar } from "~/components/header-bar";
import { Button } from "~/components/ui/button";

import type { Route } from "../../+types/root";

export function meta({}: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Chat" },
    { name: "description", content: "Chat with OpenAgents" },
  ];
}

export default function ChatLayout() {
  const location = useLocation();
  const isSessionView = location.pathname !== "/chat";

  return (
    <div className="dark fixed inset-0 flex flex-col bg-background text-foreground">
      <HeaderBar />

      {/* Main chat area */}
      <div className="flex flex-1 overflow-hidden">
        {/* Sidebar - only show in chat session view */}
        {isSessionView && (
          <div className="flex-shrink-0 w-64 border-r border-zinc-800 bg-black overflow-y-auto">
            <div className="p-4">
              <h2 className="mb-4 text-sm font-semibold text-zinc-400">
                Recent Chats
              </h2>
              <div className="space-y-2">
                <Button
                  variant="outline"
                  className="w-full justify-start bg-zinc-900 hover:bg-zinc-800 border-zinc-800"
                >
                  Chat #1
                </Button>
                <Button
                  variant="outline"
                  className="w-full justify-start bg-zinc-900 hover:bg-zinc-800 border-zinc-800"
                >
                  Chat #2
                </Button>
              </div>
            </div>
          </div>
        )}

        {/* Chat content area */}
        <div className="flex-1 min-w-0 overflow-y-auto">
          <Outlet />
        </div>
      </div>
    </div>
  );
}
