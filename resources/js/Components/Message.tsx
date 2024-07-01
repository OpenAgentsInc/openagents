import React from "react";
import { AnimatedMessage } from "./AnimatedMessage";

interface MessageProps {
  content: string;
  isUser: boolean;
  isComplete: boolean;
  messageId: string;
}

export const Message: React.FC<MessageProps> = ({
  content,
  isUser,
  isComplete,
  messageId,
}) => {
  // If the content is empty, don't render anything
  if (!content.trim()) {
    return null;
  }

  const userMessageClasses = `
    group relative inline-flex gap-2 bg-gradient-to-b from-zinc-800 from-50% to-zinc-900 
    rounded-xl ml-px pl-2.5 py-2.5 break-words text-zinc-100 transition-all 
    max-w-[75ch] flex-col shadow-[0_2px_16px_rgba(0,0,0,0.025)] min-w-[16ch] pr-6
  `;

  const aiMessageClasses = `
    group relative pt-3.5 pb-[1.125rem] px-4 rounded-2xl -tracking-[0.015em]
    bg-gradient-to-b from-zinc-900/75 to-black/0
    before:absolute before:inset-0
    before:bg-[radial-gradient(ellipse_at_left_top,_theme(colors.zinc.800/50%)_0%,_theme(colors.zinc.900/30%)_60%)]
    before:rounded-2xl before:border-[0.5px] before:border-zinc-700/15
    before:shadow-[0_4px_24px_rgba(0,0,0,0.05)]
    before:transition-[opacity,transform] before:duration-250 before:ease-out
    before:z-0 ${!isComplete ? "before:opacity-0 before:scale-[0.995]" : ""}
    bg-zinc-800
  `;

  return (
    <div
      className={isUser ? userMessageClasses : aiMessageClasses}
      data-streaming={(!isComplete).toString()}
    >
      {!isUser && !isComplete ? (
        <pre className="whitespace-pre-wrap">
          <AnimatedMessage content={content} messageId={messageId} />
        </pre>
      ) : (
        <pre className="relative whitespace-pre-wrap">{content}</pre>
      )}
    </div>
  );
};
