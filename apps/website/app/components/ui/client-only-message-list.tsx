import { useEffect, useState } from "react";
import { MessageList } from "./message-list";
import type { Message } from "./chat-message";

type ClientOnlyMessageListProps = {
  messages: Message[];
  showTimeStamps?: boolean;
  isTyping?: boolean;
};

export function ClientOnlyMessageList(props: ClientOnlyMessageListProps) {
  const [isMounted, setIsMounted] = useState(false);

  useEffect(() => {
    setIsMounted(true);
  }, []);

  if (!isMounted) {
    // Return a placeholder with the same dimensions to avoid layout shifts
    return (
      <div className="space-y-4 overflow-visible min-h-[200px] flex items-center justify-center text-muted-foreground">
        Loading messages...
      </div>
    );
  }

  return <MessageList {...props} />;
}