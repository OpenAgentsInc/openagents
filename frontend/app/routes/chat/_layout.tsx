import { Outlet } from "react-router"
import { HeaderBar } from "~/components/header-bar"

import type { Route } from "../../+types/root";

export function meta({ }: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Chat" },
    { name: "description", content: "Chat with OpenAgents" },
  ];
}

export default function ChatLayout() {
  return (
    <div className="dark fixed inset-0 flex flex-col bg-background text-foreground">
      <HeaderBar />

      {/* Main chat area */}
      <div className="flex flex-1 overflow-hidden">
        {/* Chat content area */}
        <div className="flex-1 min-w-0 overflow-y-auto">
          <Outlet />
        </div>
      </div>
    </div>
  );
}
