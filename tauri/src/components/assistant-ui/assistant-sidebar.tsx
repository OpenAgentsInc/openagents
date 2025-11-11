import { ThreadList } from "@/components/assistant-ui/thread-list";
import { Thread } from "@/components/assistant-ui/thread";

export function AssistantSidebar() {
  return (
    <div className="flex h-full w-full">
      {/* Left Sidebar */}
      <div className="flex w-64 flex-col border-r border-zinc-800 bg-zinc-950">
        {/* Header */}
        <div className="flex items-center gap-2 border-b border-zinc-800 p-4 pt-6">
          <div className="flex size-8 items-center justify-center rounded-[var(--radius-lg)]">
            <img src="/oalogo.png" alt="OpenAgents" className="size-8" />
          </div>
          <span className="font-semibold">OpenAgents</span>
        </div>

        {/* Thread List */}
        <div className="flex-1 overflow-auto p-2">
          <ThreadList />
        </div>
      </div>

      {/* Main Chat Area */}
      <div className="flex-1">
        <Thread />
      </div>
    </div>
  );
}
