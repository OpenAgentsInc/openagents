import React, { useRef, useEffect, useState } from "react";
import { ChatInput } from "../Components/ChatInput";
import { useMessageStore } from "../store";
import { useTransition, animated, config } from "@react-spring/web";

export default function AutoDev() {
  const messages = useMessageStore((state) => state.messages);
  const [animatingMessages, setAnimatingMessages] = useState<
    Array<{ id: string; content: string }>
  >([]);

  useEffect(() => {
    setAnimatingMessages(
      messages.map((message) => ({
        id: message.id,
        content: message.content,
      }))
    );
  }, [messages]);

  return (
    <div className="from-[#0a0a0a] to-black text-white font-mono min-h-screen bg-gradient-to-b bg-fixed tracking-tight">
      <div className="flex min-h-screen w-full">
        <nav className="z-20 h-screen max-md:pointer-events-none max-md:fixed"></nav>
        <div className="min-h-full w-full min-w-0 flex-1">
          <div className="flex h-screen w-full flex-col overflow-hidden">
            <div className="sticky top-0 z-10 -mb-6 flex h-14 items-center gap-3 pl-11 pr-2 md:pb-0.5 md:pl-6">
              {/* ... (header content remains the same) ... */}
            </div>
            <div className="relative flex w-full flex-1 overflow-x-hidden overflow-y-scroll pt-6 md:pr-8">
              <div className="relative mx-auto flex h-full w-full max-w-3xl flex-1 flex-col md:px-2">
                <div className="flex-1 flex flex-col gap-3 px-4 max-w-3xl mx-auto w-full pt-6">
                  {animatingMessages.length === 0 ? (
                    <p className="mt-6">AutoDev awaiting instructions.</p>
                  ) : (
                    animatingMessages.map((message, index) => (
                      <div
                        key={message.id}
                        className={`p-2 rounded ${messages[index].isUser ? "bg-zinc-900" : "bg-zinc-800"}`}
                      >
                        <AnimatedMessage
                          content={message.content}
                          messageId={message.id}
                        />
                        {!messages[index].isComplete &&
                          !messages[index].isUser && (
                            <span className="animate-pulse">▌</span>
                          )}
                      </div>
                    ))
                  )}
                </div>
                <div className="sticky bottom-0 mx-auto w-full pt-6">
                  <ChatInput />
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function AnimatedMessage({
  content,
  messageId,
}: {
  content: string;
  messageId: string;
}) {
  const [animatedContent, setAnimatedContent] = useState("");

  useEffect(() => {
    setAnimatedContent(content);
  }, [content]);

  const transitions = useTransition(
    animatedContent.split("").map((char, index) => ({
      char,
      key: `${messageId}-${index}`,
    })),
    {
      keys: (item) => item.key,
      from: { opacity: 0 },
      enter: { opacity: 1 },
      leave: { opacity: 0 },
      trail: 25,
      config: config.gentle,
    }
  );

  return transitions((style, item) => (
    <animated.span style={style}>{item.char}</animated.span>
  ));
}
