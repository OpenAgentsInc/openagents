import { ChatInput } from "~/components/chat/chat-input"
import { Thinking } from "~/components/chat/thinking"

export default function ChatSession() {
  return (
    <div className="flex h-full flex-col">
      {/* Main content area */}
      <div className="flex-1 overflow-y-auto">
        <div className="mx-auto max-w-3xl w-full">
          {/* Thinking component */}
          <div className="p-4">
            <Thinking
              state="thinking"
              duration={3}
              content={[
                "Analyzing your request...",
                "Processing information...",
                "Preparing response..."
              ]}
            />
          </div>
        </div>
      </div>

      {/* Chat input at bottom */}
      <div className="p-4">
        <ChatInput />
      </div>
    </div>
  );
}
