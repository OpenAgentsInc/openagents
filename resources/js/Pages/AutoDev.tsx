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
              <div className="from-black via-black to-black/0 absolute inset-0 -bottom-7 z-[-1] bg-gradient-to-b via-50% blur"></div>
              <div className="flex min-w-0 flex-1 shrink flex-col md:flex-row md:items-center 2xl:justify-center">
                <div className="flex min-w-0 items-center max-md:text-sm">
                  <button
                    className="inline-flex items-center justify-center relative shrink-0 ring-offset-2 ring-offset-bg-300 ring-accent-main-100 focus-visible:outline-none focus-visible:ring-1 disabled:pointer-events-none disabled:opacity-50 disabled:shadow-none disabled:drop-shadow-none text-zinc-200 transition-all active:bg-bg-400 hover:bg-bg-500/40 hover:text-text-100 rounded py-1 px-2 max-w-full whitespace-nowrap text-ellipsis overflow-hidden outline-none ring-offset-2 ring-offset-bg-300 ring-accent-main-100 focus-visible:outline-none focus-visible:ring-1 focus:backdrop-blur-xl hover:backdrop-blur-xl hover:bg-bg-400/50 !text-text-000 !shrink gap-1 !px-1 !py-0.5"
                    data-testid="chat-menu-trigger"
                    type="button"
                    id="radix-:r33:"
                    aria-haspopup="menu"
                    aria-expanded="false"
                    data-state="closed"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="truncate font-normal tracking-tight">
                        AutoDev Demo
                      </div>
                    </div>
                    <svg
                      xmlns="http://www.w3.org/2000/svg"
                      width="12"
                      height="12"
                      fill="currentColor"
                      viewBox="0 0 256 256"
                    >
                      <path d="M216.49,104.49l-80,80a12,12,0,0,1-17,0l-80-80a12,12,0,0,1,17-17L128,159l71.51-71.52a12,12,0,0,1,17,17Z"></path>
                    </svg>
                  </button>
                </div>
              </div>
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
              <div className="w-1/2 min-w-[650px] min-h-full bg-bg-100 border-l border-border-300">
                <div className="h-full flex flex-col">
                  <div className="border-b border-border-400 sticky top-0 flex items-center gap-1 px-4 py-2">
                    <button className="text-text-200 hover:bg-bg-500/40 hover:text-text-100 h-8 w-8 rounded-md">
                      <svg
                        xmlns="http://www.w3.org/2000/svg"
                        width="16"
                        height="16"
                        fill="currentColor"
                        viewBox="0 0 256 256"
                      >
                        <path d="M224,128a8,8,0,0,1-8,8H59.31l58.35,58.34a8,8,0,0,1-11.32,11.32l-72-72a8,8,0,0,1,0-11.32l72-72a8,8,0,0,1,11.32,11.32L59.31,120H216A8,8,0,0,1,224,128Z"></path>
                      </svg>
                    </button>
                    <h3 className="text-text-100 font-tiempos truncate text-sm flex-1">
                      Updated Greptile Integration Code
                    </h3>
                    <button className="text-text-200 hover:bg-bg-500/40 hover:text-text-100 h-8 w-8 rounded-md">
                      <svg
                        xmlns="http://www.w3.org/2000/svg"
                        width="16"
                        height="16"
                        fill="currentColor"
                        viewBox="0 0 256 256"
                      >
                        <path d="M205.66,194.34a8,8,0,0,1-11.32,11.32L128,139.31,61.66,205.66a8,8,0,0,1-11.32-11.32L116.69,128,50.34,61.66A8,8,0,0,1,61.66,50.34L128,116.69l66.34-66.35a8,8,0,0,1,11.32,11.32L139.31,128Z"></path>
                      </svg>
                    </button>
                  </div>
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
  const [animatedContent, setAnimatedContent] = useState<string>("");
  const queueRef = useRef<string>("");
  const animatingRef = useRef(false);

  useEffect(() => {
    if (content.length > animatedContent.length) {
      queueRef.current = content.slice(animatedContent.length);
      animateNextChar();
    }
  }, [content, animatedContent]);

  const animateNextChar = () => {
    if (animatingRef.current || queueRef.current.length === 0) return;

    animatingRef.current = true;
    const nextChar = queueRef.current[0];

    setAnimatedContent((prev) => prev + nextChar);
    queueRef.current = queueRef.current.slice(1);

    setTimeout(() => {
      animatingRef.current = false;
      animateNextChar();
    }, 5); // Adjust this value to control animation speed
  };

  const tokens = animatedContent.match(/\S+|\s+/g) || [];

  const transitions = useTransition(tokens, {
    keys: (item, index) => `${messageId}-${index}`,
    from: { opacity: 0, transform: "translateY(5px)" },
    enter: { opacity: 1, transform: "translateY(0px)" },
    leave: { opacity: 0 },
    config: config.stiff,
  });

  return (
    <span>
      {transitions((style, item) => (
        <animated.span style={style}>{item}</animated.span>
      ))}
    </span>
  );
}
