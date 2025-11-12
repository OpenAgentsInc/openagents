import { Thread } from "@/components/assistant-ui/thread";
import { ThreadList } from "@/components/assistant-ui/thread-list";
import { ModelToolbar } from "@/components/assistant-ui/model-toolbar";

export function AssistantSidebar() {
  // Removed legacy "Test ACP" manual trigger UI; runtime selection is handled via ModelToolbar

  return (
    <div className="flex h-full w-full">
      {/* Left Sidebar */}
      <div className="flex w-64 flex-col border-r border-zinc-800 bg-zinc-950">

        {/* Thread List */}
        <div className="flex-1 overflow-auto p-2">
          <ThreadList />
        </div>

        {/* Footer intentionally left empty (Test ACP removed) */}
      </div>

      {/* Main Chat Area */}
      <div className="flex-1 min-w-0 flex flex-col">
        <ModelToolbar />
        <div className="flex-1 min-h-0">
          <Thread />
        </div>
      </div>
    </div>
  );
}
