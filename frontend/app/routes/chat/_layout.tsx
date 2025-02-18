import { Outlet } from "react-router"
import { HeaderBar } from "~/components/header-bar"

export default function ChatLayout() {
  return (
    <div className="h-screen w-screen bg-black text-white">
      <HeaderBar />

      {/* Main chat area */}
      <div className="flex flex-1 h-[calc(100vh-var(--header-height))] overflow-hidden">
        {/* Sidebar */}
        <div className="w-64 border-r border-gray-800 bg-black p-4">
          <h2 className="mb-4 text-sm font-semibold text-gray-400">Recent Chats</h2>
          <div className="space-y-2">
            <button className="w-full rounded-lg bg-gray-900 p-3 text-left hover:bg-gray-800">
              Chat #1
            </button>
            <button className="w-full rounded-lg bg-gray-900 p-3 text-left hover:bg-gray-800">
              Chat #2
            </button>
          </div>
        </div>

        {/* Chat content area */}
        <div className="flex-1">
          <Outlet />
        </div>
      </div>
    </div>
  );
}
