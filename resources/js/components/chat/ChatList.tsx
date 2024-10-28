import { useEffect, useRef } from "react"
import { Separator } from "@/components/ui/separator"
import { ChatMessageActions } from "./ChatMessageActions"
import { Message } from "./Message"
import { ToolInvocation } from "./ToolInvocation"
import { ChatListProps, Message as MessageType } from "./types"

export function ChatList({ messages, streamingChatMessage }: ChatListProps) {
  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    console.log("CHATLIST HAS MESSAGES:", messages)
  }, [messages])

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamingChatMessage]);

  if (!messages.length && !streamingChatMessage) {
    console.log("returning null because messages.length is 0 and streamingChatMessage is null")
    return null;
  }

  const renderMessage = (message: MessageType | null, index: number) => {
    if (!message) return null;

    const isAssistantMessage = message.role === 'assistant';
    const hasToolInvocations = isAssistantMessage && message.toolInvocations && message.toolInvocations.length > 0;
    const hasContent = message.content && message.content.trim() !== '';

    return (
      <div key={`${message.id}-${index}`} className="flex-shrink-0">
        <div className="flex items-start">
          <Message message={message} />
          {hasToolInvocations && !hasContent && (
            <div className="flex-grow">
              {message.toolInvocations?.map((invocation, invIndex) => (
                <ToolInvocation key={`${invocation.id}-${invIndex}`} toolInvocation={invocation} />
              ))}
            </div>
          )}
        </div>

        {hasToolInvocations && hasContent && (
          <div className="mt-1">
            {message.toolInvocations?.map((invocation, invIndex) => (
              <ToolInvocation key={`${invocation.id}-${invIndex}`} toolInvocation={invocation} />
            ))}
          </div>
        )}
        <ChatMessageActions message={message} />
        {index < messages.length - 1 && <Separator className="my-4 bg-zinc-800" />}
      </div>
    );
  };

  return (
    <div className="flex-1 mx-auto max-w-4xl px-1 md:px-4 flex flex-col">
      <div className="p-6 px-4 xl:px-12 text-zinc-100 flex flex-col">
        {messages.map((message, index) => renderMessage(message, index))}
        {streamingChatMessage && renderMessage(streamingChatMessage, messages.length)}
        <div ref={messagesEndRef} className="h-0" />
      </div>
    </div>
  );
}
